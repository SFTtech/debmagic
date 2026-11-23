use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::{is_native, source_version};

pub struct DfsgVersionWithTilde;

declare_rule! {
    /// A non-native source package's Version contains `~dfsg`.
    ///
    /// Reads the Version the way `fields/version` does: the Dsc, else the
    /// newest changelog heading on a Source tree. Reported when it contains
    /// `~dfsg` and the source is not native. Extra is the Version. Severity is
    /// info and the Tag is not selected by default, as in lintian. As in
    /// lintian, the whole Version (epoch included) is tested as a substring.
    DfsgVersionWithTilde,
    code = "LN1558",
    tag = "dfsg-version-with-tilde",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DfsgVersionWithTilde {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(version) = source_version(ctx) else {
            return;
        };
        if version.contains("~dfsg") && !is_native(ctx) {
            ctx.diagnostic(Diagnostic::warn(version).with_severity(Severity::Info));
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

    fn tree(version: &str, format: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry(version))),
        ];
        if let Some(format) = format {
            files.push(("debian/source/format", TestFile::new(format)));
        }
        files
    }

    fn package(format: Option<&str>, version: &str, files: &[&str]) -> SourcePackageCase {
        let mut text = String::new();
        if let Some(format) = format {
            text.push_str(&format!("Format: {format}\n"));
        }
        text.push_str(&format!(
            "Source: example\nBinary: example\nArchitecture: all\nVersion: {version}\n\
             Maintainer: Example <ex@example.com>\nFiles:\n"
        ));
        for file in files {
            text.push_str(&format!(" d41d8cd98f00b204e9800998ecf8427e 0 {file}\n"));
        }
        let dsc = Dsc::parse(&text).expect("valid dsc");
        SourcePackageCase::from(Vec::<(&str, TestFile)>::new()).dsc(dsc)
    }

    #[test]
    fn tree_cases() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("1.0+dfsg1-1", Some("3.0 (quilt)")),
            tree("1.0~rc1-1", Some("3.0 (quilt)")),
            tree("1.0~dfsg", Some("3.0 (native)")),
            tree("1.0~dfsg", None),
            tree("1.0~dsfg1-1", Some("3.0 (quilt)")),
        ];
        let fail = vec![
            tree("1.0~dfsg-1", Some("3.0 (quilt)")),
            tree("1:1.0~dfsg-1", Some("3.0 (quilt)")),
            tree("1.0~dfsg1-1", None),
            tree("1.0~dfsg-1", Some("1.0")),
        ];
        SourceTreeTester::new(DfsgVersionWithTilde, pass, fail).test_and_snapshot();
    }

    #[test]
    fn package_cases() {
        let pass = vec![
            package(Some("3.0 (native)"), "1.0~dfsg", &[]),
            package(
                Some("3.0 (quilt)"),
                "1.0+dfsg-1",
                &["example_1.0+dfsg.orig.tar.gz"],
            ),
        ];
        let fail = vec![
            package(
                Some("3.0 (quilt)"),
                "1.0~dfsg-1",
                &["example_1.0~dfsg.orig.tar.gz"],
            ),
            package(Some("1.0"), "1.0~dfsg-1", &["example_1.0~dfsg-1.diff.gz"]),
            package(Some("1.0"), "1:1.0~dfsg-1", &["example_1.0~dfsg-1.diff.gz"]),
        ];
        SourcePackageTester::new(DfsgVersionWithTilde, pass, fail).test_and_snapshot();
    }
}
