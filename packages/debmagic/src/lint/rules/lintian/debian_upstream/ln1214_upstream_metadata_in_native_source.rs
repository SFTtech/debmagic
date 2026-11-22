use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;
use crate::lint::rules::lintian::fields_version::is_native;

pub struct UpstreamMetadataInNativeSource;

declare_rule! {
    /// A Debian-native source package has `debian/upstream/metadata`.
    ///
    /// Native is lintian's `native` (see `fields_version::is_native`). Fires
    /// when `debian/upstream/metadata` is present in the packaging File tree
    /// (Source tree, or Source package Patched files) as any kind of entry,
    /// including a symlink or a directory. Extra is empty; the path is the
    /// Location. Non-native packages are never reported here.
    UpstreamMetadataInNativeSource,
    code = "LN1214",
    tag = "upstream-metadata-in-native-source",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UpstreamMetadataInNativeSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if !is_native(ctx) {
            return;
        }
        let path = Path::new("debian/upstream/metadata");
        let present = ctx.files().is_some_and(|files| files.get(path).is_some());
        if present {
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
        dsc_files: &[&str],
        patched: bool,
    ) -> SourcePackageCase {
        let mut text = format!(
            "Format: {format}\nSource: example\nBinary: example\nArchitecture: all\nVersion: {version}\n\
             Maintainer: Example <ex@example.com>\nFiles:\n"
        );
        for file in dsc_files {
            text.push_str(&format!(" d41d8cd98f00b204e9800998ecf8427e 0 {file}\n"));
        }
        let dsc = Dsc::parse(&text).expect("valid dsc");
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry(version))),
        ];
        if patched {
            files.push(("debian/upstream/metadata", TestFile::new("Name: x\n")));
        }
        SourcePackageCase::from(files).dsc(dsc)
    }

    #[test]
    fn source() {
        let metadata = ("debian/upstream/metadata", TestFile::new("Name: x\n"));
        let pass = vec![
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                std::slice::from_ref(&metadata),
            ),
            tree("1.0-1", None, std::slice::from_ref(&metadata)),
            tree("1.0", Some("3.0 (quilt)"), &[]),
            tree(
                "1.0",
                Some("3.0 (native)"),
                &[("debian/upstream.yaml", TestFile::new("x"))],
            ),
        ];
        let fail = vec![
            tree("1.0", Some("3.0 (native)"), std::slice::from_ref(&metadata)),
            tree("1.0", None, std::slice::from_ref(&metadata)),
            tree(
                "1.0",
                Some("3.0 (native)"),
                &[("debian/upstream/metadata", TestFile::symlink("other"))],
            ),
            tree(
                "1.0",
                Some("3.0 (native)"),
                &[("debian/upstream/metadata", TestFile::directory())],
            ),
        ];
        SourceTreeTester::new(UpstreamMetadataInNativeSource, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            package("3.0 (quilt)", "1.0-1", &["example_1.0.orig.tar.gz"], true),
            package("1.0", "1.0-1", &["example_1.0-1.diff.gz"], true),
            package("3.0 (native)", "1.0", &["example_1.0.tar.gz"], false),
        ];
        let fail = vec![package(
            "3.0 (native)",
            "1.0",
            &["example_1.0.tar.gz"],
            true,
        )];
        SourcePackageTester::new(UpstreamMetadataInNativeSource, pass, fail).test_and_snapshot();
    }
}
