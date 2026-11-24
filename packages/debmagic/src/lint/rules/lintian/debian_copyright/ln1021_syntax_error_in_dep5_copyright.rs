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

    /// Lintian's Deb822 reader (`new empty field?` branch) stores an empty
    /// field without the duplicate check, so repeating an empty field is not
    /// a syntax error, and its continuation lines still form the value.
    #[test]
    fn repeated_empty_field_is_not_a_duplicate() {
        let pass = vec![vec![(
            "debian/copyright",
            copyright(
                "
Files: *
Copyright:
Copyright:
 2024 Someone
License: MIT
",
            ),
        )]];
        SourceTreeTester::new(SyntaxErrorInDep5Copyright, pass, vec![]).test();
    }

    /// Lintian's `exists` check on the valued field branch dies on a repeat
    /// of a field that already has a value.
    #[test]
    fn repeat_of_valued_field_is_a_duplicate() {
        let fail = vec![vec![(
            "debian/copyright",
            copyright(
                "
Files: *
Copyright: 2023 Someone
Copyright: 2024 Someone
License: MIT
",
            ),
        )]];
        SourceTreeTester::new(SyntaxErrorInDep5Copyright, vec![], fail).test();
    }

    /// A valued repeat after an empty field is still a duplicate: the empty
    /// field is in the paragraph, so `exists` is true.
    #[test]
    fn valued_repeat_of_empty_field_is_a_duplicate() {
        let fail = vec![vec![(
            "debian/copyright",
            copyright(
                "
Files: *
Copyright:
Copyright: 2024 Someone
License: MIT
",
            ),
        )]];
        SourceTreeTester::new(SyntaxErrorInDep5Copyright, vec![], fail).test();
    }

    /// An empty repeat after a valued field overwrites it without a
    /// duplicate error (lintian's empty-field branch).
    #[test]
    fn empty_repeat_of_valued_field_is_not_a_duplicate() {
        let pass = vec![vec![(
            "debian/copyright",
            copyright(
                "
Files: *
Copyright: 2024 Someone
Copyright:
License: MIT
",
            ),
        )]];
        SourceTreeTester::new(SyntaxErrorInDep5Copyright, pass, vec![]).test();
    }
}
