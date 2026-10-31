use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::rules_text::{debhelper_lines, read_rules};

pub struct DebhelperToolsFromAutotoolsDevAreDeprecated;

declare_rule! {
    /// `debian/rules` still calls the autotools-dev debhelper tools.
    ///
    /// After `debian/rules` has shown a dh command, scans the file the way
    /// lintian's `debhelper` Check does: `\` continuations are joined first
    /// (the newline stays), then comment lines are dropped. A continued line
    /// points at its first physical line, and the lines consumed by the join
    /// do not advance the counter. An indented `dh_autotools-dev_updateconfig`
    /// or `dh_autotools-dev_restoreconfig` reports that command. An indented
    /// `dh` sequencer whose `--with` list names `autotools-dev` (or
    /// `autotools_dev`) reports `dh ... --with` plus the addon as written.
    /// A `Build-Driver` other than `debian-rules` is not scanned. Applies to
    /// the Source tree and Source package.
    DebhelperToolsFromAutotoolsDevAreDeprecated,
    code = "LN0217",
    tag = "debhelper-tools-from-autotools-dev-are-deprecated",
    default_selected = true,
    experimental = false,
}

static DH_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+[@+-]?(?:\S+=\S+\s+)*(dh_\S+)").expect("valid regex"));
static DH_SEQUENCER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\s+[@+-]?(?:\S+=\S+\s+)*)dh\s+").expect("valid regex"));
static WITH_ADDON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\s--with(?:=|\s+)(?:"([^"]*)"|'([^']*)'|(\S+))"#).expect("valid regex")
});

const UPDATE: &str = "dh_autotools-dev_updateconfig";
const RESTORE: &str = "dh_autotools-dev_restoreconfig";

impl SourceRule for DebhelperToolsFromAutotoolsDevAreDeprecated {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() {
            return;
        }
        let Some(contents) = ctx.files().and_then(read_rules) else {
            return;
        };
        for (number, line) in debhelper_lines(&contents) {
            for extra in extras(&line) {
                ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                    path: PathBuf::from("debian/rules"),
                    line: Some(number),
                    column: None,
                }));
            }
        }
    }
}

fn extras(line: &str) -> Vec<String> {
    if let Some(found) = DH_COMMAND.captures(line) {
        let command = found[1].to_string();
        if command == UPDATE || command == RESTORE {
            return vec![command];
        }
        return Vec::new();
    }
    if !DH_SEQUENCER.is_match(line) {
        return Vec::new();
    }
    let mut extras = Vec::new();
    for found in WITH_ADDON.captures_iter(line) {
        let Some(written) = found
            .get(1)
            .or_else(|| found.get(2))
            .or_else(|| found.get(3))
        else {
            continue;
        };
        for addon in written.as_str().split(',') {
            if addon.replace('-', "_") == "autotools_dev" {
                extras.push(format!("dh ... --with {addon}"));
            }
        }
    }
    extras
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

    fn package(control: &str, rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control)),
            ("debian/rules", TestFile::new(rules)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package(CONTROL, "#!/usr/bin/make -f\nbuild:\n\ttrue\n"),
            package(CONTROL, "\t# dh_autotools-dev_updateconfig\n"),
            package(CONTROL, "# dh $@ --with autotools-dev\n"),
            // The command has to be indented.
            package(CONTROL, "dh_autotools-dev_updateconfig\n"),
            package(CONTROL, "\tdh $@ --with python3\n"),
            package(CONTROL, "\tdh $@ --with quilt\n"),
            package(
                "\
Source: example\n\
Build-Driver: debputy\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
                "\tdh_autotools-dev_updateconfig\n",
            ),
            // A comment continuation swallows the next physical line.
            package(CONTROL, "# kept \\\n\tdh_autotools-dev_updateconfig\n"),
        ];
        let fail = vec![
            package(CONTROL, "\tdh_autotools-dev_updateconfig\n"),
            package(CONTROL, "\tdh_autotools-dev_restoreconfig --foo\n"),
            package(CONTROL, "\t@dh_autotools-dev_updateconfig\n"),
            package(CONTROL, "\t-dh_autotools-dev_restoreconfig\n"),
            package(CONTROL, "\tFOO=bar dh_autotools-dev_updateconfig\n"),
            package(CONTROL, "\tdh $@ --with autotools-dev\n"),
            package(CONTROL, "\tdh $@ --with=autotools-dev\n"),
            package(CONTROL, "\tdh $@ --with autotools-dev,python3\n"),
            package(CONTROL, "\tdh $@ --with \"autotools-dev\"\n"),
            package(CONTROL, "\tdh $@ --with autotools_dev\n"),
            // First physical line, and the next hint lags by the consumed line.
            package(
                CONTROL,
                "\tdh $@ \\\n\t--with autotools-dev\n\tdh_autotools-dev_updateconfig\n",
            ),
            package(
                CONTROL,
                "\t# dh_autotools-dev_updateconfig\n\tdh_autotools-dev_restoreconfig\n",
            ),
        ];
        SourceTreeTester::new(DebhelperToolsFromAutotoolsDevAreDeprecated, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            CONTROL,
            "\tdh $@ --with python3\n",
        ))];
        let fail = vec![SourcePackageCase::from(package(
            CONTROL,
            "\tdh $@ --with autotools-dev\n",
        ))];
        SourcePackageTester::new(DebhelperToolsFromAutotoolsDevAreDeprecated, pass, fail)
            .test_and_snapshot();
    }
}
