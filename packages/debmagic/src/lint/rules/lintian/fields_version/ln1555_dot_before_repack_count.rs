use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::{is_native, source_version};

static DOT_BEFORE_REPACK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"dfsg\.\d+").expect("valid regex"));

pub struct DotBeforeRepackCount;

declare_rule! {
    /// A non-native source package's Version has a period between `dfsg` and
    /// its repack count, as in `1.0+dfsg.2-1`.
    ///
    /// Lintian's `fields/version/repack/count` Check. The repack count goes
    /// straight after `dfsg` (`+dfsg2`), so a period there is a typo. The
    /// Version is read the way `fields/version` reads it: the Dsc, else the
    /// newest changelog heading on a Source tree. Native sources are skipped,
    /// as lintian does. Extra is the Version.
    DotBeforeRepackCount,
    code = "LN1555",
    tag = "dot-before-repack-count",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DotBeforeRepackCount {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let Some(version) = source_version(ctx) else {
            return;
        };
        if DOT_BEFORE_REPACK.is_match(&version) {
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
            tree("1.0+dfsg2-1", Some("3.0 (quilt)")),
            tree("1.0+dfsg2", Some("3.0 (quilt)")),
            tree("1.0-1", Some("3.0 (quilt)")),
            tree("1.0.dfsg.2-1", Some("3.0 (native)")),
        ];
        let fail = vec![
            tree("1.0+dfsg.2-1", Some("3.0 (quilt)")),
            tree("1.0.dfsg.10-1", Some("3.0 (quilt)")),
            tree("1:1.0+dfsg.2-1", None),
        ];
        SourceTreeTester::new(DotBeforeRepackCount, pass, fail).test_and_snapshot();
    }

    #[test]
    fn package_cases() {
        let pass = vec![
            package(
                Some("3.0 (quilt)"),
                "1.0+dfsg2-1",
                &["example_1.0+dfsg2-1.debian.tar.xz"],
            ),
            package(Some("1.0"), "1.0-1", &["example_1.0-1.diff.gz"]),
        ];
        let fail = vec![
            package(
                Some("3.0 (quilt)"),
                "1.0+dfsg.2-1",
                &["example_1.0+dfsg.2-1.debian.tar.xz"],
            ),
            package(
                Some("1.0"),
                "1.0+dfsg.2-1",
                &["example_1.0+dfsg.2-1.diff.gz"],
            ),
        ];
        SourcePackageTester::new(DotBeforeRepackCount, pass, fail).test_and_snapshot();
    }
}
