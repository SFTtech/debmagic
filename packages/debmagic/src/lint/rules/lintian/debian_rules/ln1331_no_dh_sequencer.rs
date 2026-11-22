use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct NoDhSequencer;

declare_rule! {
    /// `debian/rules` does not call the `dh` sequencer.
    ///
    /// Reported when no recipe line (starting with a tab, optionally
    /// prefixed by `-`, `+`, or `@`, and optionally with `VAR=value`
    /// assignments) runs `dh` followed by one of the rule targets `$@`,
    /// `${@}`, `$*` or `$(@)`, optionally quoted. Comment lines that start
    /// with a single horizontal whitespace character and `#` are dropped
    /// first, as lintian does. The extra is empty; the location is
    /// `debian/rules` without a line. Applies to the Source tree and Source
    /// package. A missing `debian/rules` is not reported. A `debian/rules`
    /// that cannot be read (a symlink or directory) counts as having no
    /// recipes, so it is reported.
    NoDhSequencer,
    code = "LN1331",
    tag = "no-dh-sequencer",
    default_selected = false,
    experimental = false,
}

/// Single-character leading whitespace then `#`, through the end of the line
/// and its newline (Perl `s/^\h#.*\R?//mg`).
static COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[ \t]#[^\n]*(?:\r\n|\n|\r)?").expect("valid regex"));

/// A recipe line that runs `dh` with a rule target.
static DH_SEQUENCER: LazyLock<Regex> = LazyLock::new(|| {
    let target = r#"(?:\$@|\$\{@\}|\$\*|\$\(@\))"#;
    let target = format!(r#"(?:{target}|'{target}'|"{target}")"#);
    Regex::new(&format!(
        r"(?m)^\t+(?:[+@-])?(?:[^=]+=\S+ )?dh[ \t]+{target}"
    ))
    .expect("valid regex")
});

impl SourceRule for NoDhSequencer {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get("debian/rules") else {
            return;
        };
        let bytes = files.read(entry).unwrap_or_default();
        let stripped = COMMENT.replace_all(&bytes, &b""[..]);
        if DH_SEQUENCER.is_match(&stripped) {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn("")
                .with_severity(Severity::Info)
                .with_location(Location {
                    path: PathBuf::from("debian/rules"),
                    line: None,
                    column: None,
                }),
        );
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
                TestFile::new(format!("#!/usr/bin/make -f\n\n{body}")),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            rules("%:\n\tdh $@\n"),
            rules("%:\n\tdh ${@}\n"),
            rules("%:\n\tdh $*\n"),
            rules("%:\n\tdh $(@)\n"),
            rules("%:\n\tdh \"$@\"\n"),
            rules("%:\n\tdh '$@'\n"),
            rules("%:\n\t-dh $@\n"),
            rules("%:\n\t+dh $@\n"),
            rules("%:\n\t@dh $@\n"),
            rules("%:\n\tDEB_X=1 dh $@\n"),
            rules("%:\n\tdh\t$@ --with python3\n"),
            // A target that starts with a rule target (no end anchor).
            rules("%:\n\tdh $@x\n"),
        ];
        let fail = vec![
            rules("%:\n\tdh_auto_build\n"),
            rules("%:\n\tdh\n"),
            rules("%:\n\tdh foo\n"),
            // A comment with a single leading whitespace is dropped.
            rules("%:\n #\tdh $@\n"),
            // A tab-indented comment is dropped.
            rules("%:\n\t# dh $@\n"),
            // Two-space indent is not a single whitespace comment; the
            // recipe is not a dh call.
            rules("%:\n  # dh $@\n"),
            // Not a recipe line: no leading tab.
            rules("%:\ndh $@\n"),
            // Text before `dh` without a `VAR=value` assignment.
            rules("%:\n\tfoo dh $@\n"),
        ];
        SourceTreeTester::new(NoDhSequencer, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("%:\n\tdh $@\n"))];
        let fail = vec![SourcePackageCase::from(rules("%:\n\tdh_auto_build\n"))];
        SourcePackageTester::new(NoDhSequencer, pass, fail).test_and_snapshot();
    }
}
