use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5_scan::{check_wildcards, wildcards};

pub struct DuplicateGlobbingPatterns;

declare_rule! {
    /// The same `Files` pattern is used by more than one paragraph.
    ///
    /// Reads the machine-readable `debian/copyright` and
    /// `debian/<installable>.copyright`. A pattern in two or more paragraphs is
    /// reported once, with the `Files` line of each paragraph. Skipped when a
    /// comma in `Files` makes the globbing unreliable (see the comma Rule).
    /// Extra is the pattern and `(lines N ...)`; the Location is the file.
    DuplicateGlobbingPatterns,
    code = "LN0876",
    tag = "duplicate-globbing-patterns",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DuplicateGlobbingPatterns {
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
            if !check_wildcards(followers, shipped.file_names()) {
                continue;
            }
            for (pattern, indexes) in
                wildcards(followers, shipped.file_names()).sections_by_wildcard
            {
                if indexes.len() < 2 {
                    continue;
                }
                let lines: Vec<String> = indexes
                    .iter()
                    .filter_map(|&index| followers[index].position("Files"))
                    .map(|line| line.to_string())
                    .collect();
                ctx.diagnostic(
                    Diagnostic::error(format!("{pattern} (lines {})", lines.join(" ")))
                        .with_location(Location {
                            path: path.clone(),
                            line: None,
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

    fn copyright(files: &[&str]) -> TestFile {
        let sections: String = files
            .iter()
            .map(|pattern| format!("\nFiles: {pattern}\nCopyright: 2024 Someone\nLicense: MIT\n"))
            .collect();
        TestFile::new(format!("{FORMAT}{sections}"))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/copyright", copyright(&["a", "b", "c/*"]))],
            // A field with several patterns is not a duplicate of itself.
            vec![("debian/copyright", copyright(&["a b", "c"]))],
            // A comma makes the globbing unreliable, unless a shipped name has one.
            vec![("debian/copyright", copyright(&["a, b", "a, b"]))],
        ];
        let fail = vec![
            vec![("debian/copyright", copyright(&["debian/*", "debian/*"]))],
            vec![(
                "debian/copyright",
                copyright(&["debian/*", "x", "debian/*", "debian/*"]),
            )],
        ];
        SourceTreeTester::new(DuplicateGlobbingPatterns, pass, fail).test_and_snapshot();
    }
}
