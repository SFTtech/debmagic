use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, bail};
use regex::Regex;
use tempfile::TempDir;

use super::binary_package_reader::{guess_package_name, header_mtime, normalize_member_path};
use super::debian_control::ControlParagraph;
use super::dsc::Dsc;
use super::file_tree::FileTree;

/// `.dsc` identity for extract: Dsc plus the Files list and path.
#[derive(Debug)]
struct DscIdentity {
    path: PathBuf,
    dsc: Dsc,
    files: Vec<String>,
}

impl DscIdentity {
    fn load(path: &Path) -> anyhow::Result<Self> {
        let dsc = Dsc::load(path)?;
        let files = named_files(dsc.fields());
        if files.is_empty() {
            bail!("{} names no files", path.display());
        }
        Ok(Self {
            path: path.to_path_buf(),
            dsc,
            files,
        })
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
        !self.files.iter().any(|name| name == &diffname)
    }

    fn orig_components(&self) -> anyhow::Result<Vec<(String, String)>> {
        let name = self.package_name();
        let noepoch = strip_epoch(self.version());
        let upstream = strip_debian_revision(noepoch);
        let base = format!("{name}_{upstream}");
        let pattern = Regex::new(&format!(
            r"^{}\.orig(?:-(.*))?\.tar\.(?:gz|bz2|lzma|xz|zst)$",
            regex::escape(&base)
        ))
        .expect("orig tarball name pattern");
        let mut found = Vec::new();
        for filename in &self.files {
            let basename = Path::new(filename)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(filename);
            if let Some(captures) = pattern.captures(basename) {
                let component = captures
                    .get(1)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                found.push((filename.clone(), component));
            }
        }
        if found.is_empty() {
            bail!(
                "{} is not Native and names no orig tarball",
                self.path.display()
            );
        }
        Ok(found)
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
        ensure_named_files_exist(&identity)?;
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

fn ensure_named_files_exist(identity: &DscIdentity) -> anyhow::Result<()> {
    let dir = identity.directory();
    for filename in &identity.files {
        let path = dir.join(filename);
        if !path.is_file() {
            bail!(
                "{} names missing file {}",
                identity.path.display(),
                path.display()
            );
        }
    }
    Ok(())
}

fn extract_patched(dsc: &Path, dest: &Path) -> anyhow::Result<Vec<String>> {
    let output = Command::new("dpkg-source")
        .args(["-q", "--no-check", "--extract"])
        .arg(dsc)
        .arg(dest)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            if error.kind() == ErrorKind::NotFound {
                anyhow::anyhow!("dpkg-source is unavailable")
            } else {
                anyhow::anyhow!("running dpkg-source failed: {error}")
            }
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "dpkg-source --extract {} failed{}",
            dsc.display(),
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", stderr.trim())
            }
        );
    }
    Ok(unpack_message_lines(&output.stderr))
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
    extract_tarball(&identity.directory().join(tarball), &stage)?;
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
    let messages = extract_patched(&identity.path, patched_root)?;
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

fn extract_orig(identity: &DscIdentity, orig_root: &Path) -> anyhow::Result<Vec<String>> {
    let dir = identity.directory();
    let mut messages = Vec::new();
    for (index, (filename, component)) in identity.orig_components()?.into_iter().enumerate() {
        let tarball = dir.join(&filename);
        let stage = orig_root
            .parent()
            .unwrap_or(orig_root)
            .join(format!("orig-stage-{index}"));
        let mut tarball_messages = Vec::<String>::new();
        for line in unpack_message_lines(&extract_tarball(&tarball, &stage)?) {
            if !tarball_messages.contains(&line) {
                tarball_messages.push(line);
            }
        }
        messages.extend(
            tarball_messages
                .into_iter()
                .map(|line| format!("{filename} . {line}")),
        );
        let stripped = apply_common_prefix(&stage)?;
        let dest = if component.is_empty() {
            orig_root.to_path_buf()
        } else {
            orig_root.join(&component)
        };
        merge_tree(&stripped, &dest)?;
    }
    Ok(messages)
}

/// Extracts `tarball` into `dest` and returns the stderr lines tar printed.
/// `--touch` keeps the header time stamps off the extracted files, as
/// lintian's extraction does, so tar does not warn about ancient ones.
fn extract_tarball(tarball: &Path, dest: &Path) -> anyhow::Result<Vec<u8>> {
    fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?;
    let output = Command::new("tar")
        .arg("-C")
        .arg(dest)
        .args(["--no-same-owner", "--touch", "-xf"])
        .arg(tarball)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            if error.kind() == ErrorKind::NotFound {
                anyhow::anyhow!("tar is unavailable")
            } else {
                anyhow::anyhow!("running tar failed: {error}")
            }
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "extracting {} failed{}",
            tarball.display(),
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", stderr.trim())
            }
        );
    }
    Ok(output.stderr)
}

