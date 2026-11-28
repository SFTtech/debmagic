use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::process::{Command, Stdio};

use anyhow::{Context, bail};
use debmagic_common::debian::binary::{header_mtime, normalize_member_path};
use debmagic_common::debian::source_extract;
use debmagic_common::{checksums, compression};
use regex::Regex;
use tempfile::TempDir;

use super::binary_package_reader::guess_package_name;
use super::dsc::Dsc;
use super::file_tree::FileTree;

/// `.dsc` identity for extract: Dsc, path, and its two file lists.
#[derive(Debug)]
struct DscIdentity {
    path: PathBuf,
    dsc: Dsc,
    /// Every file the checksum fields list: what `dpkg-source` needs.
    files: Vec<String>,
    /// lintian's `files` ([`Dsc::files`]): what its native and orig
    /// decisions look at.
    listed: Vec<String>,
}

impl DscIdentity {
    fn new(path: &Path, dsc: Dsc) -> Self {
        Self {
            path: path.to_path_buf(),
            files: checksums::filenames(dsc.fields().paragraph()),
            listed: dsc.files(),
            dsc,
        }
    }

    fn load(path: &Path) -> anyhow::Result<Self> {
        let identity = Self::new(path, Dsc::load(path)?);
        if identity.files.is_empty() {
            bail!("{} names no files", path.display());
        }
        Ok(identity)
    }

    fn source(&self) -> &str {
        self.dsc.fields().get("Source").unwrap_or_default().trim()
    }

    fn format(&self) -> &str {
        self.dsc.fields().get("Format").unwrap_or("1.0").trim()
    }

    fn version(&self) -> &str {
        self.dsc.fields().get("Version").unwrap_or_default().trim()
    }

    fn package_name(&self) -> String {
        if self.source().is_empty() {
            guess_package_name(&self.path)
        } else {
            self.source().to_string()
        }
    }

    fn directory(&self) -> &Path {
        self.path.parent().unwrap_or_else(|| Path::new("."))
    }

    fn is_native(&self) -> bool {
        let format = self.format();
        if regex_is_match(r"^\s*2\.0\s*$", format) {
            return false;
        }
        if regex_is_match(r"^\s*3\.0\s+\((?:quilt|git)\)\s*$", format) {
            return false;
        }
        if regex_is_match(r"^\s*3\.0\s+\(native\)\s*$", format) {
            return true;
        }
        if self.version().is_empty() {
            return false;
        }
        let version = strip_epoch(self.version());
        let diffname = format!("{}_{version}.diff.gz", self.package_name());
        !self.listed.iter().any(|name| name == &diffname)
    }

    /// lintian's `components` (`Processable/Source/Components.pm`): each
    /// listed `{source}_{upstream}.orig[-{component}].tar.{gz,bz2,lzma,xz}`,
    /// and the `{source}_{version}.tar.*` a 1.0 package may carry, with its
    /// component (empty for the main tarball). `{source}` is the `Source`
    /// field as written. Sorted as lintian unpacks them,
    /// `sort_by { $components{$_} }`: main first, then by component, ties in
    /// file order. Empty when none matches; lintian then indexes no orig files.
    fn orig_components(&self) -> Vec<(String, String)> {
        let noepoch = strip_epoch(self.version());
        // `s/(.+)-(?:.*)$/$1/`: the last `-` that has something before it.
        let upstream = match noepoch.rfind('-') {
            Some(index) if index > 0 => &noepoch[..index],
            _ => noepoch,
        };
        let source = self.source();
        let pattern = Regex::new(&format!(
            r"^(?:{}\.orig(?:-(.*))?|{})\.tar\.(?:gz|bz2|lzma|xz)$",
            regex::escape(&format!("{source}_{upstream}")),
            regex::escape(&format!("{source}_{noepoch}")),
        ))
        .expect("orig tarball name pattern");
        let mut found: Vec<(String, String)> = self
            .listed
            .iter()
            .filter_map(|filename| {
                let captures = pattern.captures(filename)?;
                let component = captures.get(1).map_or("", |m| m.as_str());
                Some((filename.clone(), component.to_string()))
            })
            .collect();
        found.sort_by(|left, right| left.1.cmp(&right.1));
        found
    }
}

