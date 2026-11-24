use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::control_text::{build_depends_all_names, names_package};
use super::dh_exec::dh_exec_scripts;

pub struct PackageUsesDhExecButLacksBuildDepends;

declare_rule! {
    /// Packaging uses a dh-exec script without a build dependency on dh-exec.
    ///
    /// A dh-exec script is a file directly under `debian/` that lintian's
    /// debhelper check reads as a filename config (`install`, `links`,
    /// `<package>.manpages`, ...) whose `#!` line names `dh-exec`. It counts
    /// only when `debian/rules` runs debhelper and the compat level is 9 or
    /// higher. Maintainer scripts, `debian/rules`, subfolders, and other text
    /// files (a changelog, for example) do not count. The check is the name,
    /// not a relation satisfiability test, and it does not look at the
    /// executable bit. `Build-Depends`, `Build-Depends-Indep` and
    /// `Build-Depends-Arch` are read from the `.dsc` of a source package, as
    /// lintian does, and from `debian/control` on a source tree. One report,
    /// located at `debian/rules` with no line. Applies to the Source tree and
    /// Source package.
    PackageUsesDhExecButLacksBuildDepends,
    code = "LN0221",
    tag = "package-uses-dh-exec-but-lacks-build-depends",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PackageUsesDhExecButLacksBuildDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let names = ctx
            .relation_fields()
            .map(build_depends_all_names)
            .unwrap_or_default();
        if names_package(&names, "dh-exec") {
            return;
        }
        if dh_exec_scripts(ctx).is_empty() {
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
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const COMPAT: &str = "Build-Depends: debhelper-compat (= 13)";

    const RULES: &str = "#!/usr/bin/make -f\n\n%:\n\tdh $@\n";

    const INSTALL: [(&str, &str); 1] = [("debian/install", "#! /usr/bin/dh-exec\n")];

    fn control(fields: &[&str]) -> String {
        let mut text = String::from(
            "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: 4.7.2\n",
        );
        for field in fields {
            text.push_str(field);
            text.push('\n');
        }
        text.push_str("\nPackage: example\nArchitecture: all\nDescription: example\n extra\n");
        text
    }

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

    fn dsc(fields: &str) -> Dsc {
        Dsc::parse(&format!("Source: example\nVersion: 1.0\n{fields}"))
            .expect("generated .dsc paragraph")
    }

    /// lintian's debhelper Check walks only the direct children of `debian/`
    /// (`$droot->children`) and skips maintainer scripts; only a filename
    /// config whose `#!` names dh-exec sets `$uses_dh_exec`.
    #[test]
    fn only_debhelper_config_files_count() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            package(&control(&[COMPAT]), RULES, &[]),
            // The rules file alone is not a config file.
            package(&control(&[COMPAT]), "# uses dh-exec\n", &[]),
            // A changelog mention is not a config file.
            package(
                &control(&[COMPAT]),
                RULES,
                &[("debian/changelog", "dh-exec support\n")],
            ),
            // Not a filename category, a maintainer script, or in a subfolder.
            package(
                &control(&[COMPAT]),
                RULES,
                &[
                    ("debian/NEWS", "#! /usr/bin/dh-exec\n"),
                    ("debian/foo.postinst", "#! /usr/bin/dh-exec\n"),
                    ("debian/tests/control", "#! /usr/bin/dh-exec\n"),
                ],
            ),
            // Outside `debian/`, or no rules file means no debhelper.
            vec![
                ("debian/control", TestFile::new(control(&[COMPAT]))),
                ("README", TestFile::new("#! /usr/bin/dh-exec\n")),
            ],
            vec![
                ("debian/control", TestFile::new(control(&[COMPAT]))),
                ("debian/install", TestFile::new("#! /usr/bin/dh-exec\n")),
            ],
        ];
        let fail = vec![
            package(&control(&[COMPAT]), RULES, &INSTALL),
            package(
                &control(&[COMPAT]),
                RULES,
                &[("debian/example.links", "#! /usr/bin/dh-exec\n")],
            ),
            // Several configs are still one report.
            package(
                &control(&[COMPAT]),
                "%:\n\tdh $@\n# dh-exec\n",
                &[
                    ("debian/install", "#! /usr/bin/dh-exec\n"),
                    ("debian/links", "#! /usr/bin/dh-exec\n"),
                ],
            ),
        ];
        SourceTreeTester::new(PackageUsesDhExecButLacksBuildDepends, pass, fail)
            .test_and_snapshot();
    }

    /// lintian reads the dh-exec features only when `$debhelper_level >= 9`
    /// (`$USES_EXECUTABLE_FILES`).
    #[test]
    fn compat_below_nine_is_not_checked() {
        let pass = vec![package(
            &control(&["Build-Depends: debhelper-compat (= 8)"]),
            RULES,
            &INSTALL,
        )];
        let fail = vec![package(
            &control(&["Build-Depends: debhelper-compat (= 9)"]),
            RULES,
            &INSTALL,
        )];
        SourceTreeTester::new(PackageUsesDhExecButLacksBuildDepends, pass, fail).test();
    }

    /// lintian's `$build_prerequisites->satisfies('dh-exec')` is checked
    /// against `Build-Depends-All`, so any of the three fields satisfies it;
    /// a different package name only sharing a prefix does not.
    #[test]
    fn build_depends_all_fields_satisfy_the_check() {
        let pass = vec![
            package(
                &control(&["Build-Depends: debhelper-compat (= 13), dh-exec (>= 0.1)"]),
                RULES,
                &INSTALL,
            ),
            package(
                &control(&[COMPAT, "Build-Depends-Indep: something | dh-exec"]),
                RULES,
                &INSTALL,
            ),
            package(
                &control(&["Build-Depends-Arch: dh-exec:any", COMPAT]),
                RULES,
                &INSTALL,
            ),
        ];
        let fail = vec![package(
            &control(&["Build-Depends: debhelper-compat (= 13), dh-exec-tools"]),
            RULES,
            &INSTALL,
        )];
        SourceTreeTester::new(PackageUsesDhExecButLacksBuildDepends, pass, fail).test();
    }

    /// lintian reads the relation fields of a source package from its `.dsc`
    /// (`Lintian::Processable::Source::init_from_file`), not from
    /// `debian/control`, so a disagreeing `debian/control` is ignored.
    #[test]
    fn source_package_relations_come_from_the_dsc() {
        let pass = vec![
            SourcePackageCase::from(package(&control(&[COMPAT]), RULES, &INSTALL))
                .dsc(dsc("Build-Depends: debhelper-compat (= 13), dh-exec\n")),
        ];
        let fail = vec![
            SourcePackageCase::from(package(&control(&[COMPAT]), RULES, &INSTALL)),
            SourcePackageCase::from(package(
                &control(&[COMPAT, "Build-Depends-Indep: dh-exec"]),
                RULES,
                &INSTALL,
            ))
            .dsc(dsc("Build-Depends: debhelper-compat (= 13)\n")),
        ];
        SourcePackageTester::new(PackageUsesDhExecButLacksBuildDepends, pass, fail)
            .test_and_snapshot();
    }
}
