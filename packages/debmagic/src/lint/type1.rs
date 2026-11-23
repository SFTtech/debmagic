//! Type 1 font programs read from Entry bytes, as lintian's
//! `Lintian::Check::Fonts::Postscript::Type1` reads them through
//! `t1disasm`.
//!
//! The Check matches lines of the disassembled text: the cleartext of the
//! font, then the `eexec` encrypted part decrypted. This module does that
//! decryption itself (no `t1disasm`): the PFA form (`currentfile eexec`
//! followed by binary or hex digits) and the PFB form (segments `0x80 0x01`
//! ASCII, `0x80 0x02` binary, `0x80 0x03` end). Only the `eexec` layer is
//! decrypted; charstrings keep their own encryption, as the Tags do not look
//! into them.
//!
//! [`Type1Font::read`] keeps a parse error on the object: the text up to the
//! damage is still there for a Rule to use.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::content_type::is_type1_font;
use super::elf::entry_bytes;
use super::file_tree::{EntryKind, FileTree};

/// The `eexec` key, the initial value of the cipher state `r`.
const EEXEC_KEY: u16 = 55665;
const C1: u16 = 52845;
const C2: u16 = 22719;
/// The random bytes that open every `eexec` plaintext; dropped.
const EEXEC_PREFIX: usize = 4;

/// Decrypt an `eexec` section (the Type 1 specification's cipher, key 55665),
/// dropping its four leading bytes.
fn eexec_decrypt(cipher: &[u8]) -> Vec<u8> {
    let mut r = EEXEC_KEY;
    let mut plain = Vec::with_capacity(cipher.len());
    for &c in cipher {
        plain.push(c ^ (r >> 8) as u8);
        r = (u16::from(c))
            .wrapping_add(r)
            .wrapping_mul(C1)
            .wrapping_add(C2);
    }
    plain.drain(..EEXEC_PREFIX.min(plain.len()));
    plain
}

/// A Type 1 font program, as the text lintian's `t1disasm` step shows: the
/// cleartext followed by the decrypted `eexec` part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type1Font {
    text: String,
    error: Option<String>,
}

/// The fragment of a non-free Adobe license the `fonts/postscript/type1`
/// Check reports, one per font.
#[allow(dead_code)] // seam API; no Rule reads it yet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdobeFragment {
    /// `Copyright (c) 19xx-19xx Adobe Systems Incorporated.` followed by an
    /// `All Rights Reserved.` line: the Tag without `no-credit`.
    CopyrightedFragment,
    /// `UniqueID 6859` with no Adobe copyright before it: the `no-credit`
    /// Tag.
    NoCredit,
}

impl Type1Font {
    /// The font in `bytes`, or `None` when they do not start like a Type 1
    /// font ([`is_type1_font`]).
    pub fn read(bytes: &[u8]) -> Option<Self> {
        if !is_type1_font(bytes) {
            return None;
        }
        let (clear, encrypted, error) = if bytes.first() == Some(&0x80) {
            read_pfb(bytes)
        } else {
            read_pfa(bytes)
        };
        let mut text = clear;
        text.extend(eexec_decrypt(&encrypted));
        Some(Self {
            text: String::from_utf8_lossy(&text).into_owned(),
            error,
        })
    }

    /// The disassembled text: the cleartext, then the decrypted `eexec`
    /// part. Non-UTF-8 bytes are replaced.
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Why the font could not be read in full (a truncated PFB segment, bad
    /// hex digits in the PFA `eexec` part). The text holds what was read.
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The first Adobe fragment that lintian's Check finds, as its line scan
    /// finds it: the scan stops at the first line that completes either
    /// pattern, so a font has at most one of the two.
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn adobe_fragment(&self) -> Option<AdobeFragment> {
        static COPYRIGHT: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(
                r"(?i)\A[%\s]*Copyright\s*\(c\)\s*19\d{2}[-\s]19\d{2}\s*Adobe\s*Systems\s*Incorporated\.?\s*\z",
            )
            .expect("copyright regex")
        });
        static ALL_RIGHTS: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?i)\A[%\s]*All\s*Rights\s*Reserved\.?\s*\z").expect("all rights regex")
        });
        static UNIQUE_ID: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"UniqueID\s*6859").expect("UniqueID regex"));

        let mut after_copyright = false;
        for line in self.text.split('\n') {
            if after_copyright && ALL_RIGHTS.is_match(line) {
                return Some(AdobeFragment::CopyrightedFragment);
            }
            if COPYRIGHT.is_match(line) {
                after_copyright = true;
            }
            if UNIQUE_ID.is_match(line) {
                return Some(AdobeFragment::NoCredit);
            }
        }
        None
    }
}

