use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree};

pub struct DebianRulesCallsDebhelperInOddOrder;

declare_rule! {
    /// A `debian/rules` recipe calls debhelper programs out of order.
    ///
    /// `dh_makeshlibs` is group 1, `dh_shlibdeps`, `dh_installdeb`, and
    /// `dh_gencontrol` are group 2, and `dh_builddeb` is group 3. A call whose
    /// group is lower than the last call for the same package is reported.
    /// The package is the first `-p` or `--package=` argument, or empty.
    /// Unknown `dh_*` commands are ignored. A new target starts the order
    /// over; `.PHONY` does not, and neither does `ifeq`. Extra is the command
    /// name. Location is `debian/rules` at the physical line where the logical
    /// line ends. Applies to the Source tree and Source package.
    DebianRulesCallsDebhelperInOddOrder,
    code = "LN0180",
    tag = "debian-rules-calls-debhelper-in-odd-order",
    default_selected = true,
    experimental = false,
}

static PHONY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[^:]+\s)?\.PHONY(?:\s[^:]+)?:(.+)").expect("valid regex"));
static CONDITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ifn?(?:eq|def)\s").expect("valid regex"));
/// A target rule. `FOO := value` matches too, and lintian treats that as a
/// new target, which clears the remembered debhelper group.
static TARGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^\s:][^:]*):+").expect("valid regex"));
static RECIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+[^#]").expect("valid regex"));
static DH_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+(dh_\S+)\b").expect("valid regex"));
static PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s(?:-p|--package=)(\S+)").expect("valid regex"));

impl SourceRule for DebianRulesCallsDebhelperInOddOrder {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, command) in odd_debhelper_calls(&contents) {
            ctx.diagnostic(Diagnostic::warn(command).with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: Some(number),
                column: None,
            }));
        }
    }
}

fn debhelper_group(command: &str) -> Option<u8> {
    Some(match command {
        "dh_makeshlibs" => 1,
        "dh_shlibdeps" | "dh_installdeb" | "dh_gencontrol" => 2,
        "dh_builddeb" => 3,
        _ => return None,
    })
}

