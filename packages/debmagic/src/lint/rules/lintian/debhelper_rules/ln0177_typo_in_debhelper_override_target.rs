use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::super::debian_rules::{complete_lines, read_rules_from_tree};
use super::commands::{is_known_target, missing_auto, nearest_target};
use super::scans_debian_rules;

pub struct TypoInDebhelperOverrideTarget;

declare_rule! {
    /// A `debian/rules` target looks like a misspelled debhelper override.
    ///
    /// A target written `override dh_…`, `execute_before dh_…`, or
    /// `execute_after dh_…` (a space where `_` belongs) is reported as that
    /// text joined with `_`. Otherwise a target containing `_dh_`, with no `%`
    /// wildcard, that is not a known `override_` / `execute_before_` /
    /// `execute_after_` command (optional `-arch` or `-indep`) is reported
    /// when another known target is within Levenshtein distance 2. An unknown
    /// `…_dh_cmd` whose `dh_auto_cmd` exists is also reported as
    /// `…_dh_cmd => …_dh_auto_cmd`. Comments are dropped and `\` continuations
    /// are joined. Text after the first `:` is ignored. A missing rules file,
    /// or a `Build-Driver` other than `debian-rules`, is not scanned. Extra is
    /// the found target, `=>`, and the suggestion. Location is `debian/rules`
    /// at the logical line. Applies to the Source tree and Source package.
    TypoInDebhelperOverrideTarget,
    code = "LN0177",
    tag = "typo-in-debhelper-override-target",
    default_selected = true,
    experimental = false,
}

/// A space between the timing word and `dh_…`, which should have been `_`.
static SPACED_TARGET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[^:]*(override|execute_(?:after|before))\s+(dh_[^:]*):").expect("valid regex")
});

/// Everything before the first `:` when that text contains `_dh_`.
static DH_TARGET_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^:]*_dh_[^:]*):").expect("valid regex"));

impl SourceRule for TypoInDebhelperOverrideTarget {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if !scans_debian_rules(ctx.debian_control()) {
            return;
        }
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            if let Some(captures) = SPACED_TARGET.captures(&line) {
                let found = format!("{} {}", &captures[1], &captures[2]);
                let suggestion = format!("{}_{}", &captures[1], &captures[2]);
                ctx.diagnostic(at_rules(number, format!("{found} => {suggestion}")));
                continue;
            }
            let Some(captures) = DH_TARGET_LINE.captures(&line) else {
                continue;
            };
            let targets: Vec<&str> = captures[1]
                .split_whitespace()
                .filter(|target| target.contains("_dh_"))
                .filter(|target| !target.contains('%'))
                .collect();
            for target in &targets {
                if is_known_target(target) {
                    continue;
                }
                if let Some(nearest) = nearest_target(target) {
                    ctx.diagnostic(at_rules(number, format!("{target} => {nearest}")));
                }
            }
            for target in &targets {
                if let Some(message) = missing_auto(target) {
                    ctx.diagnostic(at_rules(number, message));
                }
            }
        }
    }
}

fn at_rules(line: u32, message: String) -> Diagnostic {
    Diagnostic::warn(message).with_location(Location {
        path: PathBuf::from("debian/rules"),
        line: Some(line),
        column: None,
    })
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

    fn rules(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(CONTROL)),
            (
                "debian/rules",
                TestFile::new(format!("#!/usr/bin/make -f\n\n%:\n\tdh $@\n\n{body}")),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            vec![("debian/control", TestFile::new(CONTROL))],
            rules(
                "override_dh_install:\noverride_dh_install :\n execute_before_dh_install:\nexecute_after_dh_install:\n",
            ),
            rules("# override_dh_nump:\n# override dh_install:\n"),
            rules(
                ".PHONY: override_dh_ucfs\nprefix_override_dh_gconfs:\noverride_dh_will_never_exist:\n",
            ),
            rules("override_dh_auto_configure-% override_dh_auto_install-%:\n"),
            rules("override_dh_install: override_dh_ucfs\n"),
            vec![
                (
                    "debian/control",
                    TestFile::new(
                        "Source: example\n\
                         Build-Driver: debputy\n\
                         \n\
                         Package: example\n\
                         Architecture: all\n\
                         Description: example\n extra\n",
                    ),
                ),
                (
                    "debian/rules",
                    TestFile::new(
                        "#!/usr/bin/make -f\noverride_dh_instakk:\noverride dh_install:\n",
                    ),
                ),
            ],
        ];
        let fail = vec![rules(
            "\
override_dh_nump: # one nearest neighbour
override_dh_instakk:
 override_dh_installcrons:
override_dh_install_examples:
override_dh_install_changelogs :
override_dh_perls override_dh_python3 :
override_dh_python3 override_dh_perls :
override_dh_instakk override_dh_install_examples :
foo override_dh_installdebs bar override_dh_installxmlcatalog:
override_dh_testdirs: override_dh_ucfs
override_dh_build:
override dh_install:
execute_after_dh_nope:
execute_before_dh_nope:
oeverride_dh_missing:
execute_before dh_install:
",
        )];
        SourceTreeTester::new(TypoInDebhelperOverrideTarget, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("override_dh_install:\n"))];
        let fail = vec![SourcePackageCase::from(rules("override_dh_instakk:\n"))];
        SourcePackageTester::new(TypoInDebhelperOverrideTarget, pass, fail).test_and_snapshot();
    }
}
