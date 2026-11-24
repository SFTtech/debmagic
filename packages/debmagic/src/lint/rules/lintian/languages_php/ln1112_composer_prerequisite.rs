use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct ComposerPrerequisite;

declare_rule! {
    /// The packaging relates to the PHP `composer` tool, which should not be
    /// pulled in as a packaging relationship.
    ///
    /// The source paragraph's Build-Depends, Build-Depends-Indep,
    /// Build-Conflicts and Build-Conflicts-Indep, and each installable's
    /// Pre-Depends, Depends, Recommends, Suggests, Breaks, Conflicts,
    /// Provides, Replaces and Enhances, are reported when the field's Package
    /// relation satisfies `composer`. Extras are the field name and
    /// `(in source paragraph)` or `(in section for <installable>)`. Location is
    /// `debian/control` at the field's line, found in the packaging tree's
    /// file. Installables are the paragraphs with a valid Package name, as in
    /// Debian control. A `debian/control` that fails to parse is skipped.
    ComposerPrerequisite,
    code = "LN1112",
    tag = "composer-prerequisite",
    default_selected = true,
    experimental = false,
}

const SOURCE_FIELDS: &[&str] = &[
    "Build-Depends",
    "Build-Depends-Indep",
    "Build-Conflicts",
    "Build-Conflicts-Indep",
];
const INSTALLABLE_FIELDS: &[&str] = &[
    "Pre-Depends",
    "Depends",
    "Recommends",
    "Suggests",
    "Breaks",
    "Conflicts",
    "Provides",
    "Replaces",
    "Enhances",
];

impl SourceRule for ComposerPrerequisite {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };

        let mut hits = Vec::new();
        for field in SOURCE_FIELDS {
            let satisfies = control
                .source()
                .relation(field)
                .is_some_and(|relation| relation.satisfies("composer"));
            if satisfies {
                hits.push((
                    control.source().position(field),
                    format!("{field} (in source paragraph)"),
                ));
            }
        }
        for installable in control.installables() {
            for field in INSTALLABLE_FIELDS {
                let satisfies = installable
                    .fields()
                    .relation(field)
                    .is_some_and(|relation| relation.satisfies("composer"));
                if satisfies {
                    hits.push((
                        installable.fields().position(field),
                        format!("{field} (in section for {})", installable.name()),
                    ));
                }
            }
        }
        for (line, extra) in hits {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path: PathBuf::from("debian/control"),
                line,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str, installable: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\n{fields}Standards-Version: 4.7.2\n\n\
                 Package: example\nArchitecture: all\n{installable}\
                 Description: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control("", ""),
            control("Build-Depends: debhelper-compat (= 14)\n", ""),
            // Other packages, and names that only start with composer.
            control("Build-Depends: composer-foo\n", ""),
            control("", "Depends: composer-foo\n"),
            control("", "Depends: composer | foo\n"),
            control("", "Recommends: php-composer-foo\n"),
            // Not a relation field.
            control("X-Composer: composer\n", ""),
        ];
        let fail = vec![
            control("Build-Depends: composer\n", ""),
            control("Build-Depends-Indep: composer (>= 2)\n", ""),
            control("Build-Conflicts: composer\n", ""),
            control("Build-Conflicts-Indep: composer\n", ""),
            control("", "Depends: foo, composer\n"),
            control("", "Recommends: composer\n"),
            control("", "Suggests: composer\n"),
            control("", "Breaks: composer\n"),
            control("", "Conflicts: composer\n"),
            control("", "Provides: composer\n"),
            control("", "Replaces: composer\n"),
            control("", "Enhances: composer\n"),
            // Pre-Depends in an installable is reported with its section name.
            control("", "Pre-Depends: composer\n"),
            // The source paragraph is checked with its own line.
            control(
                "Build-Depends: composer\nBuild-Depends-Indep: composer\n",
                "",
            ),
        ];
        SourceTreeTester::new(ComposerPrerequisite, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(control(
            "Build-Depends: debhelper-compat (= 14)\n",
            "",
        ))];
        let fail = vec![SourcePackageCase::from(control(
            "Build-Depends: composer\n",
            "",
        ))];
        SourcePackageTester::new(ComposerPrerequisite, pass, fail).test_and_snapshot();
    }
}
