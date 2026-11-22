use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::pear::PearSource;

pub struct PearPackageButMissingDependency;

declare_rule! {
    /// A PEAR package (its `package.xml` or `package2.xml`) whose PEAR
    /// packaging is set up (Build-Depends satisfies `pkg-php-tools` or
    /// `dh-sequence-phppear`) does not have the pkg-php-tools substvars in
    /// the first installable's relations: `${phppear:Debian-Depends}` in
    /// Depends, `${phppear:Debian-Recommends}` in Recommends and
    /// `${phppear:Debian-Breaks}` in Breaks. Each missing one is reported
    /// with the field name as extra and no Location. A Source without an
    /// installable reports all three.
    ///
    /// Only when a `.php` file is shipped. The Rule is the same for PECL
    /// packages, which have the same `package.xml`.
    PearPackageButMissingDependency,
    code = "LN1285",
    tag = "pear-package-but-missing-dependency",
    default_selected = true,
    experimental = false,
}

const SUBSTVARS: &[(&str, &str)] = &[
    ("Depends", "${phppear:Debian-Depends}"),
    ("Recommends", "${phppear:Debian-Recommends}"),
    ("Breaks", "${phppear:Debian-Breaks}"),
];

impl SourceRule for PearPackageButMissingDependency {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(pear) = PearSource::new(ctx) else {
            return;
        };
        if !pear.is_pear() || !pear.has_pear_builddep() {
            return;
        }
        let installable = ctx
            .debian_control()
            .and_then(|control| control.installables().first());
        let missing: Vec<&str> = SUBSTVARS
            .iter()
            .filter(|(field, substvar)| {
                let implied = installable
                    .and_then(|installable| installable.fields().relation(field))
                    .unwrap_or_default()
                    .satisfies(substvar);
                !implied
            })
            .map(|(field, _)| *field)
            .collect();
        for field in missing {
            ctx.diagnostic(Diagnostic::warn(field));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    fn tree(build_depends: &str, binary: &str) -> TestCase {
        TestCase::source_tree([
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: foo\nBuild-Depends: {build_depends}\n\nPackage: foo\nArchitecture: all\n{binary}Description: foo\n extra\n"
                )),
            ),
            ("package.xml", TestFile::new("<package/>\n")),
            ("foo.php", TestFile::new("<?php\n")),
        ])
    }

    const ALL: &str = "Depends: ${phppear:Debian-Depends}\nRecommends: ${phppear:Debian-Recommends}\nBreaks: ${phppear:Debian-Breaks}\n";

    #[test]
    fn source() {
        let pass = vec![
            tree("pkg-php-tools", ALL),
            tree("dh-sequence-phppear", ALL),
            // Other fields may be present too.
            tree(
                "pkg-php-tools",
                "Depends: php, ${phppear:Debian-Depends}\nRecommends: ${phppear:Debian-Recommends}\nBreaks: ${phppear:Debian-Breaks}\n",
            ),
        ];
        let fail = vec![
            tree(
                "pkg-php-tools",
                "Depends: ${phppear:Debian-Depends}\nBreaks: ${phppear:Debian-Breaks}\n",
            ),
            tree(
                "pkg-php-tools",
                "Depends: php\nRecommends: ${phppear:Debian-Recommends}\nBreaks: ${phppear:Debian-Breaks}\n",
            ),
            // Only the source paragraph is there: all three are missing.
            tree("pkg-php-tools", ""),
        ];
        SourceTreeTester::new(PearPackageButMissingDependency, pass, fail).test_and_snapshot();
    }
}
