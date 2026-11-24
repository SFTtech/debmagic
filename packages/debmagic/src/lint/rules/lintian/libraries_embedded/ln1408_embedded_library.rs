use std::sync::LazyLock;

use fancy_regex::{Regex, RegexBuilder};
use regex::RegexSet;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::data;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct EmbeddedLibrary;

declare_rule! {
    /// An ELF file's `strings` contain the message of a library lintian knows
    /// to be embedded, so the library was linked statically in.
    ///
    /// The table is lintian's `binaries/embedded-libs`: each line gives the
    /// library name (`libname`, default the label), the source packages that
    /// own it (`source` or `source-regex`), and a Perl pattern matched against
    /// the printable strings of the file, one per line. A file whose source
    /// package is the owner is not reported. The extra is the library name and
    /// the Location is the file with no line. Only ELF Data files are checked
    /// (regular files and hardlinks, outside `usr/lib/debug/`). Udebs are
    /// checked too.
    ///
    /// Difference from lintian: the patterns run with `fancy-regex`, which
    /// accepts the same lookaround syntax; the package's source name is the
    /// Binary package's `Source` field, as `ctx.source_name()` gives it.
    EmbeddedLibrary,
    code = "LN1408",
    tag = "embedded-library",
    default_selected = true,
    experimental = false,
}

/// One line of `binaries/embedded-libs`, with its pattern matched through
/// [`Table::set`] or by backtracking.
struct Embedded {
    libname: &'static str,
    source: &'static str,
    source_regex: Option<Regex>,
    pattern: Pattern,
}

/// How a line's pattern decides whether a file contains the library.
enum Pattern {
    /// No lookaround: the verdict is the pattern's index in [`Table::set`].
    Set(usize),
    /// Lookaround: `pattern` runs with backtracking, but only for a file
    /// where `prefilter`, the pattern without its lookaround groups (a
    /// superset of its matches), is in [`Table::set`] and matches.
    Backtrack { pattern: Regex, prefilter: usize },
}

/// The table, compiled once per process.
struct Table {
    libraries: Vec<Embedded>,
    /// Every pattern (or prefilter) of [`Pattern::Set`] and
    /// [`Pattern::Backtrack`], in one automaton, so one pass over a file's
    /// strings tells which patterns can match it.
    set: RegexSet,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let mut set_patterns: Vec<String> = Vec::new();
    let mut libraries = Vec::new();
    for line in data::lines(include_str!("../../../data/binaries/embedded-libs")) {
        let Some(parsed) = parse_line(line) else {
            continue;
        };
        let pattern = if needs_backtracking(parsed.pattern) {
            let Some(pattern) = string_pattern(parsed.pattern) else {
                continue;
            };
            set_patterns.push(without_lookaround(parsed.pattern));
            Pattern::Backtrack {
                pattern,
                prefilter: set_patterns.len() - 1,
            }
        } else {
            set_patterns.push(parsed.pattern.to_owned());
            Pattern::Set(set_patterns.len() - 1)
        };
        libraries.push(Embedded {
            libname: parsed.libname,
            source: parsed.source,
            source_regex: parsed.source_regex,
            pattern,
        });
    }
    let set = RegexSet::new(&set_patterns)
        .expect("the embedded-libs patterns, without lookaround, compile as regex");
    Table { libraries, set }
});

/// A line's fields, before its pattern is compiled.
struct Line {
    libname: &'static str,
    source: &'static str,
    source_regex: Option<Regex>,
    pattern: &'static str,
}

/// `label || key=value ... || pattern`: the label is the text before the first
/// `||`, the pairs and the pattern are split at the next `||`.
fn parse_line(line: &'static str) -> Option<Line> {
    let (label, details) = line.split_once("||")?;
    let label = label.trim();
    let (pairs, pattern) = details.split_once("||")?;
    let mut libname = None;
    let mut source = None;
    let mut source_regex = None;
    for pair in pairs.split_whitespace() {
        let (key, value) = pair.split_once('=')?;
        match key {
            "libname" => libname = Some(value),
            "source" => source = Some(value),
            "source-regex" => source_regex = Some(value),
            _ => return None,
        }
    }
    let source_regex = match source_regex {
        Some(value) => Some(Regex::new(value).ok()?),
        None => None,
    };
    Some(Line {
        libname: libname.unwrap_or(label),
        source: source.unwrap_or(label),
        source_regex,
        pattern,
    })
}

/// Lookaround syntax, which only the backtracking engine supports.
const LOOKAROUND: [&str; 4] = ["(?=", "(?!", "(?<=", "(?<!"];

