use std::collections::HashSet;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5_scan::matched;

pub struct FileIncludedAlready;

declare_rule! {
    /// A `Files-Included` pattern names a shipped file that is not excluded.
    ///
    /// Reads the machine-readable `debian/copyright` and
    /// `debian/<installable>.copyright`. The header's `Files-Included` patterns
    /// are matched against the shipped files (with the directory rule of
    /// `Files-Excluded`); each matched file that no `Files-Excluded` pattern
    /// also matches is reported once. Extra is the file name; the Location is
    /// the `Files-Included` line.
    FileIncludedAlready,
    code = "LN0877",
    tag = "file-included-already",
    default_selected = true,
    experimental = false,
}

impl SourceRule for FileIncludedAlready {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        if copyrights.is_empty() {
            return;
        }
        let Some(shipped) = ctx.shipped_files() else {
            return;
        };
        for (path, copyright) in copyrights {
            let Some(header) = copyright.paragraphs().and_then(<[_]>::first) else {
                continue;
            };
            let excluded: HashSet<String> = matched(header, "Files-Excluded", &shipped)
                .into_iter()
                .collect();
            let mut seen = HashSet::new();
            for name in matched(header, "Files-Included", &shipped) {
                if excluded.contains(&name) || !seen.insert(name.clone()) {
                    continue;
                }
                ctx.diagnostic(Diagnostic::error(name).with_location(Location {
                    path: path.clone(),
                    line: header.position("Files-Included"),
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

    fn copyright(header: &str) -> TestFile {
        TestFile::new(format!(
            "{FORMAT}{header}\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            // The file is excluded again, so it is not included already.
            vec![
                (
                    "debian/copyright",
                    copyright("Files-Excluded: include-this\nFiles-Included: include-this"),
                ),
                ("include-this", TestFile::new("x")),
            ],
            // Nothing shipped matches the pattern.
            vec![(
                "debian/copyright",
                copyright("Files-Included: include-this"),
            )],
        ];
        let fail = vec![
            vec![
                (
                    "debian/copyright",
                    copyright("Files-Included: include-this"),
                ),
                ("include-this", TestFile::new("x")),
            ],
            // A directory pattern covers its shipped contents, once each.
            vec![
                ("debian/copyright", copyright("Files-Included: dir dir/*")),
                ("dir/a", TestFile::new("x")),
                ("dir/b", TestFile::new("x")),
            ],
        ];
        SourceTreeTester::new(FileIncludedAlready, pass, fail).test_and_snapshot();
    }
}
