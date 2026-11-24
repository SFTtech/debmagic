use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dh_exec::dh_exec_scripts;

pub struct DhExecScriptWithoutDhExecFeatures;

declare_rule! {
    /// A dh-exec script uses none of dh-exec's features.
    ///
    /// Only reached after `debian/rules` has shown a dh command, and only at
    /// compat level 9 or newer. A direct child of `debian/` named after a
    /// filename config (`dirs`, `docs`, `examples`, `info`, `install`,
    /// `links`, `manpages`, `mime`, `sgmlcatalogs`, `sharedmimeinfo`, `wm`,
    /// optionally with a `<package>.` prefix) whose `#!` line names
    /// `dh-exec` is read. It reports when no line has a `${...}` substitution
    /// (which counts only below compat level 13, where `dh_install` does it
    /// itself), a ` => ` rename, or a `[...]` / `<...>` filter. Maintainer
    /// scripts and `debian/rules` are skipped. Extra is empty. Location is
    /// the path with no line. Applies to the Source tree and Source package.
    DhExecScriptWithoutDhExecFeatures,
    code = "LN0532",
    tag = "dh-exec-script-without-dh-exec-features",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DhExecScriptWithoutDhExecFeatures {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for script in dh_exec_scripts(ctx) {
            if script.substitution || script.install || script.filter {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            package("12\n", &[]),
            // A rename, a filter, or a substitution is a feature.
            package(
                "12\n",
                &[("debian/install", "#!/usr/bin/dh-exec\nfoo => bar\n")],
            ),
            package(
                "12\n",
                &[("debian/install", "#!/usr/bin/dh-exec\nfoo [linux-any]\n")],
            ),
            package(
                "12\n",
                &[("debian/install", "#!/usr/bin/dh-exec\n<!nocheck> foo\n")],
            ),
            package(
                "12\n",
                &[(
                    "debian/install",
                    "#!/usr/bin/dh-exec\nusr/lib/${DEB_HOST_MULTIARCH}\n",
                )],
            ),
            // Not a dh-exec script.
            package("12\n", &[("debian/mime", "plain text\n")]),
            package("12\n", &[("debian/mime", "#!/bin/sh\necho\n")]),
            // Not a filename config, or a maintainer script.
            package("12\n", &[("debian/other", "#!/usr/bin/dh-exec\nfoo\n")]),
            package("12\n", &[("debian/postinst", "#!/usr/bin/dh-exec\nfoo\n")]),
            package("12\n", &[("debian/pkg.prerm", "#!/usr/bin/dh-exec\nfoo\n")]),
            // Scripts only count from compat level 9.
            package("8\n", &[("debian/mime", "#!/usr/bin/dh-exec\nfoo\n")]),
            // Substitutions are done by dh_install from compat level 13.
            package(
                "13\n",
                &[("debian/install", "#!/usr/bin/dh-exec\nfoo => bar\n")],
            ),
            // Only direct children of `debian/`.
            package("12\n", &[("debian/sub/mime", "#!/usr/bin/dh-exec\nfoo\n")]),
        ];
        let fail = vec![
            package("12\n", &[("debian/mime", "#!/usr/bin/dh-exec\nfoo\n")]),
            package(
                "12\n",
                &[("debian/pkg.install", "#! /usr/bin/env dh-exec \nfoo\n")],
            ),
            package("9\n", &[("debian/links", "#!/usr/bin/dh-exec\na b\n")]),
            // A substitution is no feature at compat level 13.
            package(
                "13\n",
                &[(
                    "debian/install",
                    "#!/usr/bin/dh-exec\nusr/lib/${DEB_HOST_MULTIARCH}\n",
                )],
            ),
            package(
                "12\n",
                &[
                    ("debian/mime", "#!/usr/bin/dh-exec\nfoo\n"),
                    ("debian/pkg.docs", "#!/usr/bin/dh-exec\nREADME\n"),
                ],
            ),
        ];
        SourceTreeTester::new(DhExecScriptWithoutDhExecFeatures, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package("12\n", &[]))];
        let fail = vec![SourcePackageCase::from(package(
            "12\n",
            &[("debian/mime", "#!/usr/bin/dh-exec\nfoo\n")],
        ))];
        SourcePackageTester::new(DhExecScriptWithoutDhExecFeatures, pass, fail).test_and_snapshot();
    }
}
