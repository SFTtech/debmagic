use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_copyright::DebianCopyright;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::{copyright_texts, wildcards};

pub struct RedundantGlobbingPatterns;

declare_rule! {
    /// Two or more `Files` patterns of one paragraph cover the same shipped file.
    ///
    /// Reads the machine-readable `debian/copyright` and
    /// `debian/<installable>.copyright`. For each shipped file that more than one
    /// pattern of the same paragraph matches (repeats count), the patterns are
    /// reported sorted, with the file: `(<patterns>) for <file>`. Patterns with
    /// an illegal escape are skipped, and the check is skipped when a comma in
    /// `Files` makes the globbing unreliable. The Location is the paragraph's
    /// `Files` line. Pedantic, not selected by default.
    RedundantGlobbingPatterns,
    code = "LN1648",
    tag = "redundant-globbing-patterns",
    default_selected = false,
    experimental = false,
}

impl SourceRule for RedundantGlobbingPatterns {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(shipped) = ctx.shipped_files() else {
            return;
        };
        for (path, text) in copyright_texts(ctx) {
            let Some(all) = DebianCopyright::parse(&text)
                .paragraphs()
                .map(<[_]>::to_vec)
            else {
                continue;
            };
            let Some(followers) = all.get(1..) else {
                continue;
            };
            for (index, file, patterns) in wildcards(followers, shipped.file_names()).redundant {
                ctx.diagnostic(
                    Diagnostic::warn(format!("({}) for {file}", patterns.join(" ")))
                        .with_severity(Severity::Pedantic)
                        .with_location(Location {
                            path: path.clone(),
                            line: followers[index].position("Files"),
                            column: None,
                        }),
                );
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

    fn copyright(paragraphs: &[&str]) -> TestFile {
        let body: String = paragraphs
            .iter()
            .map(|files| format!("\nFiles: {files}\nCopyright: 2024 Someone\nLicense: MIT\n"))
            .collect();
        TestFile::new(format!("{FORMAT}{body}"))
    }

    #[test]
    fn test() {
        let pass = vec![
            // Different paragraphs may cover the same file; that is not redundant.
            vec![
                ("src/a.c", TestFile::new("x")),
                ("debian/copyright", copyright(&["*", "src/*", "src/a.c"])),
            ],
            vec![
                ("src/a.c", TestFile::new("x")),
                ("debian/copyright", copyright(&["*", "src/a.c"])),
            ],
            // Not machine-readable.
            vec![
                ("src/a.c", TestFile::new("x")),
                ("debian/copyright", TestFile::new("Copyright 2024\n")),
            ],
        ];
        let fail = vec![
            vec![
                ("src/a.c", TestFile::new("x")),
                (
                    "debian/copyright",
                    copyright(&["*", "src/* src/a.c", "src/a.c"]),
                ),
            ],
            vec![
                ("src/a.c", TestFile::new("x")),
                ("debian/copyright", copyright(&["*", "src/a.c src/*"])),
            ],
        ];
        SourceTreeTester::new(RedundantGlobbingPatterns, pass, fail).test_and_snapshot();
    }
}
