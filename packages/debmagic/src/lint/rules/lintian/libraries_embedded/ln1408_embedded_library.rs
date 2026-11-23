use std::sync::LazyLock;

use fancy_regex::{Regex, RegexBuilder};

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

/// One line of `binaries/embedded-libs`.
struct Embedded {
    libname: &'static str,
    source: &'static str,
    source_regex: Option<Regex>,
    pattern: Regex,
}

static EMBEDDED: LazyLock<Vec<Embedded>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/binaries/embedded-libs"))
        .filter_map(parse_line)
        .collect()
});

/// `label || key=value ... || pattern`: the label is the text before the first
/// `||`, the pairs and the pattern are split at the next `||`.
fn parse_line(line: &'static str) -> Option<Embedded> {
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
    Some(Embedded {
        libname: libname.unwrap_or(label),
        source: source.unwrap_or(label),
        source_regex,
        pattern: string_pattern(pattern)?,
    })
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
        for (path, text) in strings.iter() {
            for library in EMBEDDED.iter() {
                if library
                    .source_regex
                    .as_ref()
                    .is_some_and(|regex| regex.is_match(&source).unwrap_or(false))
                {
                    continue;
                }
                if !library.source.is_empty() && library.source == source {
                    continue;
                }
                if library.pattern.is_match(text).unwrap_or(false) {
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
        assert_eq!(EMBEDDED.len(), lines);
        assert!(EMBEDDED.iter().any(|library| library.libname == "zlib"));
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
}
