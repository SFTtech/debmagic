use std::path::Path;

use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;
use crate::lint::rules::lintian::fields_version::is_native;

pub struct UpstreamMetadataIsNotAFile;

declare_rule! {
    /// `debian/upstream/metadata` is not a readable regular file.
    ///
    /// Only for non-native packages (native ones get LN1214 instead). Fires
    /// when the entry exists in the packaging File tree (Source tree, or
    /// Source package Patched files) but is not an open-able file: a fifo,
    /// device, directory, dangling symlink, or a symlink escaping the tree.
    /// A regular file, a hardlink, or a symlink to one inside the tree is
    /// fine. A missing entry is not reported here (see the missing Tag).
    /// Extra is empty; the path is the Location.
    UpstreamMetadataIsNotAFile,
    code = "LN1215",
    tag = "upstream-metadata-is-not-a-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UpstreamMetadataIsNotAFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let path = Path::new("debian/upstream/metadata");
        let not_a_file = ctx.files().is_some_and(|files| {
            files
                .get(path)
                .is_some_and(|entry| follow_symlinks(files, entry).is_none())
        });
        if not_a_file {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: path.to_path_buf(),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(
        version: &str,
        format: Option<&str>,
        extra: &[(&'static str, TestFile)],
    ) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry(version))),
        ];
        if let Some(format) = format {
            files.push(("debian/source/format", TestFile::new(format)));
        }
        files.extend(extra.iter().cloned());
        files
    }

    fn package(
        format: &str,
        version: &str,
        patched: &[(&'static str, TestFile)],
    ) -> SourcePackageCase {
        let text = format!(
            "Format: {format}\nSource: example\nBinary: example\nArchitecture: all\nVersion: {version}\n\
             Maintainer: Example <ex@example.com>\nFiles:\n d41d8cd98f00b204e9800998ecf8427e 0 example_{version}.tar.gz\n"
        );
        let dsc = Dsc::parse(&text).expect("valid dsc");
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry(version))),
        ];
        files.extend(patched.iter().cloned());
        SourcePackageCase::from(files).dsc(dsc)
    }

    #[test]
    fn source() {
        let pass = vec![
            tree("1.0-1", Some("3.0 (quilt)"), &[]),
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[("debian/upstream/metadata", TestFile::new("Name: x\n"))],
            ),
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[(
                    "debian/upstream/metadata",
                    TestFile::new("Name: x\n").hardlink(),
                )],
            ),
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[
                    ("debian/upstream/metadata", TestFile::symlink("../other")),
                    ("debian/other", TestFile::new("Name: x\n")),
                ],
            ),
            // Native: LN1214 reports this, not this Rule.
            tree(
                "1.0",
                Some("3.0 (native)"),
                &[("debian/upstream/metadata", TestFile::fifo())],
            ),
        ];
        let fail = vec![
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[("debian/upstream/metadata", TestFile::fifo())],
            ),
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[("debian/upstream/metadata", TestFile::directory())],
            ),
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[("debian/upstream/metadata", TestFile::symlink("missing"))],
            ),
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[("debian/upstream/metadata", TestFile::symlink("/etc/passwd"))],
            ),
        ];
        SourceTreeTester::new(UpstreamMetadataIsNotAFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            package(
                "3.0 (quilt)",
                "1.0-1",
                &[("debian/upstream/metadata", TestFile::new("Name: x\n"))],
            ),
            package(
                "3.0 (native)",
                "1.0",
                &[("debian/upstream/metadata", TestFile::fifo())],
            ),
        ];
        let fail = vec![package(
            "3.0 (quilt)",
            "1.0-1",
            &[("debian/upstream/metadata", TestFile::fifo())],
        )];
        SourcePackageTester::new(UpstreamMetadataIsNotAFile, pass, fail).test_and_snapshot();
    }
}
