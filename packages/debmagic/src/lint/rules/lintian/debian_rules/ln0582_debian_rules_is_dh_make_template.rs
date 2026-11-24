use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{read_rules_from_tree, rules_location};

pub struct DebianRulesIsDhMakeTemplate;

declare_rule! {
    /// `debian/rules` still carries the `dh_make` template's override notice.
    ///
    /// Scans every physical line of `debian/rules`, comments included and
    /// before any `\` continuation is joined, for the text `dh_make generated
    /// override targets`. Extra is empty. Location is `debian/rules` at that
    /// line. One report per line. Applies to the Source tree and Source
    /// package. A missing, unreadable, or non-UTF-8 `debian/rules` is not
    /// scanned.
    DebianRulesIsDhMakeTemplate,
    code = "LN0582",
    tag = "debian-rules-is-dh_make-template",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianRulesIsDhMakeTemplate {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (index, line) in contents.split('\n').enumerate() {
            if !line.contains("dh_make generated override targets") {
                continue;
            }
            let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
            ctx.diagnostic(Diagnostic::error("").with_location(rules_location(number)));
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

    fn rules(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(CONTROL)),
            (
                "debian/rules",
                TestFile::new(format!("#!/usr/bin/make -f\n\n%:\n\tdh $@\n\n{body}")),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            vec![("debian/rules", TestFile::new(""))],
            rules(""),
            rules("# This file is not a template\n"),
            // The text must be exact.
            rules("# dh_make generated override\n"),
            rules("# dh_make  generated override targets\n"),
            rules("# DH_MAKE GENERATED OVERRIDE TARGETS\n"),
            // Only `debian/rules` is read.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                (
                    "debian/other",
                    TestFile::new("# dh_make generated override targets\n"),
                ),
            ],
        ];
        let fail = vec![
            rules("# dh_make generated override targets\n"),
            rules("# dh_make generated override targets #\n"),
            // Comments are not skipped: the notice is a comment in the template.
            rules("#dh_make generated override targets\n"),
            // Not only comments.
            rules("override_dh_auto_clean:\n\techo dh_make generated override targets\n"),
            // Physical lines, so a continuation does not hide or join the text.
            rules("FOO = a \\\n  dh_make generated override targets\n"),
            // One report per line.
            rules("# dh_make generated override targets\n# dh_make generated override targets\n"),
        ];
        SourceTreeTester::new(DebianRulesIsDhMakeTemplate, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("# no template\n"))];
        let fail = vec![SourcePackageCase::from(rules(
            "# dh_make generated override targets\n",
        ))];
        SourcePackageTester::new(DebianRulesIsDhMakeTemplate, pass, fail).test_and_snapshot();
    }
}
