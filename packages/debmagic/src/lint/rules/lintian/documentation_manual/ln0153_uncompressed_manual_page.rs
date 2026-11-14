use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::manual::{final_component_is_gz, section_page};

pub struct UncompressedManualPage;

declare_rule! {
    /// A manual page is not gzip-compressed.
    ///
    /// A file, hardlink, or symlink in a `usr/share/man/.../manN/` directory
    /// whose final `.`-separated component is not `gz`. A name with no dot
    /// uses the whole name, so `foo` is uncompressed and a file named `gz` is
    /// not. Perl `split` drops a trailing empty field, so `foo.1.gz.` is
    /// compressed by name. Udebs are skipped. Extra is empty; the Data files
    /// path is the Location.
    UncompressedManualPage,
    code = "LN0153",
    tag = "uncompressed-manual-page",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UncompressedManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            let Some(page) = section_page(entry) else {
                continue;
            };
            if final_component_is_gz(page.file_name) {
                continue;
            }
            hits.push(entry.path().to_path_buf());
        }
        emit(ctx, hits);
    }
}

fn emit(ctx: &mut BinaryPackageContext<'_>, hits: Vec<PathBuf>) {
    for path in hits {
        ctx.diagnostic(Diagnostic::error("").with_location(Location {
            path,
            line: None,
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn text(path: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new("man"))
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![text("usr/share/man/man1/foo.1.gz")]),
            case(vec![(
                "usr/share/man/man1/foo.1.gz".to_string(),
                TestFile::symlink("foo.1"),
            )]),
            // Named `gz` with no dot, and a trailing dot after `.gz`.
            case(vec![
                text("usr/share/man/man1/gz"),
                text("usr/share/man/man1/foo.1.gz."),
            ]),
            // Not in a section directory.
            case(vec![
                text("usr/share/man/not-compressed.1"),
                text("usr/share/man/odd/not-compressed.1"),
            ]),
            case(vec![(
                "usr/share/man/man1".to_string(),
                TestFile::directory(),
            )]),
            case(vec![text("usr/share/man/man1/not-compressed.1")]).udeb(),
        ];
        let fail = vec![
            case(vec![text("usr/share/man/man1/not-compressed.1")]),
            case(vec![text(
                "usr/share/man/de/man1/binary-without-english-manpage.1",
            )]),
            case(vec![(
                "usr/share/man/man1/linked.1".to_string(),
                TestFile::symlink("linked.1.gz"),
            )]),
            case(vec![(
                "usr/share/man/man1/linked.1".to_string(),
                TestFile::new("man").hardlink(),
            )]),
            case(vec![
                text("usr/share/man/man1/perl.1p"),
                text("usr/share/man/man6/game.6"),
                text("usr/share/man/man1/bare"),
                text("usr/share/man/man1/upper.1.GZ"),
            ]),
        ];
        BinaryPackageTester::new(UncompressedManualPage, pass, fail).test_and_snapshot();
    }
}
