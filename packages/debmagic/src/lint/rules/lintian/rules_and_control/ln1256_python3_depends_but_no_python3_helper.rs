use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::dh_scan::gated_scan;

pub struct Python3DependsButNoPython3Helper;

declare_rule! {
    /// A binary package depends on `${python3:Depends}` but no Python helper
    /// generates it.
    ///
    /// Scans `debian/rules` the way lintian's `debhelper` Check does. Reported
    /// when a `dh` sequencer line was seen, and neither a `dh_python3` command,
    /// a `python3` addon of a `dh` line, nor a `dh-sequence-python3` build
    /// dependency (with build profiles ignored) is present. Each installable
    /// whose `Pre-Depends`, `Depends`, `Recommends`, or `Suggests` mention
    /// `${python3:Depends}` is named. The extra is those names, sorted and
    /// joined by spaces; there is no Location. Only reached after a dh command
    /// has been seen; a `Build-Driver` other than `debian-rules` is not
    /// scanned. Applies to the Source tree and Source package.
    Python3DependsButNoPython3Helper,
    code = "LN1256",
    tag = "python3-depends-but-no-python3-helper",
    default_selected = true,
    experimental = false,
}

const RELATION_FIELDS: &[&str] = &["Pre-Depends", "Depends", "Recommends", "Suggests"];

impl SourceRule for Python3DependsButNoPython3Helper {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((facts, scan)) = gated_scan(ctx) else {
            return;
        };
        if !scan.sequencer {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let seen_python3 = facts
            .commands()
            .iter()
            .any(|command| command.name() == "dh_python3")
            || facts.addons().iter().any(|addon| addon == "python3")
            || control
                .source()
                .relations_and(&["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"])
                .restriction_less()
                .satisfies("dh-sequence-python3");
        if seen_python3 {
            return;
        }
        let mut names: Vec<&str> = control
            .installables()
            .iter()
            .filter(|installable| {
                installable
                    .fields()
                    .relations_and(RELATION_FIELDS)
                    .satisfies("${python3:Depends}")
            })
            .map(|installable| installable.name())
            .collect();
        if names.is_empty() {
            return;
        }
        names.sort_unstable();
        names.dedup();
        ctx.diagnostic(Diagnostic::error(names.join(" ")));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(packages: &str) -> String {
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
Build-Depends: debhelper-compat (= 13)\n\
\n\
{packages}"
        )
    }

    fn package(control_text: &str, rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control_text.to_string())),
            ("debian/rules", TestFile::new(rules.to_string())),
        ]
    }

    const PYTHON_PACKAGE: &str = "\
Package: python3-foo\n\
Architecture: all\n\
Depends: ${python3:Depends}\n\
Description: example\n extra\n";

    const PLAIN_PACKAGE: &str = "\
Package: foo\n\
Architecture: all\n\
Depends: ${misc:Depends}\n\
Description: example\n extra\n";

    #[test]
    fn test() {
        let dh = "%:\n\tdh $@\n";
        let python = control(PYTHON_PACKAGE);
        let plain = control(PLAIN_PACKAGE);
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            // No dh sequencer line.
            package(&python, "%:\n\tdh_installdocs\n"),
            // The substvar is not in a relation field.
            package(
                &control("Package: foo\nArchitecture: all\nDescription: x\n ${python3:Depends}\n"),
                dh,
            ),
            // No package depends on the substvar.
            package(&plain, dh),
            // A Python helper command.
            package(
                &python,
                "%:\n\tdh $@\n\noverride_dh_python3:\n\tdh_python3\n",
            ),
            // A python3 addon of a dh line.
            package(&python, "%:\n\tdh $@ --with python3\n"),
            // The build dependency provides the sequence.
            package(
                &python.replace(
                    "Build-Depends: debhelper-compat (= 13)",
                    "Build-Depends: debhelper-compat (= 13), dh-sequence-python3",
                ),
                dh,
            ),
        ];
        let fail = vec![
            package(&python, dh),
            package(
                &control(&format!("{PYTHON_PACKAGE}\n{PLAIN_PACKAGE}")),
                "%:\n\tdh $@\n",
            ),
            package(
                &control(&format!(
                    "Package: zeta\nArchitecture: all\nRecommends: ${{python3:Depends}}\nDescription: x\n y\n\n{PYTHON_PACKAGE}"
                )),
                dh,
            ),
        ];
        SourceTreeTester::new(Python3DependsButNoPython3Helper, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dh = "%:\n\tdh $@\n";
        let pass = vec![SourcePackageCase::from(package(
            &control(PLAIN_PACKAGE),
            dh,
        ))];
        let fail = vec![SourcePackageCase::from(package(
            &control(PYTHON_PACKAGE),
            dh,
        ))];
        SourcePackageTester::new(Python3DependsButNoPython3Helper, pass, fail).test_and_snapshot();
    }
}
