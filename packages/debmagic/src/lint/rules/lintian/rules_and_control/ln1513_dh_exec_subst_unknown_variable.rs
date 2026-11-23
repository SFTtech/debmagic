use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dh_exec::dh_exec_scripts;

pub struct DhExecSubstUnknownVariable;

declare_rule! {
    /// A dh-exec script among the debhelper config files of `debian/` (see
    /// `LN0532`) substitutes a `${...}` variable that is not one of the
    /// `DEB_BUILD_*` and `DEB_HOST_*` architecture variables dh-exec knows
    /// without the package exporting them. The extra is the variable name,
    /// and the Location is the file and the line it is on.
    ///
    /// Only the first `${...}` of each line is looked at, as in lintian. The
    /// Rule runs for compat level 9 and up, on a script whose `#!` line names
    /// `dh-exec`. Known are `DEB_(BUILD|HOST)_` followed by `ARCH`, `ARCH_OS`,
    /// `ARCH_CPU`, `ARCH_BITS`, `ARCH_ENDIAN`, `GNU_CPU`, `GNU_SYSTEM`,
    /// `GNU_TYPE` or `MULTIARCH`. Info severity and not selected by default,
    /// as in lintian.
    DhExecSubstUnknownVariable,
    code = "LN1513",
    tag = "dh-exec-subst-unknown-variable",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DhExecSubstUnknownVariable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for script in dh_exec_scripts(ctx) {
            for (line, name) in script.unknown_variables {
                report(ctx, script.path.clone(), line, name);
            }
        }
    }
}

fn report(ctx: &mut SourceContext<'_>, path: PathBuf, line: u32, name: String) {
    ctx.diagnostic(
        Diagnostic::warn(name)
            .with_severity(Severity::Info)
            .with_location(Location {
                path,
                line: Some(line),
                column: None,
            }),
    );
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

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

    fn package(compat: &str, files: &[(&'static str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut tree = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/rules", TestFile::new(DH)),
            ("debian/compat", TestFile::new(compat)),
        ];
        for (path, text) in files {
            tree.push((path, TestFile::new(*text)));
        }
        tree
    }

    #[test]
    fn source() {
        let pass = vec![
            // No dh-exec script.
            package("12\n", &[("debian/install", "usr/bin/${FOO}\n")]),
            // Known variables, of the build and of the host.
            package(
                "12\n",
                &[(
                    "debian/install",
                    "#!/usr/bin/dh-exec\nusr/lib/${DEB_HOST_MULTIARCH}\nusr/${DEB_BUILD_GNU_TYPE}\n",
                )],
            ),
            // Only the first variable of a line is looked at.
            package(
                "12\n",
                &[(
                    "debian/install",
                    "#!/usr/bin/dh-exec\nusr/${DEB_HOST_ARCH} ${FOO}\n",
                )],
            ),
            // Below compat 9 dh-exec is not honoured.
            package(
                "8\n",
                &[("debian/install", "#!/usr/bin/dh-exec\nusr/${FOO}\n")],
            ),
            // Not a filename config.
            package(
                "12\n",
                &[("debian/other", "#!/usr/bin/dh-exec\nusr/${FOO}\n")],
            ),
        ];
        let fail = vec![
            package(
                "12\n",
                &[("debian/install", "#!/usr/bin/dh-exec\nusr/${FOO}\n")],
            ),
            // The line number is the 1-based line of the variable.
            package(
                "13\n",
                &[(
                    "debian/links",
                    "#!/usr/bin/dh-exec\nusr/bin/a usr/bin/b\nusr/lib/${DEB_HOST_ARCH_FOO}\n",
                )],
            ),
            // Each line counts, and each unknown first variable is reported.
            package(
                "12\n",
                &[(
                    "debian/pkg.install",
                    "#!/usr/bin/dh-exec\n${DISTRO}/a\n${DEB_BUILD_ARCH}/b\n${OTHER}/c\n",
                )],
            ),
        ];
        SourceTreeTester::new(DhExecSubstUnknownVariable, pass, fail).test_and_snapshot();
    }
}
