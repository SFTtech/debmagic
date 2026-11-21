use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5_scan::{escape_errors, wildcards};

pub struct InvalidEscapeSequenceInDep5Copyright;

declare_rule! {
    /// A pattern of a machine-readable copyright escapes a character other
    /// than `*` or `?`.
    ///
    /// Reads the machine-readable `debian/copyright` and
    /// `debian/<installable>.copyright`. In the header, every illegal escape of
    /// `Files-Excluded` and `Files-Included` is reported with the field name.
    /// In the other paragraphs, every illegal escape of `Files` is reported; as
    /// with the globbing Rules, this is skipped when a comma in `Files` makes
    /// the globbing unreliable. An escape is a backslash and the character after
    /// it. Extra is the escape (prefixed by the field name in the header); the
    /// Location is the field line.
    InvalidEscapeSequenceInDep5Copyright,
    code = "LN0881",
    tag = "invalid-escape-sequence-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidEscapeSequenceInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        if copyrights.is_empty() {
            return;
        }
        let Some(shipped) = ctx.shipped_files() else {
            return;
        };
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            let Some((header, followers)) = paragraphs.split_first() else {
                continue;
            };
            for field in ["Files-Excluded", "Files-Included"] {
                for pattern in header.trimmed_list(field) {
                    for offender in escape_errors(pattern) {
                        ctx.diagnostic(
                            Diagnostic::warn(format!("({field}) {offender}")).with_location(
                                Location {
                                    path: path.clone(),
                                    line: header.position(field),
                                    column: None,
                                },
                            ),
                        );
                    }
                }
            }
            for (index, offender) in wildcards(followers, shipped.file_names()).escapes {
                ctx.diagnostic(Diagnostic::warn(offender).with_location(Location {
                    path: path.clone(),
                    line: followers[index].position("Files"),
                    column: None,
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn copyright(header: &str, files: &str) -> TestFile {
        TestFile::new(format!(
            "{FORMAT}{header}\nFiles: {files}\nCopyright: 2024 Someone\nLicense: MIT\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/copyright", copyright("Files-Excluded: a/*", "b*"))],
            // An escaped star or question mark is legal.
            vec![("debian/copyright", copyright("", r"b\* c\?"))],
        ];
        let fail = vec![
            vec![("debian/copyright", copyright(r"Files-Excluded: a\b", "b"))],
            vec![("debian/copyright", copyright(r"Files-Included: c\d", "b"))],
            vec![("debian/copyright", copyright("", r"invalid-escape\n"))],
        ];
        SourceTreeTester::new(InvalidEscapeSequenceInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
