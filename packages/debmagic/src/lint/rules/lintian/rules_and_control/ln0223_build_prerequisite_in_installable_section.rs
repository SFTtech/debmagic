use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct BuildPrerequisiteInInstallableSection;

declare_rule! {
    /// A source relationship field is declared on an installable.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, `Build-Conflicts`, and
    /// `Build-Conflicts-Indep` on a binary paragraph. `Build-Depends-Arch`
    /// and `Build-Conflicts-Arch` are not this Tag. Extra is the field name
    /// then `(in section for <package>)`. Location is `debian/control` at the
    /// field. Applies to the Source tree and Source package.
    BuildPrerequisiteInInstallableSection,
    code = "LN0223",
    tag = "build-prerequisite-in-installable-section",
    default_selected = true,
    experimental = false,
}

const BUILD_FIELDS: &[&str] = &[
    "Build-Depends",
    "Build-Depends-Indep",
    "Build-Conflicts",
    "Build-Conflicts-Indep",
];

impl SourceRule for BuildPrerequisiteInInstallableSection {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let mut reports = Vec::new();
        for installable in control.installables() {
            let fields = installable.fields();
            for field in BUILD_FIELDS.iter().filter(|field| fields.declares(field)) {
                reports.push((
                    format!("{field} (in section for {})", installable.name()),
                    fields.position(field),
                ));
            }
        }
        for (extra, line) in reports {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
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

    fn control(text: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(text))]
    }

    const PLAIN: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
Build-Depends: debhelper\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no control"))],
            control(PLAIN),
            control(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Build-Depends-Arch: debhelper\n\
Build-Conflicts-Arch: foo\n\
Description: example\n extra\n",
            ),
        ];
        let fail = vec![
            control(
                "\
Source: example\n\
Build-Depends: debhelper\n\
\n\
Package: example\n\
Architecture: all\n\
Build-Depends: again\n\
Description: example\n extra\n",
            ),
            control(
                "\
Source: foo++\n\
\n\
Package: foo++\n\
Architecture: all\n\
Build-Depends: test\n\
Description: plus\n signs\n",
            ),
            control(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Build-Depends-Indep: perl\n\
Build-Conflicts: foo\n\
Build-Conflicts-Indep: bar\n\
Description: example\n extra\n",
            ),
            // The pointer is the first line of a continued field.
            control(
                "Source: example\n\nPackage: example\nArchitecture: all\nBuild-Depends: foo,\n bar\nDescription: example\n extra\n",
            ),
            // The field name in the extra is the canonical spelling.
            control(
                "\
Source: example\n\
\n\
Package: left\n\
Architecture: all\n\
build-depends: foo\n\
Description: left\n package\n\
\n\
Package: right\n\
Architecture: all\n\
Build-Conflicts: bar\n\
Description: right\n package\n",
            ),
        ];
        SourceTreeTester::new(BuildPrerequisiteInInstallableSection, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(control(PLAIN))];
        let fail = vec![SourcePackageCase::from(control(
            "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Build-Conflicts: foo\n\
Description: example\n extra\n",
        ))];
        SourcePackageTester::new(BuildPrerequisiteInInstallableSection, pass, fail)
            .test_and_snapshot();
    }
}
