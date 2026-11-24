//! Content type of a file from its leading bytes, for the formats lint Rules
//! tell apart: the image formats of lintian's `images/filenames` Check, Matlab
//! v4 to v7.3 files, PDF and PostScript. No `file(1)` is run (ADR 0041); a
//! file whose bytes are none of these has no content type. The text verdicts
//! `ASCII text` and generic INI configuration are separate functions, since
//! they are not content types of their own.

/// A content type this module recognises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentType {
    Png,
    Jpeg,
    Gif,
    Tiff,
    /// Windows or OS/2 bitmap (`BM` header), `file(1)`: `PC bitmap`.
    Bmp,
    /// `/* XPM */` text with a `static` declaration (see [`is_xpm`]).
    Xpm,
    /// Netpbm `P1` to `P7` followed by white space.
    Netpbm,
    Svg,
    /// `%PDF-`.
    Pdf,
    /// `%!PS` or `%!FontType1`: PostScript, EPS, and Type 1 font programs.
    PostScript,
    /// `MATLAB <major>.<minor> MAT-file` header; `major` is the version's major
    /// number (`5` for `MATLAB 5.0 MAT-file`).
    Matlab {
        major: u32,
    },
}

impl ContentType {
    /// The content type of `bytes`, or `None` when none of the formats match.
    pub(crate) fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Some(Self::Png);
        }
        if bytes.starts_with(b"\xff\xd8\xff") {
            return Some(Self::Jpeg);
        }
        if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            return Some(Self::Gif);
        }
        if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
            return Some(Self::Tiff);
        }
        if is_xpm(bytes) {
            return Some(Self::Xpm);
        }
        if bytes.len() > 2
            && bytes[0] == b'P'
            && (b'1'..=b'7').contains(&bytes[1])
            && bytes[2].is_ascii_whitespace()
        {
            return Some(Self::Netpbm);
        }
        if is_svg(bytes) {
            return Some(Self::Svg);
        }
        if is_bmp(bytes) {
            return Some(Self::Bmp);
        }
        if bytes.starts_with(b"%PDF-") {
            return Some(Self::Pdf);
        }
        if bytes.starts_with(b"%!PS") || bytes.starts_with(b"%!FontType1") {
            return Some(Self::PostScript);
        }
        if let Some(major) = matlab_major(bytes) {
            return Some(Self::Matlab { major });
        }
        None
    }

    /// Whether the content is a raster or vector image, as `file(1)` would say
    /// `image` or `bitmap` (the formats the `images/filenames` Check names).
    #[allow(dead_code)]
    pub(crate) fn is_image(self) -> bool {
        !matches!(self, Self::Pdf | Self::PostScript | Self::Matlab { .. })
    }
}

/// Whether `bytes` start like a Type 1 font program: a PFA with a
/// `%!PS-AdobeFont` or `%!FontType1` header, or a PFB whose first segment is
/// ASCII text starting `%!`. [`ContentType::sniff`] reports these as
/// [`ContentType::PostScript`]; the Type 1 font seam is the only reader that
/// needs the font form.
pub(crate) fn is_type1_font(bytes: &[u8]) -> bool {
    if bytes.starts_with(b"%!PS-AdobeFont") || bytes.starts_with(b"%!FontType1") {
        return true;
    }
    bytes.len() > 6 && bytes[0] == 0x80 && bytes[1] == 1 && bytes[6..].starts_with(b"%!")
}

/// The text `file(1)` 5.47 describes its input as, when it describes it as
/// text. lintian's Checks match these descriptions by regex: `(ASCII|Unicode)
/// text` and `\bASCII text\b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextKind {
    /// `ASCII text`.
    Ascii,
    /// `Non-ISO extended-ASCII text`: bytes that are not UTF-8 and hold C1
    /// controls (0x80 to 0x9f). The substring `ASCII text` matches both regexes.
    ExtendedAscii,
    /// `Unicode text`: UTF-8, with or without a byte order mark, or UTF-16 with
    /// a little-endian one.
    Unicode,
}

/// The text `file(1)` 5.47 gives `bytes`, or `None` when it names them by
/// another magic (see [`is_other_magic`]) or does not call them text. Only the
/// first 64 KiB are examined for text. The bytes of ASCII text are printable
/// ASCII, BEL, backspace, the whitespace controls, SUB, ESC and NEL (0x85).
/// A single byte is `very short file (no magic)` and an empty file is
/// `empty`, so neither is text.
pub(crate) fn text_kind(bytes: &[u8]) -> Option<TextKind> {
    // `file(1)` drops trailing NUL bytes before it looks at the text.
    let end = bytes
        .iter()
        .rposition(|&byte| byte != 0)
        .map_or(0, |last| last + 1);
    let bytes = &bytes[..end];
    if bytes.len() < 2 || is_other_magic(bytes) {
        return None;
    }
    let window = &bytes[..bytes.len().min(TEXT_WINDOW)];
    if let Some(body) = window.strip_prefix(b"\xff\xfe") {
        return utf16_is_text(body, u16::from_le_bytes).then_some(TextKind::Unicode);
    }
    if let Some(body) = window.strip_prefix(b"\xfe\xff") {
        return utf16_is_text(body, u16::from_be_bytes).then_some(TextKind::Unicode);
    }
    if window.iter().all(|&byte| is_ascii_text_byte(byte)) {
        return Some(TextKind::Ascii);
    }
    if window
        .iter()
        .all(|&byte| byte >= 0x80 || is_ascii_text_byte(byte))
        && std::str::from_utf8(window).is_ok()
    {
        return Some(TextKind::Unicode);
    }
    // ISO-8859 text is Latin-1 without C1 controls, which lintian does not match.
    if window
        .iter()
        .all(|&byte| byte >= 0x80 || is_ascii_text_byte(byte))
        && window.iter().any(|&byte| (0x80..=0x9f).contains(&byte))
    {
        return Some(TextKind::ExtendedAscii);
    }
    None
}