fn apply_common_prefix(extract_dir: &Path) -> anyhow::Result<PathBuf> {
    let mut dirs = Vec::new();
    let mut other = 0usize;
    let read =
        fs::read_dir(extract_dir).with_context(|| format!("reading {}", extract_dir.display()))?;
    for entry in read {
        let entry = entry.with_context(|| format!("reading {}", extract_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("stat {}", entry.path().display()))?;
        if file_type.is_dir() && !file_type.is_symlink() {
            dirs.push(entry.path());
        } else {
            other += 1;
        }
    }
    if other == 0 && dirs.len() == 1 {
        Ok(dirs.remove(0))
    } else {
        Ok(extract_dir.to_path_buf())
    }
}

fn merge_tree(src: &Path, dest: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?;
    let read = fs::read_dir(src).with_context(|| format!("reading {}", src.display()))?;
    for entry in read {
        let entry = entry.with_context(|| format!("reading {}", src.display()))?;
        let to = dest.join(entry.file_name());
        fs::rename(entry.path(), &to)
            .with_context(|| format!("moving {} to {}", entry.path().display(), to.display()))?;
    }
    Ok(())
}

/// One entry of an Orig tarball as lintian's orig index names it: its path in
/// the Orig files, and the modification time in its tar header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrigEntry {
    pub path: PathBuf,
    pub mtime: Option<i64>,
}

/// The entries of the Orig tarballs of the `.dsc` at `dsc_path`, read from
/// their tar headers beside it (each tarball is decompressed as a stream, not
/// extracted). Empty for a Native package.
pub(crate) fn orig_entries(dsc_path: &Path, dsc: &Dsc) -> anyhow::Result<Vec<OrigEntry>> {
    let identity = DscIdentity {
        path: dsc_path.to_path_buf(),
        dsc: dsc.clone(),
        files: named_files(dsc.fields()),
    };
    if identity.is_native() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for (filename, component) in identity.orig_components()? {
        let members = tarball_members(&identity.directory().join(&filename))?;
        for (path, mtime) in drop_common_top_directory(members, &component) {
            let path = if component.is_empty() {
                path
            } else {
                Path::new(&component).join(path)
            };
            entries.push(OrigEntry { path, mtime });
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
        let entry = entry.with_context(|| format!("reading {}", path.display()))?;
        let name = entry
            .path()
            .with_context(|| format!("reading {}", path.display()))?;
        let Some(normalized) = normalize_member_path(&name) else {
            continue;
        };
        members.push(TarMember {
            path: normalized,
            is_dir: entry.header().entry_type().is_dir(),
            mtime: header_mtime(entry.header()),
        });
    }
    Ok(members)
}

/// The tarball at `path`, decompressed as a stream. bzip2 is not read.
fn open_tarball(path: &Path) -> anyhow::Result<Box<dyn std::io::Read>> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if name.ends_with(".gz") {
        Ok(Box::new(flate2::read::GzDecoder::new(file)))
    } else if name.ends_with(".xz") {
        Ok(Box::new(xz2::read::XzDecoder::new(file)))
    } else if name.ends_with(".zst") {
        Ok(Box::new(zstd::stream::read::Decoder::new(file)?))
    } else if name.ends_with(".tar") {
        Ok(Box::new(file))
    } else {
        bail!(
            "{} is compressed in a format debmagic does not read",
            path.display()
        )
    }
}

/// lintian's orig index names: the Orig files of one tarball lose the one
/// top directory that every member shares, as the extraction does, unless a
/// file of that name exists. The component directory is kept (the caller adds
/// it). Entries left with no name are dropped.
fn drop_common_top_directory(
    members: Vec<TarMember>,
    component: &str,
) -> Vec<(PathBuf, Option<i64>)> {
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
                member.path.strip_prefix(&common).ok()?.to_path_buf()
            } else {
                member.path
            };
            (!path.as_os_str().is_empty()).then_some((path, member.mtime))
        })
        .collect()
}

fn named_files(fields: &ControlParagraph) -> Vec<String> {
    let mut names = Vec::new();
    for field in ["Files", "Checksums-Sha1", "Checksums-Sha256"] {
        if let Some(value) = fields.get(field) {
            for name in filenames_from_checksums_field(value) {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
    }
    names
}

fn filenames_from_checksums_field(value: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in value.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            names.push(parts[parts.len() - 1].to_string());
        }
    }
    names
}

pub(crate) fn strip_epoch(version: &str) -> &str {
    match version.split_once(':') {
        Some((_, rest)) => rest,
        None => version,
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
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
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

    fn gzip_tar(members: &[(&str, &[u8])]) -> anyhow::Result<Vec<u8>> {
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
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar_bytes)?;
        Ok(encoder.finish()?)
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
        let components = dsc.orig_components()?;
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
}
