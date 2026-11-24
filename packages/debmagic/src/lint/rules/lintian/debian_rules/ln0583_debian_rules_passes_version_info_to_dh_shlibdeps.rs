use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree, rules_location};

pub struct DebianRulesPassesVersionInfoToDhShlibdeps;

declare_rule! {
    /// `debian/rules` passes version information to `dh_shlibdeps`.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for `dh_shlibdeps` followed later on the line by
    /// `--version-info` or `-V` as a whole option word end. Extra is empty.
    /// Location is `debian/rules` at the physical line where the logical line
    /// ends. One report per logical line. Applies to the Source tree and
    /// Source package. A missing, unreadable, or non-UTF-8 `debian/rules` is
    /// not scanned.
    DebianRulesPassesVersionInfoToDhShlibdeps,
    code = "LN0583",
    tag = "debian-rules-passes-version-info-to-dh_shlibdeps",
    default_selected = true,
    experimental = false,
}

static VERSION_INFO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"dh_shlibdeps\b.*(?:--version-info|-V)\b").expect("valid regex"));

impl SourceRule for DebianRulesPassesVersionInfoToDhShlibdeps {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        for (number, line) in complete_lines(&contents) {
            if !VERSION_INFO.is_match(&line) {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(rules_location(number)));
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
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps\n"),
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps -L libfoo\n"),
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps -- -v\n"),
            // The option must end at a word boundary.
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps -Vfoo\n"),
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps --version-information\n"),
            // The option comes after the command.
            rules("override_dh_gencontrol:\n\tdh_gencontrol -V\n"),
            rules("\techo -V; dh_shlibdeps\n"),
            // A comment is not a call.
            rules("# dh_shlibdeps -V\n"),
            rules("\t# dh_shlibdeps --version-info\n"),
        ];
        let fail = vec![
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps -V\n"),
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps --version-info\n"),
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps -Xfoo -V 'libc6 (>= 2.5)'\n"),
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps -- -V\n"),
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps -V -- foo\n"),
            // A wrapper still calls it.
            rules("\t$(shell echo dh_shlibdeps --version-info)\n"),
            // The command may be a longer path.
            rules("\t/usr/bin/dh_shlibdeps -V\n"),
            // Continuations join; the report points at the last physical line.
            rules("override_dh_shlibdeps:\n\tdh_shlibdeps \\\n\t  -V\n"),
            // One report per logical line.
            rules("\tdh_shlibdeps -V\n\tdh_shlibdeps --version-info\n"),
            // A comment does not hide a later call.
            rules("# dh_shlibdeps -V\n\tdh_shlibdeps -V\n"),
        ];
        SourceTreeTester::new(DebianRulesPassesVersionInfoToDhShlibdeps, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("\tdh_shlibdeps\n"))];
        let fail = vec![SourcePackageCase::from(rules("\tdh_shlibdeps -V\n"))];
        SourcePackageTester::new(DebianRulesPassesVersionInfoToDhShlibdeps, pass, fail)
            .test_and_snapshot();
    }
}
