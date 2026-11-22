use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::pear::{PearSource, RULES, rules_facts};

pub struct MissingPkgPhpToolsAddon;

declare_rule! {
    /// `debian/rules` does not call `dh` with the pkg-php-tools addon that
    /// matches the package: `--with phppear` for a PEAR or PECL package (its
    /// `package.xml` or `package2.xml`), `--with php` also for a PECL
    /// (`extsrc`) package, and `--with phpcomposer` for a Composer package
    /// (`composer.json` without a PEAR package file).
    ///
    /// Only when Build-Depends satisfies `pkg-php-tools`, and a `.php` file is
    /// shipped. `dh-sequence-phppear` and `dh-sequence-phpcomposer` in
    /// Build-Depends count as the addon, when `debian/rules` has a line that
    /// is not a comment. Each missing addon is one Diagnostic, with the addon
    /// name as extra and `debian/rules` as Location. Only a regular file or
    /// hardlink `debian/rules` is read; a symlink is not followed.
    MissingPkgPhpToolsAddon,
    code = "LN1282",
    tag = "missing-pkg-php-tools-addon",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingPkgPhpToolsAddon {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(pear) = PearSource::new(ctx) else {
            return;
        };
        if !pear.has_pkg_php_tools() || !(pear.is_pear() || pear.channel_xml || pear.composer_json)
        {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(facts) = rules_facts(files, &pear.build_depends) else {
            return;
        };
        let mut missing = Vec::new();
        if pear.is_pear() || pear.channel_xml {
            if !facts.addon_phppear {
                missing.push("phppear");
            }
            if pear.package_type == "extsrc" && !facts.addon_php {
                missing.push("php");
            }
        } else if !facts.addon_phpcomposer {
            missing.push("phpcomposer");
        }
        for addon in missing {
            ctx.diagnostic(Diagnostic::warn(addon).with_location(Location {
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
    const PECL_FILE: &str = "<package>\n <name>foo</name>\n <extsrcrelease/>\n</package>\n";

    fn control(build_depends: &str) -> TestFile {
        TestFile::new(format!(
            "Source: foo\nBuild-Depends: {build_depends}\n\nPackage: foo\nArchitecture: all\nDescription: foo\n extra\n"
        ))
    }

    /// A Source with `foo.php` and the given `debian/rules` body.
    fn pear_tree(build_depends: &str, package: Option<(&str, &str)>, rules: &str) -> TestCase {
        let mut files = vec![
            ("debian/control", control(build_depends)),
            ("debian/rules", TestFile::new(rules.to_string())),
            ("foo.php", TestFile::new("<?php\n")),
        ];
        if let Some((name, content)) = package {
            files.push((name, TestFile::new(content.to_string())));
        }
        TestCase::source_tree(files)
    }

    #[test]
    fn source() {
        let pass = vec![
            pear_tree(
                "debhelper-compat (= 13), pkg-php-tools",
                Some(("package.xml", PEAR_FILE)),
                "#!/usr/bin/make -f\n\t dh $@ --with phppear\n",
            ),
            // PECL needs both addons.
            pear_tree(
                "debhelper-compat (= 13), pkg-php-tools",
                Some(("package.xml", PECL_FILE)),
                "#!/usr/bin/make -f\n\tdh $@ --with phppear,php\n",
            ),
            // The addon comes from Build-Depends, with a rule line.
            pear_tree(
                "debhelper-compat (= 13), pkg-php-tools, dh-sequence-phppear",
                Some(("package.xml", PEAR_FILE)),
                "#!/usr/bin/make -f\n\tdh $@\n",
            ),
            // Composer package.
            pear_tree(
                "debhelper-compat (= 13), pkg-php-tools",
                Some(("composer.json", "{}")),
                "#!/usr/bin/make -f\n\tdh $@ --with=phpcomposer\n",
            ),
            // Without pkg-php-tools in Build-Depends, the Check is not run.
            pear_tree(
                "debhelper-compat (= 13)",
                Some(("package.xml", PEAR_FILE)),
                "#!/usr/bin/make -f\n\tdh $@\n",
            ),
        ];
        let fail = vec![
            pear_tree(
                "debhelper-compat (= 13), pkg-php-tools",
                Some(("package.xml", PEAR_FILE)),
                "#!/usr/bin/make -f\n\tdh $@\n",
            ),
            pear_tree(
                "debhelper-compat (= 13), pkg-php-tools",
                Some(("package.xml", PECL_FILE)),
                "#!/usr/bin/make -f\n\tdh $@ --with phppear\n",
            ),
            pear_tree(
                "debhelper-compat (= 13), pkg-php-tools",
                Some(("composer.json", "{}")),
                "#!/usr/bin/make -f\n\tdh $@ --with phppear\n",
            ),
            // The commented-out addon does not count.
            pear_tree(
                "debhelper-compat (= 13), pkg-php-tools",
                Some(("package.xml", PEAR_FILE)),
                "#!/usr/bin/make -f\n# \tdh $@ --with phppear\n",
            ),
        ];
        SourceTreeTester::new(MissingPkgPhpToolsAddon, pass, fail).test_and_snapshot();
    }
}
