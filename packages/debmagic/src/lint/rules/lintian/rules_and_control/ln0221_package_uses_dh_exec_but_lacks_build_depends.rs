use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::control_text::{build_depends_all_names, names_package};
use super::rules_text::{debian_texts, read_rules};

pub struct PackageUsesDhExecButLacksBuildDepends;

declare_rule! {
    /// Packaging mentions `dh-exec` without a build dependency on it.
    ///
    /// `debian/rules` or any other readable UTF-8 file under `debian/`
    /// contains the text `dh-exec`, and `Build-Depends`, `Build-Depends-Indep`,
    /// and `Build-Depends-Arch` do not name the package `dh-exec`. The check
    /// is the name, not a relation satisfiability test, and it does not look
    /// at the executable bit. One report. Location is `debian/rules` with no
    /// line, so a missing or unreadable `debian/rules` is not reported.
    /// Applies to the Source tree and Source package.
    PackageUsesDhExecButLacksBuildDepends,
    code = "LN0221",
    tag = "package-uses-dh-exec-but-lacks-build-depends",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PackageUsesDhExecButLacksBuildDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let names = ctx
            .debian_control()
            .map(|control| build_depends_all_names(control.source()))
            .unwrap_or_default();
        if names_package(&names, "dh-exec") {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        if read_rules(files).is_none() {
            return;
        }
        let mentions = debian_texts(files)
            .iter()
            .any(|(_, text)| text.contains("dh-exec"));
        if !mentions {
            return;
        }
        ctx.diagnostic(Diagnostic::error("").with_location(Location {
            path: PathBuf::from("debian/rules"),
            line: None,
            column: None,
        }));
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

    const RULES: &str = "#!/usr/bin/make -f\n\n%:\n\tdh $@\n";

    fn package(
        control: &str,
        rules: &str,
        extra: &[(&'static str, &str)],
    ) -> Vec<(&'static str, TestFile)> {
        let mut tree = vec![
            ("debian/control", TestFile::new(control)),
            ("debian/rules", TestFile::new(rules)),
        ];
        for (path, text) in extra {
            tree.push((path, TestFile::new(*text)));
        }
        tree
    }

    fn with_depends(value: &str) -> String {
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
Build-Depends: {value}\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n"
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            package(CONTROL, RULES, &[]),
            package(&with_depends("dh-exec"), "# uses dh-exec\n", &[]),
            package(
                &with_depends("debhelper, dh-exec (>= 0.1)"),
                RULES,
                &[("debian/install", "#! /usr/bin/dh-exec\n")],
            ),
            package(
                "\
Source: example\n\
Build-Depends-Indep: something | dh-exec\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
                "# dh-exec\n",
                &[],
            ),
            package(
                "\
Source: example\n\
Build-Depends-Arch: dh-exec:any\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
                RULES,
                &[("debian/manpages", "#! /usr/bin/dh-exec\n")],
            ),
            // Outside `debian/`, or no rules file to point at.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                ("README", TestFile::new("dh-exec")),
            ],
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/install", TestFile::new("#! /usr/bin/dh-exec\n")),
            ],
        ];
        let fail = vec![
            package(CONTROL, "# we call dh-exec from the recipe\n", &[]),
            package(
                CONTROL,
                RULES,
                &[("debian/install", "#! /usr/bin/dh-exec\nusr/bin/foo\n")],
            ),
            package(
                CONTROL,
                RULES,
                &[("debian/tests/control", "Depends: @, dh-exec\n")],
            ),
            // Several mentions are still one report.
            package(
                CONTROL,
                "# dh-exec\n",
                &[("debian/install", "#! /usr/bin/dh-exec\n")],
            ),
            package(
                &with_depends("dh-exec-tools"),
                RULES,
                &[("debian/links", "#! /usr/bin/dh-exec\n")],
            ),
        ];
        SourceTreeTester::new(PackageUsesDhExecButLacksBuildDepends, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(CONTROL, RULES, &[]))];
        let fail = vec![SourcePackageCase::from(package(
            CONTROL,
            RULES,
            &[("debian/install", "#! /usr/bin/dh-exec\n")],
        ))];
        SourcePackageTester::new(PackageUsesDhExecButLacksBuildDepends, pass, fail)
            .test_and_snapshot();
    }
}
