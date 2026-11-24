//! NEWS and TODO files that debhelper does not find under their correct name,
//! as lintian's `debian/filenames` Check finds them.

use std::path::PathBuf;

use crate::lint::file_tree::FileTree;

/// Problematic name and correct name pairs, in lintian's order.
const PAIRS: &[(&str, &str)] = &[
    ("debian/NEWS.Debian", "debian/NEWS"),
    ("debian/NEWS.debian", "debian/NEWS"),
    ("debian/TODO.Debian", "debian/TODO"),
    ("debian/TODO.debian", "debian/TODO"),
];

/// One misnamed packaging file.
pub(super) struct Misnamed {
    pub(super) path: PathBuf,
    pub(super) correct: &'static str,
    /// The correctly named file exists too.
    pub(super) duplicate: bool,
}

pub(super) fn misnamed(files: &FileTree) -> Vec<Misnamed> {
    PAIRS
        .iter()
        .filter(|(problematic, _)| files.get(problematic).is_some())
        .map(|&(problematic, correct)| Misnamed {
            path: PathBuf::from(problematic),
            correct,
            duplicate: files.get(correct).is_some(),
        })
        .collect()
}