/// Whether `file(1)` 5.47 describes `bytes` as `(ASCII|Unicode) text`, lintian's
/// test for the `Contents::LineLength` Check. The non-ASCII descriptions match
/// it too, since `Non-ISO extended-ASCII text` contains `ASCII text`.
pub(crate) fn is_text(bytes: &[u8]) -> bool {
    text_kind(bytes).is_some()
}

/// Whether `file(1)` 5.47 describes `bytes` as `ASCII text` in lintian's
/// `\bASCII text\b` sense, which also matches `Non-ISO extended-ASCII text`.
/// Valid UTF-8 with non-ASCII bytes is `UTF-8 Unicode text`, so it is not.
pub(crate) fn is_ascii_text(bytes: &[u8]) -> bool {
    matches!(
        text_kind(bytes),
        Some(TextKind::Ascii | TextKind::ExtendedAscii)
    )
}

/// The first 64 KiB of a file: the window `file(1)` runs its text test on.
const TEXT_WINDOW: usize = 64 * 1024;

/// The bytes of ASCII text in `file(1)`: printable ASCII, BEL, backspace, tab,
/// line feed, vertical tab, form feed, carriage return, SUB, ESC and NEL.
fn is_ascii_text_byte(byte: u8) -> bool {
    matches!(byte, 0x07..=0x0d | 0x1a | 0x1b | 0x20..=0x7e | 0x85)
}

/// Whether `file(1)` 5.47 describes `bytes` as an image that still ends in
/// `ASCII text`, such as `Netpbm image data, ... ASCII text` or `xbm image
/// (16x16), ASCII text`. lintian's `image|bitmap|JSON` test skips these, so
/// they are text for [`is_text`] and `is_ascii_text`, but not for a Check
/// that skips images.
pub(crate) fn is_ascii_image(bytes: &[u8]) -> bool {
    ContentType::sniff(bytes) == Some(ContentType::Netpbm) || is_xbm(bytes)
}

/// Whether `file(1)` names `bytes` by a magic that is not text, so that it
/// does not describe them as `ASCII text` or `Unicode text`: a sniffed format
/// other than the image formats of [`is_ascii_image`], RTF, a FontForge file,
/// a generic INI file, an OpenSSH key or PEM armour, a Perl script, a FIG
/// (`#FIG`: an image, and lintian skips it whichever way it is read) or TeX
/// file, a LyX file, a mailbox (`From ` at the start), a sendmail m4 file, a
/// Rust source file (`#![` at the start, which `file(1)` 5.47 names `Rust
/// Source file` with no text in it), or JSON.
fn is_other_magic(bytes: &[u8]) -> bool {
    matches!(ContentType::sniff(bytes), Some(kind) if kind != ContentType::Netpbm)
        || bytes.starts_with(b"{\\rtf")
        || bytes.starts_with(b"#![")
        || bytes.starts_with(b"SplineFontDB:")
        || bytes.starts_with(b"#FIG")
        || bytes.starts_with(b"%!TEX root")
        || bytes.starts_with(b"#LyX")
        || bytes.starts_with(b"From ")
        || bytes.starts_with(b"divert(-1)\n")
        || is_perl_script(bytes)
        || is_key_or_armor(bytes)
        || is_ini_config(bytes)
        || is_json(bytes)
}

/// Whether the first line is `#!` and a Perl interpreter (`file(1)` 5.47 names
/// it `Perl script text executable`): an absolute path ending `/perl`, or
/// `/usr/bin/env perl`, followed by white space (`#!./perl` is `Perl5 module
/// source`, which is text). The line must end with a newline, since
/// `#!/usr/bin/perl` alone is `ASCII text`.
fn is_perl_script(bytes: &[u8]) -> bool {
    let Some(end) = bytes.iter().position(|&byte| byte == b'\n') else {
        return false;
    };
    let Some(line) = bytes[..end].strip_prefix(b"#!") else {
        return false;
    };
    let words: Vec<&[u8]> = line
        .split(u8::is_ascii_whitespace)
        .filter(|word| !word.is_empty())
        .collect();
    // A relative `./perl` is `Perl5 module source, ASCII text` to file(1).
    match words.as_slice() {
        [path, ..] if path.starts_with(b"/") && path.ends_with(b"/perl") => true,
        [path, interpreter, ..] => {
            path.starts_with(b"/") && path.ends_with(b"/env") && *interpreter == b"perl"
        }
        _ => false,
    }
}

/// Whether the first line is `#define <name>_width <n>`, which `file(1)` 5.47
/// names `xbm image` (an X bitmap).
fn is_xbm(bytes: &[u8]) -> bool {
    let Some(rest) = bytes.strip_prefix(b"#define ") else {
        return false;
    };
    let name_end = rest
        .iter()
        .position(u8::is_ascii_whitespace)
        .unwrap_or(rest.len());
    rest[..name_end].ends_with(b"_width") && rest.get(name_end) == Some(&b' ')
}

