//! Extracting a Debian source package: `dpkg-source` for the patched tree,
//! `tar` for orig tarballs, and the merge of orig component tarballs
//! (`{name}_{version}.orig-{component}.tar.*` lands in `{component}/`).
//!
//! Which tarballs are orig components, and what the tools' messages mean, is
//! the caller's decision.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, bail};

/// `Err` naming the first of `names` that is not a file in `dir`.
pub fn ensure_files_present(dir: &Path, names: &[String]) -> anyhow::Result<()> {
    for name in names {
        let path = dir.join(name);
        if !path.is_file() {
            bail!("missing file {}", path.display());
        }
    }
    Ok(())
}

/// Unpack the source package `dsc` into `dest` with
/// `dpkg-source -q --no-check --extract`, and return what it printed on
/// stderr.
pub fn dpkg_source_extract(dsc: &Path, dest: &Path) -> anyhow::Result<Vec<u8>> {
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
    Ok(output.stderr)
}

/// Extract each `(tarball, component)` in order from `dir` into `orig_root`:
/// the tarball is unpacked into its own stage beside `orig_root`, its single
/// top directory (if any) is dropped, and the rest moves into `orig_root`, or
/// `orig_root/{component}` for a non-empty component. Returns what tar
/// printed for each tarball.
pub fn extract_orig_components(
    dir: &Path,
    components: &[(String, String)],
    orig_root: &Path,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut stderr = Vec::new();
    for (index, (filename, component)) in components.iter().enumerate() {
        let stage = orig_root
            .parent()
            .unwrap_or(orig_root)
            .join(format!("orig-stage-{index}"));
        stderr.push(extract_tarball(&dir.join(filename), &stage)?);
        let stripped = single_top_directory(&stage)?;
        let dest = if component.is_empty() {
            orig_root.to_path_buf()
        } else {
            orig_root.join(component)
        };
        merge_tree(&stripped, &dest)?;
    }
    Ok(stderr)
}

/// Extracts `tarball` into `dest` and returns the stderr lines tar printed.
/// `--touch` keeps the header time stamps off the extracted files, so tar
/// does not warn about ancient ones; owners are not restored.
pub fn extract_tarball(tarball: &Path, dest: &Path) -> anyhow::Result<Vec<u8>> {
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

/// The one directory `extract_dir` holds when it holds nothing else (the
/// usual `{name}-{version}/` top level of a tarball), else `extract_dir`
/// itself.
pub fn single_top_directory(extract_dir: &Path) -> anyhow::Result<PathBuf> {
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

/// Move every entry of `src` into `dest`, creating `dest`.
pub fn merge_tree(src: &Path, dest: &Path) -> anyhow::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `name` in `dir`, a tarball of `top` (relative to `root`).
    fn tarball(root: &Path, top: &str, dir: &Path, name: &str) -> anyhow::Result<()> {
        let status = Command::new("tar")
            .current_dir(root)
            .args(["-czf"])
            .arg(dir.join(name))
            .arg(top)
            .status()?;
        anyhow::ensure!(status.success(), "tar -c failed");
        Ok(())
    }

    #[test]
    fn orig_components_merge_below_their_names() -> anyhow::Result<()> {
        let work = tempfile::tempdir()?;
        let src = work.path().join("src");
        fs::create_dir_all(src.join("pkg-1.0/sub"))?;
        fs::write(src.join("pkg-1.0/sub/main.c"), "main")?;
        fs::create_dir_all(src.join("docs-1.0"))?;
        fs::write(src.join("docs-1.0/guide.txt"), "guide")?;
        // Two top-level entries: nothing is dropped.
        fs::create_dir_all(src.join("flat"))?;
        fs::write(src.join("flat/a"), "a")?;
        fs::write(src.join("flat/b"), "b")?;
        let dir = work.path().join("dsc");
        fs::create_dir_all(&dir)?;
        tarball(&src, "pkg-1.0", &dir, "pkg_1.0.orig.tar.gz")?;
        tarball(&src, "docs-1.0", &dir, "pkg_1.0.orig-docs.tar.gz")?;
        tarball(&src.join("flat"), ".", &dir, "pkg_1.0.orig-flat.tar.gz")?;

        let orig_root = work.path().join("out/orig");
        fs::create_dir_all(&orig_root)?;
        let components = [
            ("pkg_1.0.orig.tar.gz".to_string(), String::new()),
            ("pkg_1.0.orig-docs.tar.gz".to_string(), "docs".to_string()),
            ("pkg_1.0.orig-flat.tar.gz".to_string(), "flat".to_string()),
        ];
        let stderr = extract_orig_components(&dir, &components, &orig_root)?;
        assert_eq!(stderr.len(), 3);
        assert_eq!(fs::read_to_string(orig_root.join("sub/main.c"))?, "main");
        assert_eq!(
            fs::read_to_string(orig_root.join("docs/guide.txt"))?,
            "guide"
        );
        assert_eq!(fs::read_to_string(orig_root.join("flat/a"))?, "a");
        assert_eq!(fs::read_to_string(orig_root.join("flat/b"))?, "b");
        Ok(())
    }

    #[test]
    fn a_missing_file_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let error = ensure_files_present(dir.path(), &["absent.tar.xz".to_string()])
            .unwrap_err()
            .to_string();
        assert!(error.contains("absent.tar.xz"), "{error}");
    }
}
