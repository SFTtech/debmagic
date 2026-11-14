use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::rules_text::read_utf8;

pub struct DebhelperCompatFileContainsMultipleLevels;

declare_rule! {
    /// `debian/compat` has more than one compat level.
    ///
    /// Only reached after `debian/rules` has shown a dh command; a
    /// `Build-Driver` other than `debian-rules` is not scanned. Every line
    /// after the first that starts with a digit reports. Blank lines,
    /// comments, and indented numbers do not. A missing `debian/compat`, or one
    /// that is not readable text, reports nothing. Extra is empty. Location
    /// is `debian/compat` and the line. Applies to the Source tree and Source
    /// package.
    DebhelperCompatFileContainsMultipleLevels,
    code = "LN0528",
    tag = "debhelper-compat-file-contains-multiple-levels",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebhelperCompatFileContainsMultipleLevels {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() {
            return;
        }
        let Some(text) = ctx
            .files()
            .and_then(|files| read_utf8(files, "debian/compat"))
        else {
            return;
        };
        for (index, line) in text.split('\n').enumerate().skip(1) {
            if !line.starts_with(|ch: char| ch.is_ascii_digit()) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: PathBuf::from("debian/compat"),
                line: Some(u32::try_from(index + 1).unwrap_or(u32::MAX)),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    const DH: &str = "%:\n\tdh $@\n";

    fn package(rules: &str, compat: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/rules", TestFile::new(rules)),
        ];
        if let Some(compat) = compat {
            files.push(("debian/compat", TestFile::new(compat)));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package(DH, None),
            package(DH, Some("12\n")),
            package(DH, Some("12")),
            package(DH, Some("12\n\n# note\n  13\n")),
            // No dh command was seen.
            package("build:\n\ttrue\n", Some("12\n13\n")),
        ];
        let fail = vec![
            package(DH, Some("12\n13\n")),
            package(DH, Some("12\n\n13\n9\nfoo\n")),
            package(DH, Some("\n12\n")),
        ];
        SourceTreeTester::new(DebhelperCompatFileContainsMultipleLevels, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(DH, Some("12\n")))];
        let fail = vec![SourcePackageCase::from(package(DH, Some("12\n13\n")))];
        SourcePackageTester::new(DebhelperCompatFileContainsMultipleLevels, pass, fail)
            .test_and_snapshot();
    }
}