/// Whether the pattern needs the backtracking engine: lookaround, or a
/// backreference (`\1`..`\9`, `\k`). Other patterns go in the [`RegexSet`].
fn needs_backtracking(pattern: &str) -> bool {
    let bytes = pattern.as_bytes();
    LOOKAROUND.iter().any(|syntax| pattern.contains(syntax))
        || bytes
            .windows(2)
            .any(|pair| pair[0] == b'\\' && (matches!(pair[1], b'1'..=b'9') || pair[1] == b'k'))
}

/// The pattern with every lookaround group removed. Removing a zero-width
/// assertion only makes a pattern match more, so the result matches wherever
/// the pattern does. It is the prefilter of [`Pattern::Backtrack`].
fn without_lookaround(pattern: &str) -> String {
    let bytes = pattern.as_bytes();
    let mut kept = Vec::with_capacity(bytes.len());
    let mut copied_to = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
            continue;
        }
        let starts_lookaround = bytes[index] == b'('
            && LOOKAROUND
                .iter()
                .any(|syntax| bytes[index..].starts_with(syntax.as_bytes()));
        if starts_lookaround {
            kept.extend_from_slice(&bytes[copied_to..index]);
            index = group_end(bytes, index);
            copied_to = index;
        } else {
            index += 1;
        }
    }
    kept.extend_from_slice(&bytes[copied_to..]);
    // Only whole groups that start and end at ASCII parentheses were cut, so
    // what remains is still valid UTF-8.
    String::from_utf8(kept).expect("cutting whole groups keeps UTF-8")
}

/// The index just past the group that opens at `open`.
fn group_end(bytes: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 1,
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return index + 1;
                }
            }
            _ => {}
        }
        index += 1;
    }
    bytes.len()
}

/// The table's pattern over the whole `strings` text of a file, which can be
/// megabytes long. The default backtrack limit of `fancy-regex` is exceeded
/// on such a text, and `is_match` then fails silently, so the library is not
/// reported at all (lintian's Perl regex has no such limit). The limit is
/// lifted here.
fn string_pattern(pattern: &str) -> Option<Regex> {
    RegexBuilder::new(pattern)
        .backtrack_limit(usize::MAX)
        .build()
        .ok()
}

impl BinaryPackageRule for EmbeddedLibrary {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(strings) = ctx.data_strings() else {
            return;
        };
        let source = ctx.source_name();
        let table = &*TABLE;
        // Which libraries apply depends only on the source name, so they are
        // chosen once here, not once per file.
        let candidates: Vec<&Embedded> = table
            .libraries
            .iter()
            .filter(|library| {
                let owned_by_regex = library
                    .source_regex
                    .as_ref()
                    .is_some_and(|regex| regex.is_match(&source).unwrap_or(false));
                let owned_by_name = !library.source.is_empty() && library.source == source;
                !owned_by_regex && !owned_by_name
            })
            .collect();
        if candidates.is_empty() {
            return;
        }
        for (path, text) in strings.iter() {
            let hits = table.set.matches(text);
            for library in &candidates {
                let found = match &library.pattern {
                    Pattern::Set(index) => hits.matched(*index),
                    Pattern::Backtrack { pattern, prefilter } => {
                        hits.matched(*prefilter) && pattern.is_match(text).unwrap_or(false)
                    }
                };
                if found {
                    ctx.diagnostic(Diagnostic::error(library.libname).with_location(Location {
                        path: path.to_path_buf(),
                        line: None,
                        column: None,
                    }));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf_with(text: &str) -> TestFile {
        let bytes: Vec<u8> = TestElf::executable()
            .build()
            .into_iter()
            .chain(text.bytes())
            .collect();
        TestFile::from_bytes(bytes)
    }

    fn case(path: &str, text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, elf_with(text))])
    }

    #[test]
    fn table_lines_parse() {
        let lines = data::lines(include_str!("../../../data/binaries/embedded-libs")).count();
        assert_eq!(TABLE.libraries.len(), lines);
        assert!(
            TABLE
                .libraries
                .iter()
                .any(|library| library.libname == "zlib")
        );
    }

    #[test]
    fn lookaround_is_removed_for_the_prefilter() {
        assert_eq!(
            without_lookaround("(?m)(?<!4 )(?:in|de)flate (?:\\d)"),
            "(?m)(?:in|de)flate (?:\\d)"
        );
        assert_eq!(without_lookaround("a(?=b)c(?!d)"), "ac");
        // An escaped parenthesis is a literal, not the start of a group.
        assert_eq!(without_lookaround("\\((?=x)y"), "\\(y");
        assert!(needs_backtracking("x(?<!4 )y"));
        assert!(needs_backtracking("(a)\\1"));
        assert!(!needs_backtracking("(?m)^Punycode"));
    }

