use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::data::lines;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::{Severity, SourceRule};

pub struct VeryLongLineLengthInSourceFile;

declare_rule! {
    /// A text file in the source has a line longer than 512 characters.
    ///
    /// Walks the Patched files of a Source tree or Source package, outside
    /// `.pc/`, that are regular files and read as text (no NUL, no stray
    /// control bytes, ASCII or UTF-8). Skipped: names ending in a
    /// binary or compressed extension (`files/binary-file-extensions`,
    /// `files/compressed-file-extensions`, or `xml`, `sgml`, `svg`, `json`,
    /// `jsonl`), `LICENSES/**.txt`, and, for names containing `sql`, lines
    /// starting `INSERT ` or `SELECT `. The length of a line is its character
    /// count with the newline, or its byte count when it is not UTF-8. The
    /// first longest line is the Location; Extra is `<length> > 512`.
    /// Severity is pedantic and the Tag is experimental and not selected by
    /// default, as in lintian. "Text" is a sniff, not `file(1)`: it knows the
    /// RTF magic and nothing else that libmagic names by magic.
    VeryLongLineLengthInSourceFile,
    code = "LN1381",
    tag = "very-long-line-length-in-source-file",
    default_selected = false,
    experimental = true,
}

/// A line longer than this many characters is reported.
const VERY_LONG: usize = 512;

/// The `files/binary-file-extensions` and `files/compressed-file-extensions`
/// alternatives are case-insensitive; the rest of the Check's list is not.
static EXTENSION: LazyLock<Regex> = LazyLock::new(|| {
    let known = lines(include_str!("../../../data/files/binary-file-extensions"))
        .chain(lines(include_str!(
            "../../../data/files/compressed-file-extensions"
        )))
        .map(regex::escape)
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"[.](?:(?i:{known})|xml|sgml|svg|jsonl?)\s*$")).expect("valid regex")
});

static SQL_STATEMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:INSERT|SELECT)\s").expect("valid regex"));

impl SourceRule for VeryLongLineLengthInSourceFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let hits: Vec<(PathBuf, u32, usize)> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .into_iter()
            .filter_map(|entry| longest_line_of(files, entry))
            .map(|(path, line, length)| (path, line as u32, length))
            .collect();
        for (path, line, length) in hits {
            ctx.diagnostic(
                Diagnostic::warn(format!("{length} > {VERY_LONG}"))
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

/// The path, line number and length of the longest line, if it is too long.
fn longest_line_of(files: &FileTree, entry: &Entry) -> Option<(PathBuf, usize, usize)> {
    let path = entry.path();
    if path.starts_with(".pc") {
        return None;
    }
    let name = path.to_str()?;
    let basename = path.file_name()?.to_str()?;
    if EXTENSION.is_match(basename) || (name.starts_with("LICENSES/") && name.ends_with(".txt")) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    if !is_text(&bytes) {
        return None;
    }
    let is_sql = basename.to_lowercase().contains("sql");
    let mut longest: Option<(usize, usize)> = None;
    for (index, line) in bytes.split_inclusive(|&byte| byte == b'\n').enumerate() {
        if is_sql && SQL_STATEMENT.is_match(&String::from_utf8_lossy(line)) {
            continue;
        }
        let length = line_length(line);
        if longest.is_none_or(|(_, best)| length > best) {
            longest = Some((index + 1, length));
        }
    }
    let (line, length) = longest?;
    (length > VERY_LONG).then(|| (path.to_path_buf(), line, length))
}

/// Characters of a line: code points when UTF-8, bytes otherwise.
fn line_length(line: &[u8]) -> usize {
    match std::str::from_utf8(line) {
        Ok(text) => text.chars().count(),
        Err(_) => line.len(),
    }
}

/// `file(1)` reads it as ASCII or Unicode text: no NUL, no control bytes
/// other than the usual whitespace and escape, and valid UTF-8. A Rich Text
/// Format file is printable ASCII too, but libmagic names it by its `{\rtf`
/// magic, so it is not text.
fn is_text(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && !bytes.starts_with(b"{\\rtf")
        && bytes
            .iter()
            .all(|&byte| matches!(byte, 0x08 | 0x09..=0x0d | 0x1b | 0x20..=0x7e) || byte >= 0x80)
        && std::str::from_utf8(bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn file(path: &'static str, content: String) -> (&'static str, TestFile) {
        (path, TestFile::new(content))
    }

    fn with_control(files: Vec<(&'static str, TestFile)>) -> Vec<(&'static str, TestFile)> {
        let mut all = vec![(
            "debian/control",
            TestFile::new(
                "Source: example\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n\nPackage: example\nArchitecture: all\n\
                 Description: example\n extra\n",
            ),
        )];
        all.extend(files);
        all
    }

    #[test]
    fn source() {
        let long = "x".repeat(600);
        let pass = vec![
            with_control(vec![file("src/main.c", "int main() {}\n".to_string())]),
            with_control(vec![file("src/data.bin.gz", long.clone())]),
            with_control(vec![file("LICENSES/MIT.txt", long.clone())]),
            with_control(vec![file("db/schema_sql.txt", format!("INSERT {long}\n"))]),
            with_control(vec![file("src/exact.c", "x".repeat(512))]),
            with_control(vec![file("doc/notes.txt", format!("{{\\rtf1 {long}\n"))]),
            with_control(vec![file("src/binary.c", format!("\0{long}\n"))]),
        ];
        let fail = vec![
            with_control(vec![file("src/long.c", format!("short\n{long}\n"))]),
            with_control(vec![file("src/long.js", format!("{long}\nshort\n"))]),
            with_control(vec![file("db/schema.sql", format!("{long}\n"))]),
        ];
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test_and_snapshot();
    }
}
