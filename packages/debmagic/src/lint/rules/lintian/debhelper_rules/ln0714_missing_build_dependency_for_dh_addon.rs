use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debhelper::prerequisites::addon_prerequisite;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct MissingBuildDependencyForDhAddon;

declare_rule! {
    /// `debian/rules` passes a dh addon to `dh --with` without a build
    /// dependency on the package that provides it.
    ///
    /// Each `--with` addon of a `dh` line, with `-` read as `_`, is looked up
    /// in lintian 2.139.0's debhelper addon table: the hand-maintained
    /// overrides first, else every package that installs the addon, each as
    /// `:any` and joined by ` | ` (`debhelper` also allows `debhelper-compat`).
    /// Addons sharing a prerequisite are reported once, naming the last of
    /// them. Reported when `Build-Depends`, `Build-Depends-Indep`, and
    /// `Build-Depends-Arch` together, with versions, architecture restrictions,
    /// and build profiles dropped, do not imply the prerequisite. Make
    /// conditionals do not hide an addon. At compat level 10 or newer,
    /// `autoreconf` is not required. `python3` also needs `dh-python`,
    /// `dh-sequence-python3`, or `pybuild-plugin-pyproject` unless
    /// `python3-dev` or `python3-all-dev` is present. Only `dh` lines count;
    /// `dh_*` command lines do not. A `Build-Driver` other than `debian-rules`
    /// is not scanned. Extra is the addon and `(does not satisfy` the
    /// prerequisite `)`. Location is `debian/rules` with no line. Applies to
    /// the Source tree and Source package.
    MissingBuildDependencyForDhAddon,
    code = "LN0714",
    tag = "missing-build-dependency-for-dh-addon",
    default_selected = true,
    experimental = false,
}

/// lintian 2.139.0 `$REQUIRES_AUTOTOOLS`.
const REQUIRES_AUTOTOOLS: u32 = 10;

const BUILD_DEPENDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

/// lintian's special case for the `python3` addon.
const PYTHON3_SOURCE: &str =
    "dh-python:any | dh-sequence-python3:any | pybuild-plugin-pyproject:any";
const PYTHON3_DEV: &str = "python3-dev:any | python3-all-dev:any";

impl SourceRule for MissingBuildDependencyForDhAddon {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() {
            return;
        }
        // lintian's `%addon_by_prerequisite`: the last addon wins.
        let mut by_prerequisite: Vec<(String, &str)> = Vec::new();
        for addon in facts.addons() {
            let addon = addon.as_str();
            let Some(prerequisite) = addon_prerequisite(addon) else {
                continue;
            };
            match by_prerequisite
                .iter_mut()
                .find(|(seen, _)| *seen == prerequisite)
            {
                Some(entry) => entry.1 = addon,
                None => by_prerequisite.push((prerequisite, addon)),
            }
        }
        if by_prerequisite.is_empty() {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let build_depends = control
            .source()
            .relations_and(BUILD_DEPENDS)
            .restriction_less();
        let level = facts.level();
        let mut found = Vec::new();
        for (prerequisite, addon) in by_prerequisite {
            if level >= REQUIRES_AUTOTOOLS && addon == "autoreconf" {
                continue;
            }
            if !build_depends.satisfies(&prerequisite) {
                found.push(format!("{addon} (does not satisfy {prerequisite})"));
            } else if addon == "python3"
                && !build_depends.satisfies(PYTHON3_DEV)
                && !build_depends.satisfies(PYTHON3_SOURCE)
            {
                found.push(format!("{addon} (does not satisfy {PYTHON3_SOURCE})"));
            }
        }
        for message in found {
            ctx.diagnostic(Diagnostic::error(message).with_location(Location {
                path: PathBuf::from("debian/rules"),
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

    fn control(build_depends: Option<&str>) -> String {
        let build_depends = build_depends
            .map(|value| format!("Build-Depends: {value}\n"))
            .unwrap_or_default();
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
{build_depends}\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n"
        )
    }

    fn package(build_depends: Option<&str>, rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control(build_depends))),
            ("debian/rules", TestFile::new(rules)),
            ("debian/compat", TestFile::new("9\n")),
        ]
    }

    fn rules(body: &str) -> String {
        format!("%:\n\tdh $@\n\nbinary-indep:\n{body}")
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package(Some("quilt"), &rules("\tdh $@ --with quilt\n")),
            // Quoted addon lists are read inside the quotes.
            package(Some("quilt"), &rules("\tdh $@ --with 'quilt'\n")),
            package(
                Some("autotools-dev"),
                &rules("\tdh $@ --with=autotools-dev\n"),
            ),
            // python3-dev satisfies the prerequisite and the -dev exception.
            package(
                Some("debhelper, python3-dev"),
                &rules("\tdh $@ --with python3\n"),
            ),
            // dh-python satisfies the python3 special case.
            package(
                Some("dh-python, python3"),
                &rules("\tdh $@ --with python3\n"),
            ),
            // Unknown addon: lintian's empty prerequisite is satisfied.
            package(None, &rules("\tdh $@ --with will-never-exist\n")),
            // dh_* command lines do not carry addons.
            package(None, &rules("\tdh_lisp --with quilt\n")),
            // Build-Driver other than debian-rules is not scanned.
            vec![
                (
                    "debian/control",
                    TestFile::new(
                        "\
Source: example\n\
Build-Driver: debputy\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
                    ),
                ),
                (
                    "debian/rules",
                    TestFile::new(rules("\tdh $@ --with quilt\n")),
                ),
            ],
            // Compat 10 and newer do not require autoreconf.
            vec![
                ("debian/control", TestFile::new(control(None))),
                (
                    "debian/rules",
                    TestFile::new(rules("\tdh $@ --with autoreconf\n")),
                ),
                ("debian/compat", TestFile::new("10\n")),
            ],
        ];
        let fail = vec![
            package(None, &rules("\tdh $@ --with quilt\n")),
            // Dashes read as underscores in the reported addon.
            package(Some("debhelper"), &rules("\tdh $@ --with autotools-dev\n")),
            // Make conditionals do not hide an addon.
            package(
                None,
                &rules("ifeq \"$(A)\" \"y\"\n\tdh $@ --with quilt\nendif\n"),
            ),
            // An alternative does not imply the prerequisite.
            package(Some("foo | quilt"), &rules("\tdh $@ --with quilt\n")),
            // Compat below 10 still requires autoreconf.
            package(None, &rules("\tdh $@ --with autoreconf\n")),
            // python3 needs dh-python or the like, not only python3 itself.
            package(Some("python3"), &rules("\tdh $@ --with python3\n")),
            // Without a python3 build dependency, the prerequisite is missing.
            package(Some("dh-python"), &rules("\tdh $@ --with python3\n")),
            // Repeated addons with the same prerequisite are reported once.
            package(None, &rules("\tdh $@ --with python3 --with python3\n")),
        ];
        SourceTreeTester::new(MissingBuildDependencyForDhAddon, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            Some("quilt"),
            &rules("\tdh $@ --with quilt\n"),
        ))];
        let fail = vec![SourcePackageCase::from(package(
            None,
            &rules("\tdh $@ --with quilt\n"),
        ))];
        SourcePackageTester::new(MissingBuildDependencyForDhAddon, pass, fail).test_and_snapshot();
    }
}