/// Throwaway extract of a Source package: Orig files and Patched files on disk.
#[derive(Debug)]
pub(crate) struct SourcePackageExtract {
    pub package_name: String,
    pub dsc: Dsc,
    orig_root: Option<PathBuf>,
    patched_root: PathBuf,
    /// What dpkg-source printed while extracting the Patched files.
    patched_messages: Vec<String>,
    /// What tar printed while extracting each Orig tarball, as
    /// `<tarball> . <message>` (lintian's `orig->unpack_messages`).
    orig_messages: Vec<String>,
    _workspace: TempDir,
}

impl SourcePackageExtract {
    pub fn from_dsc(path: PathBuf) -> anyhow::Result<Self> {
        let identity = DscIdentity::load(&path)?;
        source_extract::ensure_files_present(identity.directory(), &identity.files)
            .with_context(|| format!("{} names a missing file", identity.path.display()))?;
        let workspace = TempDir::new().context("creating Source package extract directory")?;
        let patched_root = workspace.path().join("patched");
        // The Patched and Orig trees are independent (separate directories), so
        // extract them at once. A Patched failure is still reported first, as
        // it was when the Orig tree was extracted after it.
        let (patched, orig) = std::thread::scope(|scope| {
            let orig = scope.spawn(|| extract_orig_tree(&identity, workspace.path()));
            let patched = extract_patched_tree(&identity, workspace.path(), &patched_root);
            (patched, orig.join())
        });
        let patched_messages = patched?;
        let (orig_root, orig_messages) = match orig {
            Ok(result) => result?,
            Err(panic) => std::panic::resume_unwind(panic),
        };

        Ok(Self {
            package_name: identity.package_name(),
            dsc: identity.dsc,
            orig_root,
            patched_root,
            patched_messages,
            orig_messages,
            _workspace: workspace,
        })
    }

    pub fn orig_files(&self) -> anyhow::Result<FileTree> {
        match &self.orig_root {
            Some(root) => FileTree::from_host_directory(root),
            None => Ok(FileTree::from_entries(None, Vec::new())),
        }
    }

    pub fn patched_files(&self) -> anyhow::Result<FileTree> {
        FileTree::from_host_directory(&self.patched_root)
    }

    pub fn patched_root(&self) -> &Path {
        &self.patched_root
    }

    pub fn patched_messages(&self) -> &[String] {
        &self.patched_messages
    }

    pub fn orig_messages(&self) -> &[String] {
        &self.orig_messages
    }
}

/// The stderr lines lintian keeps as unpack messages: all but its
/// `tar: Ignoring ` ones.
fn unpack_message_lines(stderr: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stderr)
        .lines()
        .filter(|line| !line.starts_with("tar: Ignoring "))
        .map(str::to_string)
        .collect()
}

/// `dpkg-source --extract` skips `*/.pc` so a hostile tarball cannot plant
/// symlinks quilt would follow. Lintian still sees `debian/.pc` as shipped, so
/// put it back from the debian tarball.
fn restore_debian_pc(
    identity: &DscIdentity,
    workspace: &Path,
    patched_root: &Path,
) -> anyhow::Result<()> {
    let target = patched_root.join("debian/.pc");
    if fs::symlink_metadata(&target).is_ok() {
        return Ok(());
    }
    let Some(tarball) = identity.files.iter().find(|name| {
        let name = name.rsplit('/').next().unwrap_or(name);
        name.contains(".debian.tar.")
    }) else {
        return Ok(());
    };
    let stage = workspace.join("debian-stage");
    source_extract::extract_tarball(&identity.directory().join(tarball), &stage)?;
    let source = stage.join("debian/.pc");
    if fs::symlink_metadata(&source).is_ok() {
        fs::create_dir_all(patched_root.join("debian"))
            .with_context(|| format!("creating {}", patched_root.join("debian").display()))?;
        fs::rename(&source, &target)
            .with_context(|| format!("moving {} to {}", source.display(), target.display()))?;
    }
    Ok(())
}

/// The Patched tree under `patched_root` (with `debian/.pc` restored), and the
/// messages dpkg-source printed.
fn extract_patched_tree(
    identity: &DscIdentity,
    workspace: &Path,
    patched_root: &Path,
) -> anyhow::Result<Vec<String>> {
    let stderr = source_extract::dpkg_source_extract(&identity.path, patched_root)?;
    let messages = unpack_message_lines(&stderr);
    restore_debian_pc(identity, workspace, patched_root)?;
    Ok(messages)
}

