use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::SourceRule;

use super::files;

static CHARSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"charset=((?-u:.)*?)\\n").expect("valid regex"));

pub struct UnknownEncodingInPoFile;

declare_rule! {
    /// A translation in `debian/po` does not declare its character set.
    ///
    /// Only `.po` files that lintian can open are read: a regular file, a
    /// hardlink, or a symlink that resolves to one in the tree. A file with no
    /// `msgstr` line is left to `invalid-po-file` and skipped. The file's
    /// `charset=` header is read up to the next literal `\n`; the Rule reports
    /// when that value is empty or the template placeholder `CHARSET`. A file
    /// with no `charset=` header is not reported. Extra is empty. The Location
    /// is the file.
    UnknownEncodingInPoFile,
    code = "LN1031",
    tag = "unknown-encoding-in-po-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UnknownEncodingInPoFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(tree) = ctx.files() else {
            return;
        };
        let Some(directory) = files::po_directory(tree) else {
            return;
        };
        let paths: Vec<_> = files::po_children(tree, directory)
            .into_iter()
            .filter(|entry| files::basename(entry).is_some_and(|name| name.ends_with(".po")))
            .map(files::path_buf)
            .collect();
        let mut hits = Vec::new();
        for path in paths {
            if unknown_encoding(tree, &path) {
                hits.push(path);
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// lintian's test on one `.po` file: readable, has a `msgstr` line, and a
/// `charset=` value that is empty or `CHARSET`.
fn unknown_encoding(tree: &FileTree, path: &std::path::Path) -> bool {
    let Some(entry) = files::open_file(tree, path) else {
        return false;
    };
    let Ok(bytes) = tree.read(entry) else {
        return false;
    };
    if !has_msgstr(&bytes) {
        return false;
    }
    let Some(captures) = CHARSET.captures(&bytes) else {
        return false;
    };
    let charset = &captures[1];
    charset.is_empty() || charset == b"CHARSET"
}

fn has_msgstr(bytes: &[u8]) -> bool {
    bytes
        .split(|&byte| byte == b'\n')
        .any(|line| line.starts_with(b"msgstr"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const UTF8_PO: &str = "\
msgid \"\"\n\
msgstr \"\"\n\
\"Content-Type: text/plain; charset=UTF-8\\n\"\n\
\n\
msgid \"Hello\"\n\
msgstr \"Hallo\"\n";

    fn tree(paths: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            (
                "debian/control".to_string(),
                TestFile::from(
                    "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                ),
            ),
            ("debian/po".to_string(), TestFile::directory()),
        ];
        for (path, file) in paths {
            files.push(((*path).to_string(), file.clone()));
        }
        files
    }

    fn po(content: &str) -> TestFile {
        TestFile::new(content)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[("debian/po/de.po", po(UTF8_PO))]),
            tree(&[("debian/po/de.po", po("msgid \"a\"\nmsgstr \"b\"\n"))]),
            tree(&[(
                "debian/po/de.po",
                po("msgid \"a\"\nmsgstr \"b\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n"),
            )]),
            tree(&[("debian/po/extra", po("charset=CHARSET\\n"))]),
            tree(&[("debian/po/de.po", po("charset=CHARSET\\n"))]),
        ];
        let fail = vec![
            tree(&[(
                "debian/po/fr.po",
                po("msgstr \"\"\n\"Content-Type: text/plain; charset=CHARSET\\n\"\n"),
            )]),
            tree(&[(
                "debian/po/fr.po",
                po("msgstr \"\"\n\"Content-Type: text/plain; charset=\\n\"\n"),
            )]),
            tree(&[(
                "debian/po/fr.po",
                po("msgstr \"\"\n\"Content-Type: charset=CHARSET\\n\"\n"),
            )]),
        ];
        SourceTreeTester::new(UnknownEncodingInPoFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(&[(
            "debian/po/de.po",
            po(UTF8_PO),
        )]))];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "debian/po/fr.po",
            po("msgstr \"\"\n\"Content-Type: text/plain; charset=CHARSET\\n\"\n"),
        )]))];
        SourcePackageTester::new(UnknownEncodingInPoFile, pass, fail).test_and_snapshot();
    }
}
