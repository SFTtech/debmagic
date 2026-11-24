use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::data;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

/// Lintian's `debian-source-dir/known-files`, vendored from lintian 2.144.0.
const KNOWN_FILES: &str = include_str!("../../../data/debian-source-dir/known-files");

pub struct UnknownFileInDebianSource;

declare_rule! {
    /// A file directly in `debian/source/` whose name is not in lintian's
    /// list of known files: `format`, `git-patches`, `include-binaries`,
    /// `lintian-overrides`, `options`, `patch-header`.
    ///
    /// Every non-directory child of `debian/source/` is checked by its
    /// basename, byte for byte. Directories are skipped; nested files are not
    /// direct children. Extra is empty. The Location is the file. Applies to
    /// the Source tree and Source package.
    UnknownFileInDebianSource,
    code = "LN1210",
    tag = "unknown-file-in-debian-source",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UnknownFileInDebianSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let source_dir = Path::new("debian/source");
        let unknown: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter(|entry| entry.path().parent() == Some(source_dir))
            .filter(|entry| {
                let Some(name) = entry.path().file_name() else {
                    return false;
                };
                !data::lines(KNOWN_FILES).any(|known| known.as_bytes() == name.as_bytes())
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in unknown {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn with(name: &'static str) -> Vec<(&'static str, TestFile)> {
        vec![(name, TestFile::new("content\n"))]
    }

    #[test]
    fn source() {
        let pass = vec![
            with("debian/source/format"),
            with("debian/source/options"),
            with("debian/source/include-binaries"),
            with("debian/source/lintian-overrides"),
            with("debian/source/patch-header"),
            with("debian/source/git-patches"),
            // A directory is not checked.
            vec![("debian/source/local", TestFile::directory())],
            // Files outside debian/source/ are not checked.
            with("debian/formt"),
        ];
        let fail = vec![
            // A misspelled name.
            with("debian/source/formt"),
            // Names are compared exactly: case matters.
            with("debian/source/Format"),
            // A symlink is a file entry for this check.
            vec![("debian/source/local", TestFile::symlink("format"))],
            // A nested file is not a direct child, but an unknown direct one is.
            vec![
                ("debian/source/local/format", TestFile::new("x\n")),
                ("debian/source/unknown", TestFile::new("x\n")),
            ],
        ];
        SourceTreeTester::new(UnknownFileInDebianSource, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(with("debian/source/format"))];
        let fail = vec![SourcePackageCase::from(with("debian/source/formt"))];
        SourcePackageTester::new(UnknownFileInDebianSource, pass, fail).test_and_snapshot();
    }
}