/// The Orig tree under `workspace/orig`, with the messages tar printed. `None`
/// for a Native package, which has no Orig tree.
fn extract_orig_tree(
    identity: &DscIdentity,
    workspace: &Path,
) -> anyhow::Result<(Option<PathBuf>, Vec<String>)> {
    if identity.is_native() {
        return Ok((None, Vec::new()));
    }
    let orig_root = workspace.join("orig");
    fs::create_dir_all(&orig_root).with_context(|| format!("creating {}", orig_root.display()))?;
    let messages = extract_orig(identity, &orig_root)?;
    Ok((Some(orig_root), messages))
}

/// Extract the orig components into `orig_root` and return lintian's
/// `orig->unpack_messages`: each tarball's tar messages (but `tar: Ignoring`),
/// each once, as `<tarball> . <message>`.
fn extract_orig(identity: &DscIdentity, orig_root: &Path) -> anyhow::Result<Vec<String>> {
    let components = identity.orig_components();
    let stderr =
        source_extract::extract_orig_components(identity.directory(), &components, orig_root)?;
    let mut messages = Vec::new();
    for ((filename, _), stderr) in components.iter().zip(stderr) {
        let mut tarball_messages = Vec::<String>::new();
        for line in unpack_message_lines(&stderr) {
            if !tarball_messages.contains(&line) {
                tarball_messages.push(line);
            }
        }
        messages.extend(
            tarball_messages
                .into_iter()
                .map(|line| format!("{filename} . {line}")),
        );
    }
    Ok(messages)
}

/// One entry of an Orig tarball as lintian's orig index names it: its path in
/// the Orig files, whether it is a directory, and the modification time in its
/// tar header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrigEntry {
    /// The name lintian's orig index gives the entry: the top directory is
    /// dropped, and a `./` segment after it is kept (lintian does not strip it).
    pub path: PathBuf,
    pub is_dir: bool,
    pub mtime: Option<i64>,
    /// A regular file or a hardlink, as lintian's `is_file` counts them.
    pub is_file: bool,
}

/// The entries of the Orig tarballs of the `.dsc` at `dsc_path`, read from
/// their tar headers beside it (each tarball is decompressed as a stream, not
/// extracted). Empty for a Native package.
pub(crate) fn orig_entries(dsc_path: &Path, dsc: &Dsc) -> anyhow::Result<Vec<OrigEntry>> {
    let identity = DscIdentity::new(dsc_path, dsc.clone());
    if identity.is_native() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for (filename, component) in identity.orig_components() {
        let members = tarball_members(&identity.directory().join(&filename))?;
        for member in drop_common_top_directory(members, &component) {
            let path = if component.is_empty() {
                member.path
            } else {
                Path::new(&component).join(member.path)
            };
            entries.push(OrigEntry {
                path,
                is_dir: member.is_dir,
                mtime: member.mtime,
                is_file: member.is_file,
            });
        }
    }
    // lintian's orig index is a hash by name (`$catalog{$entry->name} = $entry`),
    // so a member the tarball lists twice is the last entry, in name order.
    let mut by_path = std::collections::BTreeMap::new();
    for entry in entries {
        by_path.insert(entry.path.clone(), entry);
    }
    Ok(by_path.into_values().collect())
}

/// A member of an Orig tarball, as its header describes it.
struct TarMember {
    path: PathBuf,
    is_dir: bool,
    is_file: bool,
    mtime: Option<i64>,
}

/// The members of the tarball at `path`, in archive order.
fn tarball_members(path: &Path) -> anyhow::Result<Vec<TarMember>> {
    let mut archive = tar::Archive::new(open_tarball(path)?);
    let mut members = Vec::new();
    for entry in archive
        .entries()
        .with_context(|| format!("reading {}", path.display()))?
    {
        let mut entry = entry.with_context(|| format!("reading {}", path.display()))?;
        // A pax global header (GitHub archives start with `pax_global_header`)
        // is metadata, not a member: tar's listing, which lintian reads, omits it.
        if entry.header().entry_type().is_pax_global_extensions() {
            continue;
        }
        let name = entry
            .path()
            .with_context(|| format!("reading {}", path.display()))?;
        let Some(normalized) = normalize_member_path(&name) else {
            continue;
        };
        let kind = entry.header().entry_type();
        let mtime = pax_mtime(&mut entry).or_else(|| tarball_header_mtime(entry.header()));
        members.push(TarMember {
            path: normalized,
            is_dir: kind.is_dir(),
            is_file: kind.is_file() || kind.is_hard_link(),
            mtime,
        });
    }
    Ok(members)
}

