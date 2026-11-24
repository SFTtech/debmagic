use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5_scan::wildcards;

pub struct GlobbingPatternsOutOfOrder;

declare_rule! {
    /// A later `Files` pattern covers a file that a deeper earlier pattern
    /// already covered.
    ///
    /// Reads the machine-readable `debian/copyright` and
    /// `debian/<installable>.copyright`. Paragraphs are checked in order; when
    /// a shipped file was covered by an earlier pattern with more `/` than the
    /// pattern of the paragraph that covers it now, that is reported. Skipped
    /// when a comma in `Files` makes the globbing unreliable (see the comma
    /// Rule). Extra is the earlier pattern, the later one and the file; the
    /// Location is the `Files` line of the later paragraph.
    GlobbingPatternsOutOfOrder,
    code = "LN0880",
    tag = "globbing-patterns-out-of-order",
    default_selected = true,
    experimental = false,
}

impl SourceRule for GlobbingPatternsOutOfOrder {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        if copyrights.is_empty() {
            return;
        }
        let Some(shipped) = ctx.shipped_files() else {
            return;
        };
        for (path, copyright) in copyrights {
            let Some(followers) = copyright.paragraphs().and_then(|all| all.get(1..)) else {
                continue;
            };
            for (index, loosing, winning, name) in
                wildcards(followers, shipped.file_names()).out_of_order
            {
                ctx.diagnostic(
                    Diagnostic::warn(format!("{loosing} {winning} {name}")).with_location(
                        Location {
                            path: path.clone(),
                            line: followers[index].position("Files"),
                            column: None,
                        },
                    ),
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

    fn copyright(files: &[&str]) -> TestFile {
        let sections: String = files
            .iter()
            .map(|pattern| format!("\nFiles: {pattern}\nCopyright: 2024 Someone\nLicense: MIT\n"))
            .collect();
        TestFile::new(format!("{FORMAT}{sections}"))
    }

    #[test]
    fn test() {
        let patch = || ("debian/patches/a.patch", TestFile::new("x"));
        let pass = vec![
            // The deeper pattern comes later, so it wins the file.
            vec![
                (
                    "debian/copyright",
                    copyright(&["debian/*", "debian/patches/*"]),
                ),
                patch(),
            ],
            // Patterns that cover different files do not overlap.
            vec![
                (
                    "debian/copyright",
                    copyright(&["debian/patches/*", "src/*"]),
                ),
                patch(),
            ],
        ];
        let fail = vec![vec![
            (
                "debian/copyright",
                copyright(&["debian/patches/*", "debian/*"]),
            ),
            patch(),
        ]];
        SourceTreeTester::new(GlobbingPatternsOutOfOrder, pass, fail).test_and_snapshot();
    }
}
