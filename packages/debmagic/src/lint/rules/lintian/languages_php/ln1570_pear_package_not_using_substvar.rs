use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::pear::PearSource;

pub struct PearPackageNotUsingSubstvar;

declare_rule! {
    /// A PEAR package whose first binary package does not use the pkg-php-tools
    /// substvars in its Description.
    ///
    /// Only for a PEAR package (`package.xml` or `package2.xml`) with a `.php`
    /// file, and Build-Depends satisfying `pkg-php-tools` or
    /// `dh-sequence-phppear`. The Description of the first installable,
    /// untrimmed, must contain `${phppear:summary}` and
    /// `${phppear:description}`; each one missing is its own finding. Extra is
    /// the missing substvar; there is no Location. A missing Description counts
    /// as empty. Severity is info and the Tag is not selected by default, as in
    /// lintian.
    PearPackageNotUsingSubstvar,
    code = "LN1570",
    tag = "pear-package-not-using-substvar",
    default_selected = false,
    experimental = false,
}

impl SourceRule for PearPackageNotUsingSubstvar {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(pear) = PearSource::new(ctx) else {
            return;
        };
        if !pear.is_pear() || !pear.has_pear_builddep() {
            return;
        }
        let description = ctx
            .debian_control()
            .and_then(|control| control.installables().first())
            .map(|installable| {
                installable
                    .fields()
                    .untrimmed_value("Description")
                    .unwrap_or_default()
                    .to_string()
            });
        let Some(description) = description else {
            return;
        };
        for substvar in ["${phppear:summary}", "${phppear:description}"] {
            if !description.contains(substvar) {
                ctx.diagnostic(Diagnostic::warn(substvar).with_severity(Severity::Info));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    fn tree(description: &str, package: &str) -> TestCase {
        TestCase::source_tree(vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: foo\nBuild-Depends: debhelper-compat (= 13), pkg-php-tools\n\nPackage: foo\nArchitecture: all\nDescription: {description}\n"
                )),
            ),
            ("foo.php", TestFile::new("<?php\n")),
            (package, TestFile::new("<package/>\n")),
        ])
    }

    #[test]
    fn source() {
        let both = "${phppear:summary}\n ${phppear:description}";
        let pass = vec![
            tree(both, "package.xml"),
            tree(&format!("x {both}"), "package2.xml"),
            // Not a PEAR package.
            tree("foo", "composer.json"),
        ];
        let fail = vec![
            tree("${phppear:summary}", "package.xml"),
            tree("foo\n ${phppear:description}", "package2.xml"),
            tree("foo", "package.xml"),
        ];
        SourceTreeTester::new(PearPackageNotUsingSubstvar, pass, fail).test_and_snapshot();
    }
}
