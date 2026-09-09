use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, bail};
use regex::Regex;
use tempfile::TempDir;

use super::binary_package_reader::guess_package_name;
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
    _workspace: TempDir,
}

impl SourcePackageExtract {
    pub fn from_dsc(path: PathBuf) -> anyhow::Result<Self> {
        let identity = DscIdentity::load(&path)?;
        ensure_named_files_exist(&identity)?;
        let workspace = TempDir::new().context("creating Source package extract directory")?;
        let patched_root = workspace.path().join("patched");
        extract_patched(&identity.path, &patched_root)?;

        let orig_root = if identity.is_native() {
            None
        } else {
            let orig_root = workspace.path().join("orig");
            fs::create_dir_all(&orig_root)
                .with_context(|| format!("creating {}", orig_root.display()))?;
            extract_orig(&identity, &orig_root)?;
            Some(orig_root)
        };

        Ok(Self {
            package_name: identity.package_name(),
            dsc: identity.dsc,
            orig_root,
            patched_root,
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

fn extract_patched(dsc: &Path, dest: &Path) -> anyhow::Result<()> {
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
    Ok(())
}

fn extract_orig(identity: &DscIdentity, orig_root: &Path) -> anyhow::Result<()> {
    let dir = identity.directory();
    for (index, (filename, component)) in identity.orig_components()?.into_iter().enumerate() {
        let tarball = dir.join(&filename);
        let stage = orig_root
            .parent()
            .unwrap_or(orig_root)
            .join(format!("orig-stage-{index}"));
        extract_tarball(&tarball, &stage)?;
        let stripped = apply_common_prefix(&stage)?;
        let dest = if component.is_empty() {
            orig_root.to_path_buf()
        } else {
            orig_root.join(&component)
        };
        merge_tree(&stripped, &dest)?;
    }
    Ok(())
}

fn extract_tarball(tarball: &Path, dest: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?;
    let output = Command::new("tar")
        .arg("-C")
        .arg(dest)
        .args(["--no-same-owner", "-xf"])
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
    Ok(())
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

fn strip_epoch(version: &str) -> &str {
    match version.split_once(':') {
        Some((_, rest)) => rest,
        None => version,
    }
}

fn strip_debian_revision(version: &str) -> &str {
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
