//! Content type of a file from its leading bytes, for the formats lint Rules
//! tell apart: the image formats of lintian's `images/filenames` Check, Matlab
//! v4 to v7.3 files, PDF and PostScript. No `file(1)` is run (ADR 0041); a
//! file whose bytes are none of these has no content type.

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
    fn only_raster_and_vector_formats_are_images() {
        assert!(ContentType::Png.is_image());
        assert!(ContentType::Bmp.is_image());
        assert!(!ContentType::Pdf.is_image());
        assert!(!ContentType::PostScript.is_image());
        assert!(!ContentType::Matlab { major: 5 }.is_image());
    }
}
