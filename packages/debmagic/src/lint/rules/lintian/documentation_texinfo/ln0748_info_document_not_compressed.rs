use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct InfoDocumentNotCompressed;

declare_rule! {
    /// An info document under `usr/share/info` does not end in `.gz`. Images
    /// (`png`, `jpg`, `jpeg`) are skipped, as are `dir` files. Symlinks count.
    /// Extra is empty.
    InfoDocumentNotCompressed,
    code = "LN0748",
    tag = "info-document-not-compressed",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InfoDocumentNotCompressed {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let plain: Vec<PathBuf> = scan::info_items(files)
            .filter(|entry| {
                let basename = scan::basename(entry);
                let pieces = scan::name_pieces(&basename);
                match pieces.last().copied() {
                    Some("gz") => false,
                    Some(extension) => !scan::is_image(extension),
                    None => true,
                }
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in plain {
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
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            format!("usr/share/info/{name}"),
            TestFile::new("text\n"),
        )])
    }

    fn link(name: &str, target: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            format!("usr/share/info/{name}"),
            TestFile::symlink(target),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            file("foo.info.gz"),
            file("foo.gz"),
            file("foo.png"),
            file("foo.info-1.jpeg"),
            file("foo.jpg"),
            file("dir"),
            file("directory"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/foo/foo.info",
                TestFile::new("text\n"),
            )]),
        ];
        let fail = vec![
            file("foo"),
            file("foo.info"),
            file("foo.txt"),
            file("foo.gz.bak"),
            link("foo", "foo.info.gz"),
            link("foo.info", "foo.info.gz"),
        ];
        BinaryPackageTester::new(InfoDocumentNotCompressed, pass, fail).test_and_snapshot();
    }
}
