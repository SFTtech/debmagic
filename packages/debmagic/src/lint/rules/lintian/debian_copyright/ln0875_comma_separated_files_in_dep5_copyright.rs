use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct CommaSeparatedFilesInDep5Copyright;

declare_rule! {
    /// A DEP-5 `Files` field separates its patterns with commas.
    ///
    /// Reads the machine-readable `debian/copyright` and
    /// `debian/<installable>.copyright`. Each paragraph after the header whose
    /// `Files` value contains a comma is reported, unless some shipped file
    /// name contains a comma (then the comma may be legal). Extra is empty; the
    /// Location is the `Files` line.
    CommaSeparatedFilesInDep5Copyright,
    code = "LN0875",
    tag = "comma-separated-files-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for CommaSeparatedFilesInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        if copyrights.is_empty() {
            return;
        }
        let Some(shipped) = ctx.shipped_files() else {
            return;
        };
        if shipped.file_names().iter().any(|name| name.contains(',')) {
            return;
        }
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            for paragraph in paragraphs.iter().skip(1) {
                if paragraph.value("Files").contains(',') {
                    ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                        path: path.clone(),
                        line: paragraph.position("Files"),
                        column: None,
                    }));
                }
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

    fn copyright(files: &str) -> TestFile {
        TestFile::new(format!(
            "{FORMAT}\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n\nFiles: {files}\nCopyright: 2024 Someone\nLicense: MIT\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/copyright", copyright("a b"))],
            // A shipped file name with a comma makes the comma possibly legal.
            vec![
                ("debian/copyright", copyright("a, b")),
                ("a,b", TestFile::new("x")),
            ],
        ];
        let fail = vec![
            vec![("debian/copyright", copyright("a, b"))],
            vec![("debian/copyright", copyright("a,b c"))],
        ];
        SourceTreeTester::new(CommaSeparatedFilesInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
