use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

use super::content::pages;

pub struct WrongManualSection;

declare_rule! {
    /// The `.TH` header's section differs from the section in the file name.
    ///
    /// Every installed manual page in a `manN/` directory whose first line
    /// is not a short `.so` or empty page is scanned. Comment lines (`.\"`)
    /// are skipped. For each `.TH` line, the section is the first quoted
    /// argument after the name (quotes stripped, backslash escapes removed),
    /// or the first word when there is no quoted argument. The name section
    /// is the component before a trailing `gz`. The Tag fires when they differ
    /// without regard to case, and the section is not empty. Extra is
    /// `<name section> != <header section>`. The Location is the page and the
    /// line of the `.TH`. Applies to deb, udeb, and ddeb.
    WrongManualSection,
    code = "LN0896",
    tag = "wrong-manual-section",
    default_selected = true,
    experimental = false,
}

static LEADING_TOKEN_WITH_COMMA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\S+,").expect("valid regex"));
static LEADING_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\S+").expect("valid regex"));
static FIRST_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\S+)").expect("valid regex"));

impl BinaryPackageRule for WrongManualSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            header_mismatches(files)
        };
        for (path, line, extra) in hits {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

/// `(path, line, extra)` for every `.TH` section that differs from the name.
fn header_mismatches(files: &FileTree) -> Vec<(std::path::PathBuf, u32, String)> {
    let mut found = Vec::new();
    for page in pages(files) {
        if page.opening.stops() {
            continue;
        }
        let name_section = name_section(&page.path).unwrap_or_default();
        for (index, line) in page.lines.iter().enumerate() {
            if line.starts_with(r#".\""#) {
                continue;
            }
            let Some(header) = th_section(line) else {
                continue;
            };
            if header.is_empty() || header.to_lowercase() == name_section.to_lowercase() {
                continue;
            }
            found.push((
                page.path.clone(),
                u32::try_from(index + 1).unwrap_or(u32::MAX),
                format!("{name_section} != {header}"),
            ));
        }
    }
    found
}

/// Perl: split the file name on `.`, drop a trailing `gz`, then the last
/// remaining component. `None` when nothing is left.
fn name_section(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let mut pieces = dot_pieces(name);
    let extension = pieces.pop()?;
    if extension != "gz" {
        pieces.push(extension);
    }
    pieces.pop().map(str::to_string)
}

/// The section a `.TH` line names, or `None` when the line is not a `.TH`
/// header. Mirrors lintian's `.TH` parsing with `extract_delimited`.
fn th_section(line: &str) -> Option<String> {
    let after_macro = line.strip_prefix(".TH")?;
    if !after_macro.starts_with(char::is_whitespace) {
        return None;
    }
    let consumed = after_macro.trim_start();

    let (delimited, mut after_names) = extract_delimited(consumed);
    if delimited.is_empty() {
        let mut rest = consumed;
        if let Some(found) = LEADING_TOKEN_WITH_COMMA.find(rest) {
            rest = &rest[found.end()..];
        }
        if let Some(found) = LEADING_TOKEN.find(rest) {
            rest = &rest[found.end()..];
        }
        after_names = rest.to_string();
    }

    let (quoted, _) = extract_delimited(&after_names);
    if !quoted.is_empty() {
        let mut characters = quoted.chars();
        characters.next();
        characters.next_back();
        return Some(unescape(characters.as_str()));
    }
    FIRST_WORD
        .captures(&after_names)
        .map(|captures| captures[1].to_string())
}

/// Text::Balanced `extract_delimited` with its default delimiters (`'`, `"`,
/// and backtick), a leading-whitespace prefix, and backslash escapes. Returns
/// the delimited text with its delimiters and the remainder. On failure the
/// first is empty and the second is the whole input.
fn extract_delimited(text: &str) -> (String, String) {
    let trimmed = text.trim_start();
    let mut characters = trimmed.char_indices();
    let Some((_, open)) = characters.next() else {
        return (String::new(), text.to_string());
    };
    if !matches!(open, '\'' | '"' | '`') {
        return (String::new(), text.to_string());
    }
    let mut escaped = false;
    for (index, character) in characters {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == open {
            let end = index + character.len_utf8();
            return (trimmed[..end].to_string(), trimmed[end..].to_string());
        }
    }
    (String::new(), text.to_string())
}

/// Perl `s/ [\\](.) /$1/gmsx`: each backslash and the character after it
/// become that character.
fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if let Some(next) = characters.next() {
                out.push(next);
            }
        } else {
            out.push(character);
        }
    }
    out
}

/// Perl `split(/\./, $name)`: leading empties stay, trailing empties go.
fn dot_pieces(name: &str) -> Vec<&str> {
    let mut pieces: Vec<&str> = name.split('.').collect();
    while pieces.last().is_some_and(|piece| piece.is_empty()) {
        pieces.pop();
    }
    pieces
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn page(path: &str, body: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new(body))
    }

    #[test]
    fn header_sections() {
        assert_eq!(th_section(".TH FOO 1").as_deref(), Some("1"));
        assert_eq!(th_section(".TH FOO \"7\" \"date\"").as_deref(), Some("7"));
        assert_eq!(th_section(".TH \"FOO\" \"1p\"").as_deref(), Some("1p"));
        assert_eq!(th_section(".TH FOO,BAR,1 x").as_deref(), Some("x"));
        assert_eq!(th_section(".TH FOO 1p").as_deref(), Some("1p"));
        assert_eq!(th_section(".TH FOO \"a\\\"b\"").as_deref(), Some("a\"b"));
        assert_eq!(th_section(".THX FOO 1"), None);
        assert_eq!(th_section(".TH"), None);
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![page(
                "usr/share/man/man1/foo.1.gz",
                ".TH FOO 1\n.SH NAME\nfoo \\- bar\n",
            )]),
            BinaryPackageCase::from(vec![page(
                "usr/share/man/man1/foo.1.gz",
                ".TH FOO \"1\" \"date\"\n",
            )]),
            BinaryPackageCase::from(vec![page("usr/share/man/man1/foo.1p.gz", ".TH FOO 1P\n")]),
            // Comments are skipped.
            BinaryPackageCase::from(vec![page(
                "usr/share/man/man1/foo.1.gz",
                ".\\\" .TH FOO 9\n.TH FOO 1\n",
            )]),
            // Not in a manN directory: never scanned.
            BinaryPackageCase::from(vec![page("usr/share/man/foo.9.gz", ".TH FOO 1\n")]),
            // A short `.so` page is not scanned.
            BinaryPackageCase::from(vec![page(
                "usr/share/man/man3/foo.3.gz",
                ".so man1/bar.1\n.TH FOO 7\n",
            )]),
            // No section in the header: nothing to compare.
            BinaryPackageCase::from(vec![page("usr/share/man/man1/foo.1.gz", ".TH FOO\n")]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![page(
                "usr/share/man/man3/foo.3.gz",
                ".TH FOO \"7\" \"date\"\n",
            )]),
            BinaryPackageCase::from(vec![page(
                "usr/share/man/man8/foo.8.gz",
                ".TH FOO 7 2020\n",
            )]),
            BinaryPackageCase::from(vec![page("usr/share/man/man1/foo.1", ".TH FOO 3\n")]),
        ];
        BinaryPackageTester::new(WrongManualSection, pass, fail).test_and_snapshot();
    }
}
