use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debhelper::prerequisites::command_prerequisite;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct MissingBuildDependencyForDhCommand;

declare_rule! {
    /// `debian/rules` runs a `dh_*` command without a build dependency on the
    /// package that provides it.
    ///
    /// Each `dh_*` command line of `debian/rules` outside make conditionals
    /// (`ifeq`, `ifneq`, `ifdef`, `ifndef` up to `endif`) is looked up in
    /// lintian 2.139.0's debhelper command table: the hand-maintained
    /// overrides first, else every package that installs the command, each
    /// as `:any` and joined by ` | ` (`debhelper` also allows
    /// `debhelper-compat`). Commands sharing a prerequisite are reported once,
    /// naming the last of them. Reported when `Build-Depends`,
    /// `Build-Depends-Indep`, and `Build-Depends-Arch` together, with
    /// versions, architecture restrictions, and build profiles dropped, do not
    /// imply the prerequisite. At compat level 10 or newer, `autotools-dev`
    /// and `dh-strip-nondeterminism` are not required. Only reached after a
    /// dh command has been seen; a `Build-Driver` other than `debian-rules` is
    /// not scanned. Extra is the command and `(does not satisfy` the
    /// prerequisite `)`. Location is `debian/rules` with no line. Applies to
    /// the Source tree and Source package.
    MissingBuildDependencyForDhCommand,
    code = "LN0705",
    tag = "missing-build-dependency-for-dh_-command",
    default_selected = true,
    experimental = false,
}

/// lintian 2.139.0 `$REQUIRES_AUTOTOOLS`.
const REQUIRES_AUTOTOOLS: u32 = 10;

/// Prerequisites lintian drops from compat level 10 on.
const BUILT_IN_FROM_LEVEL_10: &[&str] = &["autotools-dev", "dh-strip-nondeterminism"];

const BUILD_DEPENDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

impl SourceRule for MissingBuildDependencyForDhCommand {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() {
            return;
        }
        // lintian's `%command_by_prerequisite`: the last command wins.
        let mut by_prerequisite: Vec<(String, &str)> = Vec::new();
        for command in facts
            .commands()
            .iter()
            .filter(|command| !command.in_make_conditional())
        {
            let Some(prerequisite) = command_prerequisite(command.name()) else {
                continue;
            };
            match by_prerequisite
                .iter_mut()
                .find(|(seen, _)| *seen == prerequisite)
            {
                Some(entry) => entry.1 = command.name(),
                None => by_prerequisite.push((prerequisite, command.name())),
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
        for (prerequisite, command) in by_prerequisite {
            if prerequisite == "debhelper" {
                continue;
            }
            if level >= REQUIRES_AUTOTOOLS
                && BUILT_IN_FROM_LEVEL_10.contains(&prerequisite.as_str())
            {
                continue;
            }
            if build_depends.satisfies(&prerequisite) {
                continue;
            }
            found.push(format!("{command} (does not satisfy {prerequisite})"));
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

    fn rules(commands: &str) -> String {
        format!("%:\n\tdh $@\n\nbinary-indep:\n{commands}")
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package(Some("debhelper"), &rules("\tdh_md5sums\n")),
            package(Some("debhelper-compat (= 13)"), &rules("\tdh_md5sums\n")),
            // Versions, architecture restrictions, and profiles are dropped.
            package(
                Some("debhelper (>= 13) [amd64], dh-lisp <!nocheck>"),
                &rules("\tdh_lisp\n"),
            ),
            package(Some("debhelper, cli-common-dev"), &rules("\t-dh_clideps\n")),
            // Inside a make conditional nothing is checked.
            package(
                Some("debhelper"),
                &rules("ifeq \"$(A)\" \"y\"\n\tdh_python3\nendif\n"),
            ),
            // A manual entry: dh-sequence-python3 is enough.
            package(
                Some("debhelper, dh-sequence-python3"),
                &rules("\tdh_python3\n"),
            ),
            // Unknown command and comments are ignored.
            package(
                Some("debhelper"),
                &rules("\tdh_will_never_exist\n#\tdh_lisp\n"),
            ),
            // dh_autoreconf is not counted as a dh command.
            package(Some("debhelper"), &rules("\tdh_autoreconf\n")),
            // No dh command: lintian returns before.
            vec![
                ("debian/control", TestFile::new(control(None))),
                ("debian/rules", TestFile::new("build:\n\ttrue\n")),
            ],
            // Compat 10 and newer need neither of these.
            vec![
                ("debian/control", TestFile::new(control(Some("debhelper")))),
                (
                    "debian/rules",
                    TestFile::new(rules(
                        "\tdh_strip_nondeterminism\n\tdh_autotools-dev_updateconfig\n",
                    )),
                ),
                ("debian/compat", TestFile::new("10\n")),
            ],
            // Another Build-Driver is not scanned.
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
                ("debian/rules", TestFile::new(rules("\tdh_lisp\n"))),
            ],
        ];
        let fail = vec![
            package(None, &rules("\tdh_md5sums\n")),
            // Recipe debhelper-no-depends.
            package(
                None,
                &rules(
                    "\t-dh_clideps\n\t-dh_gstscancodecs\n\tdh_lisp || true\n\
                     ifeq \"$(X)\" \"y\"\n\tdh_python3\nendif\n\
                     \t-dh_strip_nondeterminism\n\t-dh_scour\n",
                ),
            ),
            // Commands sharing a prerequisite: the last one is named.
            package(Some("foo"), &rules("\tdh_testroot\n\tdh_installdocs\n")),
            // An alternative is not an implication, and a version is not kept.
            package(
                Some("debhelper | foo, dh-lisp (>= 1) | bar"),
                &rules("\tdh_installdocs\n\tdh_lisp\n\tdh_apache2\n"),
            ),
            // dh-strip-nondeterminism is still needed below compat 10.
            package(Some("debhelper"), &rules("\tdh_strip_nondeterminism\n")),
            // A stray endif leaves Perl's counter negative, which still skips;
            // only the command before it is checked.
            package(Some("debhelper"), &rules("\tdh_lisp\nendif\n\tdh_scour\n")),
        ];
        SourceTreeTester::new(MissingBuildDependencyForDhCommand, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            Some("debhelper, dh-lisp"),
            &rules("\tdh_lisp\n"),
        ))];
        let fail = vec![SourcePackageCase::from(package(
            Some("debhelper"),
            &rules("\tdh_lisp\n"),
        ))];
        SourcePackageTester::new(MissingBuildDependencyForDhCommand, pass, fail)
            .test_and_snapshot();
    }
}