/// The PEM and PGP armour headers `file(1)` 5.47 names by magic (probed). The
/// other PEM labels, such as `X509 CERTIFICATE`, `TRUSTED CERTIFICATE` and
/// `PKCS7`, are `ASCII text`.
const ARMOR_HEADERS: [&[u8]; 11] = [
    b"-----BEGIN CERTIFICATE-----",
    b"-----BEGIN CERTIFICATE REQUEST-----",
    b"-----BEGIN PUBLIC KEY-----",
    b"-----BEGIN RSA PUBLIC KEY-----",
    b"-----BEGIN PRIVATE KEY-----",
    b"-----BEGIN RSA PRIVATE KEY-----",
    b"-----BEGIN DSA PRIVATE KEY-----",
    b"-----BEGIN EC PRIVATE KEY-----",
    b"-----BEGIN ENCRYPTED PRIVATE KEY-----",
    b"-----BEGIN OPENSSH PRIVATE KEY-----",
    b"-----BEGIN PGP ",
];

/// Whether `bytes` starts like an OpenSSH key or a PEM or PGP armour: the key
/// magics `file(1)` 5.47 names at offset 0 (`ssh-rsa` and `ssh-dss` followed by
/// a space, the ED25519 and ECDSA key types as a prefix, also of their
/// certificates, and the U2F keys), or one of [`ARMOR_HEADERS`].
fn is_key_or_armor(bytes: &[u8]) -> bool {
    const KEY_PREFIXES: [&[u8]; 6] = [
        b"ssh-ed25519",
        b"ecdsa-sha2-nistp256",
        b"ecdsa-sha2-nistp384",
        b"ecdsa-sha2-nistp521",
        b"sk-ssh-ed25519@openssh.com",
        b"sk-ecdsa-sha2-nistp256@openssh.com",
    ];
    bytes.starts_with(b"ssh-rsa ")
        || bytes.starts_with(b"ssh-dss ")
        || KEY_PREFIXES.iter().any(|prefix| bytes.starts_with(prefix))
        || ARMOR_HEADERS.iter().any(|header| bytes.starts_with(header))
}

/// Whether `body`, the bytes after a UTF-16 byte order mark, is text to
/// `file(1)` (`Unicode text, UTF-16, little-endian text` or big-endian): every
/// code point is non-ASCII or an ASCII text byte, and there are no unpaired
/// surrogates. Without the mark, UTF-16 is `data`.
fn utf16_is_text(body: &[u8], decode: fn([u8; 2]) -> u16) -> bool {
    let (pairs, _odd_byte) = body.as_chunks::<2>();
    let units = pairs.iter().map(|&pair| decode(pair));
    char::decode_utf16(units).all(|unit| {
        unit.is_ok_and(|character| !character.is_ascii() || is_ascii_text_byte(character as u8))
    })
}

/// Whether `bytes` is a JSON document to `file(1)` 5.47: a sequence of
/// top-level objects or arrays separated by white space (a single one is
/// `JSON text data`, several are `New Line Delimited JSON text data`). The
/// grammar is lenient as `file(1)` is: a trailing comma, a leading zero and a
/// control byte in a string are accepted. A scalar, a bare comma, or an
/// unquoted key is not JSON. File reads at most 7 MiB, so a longer file
/// is judged by its first 7 MiB, which is then incomplete.
fn is_json(bytes: &[u8]) -> bool {
    let mut scan = JsonScan {
        bytes: &bytes[..bytes.len().min(JSON_LIMIT)],
        pos: 0,
    };
    let mut kinds = Vec::new();
    loop {
        scan.skip_whitespace();
        let Some(first) = scan.peek() else { break };
        if !matches!(first, b'{' | b'[') || scan.value(1).is_none() {
            return false;
        }
        kinds.push(first);
    }
    match kinds.as_slice() {
        [] => false,
        [_] => true,
        // `file(1)` tells a line-delimited sequence apart by its first two
        // values, which must be of the same kind.
        [first, second, ..] => first == second,
    }
}

/// The largest JSON input `file(1)` 5.47 reads: 7 MiB.
const JSON_LIMIT: usize = 7 * 1024 * 1024;

/// The deepest nested array (251) and object (250) `file(1)` 5.47 accepts,
/// probed: one level more makes the document ASCII text.
const JSON_ARRAY_MAX_DEPTH: usize = 251;
const JSON_OBJECT_MAX_DEPTH: usize = 250;

