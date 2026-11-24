use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct CarriageReturnLineFeed;

declare_rule! {
    /// `debian/control` or `debian/changelog` ends with a CRLF terminator.
    ///
    /// Reads the regular files and hardlinks named `debian/control` and
    /// `debian/changelog` in the packaging File tree (Source tree, or Source
    /// package Patched files). Like lintian, only the end of the file is
    /// judged: the bytes must end in `\r\n`, or in `\r\n\n`. Extra is empty;
    /// the path is the Location.
    CarriageReturnLineFeed,
    code = "LN0738",
    tag = "carriage-return-line-feed",
    default_selected = true,
    experimental = false,
}

const CANDIDATES: [&str; 2] = ["debian/control", "debian/changelog"];

fn ends_with_crlf(bytes: &[u8]) -> bool {
    // Perl's `\Z` also matches before one final newline.
    bytes.ends_with(b"\r\n") || bytes.ends_with(b"\r\n\n")
}

impl SourceRule for CarriageReturnLineFeed {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let paths: Vec<PathBuf> = CANDIDATES
            .iter()
            .filter_map(|name| files.get(name))
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter(|entry| files.read(entry).is_ok_and(|bytes| ends_with_crlf(&bytes)))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
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
            tree(&[("debian/copyright", TestFile::new("Format: x\r\n"))]),
            tree(&[("debian/source/control", TestFile::new("x\r\n"))]),
            tree(&[("control", TestFile::new("x\r\n"))]),
            // Only the end of the file is judged.
            replace("debian/control", TestFile::new("Source: x\r\nPackage: y\n")),
            // Directories and symlinks are not files.
            replace("debian/control", TestFile::directory()),
            replace("debian/control", TestFile::symlink("elsewhere")),
            // A bare CR is not a CRLF terminator.
            replace("debian/control", TestFile::new("Source: x\r")),
        ];
        let fail = vec![
            replace("debian/control", TestFile::new("Source: x\r\n")),
            replace("debian/changelog", TestFile::new("x (1.0) unstable;\r\n")),
            // One final newline after the CRLF still counts, as in lintian.
            replace("debian/control", TestFile::new("Source: x\r\n\n")),
            replace("debian/control", TestFile::new("Source: x\r\n").hardlink()),
            // Both candidates are reported.
            vec![
                (
                    "debian/control",
                    TestFile::new(CONTROL.replace('\n', "\r\n")),
                ),
                (
                    "debian/changelog",
                    TestFile::new(CHANGELOG.replace('\n', "\r\n")),
                ),
            ]
            .into_iter()
            .map(|(path, file)| (path.to_string(), file))
            .collect(),
        ];
        SourceTreeTester::new(CarriageReturnLineFeed, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("debian/changelog", TestFile::from(CHANGELOG)),
            ]),
            SourcePackageCase::from(vec![("debian/copyright", TestFile::new("Format: x\r\n"))]),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![(
                "debian/control",
                TestFile::new(CONTROL.replace('\n', "\r\n")),
            )]),
            SourcePackageCase::from(vec![(
                "debian/changelog",
                TestFile::new(CHANGELOG.replace('\n', "\r\n")),
            )]),
        ];
        SourcePackageTester::new(CarriageReturnLineFeed, pass, fail).test_and_snapshot();
    }
}
