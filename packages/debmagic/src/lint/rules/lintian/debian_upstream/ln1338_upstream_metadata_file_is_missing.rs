use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::fields_version::is_native;
use crate::lint::upstream_metadata::UpstreamMetadata;

pub struct UpstreamMetadataFileIsMissing;

declare_rule! {
    /// A non-native source package has no `debian/upstream/metadata`.
    ///
    /// Native is lintian's `native` (see `fields_version::is_native`).
    /// `debian/upstream/metadata` is looked up in the packaging File tree
    /// (Source tree, or Source package Patched files) as lintian's
    /// `resolve_path` does, so a dangling or escaping symlink counts as missing
    /// and any other kind of entry (a directory, a fifo, a file) as present.
    /// Reported at pedantic severity with an empty extra and no Location; a
    /// native package is the business of `upstream-metadata-in-native-source`.
    UpstreamMetadataFileIsMissing,
    code = "LN1338",
    tag = "upstream-metadata-file-is-missing",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UpstreamMetadataFileIsMissing {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let missing = ctx
            .upstream_metadata()
            .is_some_and(|metadata| *metadata == UpstreamMetadata::Missing);
        if missing {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Pedantic));
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
        metadata: bool,
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
        if metadata {
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
            // Native packages are not asked for the file.
            tree("1.0", Some("3.0 (native)"), &[]),
            tree("1.0", None, &[]),
            // A directory or a symlink that resolves is something.
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[("debian/upstream/metadata", TestFile::directory())],
            ),
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[
                    ("debian/upstream/metadata", TestFile::symlink("other")),
                    ("debian/upstream/other", TestFile::new("Name: x\n")),
                ],
            ),
        ];
        let fail = vec![
            tree("1.0-1", Some("3.0 (quilt)"), &[]),
            tree("1.0-1", None, &[]),
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[("debian/upstream.yaml", TestFile::new("x"))],
            ),
            // A dangling symlink resolves to nothing.
            tree(
                "1.0-1",
                Some("3.0 (quilt)"),
                &[("debian/upstream/metadata", TestFile::symlink("nowhere"))],
            ),
        ];
        SourceTreeTester::new(UpstreamMetadataFileIsMissing, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            package("3.0 (quilt)", "1.0-1", &["example_1.0.orig.tar.gz"], true),
            package("3.0 (native)", "1.0", &["example_1.0.tar.gz"], false),
        ];
        let fail = vec![
            package("3.0 (quilt)", "1.0-1", &["example_1.0.orig.tar.gz"], false),
            package("1.0", "1.0-1", &["example_1.0-1.diff.gz"], false),
        ];
        SourcePackageTester::new(UpstreamMetadataFileIsMissing, pass, fail).test_and_snapshot();
    }
}