    #[test]
    fn binary() {
        let pass = vec![
            case("usr/bin/a", "nothing embedded here"),
            // Not an ELF file: no strings are read.
            BinaryPackageCase::from(vec![("usr/bin/a", TestFile::new("This is a bug in bzip2"))]),
            // The owner of the library is not reported.
            case("usr/bin/a", "This is a bug in bzip2").field("Source", "bzip2"),
        ];
        let fail = vec![
            case("usr/bin/a", "This is a bug in bzip2"),
            case(
                "usr/lib/x.so",
                "A libcurl function was given a bad argument",
            )
            .udeb(),
            case("usr/bin/a", "inflate 1.2.11 Copyright"),
        ];
        BinaryPackageTester::new(EmbeddedLibrary, pass, fail).test_and_snapshot();
    }

    /// lintian `Libraries/Embedded.pm` matches each table pattern against the whole
    /// strings text of an ELF file (`$item->strings =~ $match`), which has no
    /// backtrack limit, so a banner late in a large file is found.
    #[test]
    fn pattern_found_late_in_a_large_strings_text() {
        let mut text = String::new();
        for index in 0..200_000 {
            text.push_str(&format!("symbol_name_{index} some printable text here\n"));
        }
        text.push_str("inflate 1.3.1 Copyright 1995-2024 Mark Adler ");
        let case = BinaryPackageCase::from(vec![("usr/lib/libexample.so.1", elf_with(&text))]);
        BinaryPackageTester::new(EmbeddedLibrary, vec![], vec![case]).test();
    }

    /// The same large strings text without any table pattern is not reported
    /// (the boundary of the test above).
    #[test]
    fn large_strings_text_without_a_pattern_is_not_reported() {
        let mut text = String::new();
        for index in 0..200_000 {
            text.push_str(&format!("symbol_name_{index} some printable text here\n"));
        }
        let case = BinaryPackageCase::from(vec![("usr/lib/libexample.so.1", elf_with(&text))]);
        BinaryPackageTester::new(EmbeddedLibrary, vec![case], vec![]).test();
    }

    /// Pins the matching of patterns that need no lookaround: anchors with
    /// `(?m)`, case folding, escapes and alternations.
    #[test]
    fn pattern_matching_pins() {
        let pass = vec![
            // `(?m)^` needs the line start, not just any position.
            case("usr/bin/a", "some text Punycode failed"),
            // `ieee-data` needs the dots of `DR. B. STRUCK`.
            case("usr/bin/a", "DR B STRUCK"),
            // libswscale needs `,` or a newline after `chosen`.
            case(
                "usr/bin/a",
                "Exactly one scaler algorithm must be chosen later",
            ),
            // Nothing in the table matches plain text.
            case("usr/bin/a", "hello world"),
        ];
        let fail = vec![
            case("usr/bin/a", "x\nPunycode failed"),
            // `ieee-data` is case-insensitive.
            case("usr/bin/a", "Dr. B. Struck wrote this"),
            case("usr/bin/a", "Exactly one scaler algorithm must be chosen,"),
            case(
                "usr/bin/a",
                "Caution: quantization tables are too coarse for baseline JPEG\nmore",
            )
            .field("Source", "something-else"),
        ];
        BinaryPackageTester::new(EmbeddedLibrary, pass, fail).test();
    }

    /// The zlib pattern has a negative lookbehind (`(?<!4 )`), so it needs the
    /// backtracking engine. A Pascal `4 inflate` is not zlib.
    #[test]
    fn zlib_lookbehind_pins() {
        let pass = vec![
            case("usr/bin/a", "4 inflate 1.2.11 Copyright"),
            case("usr/bin/a", "deflate without a version here"),
            case("usr/bin/a", "inflate 1.2.11 Copyright").field("Source", "zlib"),
        ];
        let fail = vec![
            case("usr/bin/a", "inflate 1.2.11 Copyright"),
            case("usr/bin/a", "deflate 1.2.8 Copyright"),
            case("usr/bin/a", "x 4inflate 1.2.11 Copyright"),
        ];
        BinaryPackageTester::new(EmbeddedLibrary, pass, fail).test();
    }

    /// Several libraries in one file are each reported. A library whose
    /// source is the package itself is skipped, and the others still report.
    #[test]
    fn several_libraries_in_one_file() {
        let text = "This is a bug in bzip2\nA libcurl function was given a bad argument";
        let fail = vec![case("usr/bin/a", text).field("Source", "other")];
        let pass = vec![
            case("usr/bin/a", "A libcurl function was given a bad argument")
                .field("Source", "curl"),
        ];
        BinaryPackageTester::new(EmbeddedLibrary, pass, fail).test();
    }
}