/// The ustar header's mtime. tar-rs rejects an mtime field that is all NULs,
/// which some tarballs write for a member dated the epoch; tar and lintian read
/// it as 0 (1970-01-01), so it is dated 0 here.
fn tarball_header_mtime(header: &tar::Header) -> Option<i64> {
    header_mtime(header).or_else(|| {
        // The `mtime` field is bytes 136..148 of the ustar header.
        let field = &header.as_bytes()[136..148];
        field
            .iter()
            .all(|&byte| byte == 0 || byte == b' ')
            .then_some(0)
    })
}

/// The whole seconds of the pax `mtime` extended header of `entry`, truncated
/// as tar does (`1745484012.237914` is `1745484012`). `None` when there is no
/// such record. The ustar header's mtime is then the one tar reports.
fn pax_mtime<R: std::io::Read>(entry: &mut tar::Entry<'_, R>) -> Option<i64> {
    let extensions = entry.pax_extensions().ok()??;
    extensions.filter_map(Result::ok).find_map(|extension| {
        if extension.key().ok()? != "mtime" {
            return None;
        }
        let value = extension.value().ok()?;
        let seconds = value.split('.').next()?;
        seconds.parse::<i64>().ok()
    })
}

/// The tarball at `path`, decompressed as a stream. Reads the compressions the
/// orig tarball pattern names: gz, bz2, xz and zst (lzma is not read).
fn open_tarball(path: &Path) -> anyhow::Result<Box<dyn std::io::Read>> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    compression::decoder(name, file).with_context(|| format!("reading {}", path.display()))
}

/// lintian's orig index names: the Orig files of one tarball lose the one
/// top directory that every member shares, as the extraction does, unless a
/// file of that name exists. The component directory is kept (the caller adds
/// it). Entries left with no name are dropped.
fn drop_common_top_directory(members: Vec<TarMember>, component: &str) -> Vec<TarMember> {
    let common: PathBuf = {
        let tops: BTreeSet<&OsStr> = members
            .iter()
            .filter_map(|member| member.path.iter().next())
            .collect();
        match tops.iter().next() {
            Some(top) if tops.len() == 1 => PathBuf::from(top),
            _ => PathBuf::new(),
        }
    };
    let conflict = members
        .iter()
        .any(|member| !member.is_dir && member.path == common);
    let strip = !common.as_os_str().is_empty()
        && !conflict
        && (common.as_os_str() != OsStr::new(component) || !component.is_empty());
    members
        .into_iter()
        .filter_map(|member| {
            let path = if strip {
                strip_top_directory(&member.path, &common)?
            } else {
                member.path.clone()
            };
            (!path.as_os_str().is_empty()).then_some(TarMember { path, ..member })
        })
        .collect()
}

/// `path` without its leading `top/` segment, every other byte kept: a `./`
/// segment after the top directory survives, as in lintian's orig index.
fn strip_top_directory(path: &Path, top: &Path) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes();
    let top = top.as_os_str().as_bytes();
    if bytes == top {
        return Some(PathBuf::new());
    }
    let rest = bytes.strip_prefix(top)?.strip_prefix(b"/")?;
    Some(PathBuf::from(OsStr::from_bytes(rest)))
}

/// lintian's `s/^\d+://`: one leading all-digit epoch. A version whose
/// prefix before `:` is not all digits is returned unchanged.
pub(crate) fn strip_epoch(version: &str) -> &str {
    match version.split_once(':') {
        Some((epoch, rest)) if !epoch.is_empty() && epoch.bytes().all(|b| b.is_ascii_digit()) => {
            rest
        }
        _ => version,
    }
}

pub(crate) fn strip_debian_revision(version: &str) -> &str {
    match version.rsplit_once('-') {
        Some((upstream, _)) => upstream,
        None => version,
    }
}

fn regex_is_match(pattern: &str, value: &str) -> bool {
    Regex::new(pattern)
        .expect("static format pattern")
        .is_match(value)
}

#[cfg(test)]
pub(crate) fn write_test_native_dsc(
    work: &Path,
    name: &str,
    extra_files: &[(&str, &[u8])],
) -> anyhow::Result<PathBuf> {
    write_test_native_tree(work, name, extra_files)?;
    build_test_native_dsc(work, name)
}

