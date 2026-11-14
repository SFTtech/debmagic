use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::rules_text::read_utf8;

pub struct DebhelperCompatFileIsEmpty;

declare_rule! {
    /// `debian/compat` has no compat level on its first line.
    ///
    /// Only reached after `debian/rules` has shown a dh command; a
    /// `Build-Driver` other than `debian-rules` is not scanned. The first line
    /// of `debian/compat`, trimmed at both ends, is empty, which includes an
    /// empty file. A missing `debian/compat`, or one that is not readable
    /// text, reports nothing. Extra is empty and there is no Location (the
    /// oracle prints no pointer). Applies to the Source tree and Source package.
    DebhelperCompatFileIsEmpty,
    code = "LN0529",
    tag = "debhelper-compat-file-is-empty",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebhelperCompatFileIsEmpty {
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
        let first = text.split('\n').next().unwrap_or("");
        if !first.trim().is_empty() {
            return;
        }
        ctx.diagnostic(Diagnostic::error(""));
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
            package(DH, Some(" 12 \n")),
            // The number does not have to be valid.
            package(DH, Some("twelve\n")),
            // Only the first line is read.
            package(DH, Some("12\n\n")),
            // No dh command was seen.
            package("build:\n\ttrue\n", Some("")),
        ];
        let fail = vec![
            package(DH, Some("")),
            package(DH, Some("\n")),
            package(DH, Some("  \t\n12\n")),
        ];
        SourceTreeTester::new(DebhelperCompatFileIsEmpty, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(DH, Some("12\n")))];
        let fail = vec![SourcePackageCase::from(package(DH, Some("\n")))];
        SourcePackageTester::new(DebhelperCompatFileIsEmpty, pass, fail).test_and_snapshot();
    }
}