/// A lenient JSON scanner over the bytes of a file (see [`is_json`]).
struct JsonScan<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl JsonScan<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn eat(&mut self, byte: u8) -> Option<()> {
        (self.peek()? == byte).then(|| self.pos += 1)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.pos += 1;
        }
    }

    /// A value at `depth`, the number of containers around it plus one.
    fn value(&mut self, depth: usize) -> Option<()> {
        match self.peek()? {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => self.string(),
            b'-' | b'0'..=b'9' => self.number(),
            b't' => self.literal(b"true"),
            b'f' => self.literal(b"false"),
            b'n' => self.literal(b"null"),
            _ => None,
        }
    }

    fn literal(&mut self, word: &[u8]) -> Option<()> {
        self.bytes[self.pos..]
            .starts_with(word)
            .then(|| self.pos += word.len())
    }

    fn array(&mut self, depth: usize) -> Option<()> {
        if depth > JSON_ARRAY_MAX_DEPTH {
            return None;
        }
        self.eat(b'[')?;
        self.skip_whitespace();
        if self.eat(b']').is_some() {
            return Some(());
        }
        loop {
            self.value(depth + 1)?;
            self.skip_whitespace();
            if self.eat(b',').is_some() {
                self.skip_whitespace();
                if self.eat(b']').is_some() {
                    return Some(());
                }
            } else {
                return self.eat(b']');
            }
        }
    }

    fn object(&mut self, depth: usize) -> Option<()> {
        if depth > JSON_OBJECT_MAX_DEPTH {
            return None;
        }
        self.eat(b'{')?;
        self.skip_whitespace();
        if self.eat(b'}').is_some() {
            return Some(());
        }
        loop {
            self.string()?;
            self.skip_whitespace();
            self.eat(b':')?;
            self.skip_whitespace();
            self.value(depth + 1)?;
            self.skip_whitespace();
            if self.eat(b',').is_some() {
                self.skip_whitespace();
                if self.eat(b'}').is_some() {
                    return Some(());
                }
            } else {
                return self.eat(b'}');
            }
        }
    }

    fn string(&mut self) -> Option<()> {
        self.eat(b'"')?;
        loop {
            match self.peek()? {
                b'"' => {
                    self.pos += 1;
                    return Some(());
                }
                b'\\' => {
                    self.pos += 1;
                    match self.peek()? {
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => self.pos += 1,
                        b'u' => {
                            self.pos += 1;
                            for _ in 0..4 {
                                if !self.peek()?.is_ascii_hexdigit() {
                                    return None;
                                }
                                self.pos += 1;
                            }
                        }
                        _ => return None,
                    }
                }
                _ => self.pos += 1,
            }
        }
    }

    fn number(&mut self) -> Option<()> {
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        self.digits()?;
        if self.peek() == Some(b'.') {
            self.pos += 1;
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            self.digits()?;
        }
        Some(())
    }

    fn digits(&mut self) -> Option<()> {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        (self.pos > start).then_some(())
    }
}

/// Whether `file(1)` 5.47 names `bytes` `Generic INItialization configuration`
/// (its `ini` magic rule), which systemd units and many other `[Section]`
/// files get instead of `ASCII text`. The rule is probed from `file`'s output,
/// not copied from its magic source, and it is narrower than `file`'s:
///
/// - The text starts with `[` or `;` (a comment in many INI files; `#` is
///   ASCII text), and the second `[` in the text, counting a `[` at the start,
///   opens a section header whose `[` is at or before byte 8193. It may sit in
///   a comment: `[x] junk` and `; see [note]` count as brackets too, and only
///   the second one is looked at (probed on both starts).
/// - A header is `[`, a name of ASCII letters, digits, spaces, `_`, `(` or
///   `)`, then `]`, then `\r\n`, or `\r` or `\n` with at least one byte after it.
///
/// Other rules that `file(1)` applies first (JSON, for one) are not modelled.
pub(crate) fn is_ini_config(bytes: &[u8]) -> bool {
    if !matches!(bytes.first(), Some(b'[' | b';')) {
        return false;
    }
    let limit = bytes.len().min(INI_HEADER_LAST_START + 1);
    (0..limit)
        .filter(|&index| bytes[index] == b'[')
        .nth(1)
        .is_some_and(|index| is_ini_header(&bytes[index..]))
}

/// The last byte offset at which an INI section header may start.
const INI_HEADER_LAST_START: usize = 8193;

/// Whether `text` starts with a section header `[name]` and a line end that
/// `file(1)` accepts after it (see [`is_ini_config`]).
fn is_ini_header(text: &[u8]) -> bool {
    let name_len = text[1..]
        .iter()
        .take_while(|&&byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'_' | b'(' | b')')
        })
        .count();
    if name_len == 0 {
        return false;
    }
    matches!(
        text.get(1 + name_len..),
        Some([b']', b'\r', b'\n', ..] | [b']', b'\r' | b'\n', _, ..])
    )
}

/// `/* XPM */` text with a `static` declaration in its first 8 KiB. The header
/// alone is `ASCII text` to `file(1)` 5.47; the declaration makes it `X pixmap
/// image text` (probed).
fn is_xpm(bytes: &[u8]) -> bool {
    bytes.starts_with(b"/* XPM */")
        && bytes[..bytes.len().min(8192)]
            .windows(7)
            .any(|window| window == b"static ")
}

/// An XML or SVG document with `<svg` within its first 8 KiB, after an optional
/// UTF-8 byte order mark. Looser than `file(1)`; see LN1395.
fn is_svg(bytes: &[u8]) -> bool {
    let body = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let trimmed = body.trim_ascii_start();
    let xml_or_svg = trimmed.starts_with(b"<?xml")
        || trimmed.starts_with(b"<svg")
        || trimmed.starts_with(b"<!DOCTYPE svg");
    let head = &body[..body.len().min(8192)];
    xml_or_svg && head.windows(4).any(|window| window == b"<svg")
}

/// `BM` and a little-endian DIB header size that a Windows or OS/2 bitmap has
/// at offset 14.
fn is_bmp(bytes: &[u8]) -> bool {
    const DIB_HEADER_SIZES: [u32; 8] = [12, 16, 40, 52, 56, 64, 108, 124];
    if !bytes.starts_with(b"BM") || bytes.len() < 18 {
        return false;
    }
    let size = u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]);
    DIB_HEADER_SIZES.contains(&size)
}

