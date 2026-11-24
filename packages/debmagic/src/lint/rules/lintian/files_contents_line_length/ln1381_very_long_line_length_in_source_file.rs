use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::content_type;
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
    /// `.pc/`, that are regular files and that `file(1)` describes as ASCII or
    /// Unicode text (the first 64 KiB, by libmagic's magics; see
    /// `content_type::text_kind`). Skipped: names ending in a
    /// binary or compressed extension (`files/binary-file-extensions`,
    /// `files/compressed-file-extensions`, or `xml`, `sgml`, `svg`, `json`,
    /// `jsonl`), `LICENSES/**.txt`, and, for names containing `sql`, lines
    /// starting `INSERT ` or `SELECT `. The length of a line is its character
    /// count with the newline, or its byte count when it is not UTF-8. The
    /// first longest line is the Location; Extra is `<length> > 512`. The
    /// autotools screen masks the hint for generated `configure`,
    /// `Makefile.in` and `m4/` files beside their sources (lintian's
    /// `autotools/long-lines`), and it is not reported.
    /// Severity is pedantic and the Tag is experimental and not selected by
    /// default, as in lintian. "Text" follows `file(1)` 5.47 for the magics
    /// that name a file other than as text (PostScript, OpenSSH keys, JSON,
    /// generic INI, RTF, FontForge, and the sniffed image and binary formats),
    /// and not for the rest: a Netpbm file is text but skipped as an image.
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
            .filter(|(path, _, _)| !masked_by_autotools_screen(files, path))
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

