use chrono::{DateTime, Datelike};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct OrigTarballContainsAncientFile;

declare_rule! {
    /// The orig tarball contains a file that claims to have been generated
    /// before 1976. This old date may carry over to the binary packages when
    /// building from the sources unpacked with `dpkg-source -x package.dsc`,
    /// for example. In this case the generated package will be rejected by the
    /// Debian archive scripts.
    ///
    /// Checked on every entry of each Orig tarball, directories included, with
    /// the modification time in its tar header. The Extra is that date as
    /// `YYYY-MM-DD` in UTC, and the location is the entry's path in the Orig
    /// files: the top directory every member shares is dropped, as lintian's
    /// orig index does. A dated 1975 or earlier is reported. Info, not selected
    /// by default. A Source tree has no Orig tarball; a Native package has none
    /// either. Tarballs compressed with bzip2 are not read, so the Rule fails
    /// the Subject for them.
    OrigTarballContainsAncientFile,
    code = "LN1461",
    tag = "orig-tarball-contains-ancient-file",
    default_selected = false,
    experimental = false,
}

/// dak's cut-off year: an entry dated this year or earlier is ancient.
const DINSTALL_CUTOFF_YEAR: i32 = 1975;

impl SourceRule for OrigTarballContainsAncientFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(entries) = ctx.orig_entries() else {
            return;
        };
        let ancient: Vec<(std::path::PathBuf, String)> = entries
            .iter()
            .filter_map(|entry| {
                let time = DateTime::from_timestamp(entry.mtime?, 0)?;
                (time.year() <= DINSTALL_CUTOFF_YEAR)
                    .then(|| (entry.path.clone(), time.format("%Y-%m-%d").to_string()))
            })
            .collect();
        for (path, date) in ancient {
            ctx.diagnostic(
                Diagnostic::warn(date)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use crate::lint::context::{SourceContext, SourcePackageData};
    use crate::lint::source_package::{write_test_native_dsc, write_test_quilt_dsc};
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    /// The (path, date) of each entry reported for the Source package `dsc`.
    fn found(dsc: PathBuf) -> Vec<(String, String)> {
        let data = SourcePackageData::new(dsc).expect("Source package");
        let mut ctx = SourceContext::bind_package(&data, &OrigTarballContainsAncientFile);
        OrigTarballContainsAncientFile.run(&mut ctx);
        ctx.finish()
            .expect("no runtime error")
            .into_iter()
            .map(|diagnostic| {
                (
                    diagnostic.location.path.display().to_string(),
                    diagnostic.message.clone(),
                )
            })
            .collect()
    }

    /// The `.dsc` of a quilt package with `orig` members in `dir`.
    fn quilt(dir: &Path, orig: &[(&str, &[u8], u64)]) -> PathBuf {
        write_test_quilt_dsc(dir, "example", orig).expect("quilt dsc")
    }

    #[test]
    fn entry_dated_in_1970_is_reported_under_its_orig_path() {
        let dir = TempDir::new().expect("temp directory");
        let dsc = quilt(
            dir.path(),
            &[
                ("example-1.0/README", b"text\n", 0),
                ("example-1.0/src/main.c", b"int main(){}\n", 1_000_000_000),
            ],
        );
        assert_eq!(
            found(dsc),
            [("README".to_string(), "1970-01-01".to_string())]
        );
    }

    /// Lintian's orig index is a hash by name (`$catalog{$entry->name} = $entry`
    /// in `create_from_piped_tar`), so a member that the tarball lists twice
    /// is judged by its last header only.
    #[test]
    fn repeated_member_is_judged_by_its_last_header() {
        let dir = TempDir::new().expect("temp directory");
        let dsc = quilt(
            dir.path(),
            &[
                ("example-1.0/README", b"text\n", 0),
                ("example-1.0/README", b"text\n", 1_000_000_000),
            ],
        );
        assert!(found(dsc).is_empty());
    }

    /// The same rule the other way round: a repeat whose last header is
    /// ancient is reported.
    #[test]
    fn repeated_member_with_ancient_last_header_is_reported() {
        let dir = TempDir::new().expect("temp directory");
        let dsc = quilt(
            dir.path(),
            &[
                ("example-1.0/README", b"text\n", 1_000_000_000),
                ("example-1.0/README", b"text\n", 0),
            ],
        );
        assert_eq!(
            found(dsc),
            [("README".to_string(), "1970-01-01".to_string())]
        );
    }

    #[test]
    fn modern_orig_reports_nothing() {
        let dir = TempDir::new().expect("temp directory");
        let dsc = quilt(
            dir.path(),
            &[("example-1.0/src/main.c", b"x", 1_000_000_000)],
        );
        assert!(found(dsc).is_empty());
    }

    #[test]
    fn native_package_has_no_orig_tarball() {
        let dir = TempDir::new().expect("temp directory");
        let dsc =
            write_test_native_dsc(dir.path(), "example", &[("README", b"text\n")]).expect("dsc");
        assert!(found(dsc).is_empty());
    }

    #[test]
    fn source_tree_has_no_orig_tarball() {
        let pass = Vec::<TestCase>::new();
        let fail = Vec::<TestCase>::new();
        SourceTreeTester::new(OrigTarballContainsAncientFile, pass, fail).test();
    }
}