/// The major version of a `MATLAB x.y MAT-file` header within its first 128
/// bytes.
fn matlab_major(bytes: &[u8]) -> Option<u32> {
    let rest = bytes.strip_prefix(b"MATLAB ")?;
    let header = &rest[..rest.len().min(128)];
    if !header.windows(8).any(|window| window == b"MAT-file") {
        return None;
    }
    let digits: Vec<u8> = header
        .iter()
        .copied()
        .take_while(u8::is_ascii_digit)
        .collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_the_raster_formats() {
        assert_eq!(
            ContentType::sniff(b"\x89PNG\r\n\x1a\nrest"),
            Some(ContentType::Png)
        );
        assert_eq!(
            ContentType::sniff(b"\xff\xd8\xffrest"),
            Some(ContentType::Jpeg)
        );
        assert_eq!(ContentType::sniff(b"GIF89a rest"), Some(ContentType::Gif));
        assert_eq!(ContentType::sniff(b"II*\0rest"), Some(ContentType::Tiff));
        assert_eq!(
            ContentType::sniff(b"/* XPM */\nstatic char *x[] = {\n"),
            Some(ContentType::Xpm)
        );
        // The header alone is ASCII text to file(1).
        assert_eq!(ContentType::sniff(b"/* XPM */\nx\n"), None);
        assert_eq!(
            ContentType::sniff(b"P4\n# c\n8 1\n\x00"),
            Some(ContentType::Netpbm)
        );
        assert_eq!(
            ContentType::sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"),
            Some(ContentType::Svg)
        );
    }

    #[test]
    fn sniffs_bmp_by_its_dib_header() {
        let mut bmp = b"BM".to_vec();
        bmp.extend_from_slice(&[0; 12]);
        bmp.extend_from_slice(&40u32.to_le_bytes());
        assert_eq!(ContentType::sniff(&bmp), Some(ContentType::Bmp));

        // Text that starts with `BM` is not a bitmap.
        assert_eq!(
            ContentType::sniff(b"BMW is a car maker, not a bitmap"),
            None
        );
        assert_eq!(ContentType::sniff(b"BM"), None);
    }

    #[test]
    fn sniffs_pdf_and_postscript() {
        assert_eq!(ContentType::sniff(b"%PDF-1.3\n"), Some(ContentType::Pdf));
        assert_eq!(
            ContentType::sniff(b"%!PS-Adobe-3.0 EPSF-3.0\n"),
            Some(ContentType::PostScript)
        );
        assert_eq!(
            ContentType::sniff(b"%!FontType1-1.0\n"),
            Some(ContentType::PostScript)
        );
    }

    #[test]
    fn sniffs_matlab_mat_files_with_their_major_version() {
        let mut v5 = b"MATLAB 5.0 MAT-file, Platform: GLNX86".to_vec();
        v5.resize(128, b' ');
        assert_eq!(
            ContentType::sniff(&v5),
            Some(ContentType::Matlab { major: 5 })
        );
        assert_eq!(
            ContentType::sniff(b"MATLAB 7.3 MAT-file, HDF5"),
            Some(ContentType::Matlab { major: 7 })
        );
        assert_eq!(ContentType::sniff(b"MATLAB script text"), None);
    }

    #[test]
    fn type_1_font_headers_and_pfb_segments() {
        assert!(is_type1_font(b"%!PS-AdobeFont-1.0: X\n"));
        assert!(is_type1_font(b"%!FontType1-1.0\n"));
        assert!(!is_type1_font(b"%!PS-Adobe-3.0 EPSF-3.0\n"));
        assert!(is_type1_font(&[
            0x80, 0x01, 2, 0, 0, 0, b'%', b'!', b'P', b'S'
        ]));
        assert!(!is_type1_font(&[0x80, 0x01, 2, 0, 0, 0, b'h', b'i']));
        assert!(!is_type1_font(&[0x80, 0x01]));
    }

    #[test]
    fn plain_text_and_empty_input_have_no_type() {
        assert_eq!(ContentType::sniff(b"plain text"), None);
        assert_eq!(ContentType::sniff(b""), None);
    }

    #[test]
    fn section_files_are_generic_ini_not_ascii_text() {
        // A unit file: a second section header after the first makes file(1)
        // say generic INI configuration.
        assert!(is_ini_config(
            b"[Unit]\nDescription=x\n\n[Service]\nExecStart=/bin/true\n"
        ));
        // Only one section header: file(1) says ASCII text.
        assert!(!is_ini_config(b"[Service]\nExecStart=/bin/true\n"));
        assert!(!is_ini_config(b"# comment\n[A]\nk=v\n[B]\nk=v\n"));
        assert!(!is_ascii_text(
            b"[Unit]\nDescription=x\n\n[Service]\nExecStart=/bin/true\n"
        ));
        assert!(is_ascii_text(b"[Service]\nExecStart=/bin/true\n"));
        assert!(is_ascii_text(b"plain text\n"));
        assert!(!is_ascii_text(b""));
        assert!(!is_ascii_text(b"caf\xc3\xa9\n"));
        assert!(!is_ascii_text(b"\x7fELF"));
    }

    /// Generated ASCII texts whose `file(1)` description is checked against
    /// [`is_ini_config`] and [`is_ascii_text`]. They avoid the starts that
    /// other magic claims first (`[]`, `[1,` JSON, `#!`, `<`).
    fn ini_fixtures() -> Vec<Vec<u8>> {
        let mut fixtures: Vec<Vec<u8>> = [
            // Section files: the second header decides.
            "[Unit]\nDescription=x\n\n[Service]\nExecStart=/bin/true\n",
            "[Unit]\nDescription=x\n[Service]\nExecStart=a\n",
            "[Unit]\n\n[Service]\nExecStart=/bin/true\n",
            "[A]\nk=v\n[B]\nk=v\n",
            "[A]\n[B]\nk=v\n",
            "[A]\n\n[B]\n\n",
            "[A]\n\n[B]\nxyz\n",
            "[A]\n\n[B]\n[C]\n",
            "[A]\nk=v\n[B]\nk=v",
            "[A]\nk=v\n[B]\r\nk=v\r\n",
            "[A]\nk=v\n[B]\r\n",
            "[A]\nk=v\n[B]\n",
            "[A]\r\nk=v\r\n[B]\r\nk=v\r\n",
            "[A]\nk=v\n[B]\rk=v\n",
            "[A]\nk=v\n[B]\tx\n",
            "[A]\nk=v\n[B] x\nk=v\n",
            "[A]\nk=v\n[B]  \nk=v\n",
            "[A]\nk=v\n[B]x\nk=v\n",
            "[A]\nk=v\nx[B]\nk=v\n",
            "[A]\nk=v\n  [B]\nk=v\n",
            "[A]\nk=v\n\t[B]\nk=v\n",
            "[A B]\nk=v\n[B C]\nk=v\n",
            "[A]\nk=v\n[B_C]\nk=v\n",
            "[A]\nk=v\n[B(C)]\nk=v\n",
            "[A]\nk=v\n[0]\nk=v\n",
            "[A]\nk=v\n[]\nk=v\n",
            "[A]\nk=v\n[B.C]\nk=v\n",
            "[A]\nk=v\n[B-C]\nk=v\n",
            "[A]\nk=v\n[B:C]\nk=v\n",
            "[A]\nk=v\n[B/C]\nk=v\n",
            "[A]\nk=v\n[B[C]]\nk=v\n",
            "[A]\nk=v\n[B\nk=v\n",
            "[A.B]\nk=v\n[B]\nk=v\n",
            "[A-B]\nk=v\n[B]\nk=v\n",
            "[]\nk=v\n[B]\nk=v\n",
            "[Service]\nExecStart=/bin/true\n",
            "[Service]\n",
            "[Service]",
            "[Section]\nkey=value\n\n",
            "[Service]\n\nk=v\n",
            "[A] x\nk=v\n[B]\nk=v\n",
            "[A]=v\nk=v\n[B]\nk=v\n",
            "[A]\t\nk=v\n[B]\nk=v\n",
            "[\nk=v\n[B]\nk=v\n",
            "[[A]\nk=v\n[B]\nk=v\n",
            "[A]\nk=v\n[B]\nk=v\n[\n",
            "[A]\nk=v\n[B]\nk=v\n[Z]x\n",
            "[x\n[y]z\n[Service]\nk=v\n",
            "[x\n#[\n[Service]\nk=v\n",
            "[A][\nk=v\n[B]\nk=v\n",
            "[x\nx[y]\n[Service]\nk=v\n",
            "[x\n[]\n[Service]\nk=v\n",
            "[x\n\n[B]\nk=v\n",
            // Other first bytes: no generic INI.
            "x=1\n[A]\nk=v\n[B]\nk=v\n",
            "#[A]\nk=v\n[B]\nk=v\n",
            "# x\n[A]\nk=v\n[B]\nk=v\n",
            "hello\n[A]\nk=v\n[B]\nk=v\n",
            "\n[A]\nk=v\n[B]\nk=v\n",
            "  ;[A]\nk=v\n[B]\nk=v\n",
            "]\n[A]\nk=v\n[B]\nk=v\n",
            "=[A]\nk=v\n[B]\nk=v\n",
            "[A]\nk=v\n\n[B]",
            // A `;` text: the second `[` anywhere decides, even in a comment.
            ";c\n[x] junk\n[A]\nk=v\n[B]\nk=v\n",
            ";c\n; [note]\n[A]\nk=v\n[B]\nk=v\n",
            ";c\n[see here\n[A]\nk=v\n[B]\nk=v\n",
            ";c\n; - the [Top] section\n[A]\nk=v\n[B]\nk=v\n",
            ";c\n[A]\n; - the [Top] section\nk=v\n[B]\nk=v\n",
            ";c\n[A] x\n[B]\nk=v\n",
            ";c\n[A]\n; x\n[B]\nk=v\n",
            ";c\n[A]\nk=v\n",
            ";c\n[A]\n[B]\n",
            ";c\n[A]\n[B]\nk=v",
            "[A]\n; [note]\n[B]\nk=v\n",
            // Plain text, and the usual text controls.
            "plain text\n",
            "a=1\nb=2\n",
            "ab\n",
            "a",
            "[A]",
            "[A]\nk=v\x1b\n",
            "[A]\nk=v\x08\n",
            "[A]\nk=v\x0b\n",
            "[A]\nk=v\x0c\n",
            "[A]\nk=\x01\n[B]\nk=v\n",
            "[A]\nk=v\x00\n[B]\nk=v\n",
            "[A]\nk=\x7f\n[B]\nk=v\n",
            "[A]\nk=v\n[B]\nk=v\x7f\n",
            "[A]\nk=v\n[B]\nk=v\n\x7f",
        ]
        .into_iter()
        .map(|text| text.as_bytes().to_vec())
        .collect();
        // The search window: the header's `[` must start at or before byte 8193.
        for start in [8190, 8192, 8193, 8194, 8200] {
            let mut bytes = b"[A]\n".to_vec();
            while bytes.len() + 4 <= start {
                bytes.extend_from_slice(b"k=v\n");
            }
            bytes.resize(start, b'x');
            bytes.extend_from_slice(b"[B]\nk=v\n");
            fixtures.push(bytes);
        }
        fixtures
    }

    /// Whether the host `file(1)` is 5.47 or newer. Older releases describe
    /// some fixtures differently (no INI detection before 5.44, and `\x1a`
    /// reads as `data`), so the differential tests need at least the version
    /// lintian ran with. CI's ubuntu-latest ships 5.45.
    fn host_file_is_recent_enough() -> bool {
        let Ok(output) = std::process::Command::new("file").arg("--version").output() else {
            return false;
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version: Vec<u32> = stdout
            .lines()
            .next()
            .and_then(|line| line.trim().strip_prefix("file-"))
            .map(|version| {
                version
                    .split('.')
                    .map_while(|part| part.parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        version >= vec![5, 47]
    }

    /// `file -b` over the fixtures, one description per fixture, or `None` when
    /// `file(1)` is not installed or older than 5.47.
    fn file_descriptions(fixtures: &[Vec<u8>]) -> Option<Vec<String>> {
        if !host_file_is_recent_enough() {
            return None;
        }
        let dir = tempfile::tempdir().ok()?;
        let paths: Vec<_> = fixtures
            .iter()
            .enumerate()
            .map(|(index, bytes)| {
                let path = dir.path().join(format!("fixture-{index}"));
                std::fs::write(&path, bytes).expect("write fixture");
                path
            })
            .collect();
        let output = std::process::Command::new("file")
            .arg("-b")
            .arg("--")
            .args(&paths)
            .output()
            .ok()?;
        assert!(output.status.success(), "file -b failed");
        let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect();
        assert_eq!(lines.len(), fixtures.len(), "one description per fixture");
        Some(lines)
    }

    #[test]
    fn ini_and_ascii_text_agree_with_file_on_generated_fixtures() {
        let fixtures = ini_fixtures();
        let Some(descriptions) = file_descriptions(&fixtures) else {
            eprintln!("file(1) 5.47 or newer is not installed; skipping the differential test");
            return;
        };
        for (bytes, description) in fixtures.iter().zip(&descriptions) {
            let ini = description.starts_with("Generic INItialization configuration");
            assert_eq!(
                is_ini_config(bytes),
                ini,
                "INI for {:?}: file says {description:?}",
                String::from_utf8_lossy(bytes)
            );
            let ascii = description.contains("ASCII text");
            assert_eq!(
                is_ascii_text(bytes),
                ascii,
                "ASCII text for {:?}: file says {description:?}",
                String::from_utf8_lossy(bytes)
            );
        }
    }

    /// Generated files for [`text_kind`]: every byte value in two contexts,
    /// the 64 KiB window edge, UTF-8 and UTF-16 forms, and the magics that name
    /// a text file otherwise (PostScript, OpenSSH keys, JSON, SVG, XPM, Netpbm).
    fn text_fixtures() -> Vec<Vec<u8>> {
        let mut fixtures: Vec<Vec<u8>> = Vec::new();
        for byte in 0..=255u8 {
            fixtures.push([b"ab".as_slice(), &[byte], b"cd\n"].concat());
            fixtures.push([b"x=".as_slice(), &[byte], b" au lait\n"].concat());
        }
        for byte in [0x01, 0x07, 0x1a, 0x1f, 0x85, 0x93, 0xe9] {
            let mut inside = vec![b'x'; 65535];
            inside.extend([byte, b'\n', b'a']);
            fixtures.push(inside);
            let mut outside = vec![b'x'; 65536];
            outside.extend([byte, b'\n']);
            fixtures.push(outside);
        }
        fixtures.push([vec![b'x'; 65535], vec![0xc3, 0xa9, b'\n']].concat());
        fixtures.push([vec![b'x'; 65535], vec![0xc3, b'\n']].concat());
        fixtures.push([vec![b'x'; 65535], vec![0xc3, 0xa9, b'\n', 0x93]].concat());
        fixtures.push(vec![b'x'; 70_000]);
        // `file(1)` names a Rust source by its inner attribute, with no text.
        fixtures.push(b"#![no_std]\nfn main() {}\n".to_vec());
        fixtures.push(b"#![cfg(feature = \"x\")]\n".to_vec());
        fixtures.push(b"// comment\n#![no_std]\n".to_vec());
        for text in [
            "a",
            "",
            "\u{80}",
            "caf\u{e9}\n",
            "\u{200b}x\n",
            "\u{fffd}x\n",
            "\u{10ffff}\n",
            "\u{feff}x\n",
            "\"str\"\n",
            "123\n",
            "true\n",
            "null\n",
            "[1, 2, 3]\n",
            "[1,\n",
            "{\"a\":1} x\n",
            "{a: 1}\n",
            "{}\n",
            "[]\n",
            "{\n  \"a\": 1\n}\n",
            "{\"a\": 1,}\n",
            "[1,2,]\n",
            "[01]\n",
            "[NaN]\n",
            "[,]\n",
            "[1,,2]\n",
            "{1:2}\n",
            "{\"a\\q\": 1}\n",
            "{\"a\": \"\\n\\u00e9\"}\n",
            "{\"a\": \"x\u{1}y\"}\n",
            "{\"a\": 1}\n{\"a\": 2}\n",
            "{\"a\": 1}\n[2]\n",
            "{}\n{}\n[]\n",
            "[1]\n{}\n",
            "{}\n\"x\"\n",
            "{}{}\n",
            "{\"a\": 1}\n\n{\"a\": 2}\n",
            "   \n",
            "# c\n{\"a\":1}\n",
            "\u{feff}{\"a\": 1}\n",
            "-5e+3\n",
            "%!PS-Adobe-3.0\nx\n",
            "%!FontType1-1.0: X\nx\n",
            "ssh-rsa AAAA user\n",
            "ssh-rsa \n",
            "ssh-rsa\tAAAA\n",
            "ssh-rsaAAAA\n",
            "ssh-dss AAAA\n",
            "ssh-dss-cert-v01@openssh.com AAAA\n",
            "ssh-ed25519\n",
            "ssh-ed25519-cert-v01@openssh.com AAAA\n",
            "ecdsa-sha2-nistp256 AAAA\n",
            "ecdsa-sha2-nistp255 AAAA\n",
            "sk-ssh-ed25519@openssh.com AAAA\n",
            "-----BEGIN OPENSSH PRIVATE KEY-----\nx\n",
            "/* XPM */\nx\n",
            "P1\n1 1\n0\n",
            "P3\n1 1\n255\n0 0 0\n",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n",
            "<?xml version=\"1.0\"?>\n<doc><svg/></doc>\n",
            "<?xml version=\"1.0\"?>\n<doc/>\n",
            "{\\rtf1 hi}\n",
            "SplineFontDB: 3.0\nx\n",
            "[A]\nk=v\n[B]\nk=v\n",
            "\u{feff}hello\n",
            "#!/usr/bin/perl\n",
            "#!/usr/bin/perl -w\n",
            "#! /usr/bin/perl\n",
            "#!/usr/bin/env perl\n",
            "#!/usr/bin/env python3\n",
            "#!/usr/bin/perl",
            "#!/usr/bin/perl5.36\n",
            "#!perl\n",
            "#!/opt/perl/bin/perl\n",
            "#define a_width 16\n#define a_height 16\n",
            "#define foo 1\n",
            "%!TEX root = x.tex\n",
            "From the man page:\n",
            "From x\n",
            "From  x\n",
            "From - Fri\n",
            "From x",
            "From\n",
            "From: me\n",
            "x\nFrom the\n",
            "from x\n",
            "#LyX 2.5 created this file\n",
            "#LyX\n",
            "abc\n\u{0}\u{0}",
            "caf\u{e9}\n\u{0}\u{0}",
            "caf\u{e9}\n\u{0}x\u{0}",
            "a\u{0}b\u{0}\u{0}",
            "\u{0}\u{0}",
            "%!TEX program = pdflatex\n",
            "%!TEX  root\n",
            "#!./perl -w\n",
            "% c\n",
            "divert(-1)\n",
            "divert(-1) \n",
            "divert(0)\n",
            "-----BEGIN CERTIFICATE-----\nx\n",
            "-----BEGIN X509 CERTIFICATE-----\nx\n",
            "-----BEGIN CERTIFICATE REQUEST-----\nx\n",
            "-----BEGIN NEW CERTIFICATE REQUEST-----\nx\n",
            "-----BEGIN PUBLIC KEY-----\nx\n",
            "-----BEGIN PRIVATE KEY-----\nx\n",
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\nx\n",
            "-----BEGIN PGP PUBLIC KEY BLOCK-----\nx\n",
            "-----BEGIN TRUSTED CERTIFICATE-----\nx\n",
            "-----BEGIN PKCS7-----\nx\n",
            "-----BEGIN FOO-----\nx\n",
            ";c\n[A]\n[B]\n",
            ";c\n[A]\nk=v\n[B]\nk=v\n",
            "; c\r\n[A]\r\nk=v\r\n[B]\r\nk=v\r\n",
            "#c\n[A]\nk=v\n[B]\nk=v\n",
        ] {
            fixtures.push(text.as_bytes().to_vec());
        }
        fixtures.push(
            "hello\n"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect(),
        );
        fixtures.push(
            [
                &[0xff, 0xfe][..],
                &"hello\n"
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<u8>>(),
            ]
            .concat(),
        );
        fixtures.push(
            [
                &[0xff, 0xfe][..],
                &"h\u{e9}llo\n"
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<u8>>(),
            ]
            .concat(),
        );
        fixtures.push([&[0xff, 0xfe][..], &[0x00, 0xd8, b'a', 0x00]].concat());
        fixtures.push(
            [
                &[0xfe, 0xff][..],
                &"hello\n"
                    .encode_utf16()
                    .flat_map(u16::to_be_bytes)
                    .collect::<Vec<u8>>(),
            ]
            .concat(),
        );
        fixtures
    }

    /// `text_kind` against `file -b` (file 5.47, the version lintian ran with)
    /// on [`text_fixtures`]. lintian's `(ASCII|Unicode) text` and
    /// `\bASCII text\b` are substring tests, so `Non-ISO extended-ASCII text`
    /// counts for both. Skipped when `file(1)` is not installed or older than 5.47.
    #[test]
    fn text_kind_agrees_with_file_on_generated_fixtures() {
        let fixtures = text_fixtures();
        let Some(descriptions) = file_descriptions(&fixtures) else {
            eprintln!("file(1) 5.47 or newer is not installed; skipping the differential test");
            return;
        };
        for (bytes, description) in fixtures.iter().zip(&descriptions) {
            let shown = String::from_utf8_lossy(&bytes[..bytes.len().min(40)]).into_owned();
            let text = description.contains("ASCII text") || description.contains("Unicode text");
            assert_eq!(
                is_text(bytes),
                text,
                "text for {shown:?}: file says {description:?}"
            );
            assert_eq!(
                is_ascii_text(bytes),
                description.contains("ASCII text"),
                "ASCII text for {shown:?}: file says {description:?}"
            );
        }
    }

    #[test]
    fn only_raster_and_vector_formats_are_images() {
        assert!(ContentType::Png.is_image());
        assert!(ContentType::Bmp.is_image());
        assert!(!ContentType::Pdf.is_image());
        assert!(!ContentType::PostScript.is_image());
        assert!(!ContentType::Matlab { major: 5 }.is_image());
    }
}
