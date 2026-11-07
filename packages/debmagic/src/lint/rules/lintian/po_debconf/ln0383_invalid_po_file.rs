use std::process::{Command, Stdio};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

use super::files;

pub struct InvalidPoFile;

declare_rule! {
    /// A gettext file in `debian/po` cannot be compiled.
    ///
    /// A `.po` file with no `msgstr` line is invalid. Otherwise `msgfmt -o
    /// /dev/null --statistics` decides, the way the Check does. Directories
    /// are not read. Extra is empty. The Location is the file.
    InvalidPoFile,
    code = "LN0383",
    tag = "invalid-po-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidPoFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(tree) = ctx.files() else {
            return;
        };
        let Some(directory) = files::po_directory(tree) else {
            return;
        };
        let children: Vec<_> = files::po_children(tree, directory)
            .into_iter()
            .map(files::path_buf)
            .collect();
        let mut hits = Vec::new();
        for path in children {
            let Some(entry) = tree.get(&path) else {
                continue;
            };
            if entry.kind() == EntryKind::Directory {
                continue;
            }
            let Some(bytes) = files::read(tree, &path) else {
                continue;
            };
            if !has_msgstr(&bytes) || msgfmt_fails(&bytes) {
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

fn has_msgstr(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes)
        .lines()
        .any(|line| line.starts_with("msgstr"))
}

fn msgfmt_fails(bytes: &[u8]) -> bool {
    let Ok(mut file) = tempfile::NamedTempFile::new() else {
        return false;
    };
    if std::io::Write::write_all(&mut file, bytes).is_err() {
        return false;
    }
    let Ok(status) = Command::new("msgfmt")
        .args(["-o", "/dev/null", "--statistics"])
        .arg(file.path())
        .env("LC_ALL", "C")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    else {
        return false;
    };
    !status.success()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const VALID_PO: &str = "\
msgid \"\"\n\
msgstr \"\"\n\
\"Content-Type: text/plain; charset=UTF-8\\n\"\n\
\"Content-Transfer-Encoding: 8bit\\n\"\n\
\n\
msgid \"Hello\"\n\
msgstr \"Hallo\"\n";

    fn tree(paths: &[(&str, &str)]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            (
                "debian/control".to_string(),
                TestFile::from(
                    "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                ),
            ),
            ("debian/po".to_string(), TestFile::directory()),
        ];
        for (path, content) in paths {
            files.push(((*path).to_string(), TestFile::new(*content)));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[("debian/po/de.po", VALID_PO)]),
            tree(&[("debian/po/extra", "")]),
        ];
        let fail = vec![
            tree(&[("debian/po/de.po", "this is not a po file\n")]),
            tree(&[("debian/po/fr.po", "msgid \"foo\nmsgstr \"bar\"\n")]),
        ];
        SourceTreeTester::new(InvalidPoFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(&[(
            "debian/po/de.po",
            VALID_PO,
        )]))];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "debian/po/de.po",
            "not a po\n",
        )]))];
        SourcePackageTester::new(InvalidPoFile, pass, fail).test_and_snapshot();
    }
}
