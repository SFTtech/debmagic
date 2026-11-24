use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct RepeatedPathSegment;

declare_rule! {
    /// A directory's final path segment appears again earlier in its path.
    ///
    /// Walks Data files directories. The path is split on `/` and trailing
    /// slashes are ignored, as lintian's `split` drops trailing empty fields.
    /// The Tag fires when the final segment occurs more than once in the
    /// whole path, such as `usr/lib/lib` or `usr/share/myprogram/share`.
    /// Extra is the repeated segment; the Location is the directory with a
    /// trailing `/`, as lintian's pointer prints a directory. Non-UTF-8
    /// paths are skipped. Udebs are checked too.
    RepeatedPathSegment,
    code = "LN1727",
    tag = "repeated-path-segment",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for RepeatedPathSegment {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = files
            .entries()
            .iter()
            .filter_map(repeated_segment)
            .collect();
        for (path, segment) in hits {
            ctx.diagnostic(
                Diagnostic::warn(segment)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// `(path, segment)` when `entry` is a directory whose final segment repeats.
fn repeated_segment(entry: &Entry) -> Option<(PathBuf, String)> {
    if !entry.is_dir() {
        return None;
    }
    let name = entry.path().to_str()?.trim_end_matches('/');
    let segments: Vec<&str> = name.split('/').collect();
    let last = *segments.last()?;
    let count = segments.iter().filter(|segment| **segment == last).count();
    // lintian's pointer for a directory ends in `/`.
    (count > 1).then(|| (PathBuf::from(format!("{name}/")), last.to_string()))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn dir(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::directory())])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            dir("usr/share/doc/foo"),
            dir("usr/lib/foo-bar"),
            // The repeat must match the final segment exactly.
            dir("usr/share/share-doc"),
            // A repeat that is not the final segment does not count.
            dir("usr/lib/lib/foo"),
            // A file is not a directory.
            BinaryPackageCase::from(vec![("usr/lib/lib", TestFile::new("x"))]),
            // A symlink is not a directory.
            BinaryPackageCase::from(vec![("usr/lib/lib", TestFile::symlink("x"))]),
        ];
        let fail = vec![
            dir("usr/share/share"),
            dir("usr/lib/lib"),
            dir("usr/share/myprogram/share"),
            // Udebs are checked too.
            dir("usr/lib/lib").udeb(),
            // A repeat deeper in the path: the hint pointer of a directory
            // keeps its trailing `/` (lintian's Item::pointer for a dir).
            dir("usr/share/a/b/c/d/c"),
        ];
        BinaryPackageTester::new(RepeatedPathSegment, pass, fail).test_and_snapshot();
    }

    /// lintian's Files::Hierarchy check reports `$item->pointer` for a
    /// directory, whose name ends in `/`, so the hint pointer keeps the slash.
    #[test]
    fn directory_pointer_keeps_trailing_slash() {
        let pass = vec![
            // A regular file with a repeated name is not a directory.
            BinaryPackageCase::from(vec![("usr/lib/lib", TestFile::new("x"))]),
            // Boundary: a repeat that is not the final segment is not reported.
            dir("usr/lib/lib/foo"),
        ];
        let fail = vec![dir("usr/lib/lib")];
        BinaryPackageTester::new(RepeatedPathSegment, pass, fail)
            .test_and_snapshot_with_suffix("trailing_slash");
    }
}
