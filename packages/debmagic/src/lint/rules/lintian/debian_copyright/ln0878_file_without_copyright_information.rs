use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5_scan::{check_wildcards, wildcards};

pub struct FileWithoutCopyrightInformation;

declare_rule! {
    /// A shipped file is not covered by any `Files` pattern.
    ///
    /// Reads the machine-readable `debian/copyright` and
    /// `debian/<installable>.copyright`. Shipped files are checked once the
    /// globbing is reliable (see the comma Rule). Quilt patch directories
    /// (`.pc/`) and license notices (`COPYING*` and `LICENSE`, at any depth) do
    /// not need a `Files` pattern. Extra is the file name; the Location is the
    /// copyright file.
    FileWithoutCopyrightInformation,
    code = "LN0878",
    tag = "file-without-copyright-information",
    default_selected = true,
    experimental = false,
}

impl SourceRule for FileWithoutCopyrightInformation {
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
            let covered = wildcards(followers, shipped.file_names()).wildcard_by_file;
            for name in shipped.file_names() {
                if name.starts_with(".pc/") || NOTICE.is_match(name) || covered.contains_key(name) {
                    continue;
                }
                ctx.diagnostic(Diagnostic::warn(name.clone()).with_location(Location {
                    path: path.clone(),
                    line: None,
                    column: None,
                }));
            }
        }
    }
}

static NOTICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|/)(COPYING[^/]*|LICENSE)$").expect("valid regex"));

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
            vec![
                ("debian/copyright", copyright(&["*"])),
                ("a", TestFile::new("x")),
                ("b", TestFile::new("x")),
            ],
            // Notices and quilt patches need no pattern.
            vec![
                ("debian/copyright", copyright(&["a", "debian/copyright"])),
                ("a", TestFile::new("x")),
                ("COPYING", TestFile::new("x")),
                ("sub/LICENSE", TestFile::new("x")),
                (".pc/patch.diff", TestFile::new("x")),
            ],
        ];
        let fail = vec![vec![
            ("debian/copyright", copyright(&["a"])),
            ("a", TestFile::new("x")),
            ("i-have-no-copyright-information", TestFile::new("x")),
        ]];
        SourceTreeTester::new(FileWithoutCopyrightInformation, pass, fail).test_and_snapshot();
    }
}
