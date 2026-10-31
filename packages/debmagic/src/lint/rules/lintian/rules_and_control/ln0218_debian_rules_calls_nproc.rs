use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::rules_text::{read_rules, rules_lines};

pub struct DebianRulesCallsNproc;

declare_rule! {
    /// `debian/rules` asks `nproc` for a parallel job count.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for `` `nproc` ``, `$(shell nproc)`, or
    /// `$$(nproc)`, each with an optional `/bin/` or `/usr/bin/` prefix.
    /// `$(shell nproc)` needs a space after `shell`. Extra is empty. Location
    /// is `debian/rules` at the physical line where the logical line ends.
    /// One report per logical line. Applies to the Source tree and Source
    /// package. A missing, unreadable, or non-UTF-8 `debian/rules` is not
    /// scanned.
    DebianRulesCallsNproc,
    code = "LN0218",
    tag = "debian-rules-calls-nproc",
    default_selected = true,
    experimental = false,
}

static NPROC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\$\$\(\s*(?:(?:/usr)?/bin/)?nproc\s*\)|\$\(shell \s*(?:(?:/usr)?/bin/)?nproc\s*\)|`\s*(?:(?:/usr)?/bin/)?nproc\s*`",
    )
    .expect("valid regex")
});

impl SourceRule for DebianRulesCallsNproc {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules) else {
            return;
        };
        for (number, line) in rules_lines(&contents) {
            if !NPROC.is_match(&line) {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: Some(number),
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

    fn package(body: &str) -> Vec<(&'static str, TestFile)> {
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
            package(""),
            package("echo Just saying the word nproc\n"),
            package("# `nproc`\n"),
            package("\t# $(shell nproc)\n"),
            // `shell` must be followed by a space, and the call is a word of its own.
            package("$(shellnproc)\n"),
            package("$(shell\tnproc)\n"),
            package("$(SHELL nproc)\n"),
            package("`nprocs`\n"),
        ];
        let fail = vec![
            package("`/usr/bin/nproc`\n"),
            package("`nproc`\n"),
            package("` /bin/nproc `\n"),
            package("$(shell nproc)\n"),
            package("$(shell  /usr/bin/nproc)\n"),
            package("$$(nproc)\n"),
            package("$$( /bin/nproc )\n"),
            // One report even when the line calls it twice.
            package("$(shell nproc) && $$(nproc)\n"),
            // Continuations join; a comment in between is not itself a hit.
            package("`/usr/bin/\\\n# not a call\nnproc`\n"),
            package("$(shell \\\n\tnproc)\n"),
        ];
        SourceTreeTester::new(DebianRulesCallsNproc, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package("echo nproc\n"))];
        let fail = vec![SourcePackageCase::from(package("$$(nproc)\n"))];
        SourcePackageTester::new(DebianRulesCallsNproc, pass, fail).test_and_snapshot();
    }
}
