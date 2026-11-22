use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::data;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsAncientFile;

declare_rule! {
    /// The Binary package ships a file or directory whose modification time
    /// is in 1975 or earlier. dak rejects such a package, and the date may be
    /// a placeholder (the epoch, say).
    ///
    /// Checked on the Data files, directories included; a directory's name
    /// ends with `/`. The date is the tar header's modification time in UTC,
    /// `YYYY-MM-DD`, and is the Extra; the Subject is the location. Names that
    /// match lintian's `files/allowed-ancient-files` (Cargo files that
    /// crates.io sets to the epoch) are skipped, and so are Entries without a
    /// modification time.
    PackageContainsAncientFile,
    code = "LN1382",
    tag = "package-contains-ancient-file",
    default_selected = true,
    experimental = false,
}

/// dak's cut-off year: a file dated this year or earlier is ancient.
const DINSTALL_CUTOFF_YEAR: i32 = 1975;

/// Lintian's `files/allowed-ancient-files`, anchored at the start, vendored in
/// `src/lint/data`.
static ALLOWED_ANCIENT_FILES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/files/allowed-ancient-files"))
        .map(|pattern| Regex::new(pattern).expect("valid regex"))
        .collect()
});

impl BinaryPackageRule for PackageContainsAncientFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, date) in ancient_entries(files.entries()) {
            ctx.diagnostic(Diagnostic::error(date).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The Entries dated in the cut-off year or earlier, with their date, in the
/// order given.
fn ancient_entries(entries: &[Entry]) -> Vec<(PathBuf, String)> {
    entries
        .iter()
        .filter_map(|entry| {
            let date = entry.mtime_date()?;
            let year: i32 = date.get(..4)?.parse().ok()?;
            (year <= DINSTALL_CUTOFF_YEAR && !is_allowed(entry))
                .then(|| (entry.path().to_path_buf(), date))
        })
        .collect()
}

/// Whether the Entry's name matches an allowed ancient file. A directory's
/// name ends with `/`, as lintian's does.
fn is_allowed(entry: &Entry) -> bool {
    let mut name = entry.path().as_os_str().as_encoded_bytes().to_vec();
    if entry.kind() == EntryKind::Directory {
        name.push(b'/');
    }
    ALLOWED_ANCIENT_FILES
        .iter()
        .any(|pattern| pattern.is_match(&name))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestCase};

    use super::*;

    fn dated(path: &str, seconds: i64) -> Entry {
        Entry::regular_file(path, None, None).with_mtime(Some(seconds))
    }

    #[test]
    fn binary() {
        // Fixture Entries carry no modification time, so nothing is ancient.
        let pass = vec![BinaryPackageCase::from(TestCase::default())];
        let fail = Vec::<BinaryPackageCase>::new();
        BinaryPackageTester::new(PackageContainsAncientFile, pass, fail).test();
    }

    #[test]
    fn cutoff_is_the_end_of_1975() {
        // 1975-12-31T23:59:59Z, 1976-01-01T00:00:00Z.
        let entries = vec![
            dated("usr/a", 189_302_399),
            dated("usr/b", 189_302_400),
            dated("usr/c", 0),
        ];
        assert_eq!(
            ancient_entries(&entries),
            [
                (PathBuf::from("usr/a"), "1975-12-31".to_string()),
                (PathBuf::from("usr/c"), "1970-01-01".to_string()),
            ]
        );
    }

    #[test]
    fn allowed_cargo_files_and_missing_times_are_skipped() {
        let entries = vec![
            dated("usr/share/cargo/registry/foo-1.0/Cargo.toml", 0),
            dated("usr/share/cargo/registry/foo-1.0/Cargo.toml.orig", 0),
            Entry::regular_file("usr/undated", None, None),
        ];
        assert_eq!(
            ancient_entries(&entries),
            [(
                PathBuf::from("usr/share/cargo/registry/foo-1.0/Cargo.toml.orig"),
                "1970-01-01".to_string()
            )]
        );
    }
}
