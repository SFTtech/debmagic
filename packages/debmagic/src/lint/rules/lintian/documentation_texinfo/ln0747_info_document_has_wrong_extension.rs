use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;
use crate::lint::file_tree::file_name_lossy;

pub struct InfoDocumentHasWrongExtension;

declare_rule! {
    /// An info document under `usr/share/info` has more than one dot-separated
    /// name piece, and its last piece before `.gz` (or before the name's end,
    /// when not gzipped) is not `info` or `info-<N>`.
    ///
    /// Symlinks count. `dir` files, and names ending in `png`, `jpg`, or
    /// `jpeg`, are skipped. Extra is empty.
    InfoDocumentHasWrongExtension,
    code = "LN0747",
    tag = "info-document-has-wrong-extension",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InfoDocumentHasWrongExtension {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let wrong: Vec<PathBuf> = scan::info_items(files)
            .filter_map(|entry| {
                let basename = file_name_lossy(entry.path());
                let mut pieces = scan::name_pieces(&basename);
                match pieces.last().copied() {
                    Some("gz") => {
                        pieces.pop();
                    }
                    Some(extension) if scan::is_image(extension) => return None,
                    _ => {}
                }
                let info = pieces.pop()?;
                (!scan::is_info_piece(info) && !pieces.is_empty())
                    .then(|| entry.path().to_path_buf())
            })
            .collect();
        for path in wrong {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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
            file("foo.info-1.gz"),
            file("foo.info-12.gz"),
            file("foo.bar.info.gz"),
            file("foo.info"),
            file("foo.gz"),
            file("foo"),
            file("foo.png"),
            file("foo.info-1.jpeg"),
            file("directory.newinfo.gz"),
            file("dir.gz"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/foo/bar.newinfo.gz",
                TestFile::new("text\n"),
            )]),
        ];
        let fail = vec![
            file("bar.newinfo.gz"),
            file("bar.newinfo"),
            file("bar.txt"),
            file("foo.bar.baz.gz"),
            file("foo.info-x.gz"),
            link("bar.newinfo.gz", "foo.info.gz"),
            link("bar.txt", "foo.info.gz"),
        ];
        BinaryPackageTester::new(InfoDocumentHasWrongExtension, pass, fail).test_and_snapshot();
    }
}
