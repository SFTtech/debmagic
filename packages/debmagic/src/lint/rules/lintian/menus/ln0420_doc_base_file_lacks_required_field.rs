use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct DocBaseFileLacksRequiredField;

declare_rule! {
    /// A doc-base section is missing a required field.
    ///
    /// Direct children of `usr/share/doc-base` are parsed when they are UTF-8.
    /// The first section requires `Document`, `Title`, and `Section`. Later
    /// sections require `Format` and `Files`. Missing names are sorted. The
    /// pointer is the blank line after the section, or the last field line at
    /// end of file. Extra is the field. Udebs are checked too.
    DocBaseFileLacksRequiredField,
    code = "LN0420",
    tag = "doc-base-file-lacks-required-field",
    default_selected = true,
    experimental = false,
}

const MAIN_REQUIRED: &[&str] = &["Document", "Section", "Title"];
const FORMAT_REQUIRED: &[&str] = &["Files", "Format"];

impl BinaryPackageRule for DocBaseFileLacksRequiredField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for (path, text) in doc_base_texts(files) {
            for (line, field) in missing_fields(&text) {
                hits.push((path.clone(), line, field));
            }
        }
        for (path, line, field) in hits {
            ctx.diagnostic(Diagnostic::error(field).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

fn doc_base_texts(files: &FileTree) -> Vec<(PathBuf, String)> {
    let mut entries: Vec<_> = files
        .entries()
        .iter()
        .filter(|entry| {
            matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                && is_doc_base_child(entry.path())
        })
        .collect();
    entries.sort_by(|left, right| left.path().cmp(right.path()));
    entries
        .into_iter()
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            let text = String::from_utf8(bytes).ok()?;
            Some((entry.path().to_path_buf(), text))
        })
        .collect()
}

fn is_doc_base_child(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    let Some(name) = text.strip_prefix("usr/share/doc-base/") else {
        return false;
    };
    !name.is_empty() && !name.contains('/')
}

struct OpenField {
    last_line: u32,
}

fn missing_fields(text: &str) -> Vec<(u32, String)> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    let mut main = true;
    let mut field: Option<OpenField> = None;
    let mut saw = Vec::new();
    let mut missing = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let position = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if let Some((name, _)) = field_start(line) {
            saw.push(name.to_string());
            field = Some(OpenField {
                last_line: position,
            });
        } else if field.is_some() && is_continuation(line) {
            if let Some(open) = field.as_mut() {
                open.last_line = position;
            }
        } else if is_separator(line) {
            let Some(open) = field.take() else {
                continue;
            };
            missing.extend(lacking(&saw, main, open.last_line + 1));
            saw.clear();
            main = false;
        }
    }
    if let Some(open) = field {
        missing.extend(lacking(&saw, main, open.last_line));
    }
    missing
}

fn field_start(line: &str) -> Option<(&str, &str)> {
    let (name, value) = line.split_once(':')?;
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return None;
    }
    Some((name, value))
}

fn is_continuation(line: &str) -> bool {
    let mut chars = line.chars();
    matches!(chars.next(), Some(' ' | '\t')) && chars.any(|ch| !ch.is_whitespace())
}

fn is_separator(line: &str) -> bool {
    line.trim().is_empty()
}

fn lacking(saw: &[String], main: bool, line: u32) -> Vec<(u32, String)> {
    let required = if main { MAIN_REQUIRED } else { FORMAT_REQUIRED };
    required
        .iter()
        .filter(|field| !saw.iter().any(|seen| seen == *field))
        .map(|field| (line, (*field).to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn doc(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/doc-base/example".to_string(),
            TestFile::new(body),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            doc(
                "Document: example\nTitle: Title\nSection: Text\n\nFormat: text\nFiles: /usr/share/doc/example/file\n",
            ),
            doc(""),
            BinaryPackageCase::from(vec![(
                "usr/share/doc-base/example".to_string(),
                TestFile::from_bytes(b"\xff".to_vec()),
            )]),
        ];
        let fail = vec![doc(
            "Document: example\nTitle: Title\nAuthor: A\n\nFormat: ESP\nIndex: /usr/share/doc/example/file\n\nIndex: /usr/share/doc/example/html/index.html\n",
        )];
        BinaryPackageTester::new(DocBaseFileLacksRequiredField, pass, fail).test_and_snapshot();
    }
}
