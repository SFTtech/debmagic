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
    /// `/* XPM */` text.
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
        if bytes.starts_with(b"/* XPM */") {
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

/// Whether `file(1)` would call `bytes` `ASCII text` (lintian's
/// `\bASCII text\b`): non-empty, every byte printable ASCII or a usual text
/// control, and not a generic INI configuration (see [`is_ini_config`]). A
/// single byte is `very short file (no magic)` and an empty file is `empty`,
/// so neither is. Valid UTF-8 with non-ASCII bytes is `UTF-8 Unicode text` to
/// `file(1)`, so it is not ASCII text either.
pub(crate) fn is_ascii_text(bytes: &[u8]) -> bool {
    bytes.len() > 1 && is_text_bytes(bytes) && !is_ini_config(bytes)
}

/// Whether every byte is printable ASCII or one of the usual text controls,
/// as `file(1)` accepts in ASCII text.
fn is_text_bytes(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|&byte| matches!(byte, 0x08 | 0x09..=0x0d | 0x1b | 0x20..=0x7e))
}

/// Whether `file(1)` 5.47 names `bytes` `Generic INItialization configuration`
/// (its `ini` magic rule), which systemd units and many other `[Section]`
/// files get instead of `ASCII text`. The rule is probed from `file`'s output,
/// not copied from its magic source, and it is narrower than `file`'s:
///
/// - The text starts with `[`, and the first `[` after its first byte opens a
///   section header whose `[` is at or before byte 8193. A header is `[`, a
///   name of ASCII letters, digits, spaces, `_`, `(` or `)`, then `]`, then
///   `\r\n`, or `\r` or `\n` with at least one byte after it.
/// - A text that starts with `;` (a comment in some INI files) is not
///   modelled: `file(1)` names some of those INI too, and no `;` rule matched
///   every probed file.
///
/// Other rules that `file(1)` applies first (JSON, for one) are not modelled.
pub(crate) fn is_ini_config(bytes: &[u8]) -> bool {
    if bytes.first() != Some(&b'[') {
        return false;
    }
    let window = &bytes[1..bytes.len().min(INI_HEADER_LAST_START + 1)];
    window
        .iter()
        .position(|&byte| byte == b'[')
        .is_some_and(|offset| is_ini_header(&bytes[1 + offset..]))
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
        assert_eq!(ContentType::sniff(b"/* XPM */\n"), Some(ContentType::Xpm));
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

    /// `file -b` over the fixtures, one description per fixture, or `None` when
    /// `file(1)` is not installed.
    fn file_descriptions(fixtures: &[Vec<u8>]) -> Option<Vec<String>> {
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
            eprintln!("file(1) is not installed; skipping the differential test");
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

    #[test]
    fn only_raster_and_vector_formats_are_images() {
        assert!(ContentType::Png.is_image());
        assert!(ContentType::Bmp.is_image());
        assert!(!ContentType::Pdf.is_image());
        assert!(!ContentType::PostScript.is_image());
        assert!(!ContentType::Matlab { major: 5 }.is_image());
    }
}
