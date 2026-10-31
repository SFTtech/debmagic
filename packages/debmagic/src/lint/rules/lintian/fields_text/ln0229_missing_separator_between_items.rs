use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct MissingSeparatorBetweenItems;

declare_rule! {
    /// Two relationship items are separated by space instead of a comma or
    /// pipe.
    ///
    /// Reads the raw field text of `Build-Depends`, `Build-Depends-Indep`,
    /// `Build-Conflicts`, and `Build-Conflicts-Indep` on the source paragraph,
    /// and `Pre-Depends`, `Depends`, `Recommends`, `Suggests`, `Breaks`,
    /// `Conflicts`, `Provides`, `Replaces`, and `Enhances` on each installable.
    /// Continuation lines are joined and `[arch]` lists removed before the
    /// match, as in lintian's `check_separators`. One pair per field, the
    /// first. Extra is `(in source paragraph)` or `(in section for pkg)` and
    /// the two items in single quotes. `Build-Depends-Arch` is not checked.
    MissingSeparatorBetweenItems,
    code = "LN0229",
    tag = "missing-separator-between-items",
    default_selected = true,
    experimental = false,
}

const SOURCE_FIELDS: &[&str] = &[
    "Build-Depends",
    "Build-Depends-Indep",
    "Build-Conflicts",
    "Build-Conflicts-Indep",
];

const INSTALLABLE_FIELDS: &[&str] = &[
    "Pre-Depends",
    "Depends",
    "Recommends",
    "Suggests",
    "Breaks",
    "Conflicts",
    "Provides",
    "Replaces",
    "Enhances",
];

static NEWLINE_INDENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n(\s)").expect("valid regex"));
static BRACKETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]*\]").expect("valid regex"));
static PAIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:^|\s)((?:\w[^\s,|$()]+|\$\{\S+:Depends\})\s*(?:\([^)]*\)\s*)?)\s+((?:\w[^\s,|$()]+|\$\{\S+:Depends\})\s*(?:\([^)]*\)\s*)?)",
    )
    .expect("valid regex")
});

impl SourceRule for MissingSeparatorBetweenItems {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let mut found =
            separator_messages(control.source(), SOURCE_FIELDS, "(in source paragraph)");
        for installable in control.installables() {
            found.extend(separator_messages(
                installable.fields(),
                INSTALLABLE_FIELDS,
                &format!("(in section for {})", installable.name()),
            ));
        }
        for message in found {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn separator_messages(fields: &ControlParagraph, names: &[&str], explainer: &str) -> Vec<String> {
    names
        .iter()
        .filter_map(|name| {
            let value = fields.untrimmed_value(name)?;
            let (prev, next) = missing_pair(value.trim())?;
            Some(format!("{explainer} '{prev}' and '{next}'"))
        })
        .collect()
}

/// First pair lintian's separator regex finds, or nothing.
fn missing_pair(value: &str) -> Option<(String, String)> {
    let folded = NEWLINE_INDENT.replace_all(value, "$1");
    let stripped = BRACKETS.replace_all(&folded, "");
    let captures = PAIR.captures(&stripped)?;
    let prev = captures.get(1)?.as_str().trim_end().to_string();
    let next = captures.get(2)?.as_str().trim_end().to_string();
    Some((prev, next))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(text: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(text))]
    }

    /// Drop the shared indent of a raw control fixture so continuation lines
    /// keep their relative leading space. A trailing `\` in a regular string
    /// would eat that space.
    fn dedent(text: &str) -> String {
        let text = text.strip_prefix('\n').unwrap_or(text);
        let indent = text
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.len() - line.trim_start_matches(' ').len())
            .min()
            .unwrap_or(0);
        let mut out = String::new();
        for line in text.lines() {
            if line.len() >= indent && line.as_bytes()[..indent].iter().all(|byte| *byte == b' ') {
                out.push_str(&line[indent..]);
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        out
    }

    fn packaged(source_relations: &str, binary_relations: &str) -> Vec<(&'static str, TestFile)> {
        control(&format!(
            "Source: example\nMaintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n{source_relations}\
             \n\
             Package: example\nArchitecture: all\n{binary_relations}\
             Description: example\n extra\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            packaged("", ""),
            packaged("Build-Depends: foo, bar\n", ""),
            packaged("Build-Depends: foo | bar\n", ""),
            packaged("Build-Depends: foo,\n bar\n", ""),
            packaged("Build-Depends: foo [amd64 i386]\n", ""),
            packaged("Build-Depends-Arch: foo bar\n", ""),
            packaged("", "Depends: a b\n"),
            packaged("", "Depends: ab c\n"),
            control("not a deb822 file\n: \n"),
            control(&dedent(
                r"
                Source: control-file-missing-separator
                Maintainer: Debian Lintian Maintainers <lintian-maint@debian.org>
                Build-Depends: debhelper-compat (= 14),
                 fiddle [amd64 powerpc
                 mips mipsel
                 hppa
                 s390],
                 faddle
                 (>>
                 2) [
                 sparc i386 amd64
                 ]
                Standards-Version: 4.7.2

                Package: control-file-missing-separator
                Architecture: all
                Depends: ${shlibs:Depends}, ${misc:Depends}
                Description: example
                 extra
                ",
            )),
        ];
        let fail = vec![
            packaged("Build-Depends: foo bar\n", ""),
            packaged("", "Depends: p1 p2 p3\nSuggests: alpha beta\n"),
            packaged("", "Depends: foo (>= 1) baz (<< 2)\n"),
            packaged("", "Depends: ${shlibs:Depends} ${some:Depends}\n"),
            packaged("", "Depends: foo [amd64] bar\n"),
            packaged("", "Depends: foo\n bar\n"),
            packaged("", "Depends: foo | bar baz\n"),
            control(&dedent(
                r"
                Source: control-file-missing-separator
                Section: devel
                Priority: optional
                Maintainer: Debian Lintian Maintainers <lintian-maint@debian.org>
                Build-Depends: debhelper-compat (= 14),
                 fiddle [amd64 powerpc
                 mips mipsel
                 hppa
                 s390],
                 faddle
                 (>>
                 2) [
                 sparc i386 amd64
                 ]
                Standards-Version: 4.7.2

                Package: control-file-missing-separator
                Architecture: all
                Depends: ${shlibs:Depends}, ${misc:Depends}, foo (>= 1)
                  baz (<< 2),
                  fizz (= 2.0)
                Suggests: p1
                 p2
                 p3
                 p4
                 p5
                Description: Test for missing separators in control files
                 This is a test.
                ",
            )),
            control(
                "Source: control-file-general\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 \n\
                 Package: control-file-general\n\
                 Architecture: all\n\
                 Depends: control-file-general, foo, baz, ${shlibs:Depends} ${some:Depends}, ${misc:Depends}\n\
                 Description: example\n extra\n",
            ),
        ];
        SourceTreeTester::new(MissingSeparatorBetweenItems, pass, fail).test_and_snapshot();
    }
}