/// lintian's `Screen/Autotools/LongLines.pm`: the hint is masked (not reported)
/// for the root `configure` beside `configure.ac` or `configure.in`, a
/// `Makefile.in` beside its `Makefile.am`, and a file under `m4/` when a root
/// `configure` exists.
///
/// A root `Makefile.in` is never masked: lintian looks for `/Makefile.am`, an
/// absolute reference, and a Source index is not anchored, so it resolves to
/// nothing.
fn masked_by_autotools_screen(files: &FileTree, path: &Path) -> bool {
    let exists = |name: &str| files.get(name).is_some();
    if path == Path::new("configure") {
        return exists("configure.ac") || exists("configure.in");
    }
    if path.file_name() == Some(OsStr::new("Makefile.in")) {
        return path.parent().is_some_and(|dir| !dir.as_os_str().is_empty())
            && files.get(path.with_file_name("Makefile.am")).is_some();
    }
    path.starts_with("m4") && exists("configure")
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

/// `file(1)` describes it as `(ASCII|Unicode) text` (see
/// [`content_type::is_text`]), and not as an image: lintian skips a description
/// that says `image`, such as `Netpbm image data, ... ASCII text` or `xbm
/// image (16x16), ASCII text`.
fn is_text(bytes: &[u8]) -> bool {
    content_type::is_text(bytes) && !content_type::is_ascii_image(bytes)
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

    /// lintian `Screen/Autotools/LongLines.pm` masks the hint for a `./configure`
    /// beside `configure.ac` or `configure.in`, a `Makefile.in` beside its
    /// `Makefile.am`, and an `m4/` macro when `./configure` exists. Only the root
    /// `configure` counts; the same files elsewhere are reported.
    #[test]
    fn autotools_generated_files_are_masked_only_beside_their_source() {
        let long = "x".repeat(600);
        let pass = vec![
            with_control(vec![
                file("configure.ac", "AC_INIT\n".to_string()),
                file("configure", long.clone()),
            ]),
            with_control(vec![
                file("configure.in", "AC_INIT\n".to_string()),
                file("configure", long.clone()),
            ]),
            with_control(vec![
                file("sub/Makefile.am", "all:\n".to_string()),
                file("sub/Makefile.in", long.clone()),
            ]),
            with_control(vec![
                file("configure", "short\n".to_string()),
                file("m4/libtool.m4", long.clone()),
            ]),
        ];
        let fail = vec![
            // No configure.ac or configure.in: the root configure is not generated.
            with_control(vec![file("configure", long.clone())]),
            // configure only counts at the root.
            with_control(vec![
                file("configure.ac", "AC_INIT\n".to_string()),
                file("sub/configure", long.clone()),
            ]),
            // Makefile.in needs its Makefile.am in the same folder.
            with_control(vec![
                file("Makefile.am", "all:\n".to_string()),
                file("sub/Makefile.in", long.clone()),
            ]),
            // A root Makefile.in is not masked: lintian's `/Makefile.am` lookup is absolute.
            with_control(vec![
                file("Makefile.am", "all:\n".to_string()),
                file("Makefile.in", long.clone()),
            ]),
            // An m4 macro needs ./configure, and must be under m4/.
            with_control(vec![file("m4/libtool.m4", long.clone())]),
            with_control(vec![
                file("configure", "short\n".to_string()),
                file("aclocal/libtool.m4", long.clone()),
            ]),
        ];
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test();
    }

    /// lintian `Check/Files/Contents/LineLength.pm` checks only files whose
    /// `file(1)` type is `(ASCII|Unicode) text`; libmagic names a FontForge
    /// `SplineFontDB:` file by its magic, so it is not text.
    #[test]
    fn spline_font_database_is_not_text() {
        let long = "x".repeat(600);
        let pass = vec![with_control(vec![file(
            "fonts/Example.sfd",
            format!("SplineFontDB: 3.0\nFontName: Example\n{long}\n"),
        )])];
        let fail = vec![
            // Neighbour: the same text without the magic is still text.
            with_control(vec![file(
                "fonts/example.txt",
                format!("SplineFont: 3.0\n{long}\n"),
            )]),
        ];
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test();
    }

    fn raw(path: &'static str, bytes: Vec<u8>) -> (&'static str, TestFile) {
        (path, TestFile::from_bytes(bytes))
    }

    /// `file(1)` 5.47 calls a file ASCII text when its bytes are printable ASCII
    /// or one of BEL, backspace, tab, line feed, vertical tab, form feed,
    /// carriage return, SUB, ESC and NEL; the other C0 controls and DEL make it
    /// `data`, so lintian does not check its line length.
    #[test]
    fn text_controls_are_bell_backspace_whitespace_sub_and_escape_only() {
        let long = "x".repeat(600);
        let line = |byte: u8| {
            let mut bytes = b"a".to_vec();
            bytes.push(byte);
            bytes.extend_from_slice(long.as_bytes());
            bytes.push(b'\n');
            bytes
        };
        let pass: Vec<_> = [0x00, 0x01, 0x06, 0x0e, 0x10, 0x19, 0x1c, 0x1f, 0x7f]
            .into_iter()
            .map(|byte| with_control(vec![raw("doc/notes", line(byte))]))
            .collect();
        let fail: Vec<_> = [0x07, 0x08, 0x09, 0x0b, 0x0c, 0x0d, 0x1a, 0x1b]
            .into_iter()
            .map(|byte| with_control(vec![raw("doc/notes", line(byte))]))
            .collect();
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test();
    }

    /// `file(1)` reads only the first 64 KiB of a file for its text test, so a
    /// control byte past that point leaves the file ASCII text, and lintian
    /// checks its long line.
    #[test]
    fn text_test_reads_only_the_first_64_kib() {
        let long = "x".repeat(600);
        let pass = vec![with_control(vec![file(
            "doc/notes",
            format!("{}\x01{long}\n", "y".repeat(100)),
        )])];
        let fail = vec![with_control(vec![file(
            "doc/notes",
            format!("{}\x01{long}\n", "y".repeat(70_000)),
        )])];
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test();
    }

    /// `file(1)` drops trailing NUL bytes before its text test, so a text
    /// followed by NULs is still ASCII text and lintian checks it. A NUL before
    /// the end makes the file `data`.
    #[test]
    fn trailing_nul_bytes_are_not_part_of_the_text() {
        let long = "x".repeat(600);
        let pass = vec![with_control(vec![file(
            "doc/notes",
            format!("a\0{long}\n"),
        )])];
        let fail = vec![with_control(vec![file(
            "doc/notes",
            format!("{long}\n\0\0\0"),
        )])];
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test();
    }

    /// `file(1)` calls bytes that are not UTF-8 but hold C1 controls (0x80 to
    /// 0x9f, such as Windows-1252 quotes) `Non-ISO extended-ASCII text`, which
    /// lintian's `(ASCII|Unicode) text` matches as a substring. Latin-1 bytes
    /// without a C1 control are `ISO-8859 text`, which it does not match.
    #[test]
    fn c1_controls_make_extended_ascii_text_and_latin1_is_not_text() {
        let long = "x".repeat(600);
        let line = |middle: &[u8]| {
            let mut bytes = b"say ".to_vec();
            bytes.extend_from_slice(middle);
            bytes.extend_from_slice(long.as_bytes());
            bytes.push(b'\n');
            bytes
        };
        let pass: Vec<_> = [&b"caf\xe9 "[..], &b"caf\xe9\xff "[..], &b"caf\xa0 "[..]]
            .into_iter()
            .map(|middle| with_control(vec![raw("doc/notes", line(middle))]))
            .collect();
        let fail: Vec<_> = [
            &b"\x93quoted\x94 "[..],
            &b"caf\x93\xe9 "[..],
            &b"caf\xc3\xa9 "[..],
        ]
        .into_iter()
        .map(|middle| with_control(vec![raw("doc/notes", line(middle))]))
        .collect();
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test();
    }

    /// `file(1)` names a PostScript program, an OpenSSH public or private key
    /// and a JSON document by their magic, not as text, so lintian skips them.
    /// The key prefixes need their trailing space or are matched as a prefix:
    /// `ssh-rsa` followed by anything but a space is still ASCII text, and so
    /// is a JavaScript object literal, which is not JSON.
    #[test]
    fn postscript_openssh_keys_and_json_are_not_text() {
        let long = "x".repeat(600);
        let pass: Vec<_> = [
            format!("%!PS-Adobe-3.0\n{long}\n"),
            format!("ssh-rsa {long}\n"),
            format!("ssh-ed25519 {long}\n"),
            format!("ecdsa-sha2-nistp256 {long}\n"),
            format!("-----BEGIN OPENSSH PRIVATE KEY-----\n{long}\n"),
            format!("{{\"k\": \"{long}\"}}\n"),
            format!("[\"{long}\"]\n"),
            format!("{{\"a\": \"{long}\"}}\n{{\"b\": 1}}\n"),
        ]
        .into_iter()
        .map(|content| with_control(vec![file("doc/notes", content)]))
        .collect();
        let fail: Vec<_> = [
            format!("ssh-rsa{long}\n"),
            format!("ssh-rsa-cert-v01@openssh.com {long}\n"),
            format!("{{a: \"{long}\"}}\n"),
            format!("{{\"a\": \"{long}\"}}\n[1]\n{{\"b\": 1}}\n"),
        ]
        .into_iter()
        .map(|content| with_control(vec![file("doc/notes", content)]))
        .collect();
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test();
    }

    /// `file(1)` names a Netpbm, an XPM with its `static` declaration, and an SVG
    /// file by their magic. The Netpbm description says `image`, which lintian
    /// skips. An XPM header without the declaration is still ASCII text.
    #[test]
    fn image_magics_are_skipped_as_images() {
        let long = "x".repeat(600);
        let pass: Vec<_> = [
            format!("P1\n{}\n", "1 ".repeat(300)),
            format!("/* XPM */\nstatic char *x[] = {{\n{long}\n"),
            format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><desc>{long}</desc></svg>\n"),
        ]
        .into_iter()
        .map(|content| with_control(vec![file("doc/icon", content)]))
        .collect();
        let fail = vec![with_control(vec![file(
            "doc/icon",
            format!("/* XPM */\n{long}\n"),
        )])];
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test();
    }

    /// `file(1)` 5.47 names some source and config files by a magic that is not
    /// text, so lintian skips them: a Perl script (`#!` with `/perl` followed by
    /// white space, on a line that ends with a newline), a sendmail m4 file
    /// (`divert(-1)`), a TeX file (`%!TEX`), a PEM certificate, and an INI file
    /// with a `;` comment and a second section. The neighbours are text.
    #[test]
    fn source_magics_that_name_a_file_other_than_text() {
        let long = "x".repeat(600);
        let pass: Vec<_> = [
            format!("#!/usr/bin/perl\n{long}\n"),
            format!("#!/usr/bin/env perl -w\n{long}\n"),
            format!("divert(-1)\n{long}\n"),
            format!("%!TEX root = x.tex\n{long}\n"),
            format!("-----BEGIN CERTIFICATE-----\n{long}\n"),
            format!("; c\n[A]\nk=v\n[B]\nk={long}\n"),
            format!("#define a_width 16\n{long}\n"),
        ]
        .into_iter()
        .map(|content| with_control(vec![file("doc/notes", content)]))
        .collect();
        let fail: Vec<_> = [
            format!("#!/usr/bin/perl{long}\n"),
            format!("#!/usr/bin/python3\n{long}\n"),
            format!("divert(0)\n{long}\n"),
            format!("% comment\n{long}\n"),
            format!("-----BEGIN FOO-----\n{long}\n"),
            format!("; c\n[A]\nk=v\n{long}\n"),
            format!("#define a 16\n{long}\n"),
        ]
        .into_iter()
        .map(|content| with_control(vec![file("doc/notes", content)]))
        .collect();
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test();
    }

    /// `file(1)` names UTF-16 with a byte order mark, either endianness,
    /// `Unicode text, UTF-16`, so lintian checks it. Without the mark it is
    /// `data`.
    #[test]
    fn utf16_with_a_byte_order_mark_is_unicode_text() {
        let long = format!("{}\n", "x".repeat(600));
        let with_mark = |mark: [u8; 2], big_endian: bool| {
            let mut bytes = mark.to_vec();
            bytes.extend(utf16_units(&long, big_endian));
            raw("doc/notes", bytes)
        };
        let pass = vec![with_control(vec![raw(
            "doc/notes",
            utf16_units(&long, false),
        )])];
        let fail = vec![
            with_control(vec![with_mark([0xff, 0xfe], false)]),
            with_control(vec![with_mark([0xfe, 0xff], true)]),
        ];
        SourceTreeTester::new(VeryLongLineLengthInSourceFile, pass, fail).test();
    }

    /// The UTF-16 code units of `text`, in little-endian or big-endian order.
    fn utf16_units(text: &str, big_endian: bool) -> Vec<u8> {
        text.encode_utf16()
            .flat_map(|unit| {
                if big_endian {
                    unit.to_be_bytes()
                } else {
                    unit.to_le_bytes()
                }
            })
            .collect()
    }
}