fn odd_debhelper_calls(contents: &str) -> Vec<(u32, String)> {
    let mut found = Vec::new();
    let mut group: HashMap<String, u8> = HashMap::new();
    for (number, line) in complete_lines(contents) {
        if PHONY.is_match(&line) {
            continue;
        }
        if !CONDITIONAL.is_match(&line) && TARGET.is_match(&line) {
            group.clear();
            continue;
        }
        if !RECIPE.is_match(&line) {
            continue;
        }
        let Some(captures) = DH_COMMAND.captures(&line) else {
            continue;
        };
        let command = captures[1].to_string();
        let Some(level) = debhelper_group(&command) else {
            continue;
        };
        let package = PACKAGE
            .captures(&line)
            .map(|captures| captures[1].to_string())
            .unwrap_or_default();
        let current = group.get(&package).copied().unwrap_or(0);
        if level < current {
            found.push((number, command));
        } else {
            group.insert(package, level);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    /// Vendored `rules-dh-order` rules. Hints are lines 11, 12, 13, 14, and 18.
    const DH_ORDER: &str = "\
#!/usr/bin/make -f\n\
%:\n\
\tdh $@\n\
\n\
# Test for calling debhelper commands out of order.  We take advantage here\n\
# of the fact checks/rules doesn't care what target the commands are called\n\
# from by using several uncalled targets.\n\
\n\
reverse:\n\
\tdh_builddeb\n\
\tdh_gencontrol\n\
\tdh_installdeb\n\
\tdh_shlibdeps\n\
\tdh_makeshlibs\n\
\n\
shared-order:\n\
\tdh_shlibdeps\n\
\tdh_makeshlibs\n\
\tdh_installdeb\n\
\tdh_gencontrol\n\
\tdh_builddeb\n\
\n\
multiple-package:\n\
\tdh_makeshlibs -pfoo\n\
\tdh_shlibdeps -pfoo\n\
\tdh_installdeb -pfoo\n\
\tdh_gencontrol -pfoo\n\
\tdh_builddeb -pfoo\n\
\n\
\tdh_makeshlibs -pbar\n\
\tdh_gencontrol -pbar\n\
\tdh_installdeb -pbar\n\
\tdh_shlibdeps -pbar\n\
\tdh_builddeb -pbar\n\
";

    fn rules(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(CONTROL)),
            (
                "debian/rules",
                TestFile::new(format!("#!/usr/bin/make -f\n\n{body}")),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            vec![("debian/rules", TestFile::new(""))],
            rules(""),
            rules(
                "t:\n\tdh_makeshlibs\n\tdh_shlibdeps\n\tdh_installdeb\n\tdh_gencontrol\n\tdh_builddeb\n",
            ),
            // Group 2 commands may follow each other in any order.
            rules("t:\n\tdh_gencontrol\n\tdh_installdeb\n\tdh_shlibdeps\n\tdh_builddeb\n"),
            // A new target starts the order over.
            rules("one:\n\tdh_builddeb\n\ntwo:\n\tdh_makeshlibs\n"),
            rules(
                "override_dh_builddeb:\n\tdh_builddeb\noverride_dh_gencontrol:\n\tdh_gencontrol\n",
            ),
            // Each `-p` / `--package=` value has its own order.
            rules(
                "multi:\n\tdh_builddeb -pfoo\n\tdh_makeshlibs -pbar\n\tdh_shlibdeps --package=bar\n",
            ),
            rules("multi:\n\tdh_makeshlibs -pfoo --package=bar\n\tdh_builddeb --package=bar\n"),
            // Unknown commands, and names that only share a prefix, are ignored.
            rules("t:\n\tdh_clean\n\tdh_makeshlibs\n"),
            rules("t:\n\tdh_builddebx\n\tdh_makeshlibs\n"),
            // Not a recipe line.
            rules("dh_builddeb\n\tdh_makeshlibs\n"),
            // `:=` matches the target pattern, so the group is cleared.
            rules("t:\n\tdh_builddeb\nFOO := 1\n\tdh_makeshlibs\n"),
            // A comment is not a call.
            rules("t:\n#\tdh_builddeb\n\tdh_makeshlibs\n"),
            rules("t:\n\t# dh_builddeb\n\tdh_gencontrol\n"),
        ];
        let fail = vec![
            rules("reverse:\n\tdh_builddeb\n\tdh_gencontrol\n"),
            rules("t:\n\tdh_shlibdeps\n\tdh_makeshlibs\n"),
            rules("t:\n\tdh_builddeb\n\tdh_gencontrol\n\tdh_shlibdeps\n\tdh_makeshlibs\n"),
            // The group stays at the higher value after an out-of-order call.
            rules("t:\n\tdh_builddeb\n\tdh_clean\n\tdh_gencontrol\n"),
            rules("t:\n\tdh_builddeb -pfoo\n\tdh_makeshlibs --package=foo\n"),
            // `-p` must be glued to the name; otherwise both calls share a package.
            rules("t:\n\tdh_builddeb -p foo\n\tdh_makeshlibs\n"),
            // `.PHONY` does not start a new target.
            rules("t:\n\tdh_builddeb\n.PHONY: t\n\tdh_makeshlibs\n"),
            // `ifeq` is not a target, even when its text contains a colon.
            rules("t:\n\tdh_builddeb\nifeq ($(FOO),a:b)\n\tdh_makeshlibs\nendif\n"),
            // `=` is not a target; `define` does not clear the group.
            rules("t:\n\tdh_builddeb\nFOO = 1\n\tdh_makeshlibs\n"),
            rules("t:\n\tdh_builddeb\ndefine FOO\n\tdh_makeshlibs\nendef\n"),
            // Continuations join into the command. The pointer is the last line.
            rules("t:\n\tdh_builddeb\n\tdh_gen\\\ncontrol\n"),
            rules("# dh_builddeb\n\tdh_builddeb \\\n\t  -- -Zxz\n\tdh_gencontrol\n"),
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/rules", TestFile::new(DH_ORDER)),
            ],
        ];
        SourceTreeTester::new(DebianRulesCallsDebhelperInOddOrder, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "t:\n\tdh_makeshlibs\n\tdh_builddeb\n",
        ))];
        let fail = vec![SourcePackageCase::from(rules(
            "t:\n\tdh_builddeb\n\tdh_makeshlibs\n",
        ))];
        SourcePackageTester::new(DebianRulesCallsDebhelperInOddOrder, pass, fail)
            .test_and_snapshot();
    }
}
