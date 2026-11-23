use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::{is_native, source_version};

static ANTICIPATED_REPACK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"dfsg[01]-").expect("valid regex"));

pub struct AnticipatedRepackCount;

declare_rule! {
    /// A non-native source package's Version contains `dfsg0-` or `dfsg1-`.
    ///
    /// Lintian's `fields/version/repack/count` Check. Such a Version is the
    /// anticipated form of a repacked upstream tarball (`+dfsg1`) with its
    /// repack count written before the `dfsg` marker, which is the form
    /// lintian wants used only after a repack has happened. The Version is
    /// read the way `fields/version` reads it: the Dsc, else the newest
    /// changelog heading on a Source tree. Native sources are skipped, as
    /// lintian does. Extra is the Version.
    AnticipatedRepackCount,
    code = "LN1554",
    tag = "anticipated-repack-count",
    default_selected = false,
    experimental = false,
}

impl SourceRule for AnticipatedRepackCount {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let Some(version) = source_version(ctx) else {
            return;
        };
        if ANTICIPATED_REPACK.is_match(&version) {
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
            tree("1.0+dfsg1-1", Some("3.0 (native)")),
            tree("1.0+dfsg.1-1", Some("3.0 (quilt)")),
            tree("1.0.dfsg2-1", Some("3.0 (quilt)")),
            tree("1.0+dfsg1", Some("3.0 (quilt)")),
            tree("1.0-1", Some("3.0 (quilt)")),
        ];
        let fail = vec![
            tree("1.0.dfsg1-1", Some("3.0 (quilt)")),
            tree("1.0+dfsg0-1", Some("3.0 (quilt)")),
            tree("1.0+dfsg1-1", Some("1.0")),
            tree("1:1.0.dfsg0-1", None),
        ];
        SourceTreeTester::new(AnticipatedRepackCount, pass, fail).test_and_snapshot();
    }

    #[test]
    fn package_cases() {
        let pass = vec![
            package(
                Some("3.0 (quilt)"),
                "1.0+dfsg.2-1",
                &["example_1.0+dfsg.2-1.debian.tar.xz"],
            ),
            package(Some("1.0"), "1.0-1", &["example_1.0-1.diff.gz"]),
        ];
        let fail = vec![
            package(
                Some("3.0 (quilt)"),
                "1.0+dfsg1-1",
                &["example_1.0+dfsg1-1.debian.tar.xz"],
            ),
            package(Some("1.0"), "1.0+dfsg0-1", &["example_1.0+dfsg0-1.diff.gz"]),
        ];
        SourcePackageTester::new(AnticipatedRepackCount, pass, fail).test_and_snapshot();
    }
}
