use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct NoNewlineAtEnd;

declare_rule! {
    /// `debian/changelog`, `debian/control`, or `debian/rules` does not end
    /// in a newline.
    ///
    /// Reads the regular files and hardlinks with these names in the
    /// packaging File tree (Source tree, or Source package Patched files).
    /// Only valid UTF-8 is judged, as in lintian, and an empty file counts as
    /// lacking its newline. Extra is empty; the path is the Location.
    NoNewlineAtEnd,
    code = "LN0830",
    tag = "no-newline-at-end",
    default_selected = true,
    experimental = false,
}

const CANDIDATES: [&str; 3] = ["debian/changelog", "debian/control", "debian/rules"];

fn lacks_newline_at_end(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok() && !bytes.ends_with(b"\n")
}

impl SourceRule for NoNewlineAtEnd {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let paths: Vec<PathBuf> = CANDIDATES
            .iter()
            .filter_map(|name| files.get(name))
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter(|entry| {
                files
                    .read(entry)
                    .is_ok_and(|bytes| lacks_newline_at_end(&bytes))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
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
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const CHANGELOG: &str = "example (1.0-1) unstable; urgency=medium\n\
         \n  * Initial release.\n\
         \n -- Example <ex@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n";

    fn tree(extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/changelog".to_string(), TestFile::from(CHANGELOG)),
        ];
        files.extend(
            extra
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone())),
        );
        files
    }

    fn replace(path: &str, file: TestFile) -> Vec<(String, TestFile)> {
        let mut files = tree(&[]);
        files.retain(|(name, _)| name != path);
        files.push((path.to_string(), file));
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            // Only the packaging files on the candidate list count.
            tree(&[("debian/copyright", TestFile::new("Format: x"))]),
            tree(&[("debian/source/control", TestFile::new("x"))]),
            tree(&[("control", TestFile::new("x"))]),
            // Only the end of the file is judged, and a final newline is enough.
            replace("debian/rules", TestFile::new("all:\n\ttrue\n")),
            replace("debian/control", TestFile::new("Source: x\n\n")),
            // A file that is not UTF-8 is not judged.
            replace("debian/rules", TestFile::from_bytes(vec![0xff, b'x'])),
            // Directories and symlinks are not files.
            replace("debian/control", TestFile::directory()),
            replace("debian/control", TestFile::symlink("elsewhere")),
        ];
        let fail = vec![
            replace("debian/control", TestFile::new("Source: x")),
            replace("debian/changelog", TestFile::new("x (1.0) unstable;")),
            // An empty file has no final newline.
            replace("debian/rules", TestFile::new("")),
            replace("debian/rules", TestFile::new("all:\n\ttrue")),
            replace("debian/control", TestFile::new("Source: x").hardlink()),
            // Each candidate is reported.
            vec![
                ("debian/control", TestFile::new(CONTROL.trim_end())),
                ("debian/changelog", TestFile::new(CHANGELOG.trim_end())),
            ]
            .into_iter()
            .map(|(path, file)| (path.to_string(), file))
            .collect(),
        ];
        SourceTreeTester::new(NoNewlineAtEnd, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("debian/changelog", TestFile::from(CHANGELOG)),
            ]),
            SourcePackageCase::from(vec![("debian/copyright", TestFile::new("Format: x"))]),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![("debian/control", TestFile::new(CONTROL.trim_end()))]),
            SourcePackageCase::from(vec![(
                "debian/changelog",
                TestFile::new(CHANGELOG.trim_end()),
            )]),
        ];
        SourcePackageTester::new(NoNewlineAtEnd, pass, fail).test_and_snapshot();
    }
}
