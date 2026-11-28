//! Sibling files of a `.dsc`: the files the `.dsc` lists next to it, read from
//! the directory the `.dsc` lives in. Lintian's `processable->files` of a
//! Source package is built from the `.dsc` `Files` field (and the Sha1 and
//! Sha256 checksum fields), keeping each entry's basename. Names are taken
//! from the `.dsc` alone; bytes are read only when a Rule asks for them.

use std::fs;
use std::path::{Path, PathBuf};

use super::dsc::Dsc;

/// The files a `.dsc` lists, by basename, sorted. `read` reads one of them
/// from the directory beside the `.dsc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SiblingFiles {
    dir: PathBuf,
    names: Vec<String>,
}

impl SiblingFiles {
    /// The names `dsc` lists (none without a `.dsc`), and the directory of
    /// `dsc_path` the bytes are read from.
    pub fn from_dsc(dsc_path: &Path, dsc: Option<&Dsc>) -> Self {
        let dir = dsc_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let names = dsc.map(Dsc::files).unwrap_or_default();
        Self { dir, names }
    }

    /// Basenames the `.dsc` lists, sorted.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Whether the `.dsc` lists a file with this basename.
    pub fn contains(&self, name: &str) -> bool {
        self.names
            .binary_search_by(|candidate| candidate.as_str().cmp(name))
            .is_ok()
    }

    /// The bytes of the listed file `name`, read now from the directory beside
    /// the `.dsc`. `None` when the `.dsc` does not list it or it cannot be read.
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

    fn dsc(text: &str) -> Dsc {
        Dsc::parse(text).expect("dsc paragraph")
    }

    const FILES: &str = "Source: pkg\nVersion: 1.0\nFiles:\n 0 0 pkg_1.0.orig.tar.gz\n 0 0 pkg_1.0.orig.tar.gz.asc\n 0 0 pkg_1.0.debian.tar.xz\n";

    #[test]
    fn names_are_the_files_field_basenames_only() {
        let dir = scratch("names");
        fs::write(dir.join("pkg_1.0.orig.tar.gz"), b"tarball").unwrap();
        fs::write(dir.join("pkg_1.0.orig-component.tar.gz"), b"unlisted").unwrap();
        let siblings = SiblingFiles::from_dsc(&dir.join("pkg_1.0.dsc"), Some(&dsc(FILES)));
        assert_eq!(
            siblings.names(),
            [
                "pkg_1.0.debian.tar.xz",
                "pkg_1.0.orig.tar.gz",
                "pkg_1.0.orig.tar.gz.asc"
            ]
        );
        assert!(siblings.contains("pkg_1.0.orig.tar.gz.asc"));
        assert!(!siblings.contains("pkg_1.0.orig-component.tar.gz"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn checksum_fields_add_names_and_slashed_names_are_left_out() {
        let text = "Source: pkg\nVersion: 1.0\nFiles:\n 0 0 a.tar.gz\n 0 0 sub/b.tar.gz\n\
                    Checksums-Sha256:\n 0 0 c.tar.gz\n";
        let siblings = SiblingFiles::from_dsc(Path::new("/nonexistent/pkg.dsc"), Some(&dsc(text)));
        assert_eq!(siblings.names(), ["a.tar.gz", "c.tar.gz"]);
    }

    #[test]
    fn reads_listed_files_only() {
        let dir = scratch("read");
        fs::write(dir.join("pkg_1.0.orig.tar.gz.asc"), b"-----BEGIN").unwrap();
        fs::write(dir.join("pkg_1.0.orig-component.tar.gz"), b"unlisted").unwrap();
        let siblings = SiblingFiles::from_dsc(&dir.join("pkg_1.0.dsc"), Some(&dsc(FILES)));
        assert_eq!(
            siblings.read("pkg_1.0.orig.tar.gz.asc").as_deref(),
            Some(&b"-----BEGIN"[..])
        );
        // Listed but absent: no bytes.
        assert_eq!(siblings.read("pkg_1.0.orig.tar.gz"), None);
        // On disk but not listed: not a sibling.
        assert_eq!(siblings.read("pkg_1.0.orig-component.tar.gz"), None);
        assert_eq!(siblings.read("../escape"), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_dsc_lists_nothing() {
        let siblings = SiblingFiles::from_dsc(Path::new("/nonexistent/pkg.dsc"), None);
        assert!(siblings.names().is_empty());
        assert_eq!(siblings.read("anything"), None);
    }
}
