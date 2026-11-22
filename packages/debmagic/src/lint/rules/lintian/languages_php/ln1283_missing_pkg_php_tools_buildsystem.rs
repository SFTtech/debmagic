use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::pear::{PearSource, RULES, rules_facts};

pub struct MissingPkgPhpToolsBuildsystem;

declare_rule! {
    /// A PEAR or PECL package (its `package.xml` or `package2.xml`, or a
    /// `channel.xml`) does not build with `dh --buildsystem=phppear` in
    /// `debian/rules`.
    ///
    /// Only when Build-Depends satisfies `pkg-php-tools`, a `.php` file is
    /// shipped, and `debian/rules` is a regular file or hardlink (a symlink is
    /// not followed). A line with a `dh` call and `--buildsystem` naming
    /// `phppear` counts; a commented-out line does not. The Diagnostic has
    /// `phppear` as extra and `debian/rules` as Location.
    MissingPkgPhpToolsBuildsystem,
    code = "LN1283",
    tag = "missing-pkg-php-tools-buildsystem",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingPkgPhpToolsBuildsystem {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(pear) = PearSource::new(ctx) else {
            return;
        };
        if !pear.has_pkg_php_tools() || !(pear.is_pear() || pear.channel_xml) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(facts) = rules_facts(files, &pear.build_depends) else {
            return;
        };
        if !facts.buildsystem_phppear {
            ctx.diagnostic(Diagnostic::warn("phppear").with_location(Location {
                path: PathBuf::from(RULES),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    const PEAR_FILE: &str = "<package>\n <name>foo</name>\n</package>\n";

    fn pear_tree(rules: &str) -> TestCase {
        TestCase::source_tree([
            (
                "debian/control",
                TestFile::new(
                    "Source: foo\nBuild-Depends: debhelper-compat (= 13), pkg-php-tools\n\nPackage: foo\nArchitecture: all\nDescription: foo\n extra\n",
                ),
            ),
            ("debian/rules", TestFile::new(rules.to_string())),
            ("package.xml", TestFile::new(PEAR_FILE)),
            ("foo.php", TestFile::new("<?php\n")),
        ])
    }

    #[test]
    fn source() {
        let pass = vec![
            pear_tree("#!/usr/bin/make -f\n\tdh $@ --buildsystem=phppear\n"),
            pear_tree("#!/usr/bin/make -f\n\tdh $@ --buildsystem phppear --with phppear\n"),
            pear_tree("#!/usr/bin/make -f\n\tdh $@ --buildsystem=phppear,foo\n"),
        ];
        let fail = vec![
            pear_tree("#!/usr/bin/make -f\n\tdh $@\n"),
            // Not a dh call line, or commented out.
            pear_tree("#!/usr/bin/make -f\n# \tdh $@ --buildsystem=phppear\n"),
            // Only the addon, not the build system.
            pear_tree("#!/usr/bin/make -f\n\tdh $@ --with phppear\n"),
        ];
        SourceTreeTester::new(MissingPkgPhpToolsBuildsystem, pass, fail).test_and_snapshot();
    }
}
