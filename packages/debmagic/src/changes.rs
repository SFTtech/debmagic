use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::control::{read_control, write_control};

/// Locate an existing `orig` tarball in `dir`, in any compression: the
/// main one, or a component's when `component` is set.
pub fn find_orig_in_dir(
    dir: &Path,
    name: &str,
    upstream_version: &str,
    component: Option<&str>,
) -> Option<PathBuf> {
    debmagic_common::changes::orig_filenames(name, upstream_version, component)
        .into_iter()
        .map(|filename| dir.join(filename))
        .find(|path| path.is_file())
}

/// Add one `orig` tarball entry (or, with `None`, remove all orig
/// entries) from a `.changes` file's `Files:`/`Checksums-*:` listings.
/// Adding requires the tarball to exist next to the `.changes`.
/// Returns whether the file was modified.
pub fn set_changes_orig(changes_file: &Path, orig_name: Option<&str>) -> anyhow::Result<bool> {
    let dir = changes_file
        .parent()
        .context("changes file has no parent directory")?;
    let mut control = read_control(changes_file)?;

    let tarball_data: Vec<u8> = match orig_name {
        Some(name) => {
            let path = dir.join(name);
            std::fs::read(&path)
                .with_context(|| format!("failed to read the orig tarball {}", path.display()))?
        }
        None => Vec::new(),
    };

    let modified =
        debmagic_common::changes::set_changes_orig_entries(&mut control, orig_name, &tarball_data);

    if modified {
        write_control(&control, changes_file)?;
    }
    Ok(modified)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_changes_orig_adds_and_removes() {
        let dir =
            std::env::temp_dir().join(format!("debmagic-orig-changes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let changes = dir.join("pkg_1.0-1_source.changes");
        std::fs::write(
            &changes,
            "Format: 1.8\nSource: pkg\nFiles:\n abc 3 pkg_1.0-1.debian.tar.xz\n def 4 pkg_1.0-1.dsc\nChecksums-Sha256:\n xyz 3 pkg_1.0-1.debian.tar.xz\n zzz 4 pkg_1.0-1.dsc\n",
        )
        .unwrap();
        std::fs::write(dir.join("pkg_1.0.orig.tar.xz"), "tarball").unwrap();

        // add
        assert!(set_changes_orig(&changes, Some("pkg_1.0.orig.tar.xz")).unwrap());
        let content = std::fs::read_to_string(&changes).unwrap();
        assert!(content.contains("pkg_1.0.orig.tar.xz"));
        assert_eq!(content.matches("pkg_1.0.orig.tar.xz").count(), 2);

        // adding again is a no-op
        assert!(!set_changes_orig(&changes, Some("pkg_1.0.orig.tar.xz")).unwrap());

        // adding a component orig keeps the main one
        std::fs::write(dir.join("pkg_1.0.orig-bar.tar.xz"), "component").unwrap();
        assert!(set_changes_orig(&changes, Some("pkg_1.0.orig-bar.tar.xz")).unwrap());
        let content = std::fs::read_to_string(&changes).unwrap();
        assert!(content.contains("pkg_1.0.orig.tar.xz"));
        assert!(content.contains("pkg_1.0.orig-bar.tar.xz"));

        // remove drops all origs, main and component
        assert!(set_changes_orig(&changes, None).unwrap());
        let content = std::fs::read_to_string(&changes).unwrap();
        assert!(!content.contains("orig.tar"));
        assert!(!content.contains("orig-bar"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_find_orig_in_dir() {
        let dir = std::env::temp_dir().join(format!("debmagic-find-orig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // nothing there yet
        assert_eq!(find_orig_in_dir(&dir, "pkg", "1.0", None), None);

        // a main orig for the version is found, any compression
        std::fs::write(dir.join("pkg_1.0.orig.tar.xz"), "x").unwrap();
        assert_eq!(
            find_orig_in_dir(&dir, "pkg", "1.0", None),
            Some(dir.join("pkg_1.0.orig.tar.xz"))
        );
        // a different version is not
        assert_eq!(find_orig_in_dir(&dir, "pkg", "1.1", None), None);

        // component origs are only found for their component
        std::fs::write(dir.join("pkg_1.0.orig-bar.tar.xz"), "x").unwrap();
        assert_eq!(
            find_orig_in_dir(&dir, "pkg", "1.0", Some("bar")),
            Some(dir.join("pkg_1.0.orig-bar.tar.xz"))
        );
        assert_eq!(find_orig_in_dir(&dir, "pkg", "1.0", Some("baz")), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
