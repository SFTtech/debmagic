use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dh_exec::dh_exec_scripts;

pub struct DhExecInstallNotAllowedHere;

declare_rule! {
    /// A dh-exec script renames files in a debhelper config that cannot.
    ///
    /// Only reached after `debian/rules` has shown a dh command, and only at
    /// compat level 9 or newer. A direct child of `debian/` named after a
    /// filename config (optionally with a `<package>.` prefix) whose `#!`
    /// line names `dh-exec` and that has a line with a ` => ` rename reports
    /// unless the config is `install` or `manpages`, the only ones that accept
    /// the rename. Maintainer scripts and `debian/rules` are skipped. Extra
    /// is empty. Location is the path with no line. Applies to the Source tree
    /// and Source package.
    DhExecInstallNotAllowedHere,
    code = "LN0533",
    tag = "dh-exec-install-not-allowed-here",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DhExecInstallNotAllowedHere {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for script in dh_exec_scripts(ctx) {
            if !script.install || script.category == "install" || script.category == "manpages" {
                continue;
            }
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: script.path,
                line: None,
                column: None,
            }));
        }
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

    const RENAME: &str = "#!/usr/bin/dh-exec\nfoo => bar\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            package("12\n", &[]),
            package("12\n", &[("debian/install", RENAME)]),
            package("12\n", &[("debian/pkg.install", RENAME)]),
            package("12\n", &[("debian/manpages", RENAME)]),
            // No rename in the file.
            package(
                "12\n",
                &[("debian/links", "#!/usr/bin/dh-exec\na [linux-any] b\n")],
            ),
            // A rename needs a space or tab on both sides.
            package("12\n", &[("debian/links", "#!/usr/bin/dh-exec\na=>b\n")]),
            // Not a dh-exec script.
            package("12\n", &[("debian/links", "a => b\n")]),
            // Not a filename config.
            package("12\n", &[("debian/other", RENAME)]),
            // Scripts only count from compat level 9.
            package("8\n", &[("debian/links", RENAME)]),
        ];
        let fail = vec![
            package("12\n", &[("debian/links", RENAME)]),
            package(
                "9\n",
                &[("debian/pkg.docs", "#!/usr/bin/dh-exec\na\tb => c\n")],
            ),
            package("13\n", &[("debian/dirs", RENAME), ("debian/mime", RENAME)]),
        ];
        SourceTreeTester::new(DhExecInstallNotAllowedHere, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            "12\n",
            &[("debian/install", RENAME)],
        ))];
        let fail = vec![SourcePackageCase::from(package(
            "12\n",
            &[("debian/links", RENAME)],
        ))];
        SourcePackageTester::new(DhExecInstallNotAllowedHere, pass, fail).test_and_snapshot();
    }
}
