use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_copyright::DebianCopyright;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct SyntaxErrorInDep5Copyright;

declare_rule! {
    /// A machine-readable `debian/copyright` (or `debian/<installable>.copyright`)
    /// does not parse as Deb822.
    ///
    /// Only copyright-format 1.0 and post-overhaul drafts are read, as lintian
    /// does: an older draft is never parsed, so it is never reported here. The
    /// extra is the parser's message in lintian's words, for example
    /// `Continuation line not in paragraph (line 13). Missing a dot on the
    /// previous line?`. The Location is the copyright file, without a line.
    /// debmagic does not read PGP armour, so a signed copyright file is reported
    /// with the parser's own message (lintian reads through the armour).
    SyntaxErrorInDep5Copyright,
    code = "LN1021",
    tag = "syntax-error-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SyntaxErrorInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, copyright) in ctx.debian_copyrights() {
            let DebianCopyright::MachineReadable {
                paragraphs: Err(message),
            } = copyright
            else {
                continue;
            };
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn copyright(text: &str) -> TestFile {
        TestFile::new(format!("{FORMAT}{text}"))
    }

    #[test]
    fn test() {
        let pass = vec![
            // Well-formed.
            vec![(
                "debian/copyright",
                copyright("\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"),
            )],
            // An outdated draft is not parsed, so its syntax is not checked.
            vec![(
                "debian/copyright",
                TestFile::new(
                    "Format: http://svn.debian.org/wsvn/dep/web/deps/dep5.mdwn?op=file&rev=147\n\n continuation\n",
                ),
            )],
            // Not machine-readable at all.
            vec![(
                "debian/copyright",
                TestFile::new("Copyright 2024 Someone\n continuation\n"),
            )],
        ];
        let fail = vec![
            // A continuation line before any field.
            vec![("debian/copyright", copyright("\n continuation\n"))],
            // A duplicate field is a syntax error too.
            vec![(
                "debian/copyright",
                copyright("\nFiles: *\nFiles: a\nCopyright: 2024 Someone\nLicense: MIT\n"),
            )],
        ];
        SourceTreeTester::new(SyntaxErrorInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