/// The cleartext, the encrypted bytes, and the parse error of a PFB file.
fn read_pfb(bytes: &[u8]) -> (Vec<u8>, Vec<u8>, Option<String>) {
    let mut clear = Vec::new();
    let mut encrypted = Vec::new();
    let mut rest = bytes;
    while rest.len() >= 2 && rest[0] == 0x80 && rest[1] != 3 {
        let Some(header) = rest.get(2..6) else {
            return (
                clear,
                encrypted,
                Some("truncated PFB segment header".into()),
            );
        };
        let len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let body = &rest[6..];
        let Some(data) = body.get(..len) else {
            return (clear, encrypted, Some("truncated PFB segment".into()));
        };
        match rest[1] {
            1 => clear.extend_from_slice(data),
            2 => encrypted.extend_from_slice(data),
            _ => {}
        }
        rest = &body[len..];
    }
    let error = (rest.len() >= 2 && rest[0] != 0x80).then(|| "bad PFB segment header".to_string());
    (clear, encrypted, error)
}

/// The cleartext up to and including `eexec`, and the bytes after it
/// (hex-decoded when they start with four hex digits), with the parse error.
fn read_pfa(bytes: &[u8]) -> (Vec<u8>, Vec<u8>, Option<String>) {
    let Some(at) = bytes.windows(5).position(|window| window == b"eexec") else {
        return (bytes.to_vec(), Vec::new(), None);
    };
    let after = &bytes[at + 5..];
    let blank = after.len() - after.trim_ascii_start().len();
    let clear = bytes[..at + 5 + blank].to_vec();
    let rest = after.trim_ascii_start();
    let is_hex = rest.len() >= 4 && rest[..4].iter().all(u8::is_ascii_hexdigit);
    if !is_hex {
        return (clear, rest.to_vec(), None);
    }
    let mut decoded = Vec::with_capacity(rest.len() / 2);
    let mut pending: Option<u8> = None;
    for &byte in rest.iter().filter(|byte| !byte.is_ascii_whitespace()) {
        let Some(digit) = (byte as char).to_digit(16) else {
            return (
                clear,
                decoded,
                Some("bad hex digit in eexec section".into()),
            );
        };
        match pending.take() {
            None => pending = Some(digit as u8),
            Some(high) => decoded.push(high << 4 | digit as u8),
        }
    }
    let error = pending.map(|_| "odd number of hex digits in eexec section".to_string());
    (clear, decoded, error)
}

/// The Type 1 fonts among the regular files and hardlinks of a File tree,
/// by path.
#[derive(Debug, Default)]
pub struct Type1Fonts {
    fonts: BTreeMap<PathBuf, Type1Font>,
}

impl Type1Fonts {
    /// Entries whose bytes start like a Type 1 font. Entries that cannot be
    /// read are left out.
    pub fn from_tree(files: &FileTree) -> Self {
        let fonts = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter_map(|entry| {
                let font = Type1Font::read(&entry_bytes(files, entry)?)?;
                Some((entry.path().to_path_buf(), font))
            })
            .collect();
        Self { fonts }
    }

    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn get(&self, path: impl AsRef<Path>) -> Option<&Type1Font> {
        self.fonts.get(path.as_ref())
    }

