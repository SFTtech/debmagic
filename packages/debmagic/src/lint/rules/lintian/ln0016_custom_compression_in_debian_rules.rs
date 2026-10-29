use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::debian_rules::{complete_lines, read_rules_from_tree};

pub struct CustomCompressionInDebianRules;

declare_rule! {
    /// `debian/rules` calls `dh_builddeb` with its own compression settings.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for `dh_builddeb`, later `--`, and later `-z`,
    /// `-Z`, or `-S`. Extra is the text from `dh_builddeb` to the end of the
    /// line. Location is `debian/rules` at the physical line where the logical
    /// line ends. Applies to the Source tree and Source package. A missing,
    /// unreadable, or non-UTF-8 `debian/rules` is not scanned.
    CustomCompressionInDebianRules,
    code = "LN0016",
    tag = "custom-compression-in-debian-rules",
    default_selected = true,
    experimental = false,
}

static CUSTOM_COMPRESSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(dh_builddeb\b.*--.*-[zZS].*)$").expect("valid regex"));

impl SourceRule for CustomCompressionInDebianRules {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            let Some(found) = CUSTOM_COMPRESSION.captures(&line) else {
                continue;
            };
            ctx.diagnostic(
                Diagnostic::warn(found[1].to_string()).with_location(Location {
                    path: PathBuf::from("debian/rules"),
                    line: Some(number),
                    column: None,
                }),
            );
        }
    }
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
            vec![("debian/rules", TestFile::new(""))],
            rules(""),
            rules("override_dh_builddeb:\n\tdh_builddeb\n"),
            // No `--`: the options are for dh_builddeb itself.
            rules("override_dh_builddeb:\n\tdh_builddeb -Zxz\n"),
            // Other options after `--`.
            rules("override_dh_builddeb:\n\tdh_builddeb -- --root-owner-group\n"),
            // Another command takes the options.
            rules("override_dh_gencontrol:\n\tdh_gencontrol -- -Zxz\n"),
            // A comment is not a call.
            rules("# dh_builddeb -- -Zxz\n"),
            rules("  # dh_builddeb -- -Zxz\n"),
            // `dh_builddeb` is a word of its own; the `--` must come after it.
            rules("override_dh_builddeb:\n\t-Zxz -- dh_builddeb\n"),
            rules("override_dh_builddeb:\n\tdh_builddebx -- -Zxz\n"),
        ];
        let fail = vec![
            rules("override_dh_builddeb:\n\tdh_builddeb -- -Zgzip\n"),
            rules("override_dh_builddeb:\n\tdh_builddeb --  -Zxz  -z 9\n"),
            rules("override_dh_builddeb:\n\tdh_builddeb -- -z9 -Z xz\n"),
            rules("override_dh_builddeb:\n\tdh_builddeb -- -Zxz -Sextreme -z9\n"),
            rules("override_dh_builddeb:\n\tdh_builddeb -- -Sextreme\n"),
            // Any `-z`, `-Z`, or `-S` after `--` counts, even inside another word.
            rules("override_dh_builddeb:\n\tdh_builddeb -- --foo-zebra\n"),
            // A shell wrapper still calls it.
            rules("override_dh_builddeb:\n\t$(shell echo dh_builddeb -- -Zxz)\n"),
            // Continuations join into one line.
            rules("override_dh_builddeb:\n\tdh_builddeb -- \\\n\t  -Zxz\n"),
            // One report per call.
            rules("override_dh_builddeb:\n\tdh_builddeb -- -Zxz\n\tdh_builddeb -- -Zgzip\n"),
        ];
        SourceTreeTester::new(CustomCompressionInDebianRules, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "override_dh_builddeb:\n\tdh_builddeb\n",
        ))];
        let fail = vec![SourcePackageCase::from(rules(
            "override_dh_builddeb:\n\tdh_builddeb -- -Zxz\n",
        ))];
        SourcePackageTester::new(CustomCompressionInDebianRules, pass, fail).test_and_snapshot();
    }
}
