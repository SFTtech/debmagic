//! Sibling files of a `.dsc`: the regular files in the directory the `.dsc`
//! lives in. Lintian's `processable->files` of a Source package is a superset
//! of the `.dsc` `Files` list: it also holds the `.asc` signatures and the
//! orig component tarballs the `.dsc` does not name. Names are listed once;
//! bytes are read only when a Rule asks for them.

use std::fs;
use std::path::{Path, PathBuf};

/// The regular files next to a `.dsc`, sorted by name. Entries that are not
/// regular files (directories, dangling symlinks) and names that are not
/// UTF-8 are left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SiblingFiles {
    dir: PathBuf,
    names: Vec<String>,
}

impl SiblingFiles {
    /// List the directory of `dsc`. An unreadable directory lists nothing.
    pub fn load(dsc: &Path) -> Self {
        let dir = dsc
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let mut names: Vec<String> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        names.sort();
        Self { dir, names }
    }

    /// Names of the sibling files, sorted, relative to the directory.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Whether a sibling file has this name.
    pub fn contains(&self, name: &str) -> bool {
        self.names
            .binary_search_by(|candidate| candidate.as_str().cmp(name))
            .is_ok()
    }

    /// The bytes of the sibling file `name`, read now. `None` when there is no
    /// such sibling or it cannot be read.
    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        if !self.contains(name) {
            return None;
        }
        fs::read(self.dir.join(name)).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("debmagic-siblings-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("scratch directory");
        dir
    }

    #[test]
    fn lists_regular_files_sorted_and_skips_directories() {
        let dir = scratch("list");
        fs::write(dir.join("pkg_1.0.dsc"), b"dsc").unwrap();
        fs::write(dir.join("pkg_1.0.orig.tar.gz"), b"orig").unwrap();
        fs::write(dir.join("pkg_1.0.orig.tar.gz.asc"), b"sig").unwrap();
        fs::create_dir(dir.join("subdir")).unwrap();

        let siblings = SiblingFiles::load(&dir.join("pkg_1.0.dsc"));
        assert_eq!(
            siblings.names(),
            [
                "pkg_1.0.dsc",
                "pkg_1.0.orig.tar.gz",
                "pkg_1.0.orig.tar.gz.asc"
            ]
        );
        assert!(siblings.contains("pkg_1.0.orig.tar.gz.asc"));
        assert!(!siblings.contains("subdir"));
        assert!(!siblings.contains("pkg_1.0.orig"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_bytes_lazily_and_only_of_siblings() {
        let dir = scratch("read");
        fs::write(dir.join("pkg_1.0.orig.tar.gz.asc"), b"-----BEGIN").unwrap();
        let siblings = SiblingFiles::load(&dir.join("pkg_1.0.dsc"));
        assert_eq!(
            siblings.read("pkg_1.0.orig.tar.gz.asc").as_deref(),
            Some(&b"-----BEGIN"[..])
        );
        assert_eq!(siblings.read("missing.asc"), None);
        assert_eq!(siblings.read("../escape"), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_symlink_to_a_file_is_a_sibling() {
        let dir = scratch("symlink");
        fs::write(dir.join("real.asc"), b"sig").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("real.asc"), dir.join("link.asc")).unwrap();
        #[cfg(unix)]
        {
            let siblings = SiblingFiles::load(&dir.join("pkg.dsc"));
            assert!(siblings.contains("link.asc"));
            assert_eq!(siblings.read("link.asc").as_deref(), Some(&b"sig"[..]));
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_directory_lists_nothing() {
        let siblings = SiblingFiles::load(Path::new("/nonexistent-debmagic-dir/pkg.dsc"));
        assert!(siblings.names().is_empty());
    }
}