    /// In path order.
    #[allow(dead_code)] // seam API; no Rule reads it yet
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &Type1Font)> {
        self.fonts.iter().map(|(path, font)| (path.as_path(), font))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::file_tree::Entry;

    /// The `eexec` cipher in the other direction, for building fonts.
    fn eexec_encrypt(plain: &[u8]) -> Vec<u8> {
        let mut r = EEXEC_KEY;
        let mut cipher = Vec::new();
        for &p in [0u8; EEXEC_PREFIX].iter().chain(plain) {
            let c = p ^ (r >> 8) as u8;
            r = (u16::from(c))
                .wrapping_add(r)
                .wrapping_mul(C1)
                .wrapping_add(C2);
            cipher.push(c);
        }
        cipher
    }

    const CLEAR: &[u8] = b"%!FontType1-1.0: Test\n/FontName /Test def\ncurrentfile eexec\n";
    const PRIVATE: &[u8] = b"dup /Private 8 dict dup begin\n/UniqueID 6859 def\nend\n";

    fn pfa(encrypted: &[u8]) -> Vec<u8> {
        [CLEAR, encrypted].concat()
    }

    fn pfb_segment(kind: u8, data: &[u8]) -> Vec<u8> {
        let mut segment = vec![0x80, kind];
        segment.extend_from_slice(&(data.len() as u32).to_le_bytes());
        segment.extend_from_slice(data);
        segment
    }

    #[test]
    fn eexec_round_trips_and_drops_the_prefix() {
        assert_eq!(eexec_decrypt(&eexec_encrypt(PRIVATE)), PRIVATE);
        assert_eq!(eexec_decrypt(&eexec_encrypt(b"")), b"");
    }

    #[test]
    fn reads_a_pfa_with_binary_eexec_section() {
        let font = Type1Font::read(&pfa(&eexec_encrypt(PRIVATE))).expect("type 1");
        assert_eq!(font.error(), None);
        assert_eq!(font.text().as_bytes(), [CLEAR, PRIVATE].concat().as_slice());
    }

    #[test]
    fn reads_a_pfa_with_hex_eexec_section() {
        let hex: String = eexec_encrypt(PRIVATE)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .chunks(32)
            .map(|line| line.concat())
            .collect::<Vec<_>>()
            .join("\r\n");
        let font = Type1Font::read(&pfa(hex.as_bytes())).expect("type 1");
        assert_eq!(font.error(), None);
        assert!(font.text().ends_with("/UniqueID 6859 def\nend\n"));
    }

    #[test]
    fn reads_a_pfb_with_ascii_and_binary_segments() {
        let encrypted = eexec_encrypt(PRIVATE);
        let pfb = [
            pfb_segment(1, CLEAR),
            pfb_segment(2, &encrypted),
            vec![0x80, 3],
        ]
        .concat();
        let font = Type1Font::read(&pfb).expect("type 1");
        assert_eq!(font.error(), None);
        assert_eq!(font.text().as_bytes(), [CLEAR, PRIVATE].concat().as_slice());
    }

    #[test]
    fn a_truncated_pfb_segment_keeps_the_text_read_so_far() {
        let mut pfb = pfb_segment(1, CLEAR);
        pfb.extend_from_slice(&[0x80, 2, 0xff, 0, 0, 0, 1, 2]);
        let font = Type1Font::read(&pfb).expect("type 1");
        assert_eq!(font.error(), Some("truncated PFB segment"));
        assert_eq!(font.text().as_bytes(), CLEAR);
    }

    #[test]
    fn bad_hex_digits_keep_the_cleartext() {
        let font = Type1Font::read(&pfa(b"0011zz 11")).expect("type 1");
        assert_eq!(font.error(), Some("bad hex digit in eexec section"));
        assert_eq!(font.text().as_bytes(), CLEAR);
    }

    #[test]
    fn a_font_without_eexec_is_its_whole_text() {
        let text =
            b"%!PS-AdobeFont-1.0: X\n% Copyright (c) 1987-1990 Adobe Systems Incorporated.\n";
        let font = Type1Font::read(text).expect("type 1");
        assert_eq!(font.error(), None);
        assert_eq!(font.text().as_bytes(), text);
    }

    #[test]
    fn only_type_1_headers_are_fonts() {
        assert!(Type1Font::read(b"%!PS-Adobe-3.0 EPSF-3.0\n").is_none());
        assert!(Type1Font::read(b"plain text").is_none());
        assert!(Type1Font::read(b"%!FontType1-1.0\n").is_some());
    }

    fn font_text(lines: &str) -> Type1Font {
        Type1Font::read(format!("%!FontType1-1.0\n{lines}").as_bytes()).expect("type 1")
    }

    #[test]
    fn adobe_copyright_then_all_rights_is_the_copyrighted_fragment() {
        let font = font_text(
            "% Copyright (c) 1987-1990 Adobe Systems Incorporated.\n% All Rights Reserved.\n",
        );
        assert_eq!(
            font.adobe_fragment(),
            Some(AdobeFragment::CopyrightedFragment)
        );
    }

    #[test]
    fn uniqueid_is_the_no_credit_fragment() {
        let font = font_text("trick { /UniqueID 6859 } def\n");
        assert_eq!(font.adobe_fragment(), Some(AdobeFragment::NoCredit));
    }

    #[test]
    fn a_later_all_rights_line_still_completes_the_fragment() {
        // lintian's scan keeps the copyright flag over the lines in between.
        let font = font_text(
            "% Copyright (c) 1987-1990 Adobe Systems Incorporated.\n% Other.\n% All Rights Reserved.\n",
        );
        assert_eq!(
            font.adobe_fragment(),
            Some(AdobeFragment::CopyrightedFragment)
        );
    }

    #[test]
    fn all_rights_without_a_copyright_line_is_not_a_fragment() {
        let font = font_text("% All Rights Reserved.\n");
        assert_eq!(font.adobe_fragment(), None);
    }

    #[test]
    fn the_first_completing_line_decides() {
        let font = font_text(
            "/UniqueID 6859 def\n% Copyright (c) 1987-1990 Adobe Systems Incorporated.\n% All Rights Reserved.\n",
        );
        assert_eq!(font.adobe_fragment(), Some(AdobeFragment::NoCredit));
    }

    #[test]
    fn fonts_index_regular_files_and_hardlinks_only() {
        let tree = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("usr/share/fonts/a.pfa", None, Some(CLEAR.to_vec())),
                Entry::hardlink("usr/share/fonts/b.pfa", None, Some(CLEAR.to_vec())),
                Entry::regular_file("usr/share/doc/README", None, Some(b"text".to_vec())),
                Entry::symlink("usr/share/fonts/c.pfa", "a.pfa"),
            ],
        );
        let fonts = Type1Fonts::from_tree(&tree);
        let paths: Vec<&Path> = fonts.iter().map(|(path, _)| path).collect();
        assert_eq!(
            paths,
            [
                Path::new("usr/share/fonts/a.pfa"),
                Path::new("usr/share/fonts/b.pfa"),
            ]
        );
    }
}