/// The `src` tree of a native package in `work`: `debian/` and `extra_files`,
/// not yet built. A test may age its files before [`build_test_native_dsc`].
#[cfg(test)]
pub(crate) fn write_test_native_tree(
    work: &Path,
    name: &str,
    extra_files: &[(&str, &[u8])],
) -> anyhow::Result<PathBuf> {
    let tree = work.join("src");
    fs::create_dir_all(tree.join("debian/source"))?;
    fs::write(
        tree.join("debian/control"),
        format!(
            "Source: {name}\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             \n\
             Package: {name}\n\
             Architecture: all\n\
             Description: example\n extra\n"
        ),
    )?;
    fs::write(
        tree.join("debian/changelog"),
        format!(
            "{name} (1.0) unstable; urgency=low\n\n  * test\n\n -- a <a@localhost>  Tue, 30 Dec 2008 17:34:02 -0800\n"
        ),
    )?;
    let rules = tree.join("debian/rules");
    fs::write(&rules, "#!/usr/bin/make -f\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&rules)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&rules, permissions)?;
    }
    fs::write(tree.join("debian/source/format"), "3.0 (native)\n")?;
    for (relative, contents) in extra_files {
        let path = tree.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)?;
    }
    Ok(tree)
}

/// Builds the `src` tree of [`write_test_native_tree`] into `<name>_1.0.dsc`
/// in `work`, and returns that `.dsc`.
#[cfg(test)]
pub(crate) fn build_test_native_dsc(work: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let output = Command::new("dpkg-source")
        .current_dir(work)
        .args(["-b", "src"])
        .stdin(Stdio::null())
        .output()
        .context("running dpkg-source -b")?;
    if !output.status.success() {
        bail!(
            "dpkg-source -b failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let dsc = work.join(format!("{name}_1.0.dsc"));
    if !dsc.is_file() {
        bail!("dpkg-source -b did not produce {}", dsc.display());
    }
    Ok(dsc)
}

/// Writes a `3.0 (quilt)` package `<name>_1.0-1.dsc` into `work`, with an orig
/// tarball of `orig` members (name, contents, header mtime) and a gzipped
/// debian tarball. Built by hand, not by `dpkg-source -b`, so a member name can
/// be one tar would warn about. `dpkg-source --no-check` extracts it.
#[cfg(test)]
pub(crate) fn write_test_quilt_dsc(
    work: &Path,
    name: &str,
    orig: &[(&str, &[u8], u64)],
) -> anyhow::Result<PathBuf> {
    let orig_name = format!("{name}_1.0.orig.tar.gz");
    let debian_name = format!("{name}_1.0-1.debian.tar.gz");
    let orig_bytes = tar_gz_with_mtimes(orig)?;
    let debian_bytes = tar_gz_with_mtimes(&[
        ("debian/source/format", b"3.0 (quilt)\n", 0),
        ("debian/rules", b"#!/usr/bin/make -f\n", 0),
    ])?;
    fs::write(work.join(&orig_name), &orig_bytes)?;
    fs::write(work.join(&debian_name), &debian_bytes)?;
    let checksum = "0".repeat(32);
    let dsc = format!(
        "Format: 3.0 (quilt)\n\
         Source: {name}\n\
         Binary: {name}\n\
         Architecture: all\n\
         Version: 1.0-1\n\
         Maintainer: Example <ex@example.com>\n\
         Files:\n \
         {checksum} {} {orig_name}\n \
         {checksum} {} {debian_name}\n",
        orig_bytes.len(),
        debian_bytes.len(),
    );
    let path = work.join(format!("{name}_1.0-1.dsc"));
    fs::write(&path, dsc)?;
    Ok(path)
}

/// A gzipped tar of `members` (name, contents, header mtime).
#[cfg(test)]
fn tar_gz_with_mtimes(members: &[(&str, &[u8], u64)]) -> anyhow::Result<Vec<u8>> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        for (member, contents, mtime) in members {
            let mut header = tar::Header::new_gnu();
            // Raw name bytes: `set_path` refuses an absolute member name, which
            // a test of tar's leading-slash warning needs.
            let name = member.as_bytes();
            header.as_old_mut().name[..name.len()].copy_from_slice(name);
            if member.ends_with('/') {
                header.set_entry_type(tar::EntryType::Directory);
                header.set_mode(0o755);
            } else {
                header.set_mode(0o644);
            }
            header.set_size(contents.len() as u64);
            header.set_mtime(*mtime);
            header.set_cksum();
            builder.append(&header, *contents)?;
        }
        builder.finish()?;
    }
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&tar_bytes)?;
    Ok(encoder.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    #[test]
    fn strip_epoch_strips_one_all_digit_epoch() {
        assert_eq!(strip_epoch("1:1.0-1"), "1.0-1");
        assert_eq!(strip_epoch("10:1.0"), "1.0");
        assert_eq!(strip_epoch("1.0-1"), "1.0-1");
        assert_eq!(strip_epoch("a:1.0"), "a:1.0");
        assert_eq!(strip_epoch(":1.0"), ":1.0");
        assert_eq!(strip_epoch("1:2:3"), "2:3");
    }
    use std::io::Write;
    use tempfile::TempDir;

    fn write_dsc(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("example_1.0.dsc");
        fs::write(&path, body).expect("write dsc");
        path
    }

    fn dsc_with_files(header: &str, files: &[&str]) -> String {
        let mut body = header.to_string();
        if !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str("Files:\n");
        for file in files {
            body.push(' ');
            body.push_str(file);
            body.push('\n');
        }
        body
    }

    fn plain_tar(members: &[(&str, &[u8])]) -> anyhow::Result<Vec<u8>> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for (member, contents) in members {
                let mut header = tar::Header::new_gnu();
                header.set_path(*member)?;
                header.set_size(contents.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append(&header, *contents)?;
            }
            builder.finish()?;
        }
        Ok(tar_bytes)
    }

    fn gzip_tar(members: &[(&str, &[u8])]) -> anyhow::Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&plain_tar(members)?)?;
        Ok(encoder.finish()?)
    }

    fn bzip2_tar(members: &[(&str, &[u8])]) -> anyhow::Result<Vec<u8>> {
        let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
        encoder.write_all(&plain_tar(members)?)?;
        Ok(encoder.finish()?)
    }

    /// A member whose ustar header says mtime 0 but whose pax `mtime` record
    /// gives the real time is dated by the record, truncated to whole seconds.
    #[test]
    fn pax_mtime_overrides_the_header_mtime() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example_1.0.orig.tar");
        {
            let mut builder = tar::Builder::new(fs::File::create(&path)?);
            builder.append_pax_extensions([("mtime", b"1745484012.237914".as_slice())])?;
            let mut header = tar::Header::new_ustar();
            header.set_path("example-1.0/README")?;
            header.set_size(5);
            header.set_mode(0o644);
            header.set_mtime(0);
            header.set_cksum();
            builder.append(&header, &b"text\n"[..])?;
            builder.finish()?;
        }
        let members = tarball_members(&path)?;
        let readme = members
            .iter()
            .find(|member| member.path == Path::new("example-1.0/README"))
            .expect("README member");
        assert_eq!(readme.mtime, Some(1_745_484_012));
        Ok(())
    }

    /// A member whose ustar mtime field is all NULs (not octal zeros) is dated
    /// the epoch, as tar and lintian read it, rather than left undated.
    #[test]
    fn all_nul_mtime_field_is_the_epoch() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example_1.0.orig.tar");
        {
            let mut builder = tar::Builder::new(fs::File::create(&path)?);
            let mut header = tar::Header::new_ustar();
            header.set_path("example-1.0/Cargo.toml")?;
            header.set_size(5);
            header.set_mode(0o644);
            header.as_mut_bytes()[136..148].fill(0);
            header.set_cksum();
            builder.append(&header, &b"text\n"[..])?;
            builder.finish()?;
        }
        let members = tarball_members(&path)?;
        assert_eq!(members[0].mtime, Some(0));
        Ok(())
    }

    #[test]
    fn pax_global_header_is_not_a_member() -> anyhow::Result<()> {
        // GitHub archives start with a `pax_global_header` entry; counting it
        // as a member would leave no common top directory to drop.
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            let comment = b"52 comment=44380e5a44706ab7347f400698c703eb2a196202\n";
            let mut global = tar::Header::new_ustar();
            global.set_path("pax_global_header")?;
            global.set_entry_type(tar::EntryType::XGlobalHeader);
            global.set_size(comment.len() as u64);
            global.set_cksum();
            builder.append(&global, &comment[..])?;
            let mut file = tar::Header::new_gnu();
            file.set_path("example-1.0/test/catch.hpp")?;
            file.set_size(1);
            file.set_mode(0o644);
            file.set_cksum();
            builder.append(&file, &b"x"[..])?;
            builder.finish()?;
        }
        let dir = TempDir::new()?;
        let tarball = dir.path().join("example_1.0.orig.tar");
        fs::write(&tarball, tar_bytes)?;
        let members = tarball_members(&tarball)?;
        let names: Vec<_> = drop_common_top_directory(members, "")
            .into_iter()
            .map(|member| member.path)
            .collect();
        assert_eq!(names, vec![PathBuf::from("test/catch.hpp")]);
        Ok(())
    }

    #[test]
    fn orig_tarball_bz2_is_read() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        fs::write(
            dir.path().join("example_1.0.orig.tar.bz2"),
            bzip2_tar(&[("example-1.0/src/main.c", b"int main(){}\n")])?,
        )?;
        fs::write(dir.path().join("example_1.0-1.debian.tar.xz"), b"x")?;
        let path = write_dsc(
            dir.path(),
            &dsc_with_files(
                "Format: 3.0 (quilt)\nSource: example\nVersion: 1.0-1\n",
                &[
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0.orig.tar.bz2",
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0-1.debian.tar.xz",
                ],
            ),
        );
        let dsc = DscIdentity::load(&path)?;
        let members = tarball_members(&dir.path().join("example_1.0.orig.tar.bz2"))?;
        assert!(
            members
                .iter()
                .any(|m| m.path == Path::new("example-1.0/src/main.c"))
        );
        let orig_root = dir.path().join("orig");
        fs::create_dir_all(&orig_root)?;
        extract_orig(&dsc, &orig_root)?;
        assert!(orig_root.join("src/main.c").is_file());
        Ok(())
    }

    #[test]
    fn native_format_3_0_native() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("example_1.0.tar.xz"), b"not a tar")?;
        let path = write_dsc(
            dir.path(),
            &dsc_with_files(
                "Format: 3.0 (native)\nSource: example\nVersion: 1.0\n",
                &["d41d8cd98f00b204e9800998ecf8427e 9 example_1.0.tar.xz"],
            ),
        );
        let dsc = DscIdentity::load(&path)?;
        assert!(dsc.is_native());
        assert_eq!(dsc.package_name(), "example");
        Ok(())
    }

    #[test]
    fn quilt_is_not_native_and_finds_orig_component() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("example_1.0.orig.tar.gz"), b"x")?;
        fs::write(dir.path().join("example_1.0.orig-docs.tar.gz"), b"x")?;
        fs::write(dir.path().join("example_1.0-1.debian.tar.xz"), b"x")?;
        let path = write_dsc(
            dir.path(),
            &dsc_with_files(
                "Format: 3.0 (quilt)\nSource: example\nVersion: 1.0-1\n",
                &[
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0.orig.tar.gz",
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0.orig-docs.tar.gz",
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0-1.debian.tar.xz",
                ],
            ),
        );
        let dsc = DscIdentity::load(&path)?;
        assert!(!dsc.is_native());
        let components = dsc.orig_components();
        assert_eq!(
            components,
            vec![
                ("example_1.0.orig.tar.gz".into(), String::new()),
                ("example_1.0.orig-docs.tar.gz".into(), "docs".into()),
            ]
        );
        Ok(())
    }

    #[test]
    fn orig_components_follow_lintians_pattern() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = write_dsc(
            dir.path(),
            &dsc_with_files(
                "Format: 3.0 (quilt)\nSource: example\nVersion: 2:1.0-1\n",
                &[
                    // lintian 2.144 does not know zst as an orig compression.
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0.orig.tar.zst",
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0.orig-b.tar.lzma",
                    // The native name `{source}_{version}` counts too.
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0-1.tar.gz",
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0-1.debian.tar.xz",
                ],
            ),
        );
        let dsc = DscIdentity::load(&path)?;
        assert_eq!(
            dsc.orig_components(),
            vec![
                ("example_1.0-1.tar.gz".into(), String::new()),
                ("example_1.0.orig-b.tar.lzma".into(), "b".into()),
            ]
        );
        Ok(())
    }

    #[test]
    fn no_orig_tarball_is_an_empty_list_not_an_error() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = write_dsc(
            dir.path(),
            &dsc_with_files(
                "Format: 3.0 (quilt)\nSource: example\nVersion: 1.0-1\n",
                &["d41d8cd98f00b204e9800998ecf8427e 1 example_1.0.orig.tar.zst"],
            ),
        );
        let dsc = DscIdentity::load(&path)?;
        assert!(!dsc.is_native());
        assert!(dsc.orig_components().is_empty());
        Ok(())
    }

    #[test]
    fn format_1_0_with_diff_gz_is_not_native() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("example_1.0.orig.tar.gz"), b"x")?;
        fs::write(dir.path().join("example_1.0.diff.gz"), b"x")?;
        let path = write_dsc(
            dir.path(),
            &dsc_with_files(
                "Source: example\nVersion: 1.0\n",
                &[
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0.orig.tar.gz",
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0.diff.gz",
                ],
            ),
        );
        let dsc = DscIdentity::load(&path)?;
        assert!(!dsc.is_native());
        Ok(())
    }

    #[test]
    fn missing_named_file_is_runtime_failure() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = write_dsc(
            dir.path(),
            &dsc_with_files(
                "Format: 3.0 (native)\nSource: example\nVersion: 1.0\n",
                &["d41d8cd98f00b204e9800998ecf8427e 9 example_1.0.tar.xz"],
            ),
        );
        let error = SourcePackageExtract::from_dsc(path).expect_err("missing named file");
        assert!(error.to_string().contains("missing file"), "{error}");
        Ok(())
    }

    #[test]
    fn empty_dsc_is_runtime_failure() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = write_dsc(dir.path(), "");
        let error = Dsc::load(&path).expect_err("empty dsc");
        assert!(
            error.to_string().contains("Deb822")
                || error.to_string().contains("paragraph")
                || error.to_string().contains("readable .dsc"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn orig_extract_drops_common_prefix_and_merges_component() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        fs::write(
            dir.path().join("example_1.0.orig.tar.gz"),
            gzip_tar(&[("example-1.0/src/main.c", b"int main(){}\n")])?,
        )?;
        fs::write(
            dir.path().join("example_1.0.orig-docs.tar.gz"),
            gzip_tar(&[("docs-1.0/manual.pdf", b"%PDF\n")])?,
        )?;
        fs::write(dir.path().join("example_1.0-1.debian.tar.xz"), b"x")?;
        let path = write_dsc(
            dir.path(),
            &dsc_with_files(
                "Format: 3.0 (quilt)\nSource: example\nVersion: 1.0-1\n",
                &[
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0.orig.tar.gz",
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0.orig-docs.tar.gz",
                    "d41d8cd98f00b204e9800998ecf8427e 1 example_1.0-1.debian.tar.xz",
                ],
            ),
        );
        let dsc = DscIdentity::load(&path)?;
        let orig_root = dir.path().join("orig");
        fs::create_dir_all(&orig_root)?;
        extract_orig(&dsc, &orig_root)?;
        assert!(orig_root.join("src/main.c").is_file());
        assert!(orig_root.join("docs/manual.pdf").is_file());
        assert!(!orig_root.join("example-1.0").exists());
        Ok(())
    }

    #[test]
    fn package_name_falls_back_to_filename() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("foo_1.0.tar.xz"), b"x")?;
        let path = dir.path().join("foo_1.0.dsc");
        fs::write(
            &path,
            dsc_with_files(
                "Format: 3.0 (native)\nVersion: 1.0\n",
                &["d41d8cd98f00b204e9800998ecf8427e 1 foo_1.0.tar.xz"],
            ),
        )?;
        let dsc = DscIdentity::load(&path)?;
        assert_eq!(dsc.package_name(), "foo");
        Ok(())
    }

    fn member(path: &str, is_dir: bool) -> TarMember {
        TarMember {
            path: PathBuf::from(path),
            is_dir,
            is_file: !is_dir,
            mtime: None,
        }
    }

    #[test]
    fn orig_names_keep_a_dot_segment_after_the_top_directory() {
        // vulkan-loader's orig tarball lists `top/./loader/x`; lintian's index
        // drops only `top/`, so the name keeps its `./` (LN0008 depends on it).
        let members = vec![
            member("vulkan-loader-1.0", true),
            member("vulkan-loader-1.0/./loader/cJSON.c", false),
            member("vulkan-loader-1.0/loader/plain.c", false),
        ];
        let names: Vec<PathBuf> = drop_common_top_directory(members, "")
            .into_iter()
            .map(|member| member.path)
            .collect();
        assert_eq!(
            names,
            vec![
                PathBuf::from("./loader/cJSON.c"),
                PathBuf::from("loader/plain.c")
            ]
        );
    }
}
