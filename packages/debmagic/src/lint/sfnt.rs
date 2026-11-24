//! sfnt (TrueType / OpenType) font metadata read from Entry bytes.
//!
//! Two independent views of the same bytes, each pinned to the tool lintian
//! 2.139.0 uses for it:
//!
//! - [`SfntMagic`] is which font check lintian selects a file for. Lintian
//!   selects on the `file(1)` type string (`^TrueType Font data`,
//!   `^OpenType font data`), not on the file name, so this replicates the
//!   sfnt entries of `file` 5.47's `Magdir/fonts`.
//! - [`Sfnt`] is what `Font::TTF::Font->open` (libfont-ttf-perl 1.06) reads:
//!   the table directory and the OS/2 `fsType` field.

/// The sfnt font kind `file(1)` reports at the start of its type string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfntMagic {
    /// `TrueType Font data`: sfnt version `00 01 00 00`, `true`, or `typ1`,
    /// fewer than 48 tables, and a plausible first table tag.
    TrueType,
    /// `OpenType font data`: the bytes start with `OTTO`.
    OpenType,
}

/// Where `file`'s `regex/4l` table-tag test starts (the first table record).
const TABLE_TAG_OFFSET: usize = 12;
/// `regex/4l`: four lines, which `file` turns into a window of 4 × 80 bytes.
const REGEX_LINES: usize = 4;
const REGEX_LINE_BYTES: usize = 80;

impl SfntMagic {
    /// Classify bytes as `file` 5.47 does for sfnt fonts.
    ///
    /// Only the font entries are replicated, not the rest of the magic
    /// database: a file another, stronger magic entry claims first would not
    /// be a font for `file` but is one here.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"OTTO") {
            return Some(Self::OpenType);
        }
        let version = bytes.get(..4)?;
        if version != b"\x00\x01\x00\x00" && version != b"true" && version != b"typ1" {
            return None;
        }
        // `>4 ubeshort <48`
        if read_u16(bytes, 4)? >= 48 {
            return None;
        }
        // `>>12 regex/4l \^[A-Za-z][A-Za-z2][A-Za-z/][A-Za-z2\ ]`; the version
        // is never `OTTO` here, so `file` prints `TrueType` + ` Font data`.
        if !first_tag_is_plausible(bytes) {
            return None;
        }
        Some(Self::TrueType)
    }
}

/// `file`'s `regex/4l` test at offset 12, including how it cuts the search
/// window (softmagic.c `mcopy` and `magiccheck`, `FILE_REGEX`): at most 320
/// bytes, ended after the fourth line terminator, minus its last byte, and
/// searched as a C string (up to the first NUL) with `REG_NEWLINE`.
fn first_tag_is_plausible(bytes: &[u8]) -> bool {
    let Some(rest) = bytes.get(TABLE_TAG_OFFSET..) else {
        return false;
    };
    let window = &rest[..rest.len().min(REGEX_LINES * REGEX_LINE_BYTES)];
    let window = &window[..regex_window_end(window)];
    // The copy's last byte becomes the terminating NUL.
    let Some((_, searched)) = window.split_last() else {
        return false;
    };
    let searched = match searched.iter().position(|&byte| byte == 0) {
        Some(nul) => &searched[..nul],
        None => searched,
    };
    (0..searched.len()).any(|start| {
        (start == 0 || searched[start - 1] == b'\n')
            && searched.get(start..start + 4).is_some_and(is_plausible_tag)
    })
}

/// `file`'s line-count loop over the window, quirks included: a `\r` counts
/// only when no `\n` is left, and each search resumes one byte past the start
/// of the next line. With fewer than four terminators the whole window stays.
fn regex_window_end(window: &[u8]) -> usize {
    let end = window.len();
    let mut last = end;
    let mut lines = REGEX_LINES;
    let mut from = 0;
    while lines > 0 && from < end {
        let rest = &window[from..];
        let Some(found) = rest
            .iter()
            .position(|&byte| byte == b'\n')
            .or_else(|| rest.iter().position(|&byte| byte == b'\r'))
        else {
            break;
        };
        let mut at = from + found;
        if at + 1 < end && window[at] == b'\r' && window[at + 1] == b'\n' {
            at += 1;
        }
        if at + 1 < end && window[at] == b'\n' {
            at += 1;
        }
        last = at;
        lines -= 1;
        from = at + 1;
    }
    if lines > 0 { end } else { last }
}

/// `[A-Za-z][A-Za-z2][A-Za-z/][A-Za-z2 ]`
fn is_plausible_tag(tag: &[u8]) -> bool {
    matches!(
        tag,
        [a, b, c, d]
            if a.is_ascii_alphabetic()
                && (b.is_ascii_alphabetic() || *b == b'2')
                && (c.is_ascii_alphabetic() || *c == b'/')
                && (d.is_ascii_alphabetic() || *d == b'2' || *d == b' ')
    )
}

/// An sfnt font as `Font::TTF::Font->open` reads it: the table directory,
/// keeping what Rules need from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sfnt {
    fs_type: Option<u16>,
}

/// Why `Font::TTF` would not open the bytes as a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfntError {
    /// The sfnt version is not `00 01 00 00`, `OTTO`, or `true`
    /// (`Font::TTF` returns no font; `typ1` is not accepted).
    UnknownVersion,
    /// The table directory ends inside a table record (`Font::TTF` dies on
    /// an empty read and reads garbage from a short one).
    TruncatedTableDirectory,
}

const OS2_TAG: &[u8; 4] = b"OS/2";
const TABLE_RECORD_SIZE: usize = 16;
/// OS/2 versions `Font::TTF` 1.06 has a field layout for (`@lens`).
const OS2_KNOWN_VERSIONS: u16 = 5;
/// `fsType` follows `version`, `xAvgCharWidth`, `usWeightClass`, and
/// `usWidthClass`.
const OS2_FS_TYPE_OFFSET: usize = 8;

impl Sfnt {
    /// Read the table directory. WOFF is not supported (`Font::TTF` reads it;
    /// neither `file` magic Rules select on matches it).
    pub fn parse(bytes: &[u8]) -> Result<Self, SfntError> {
        let version = bytes.get(..4).ok_or(SfntError::UnknownVersion)?;
        if version != b"\x00\x01\x00\x00" && version != b"OTTO" && version != b"true" {
            return Err(SfntError::UnknownVersion);
        }
        // A missing count reads as no tables.
        let num_tables = usize::from(read_u16(bytes, 4).unwrap_or(0));
        let mut os2_offset = None;
        for index in 0..num_tables {
            let start = 12 + index * TABLE_RECORD_SIZE;
            let record = bytes
                .get(start..start + TABLE_RECORD_SIZE)
                .ok_or(SfntError::TruncatedTableDirectory)?;
            // Records are keyed by tag; a later duplicate replaces an earlier one.
            if &record[..4] == OS2_TAG {
                os2_offset = Some(read_u32(record, 8).expect("record has 16 bytes"));
            }
        }
        let fs_type = os2_offset.and_then(|offset| os2_fs_type(bytes, offset));
        Ok(Self { fs_type })
    }

    /// OS/2 `fsType` (embedding permissions), as `Font::TTF` reads it: from
    /// the OS/2 table's offset in the file, regardless of the table length
    /// in the directory. `None` without an OS/2 table, when the file ends
    /// before the field, or when the OS/2 version is 5 or later
    /// (`Font::TTF` 1.06 reads no fields for those).
    pub fn fs_type(&self) -> Option<u16> {
        self.fs_type
    }
}

fn os2_fs_type(bytes: &[u8], offset: u32) -> Option<u16> {
    let offset = usize::try_from(offset).ok()?;
    let fs_type = read_u16(bytes, offset.checked_add(OS2_FS_TYPE_OFFSET)?)?;
    if read_u16(bytes, offset)? >= OS2_KNOWN_VERSIONS {
        return None;
    }
    Some(fs_type)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let raw = bytes.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_be_bytes([raw[0], raw[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let raw = bytes.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

#[cfg(test)]
pub(crate) mod testing {
    /// A minimal sfnt font: `version`, then one table record per `(tag,
    /// table)` with the tables laid out after the directory.
    pub(crate) fn sfnt(version: &[u8; 4], tables: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let num_tables = u16::try_from(tables.len()).expect("few tables");
        let mut bytes = version.to_vec();
        bytes.extend_from_slice(&num_tables.to_be_bytes());
        bytes.extend_from_slice(&[0; 6]);
        let mut offset = 12 + 16 * tables.len();
        for (tag, table) in tables {
            bytes.extend_from_slice(*tag);
            bytes.extend_from_slice(&[0; 4]);
            bytes.extend_from_slice(&u32::try_from(offset).expect("small").to_be_bytes());
            bytes.extend_from_slice(&u32::try_from(table.len()).expect("small").to_be_bytes());
            offset += table.len();
        }
        for (_, table) in tables {
            bytes.extend_from_slice(table);
        }
        bytes
    }

    /// An OS/2 table of `version` with `fs_type`, long enough for version 0.
    pub(crate) fn os2(version: u16, fs_type: u16) -> Vec<u8> {
        let mut table = vec![0; 78];
        table[..2].copy_from_slice(&version.to_be_bytes());
        table[8..10].copy_from_slice(&fs_type.to_be_bytes());
        table
    }

    /// A TrueType font whose only tables are `cmap` and an OS/2 table.
    pub(crate) fn truetype(fs_type: u16) -> Vec<u8> {
        sfnt(
            b"\x00\x01\x00\x00",
            &[(b"cmap", &[0; 4]), (b"OS/2", &os2(4, fs_type))],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{os2, sfnt, truetype};
    use super::*;

    /// Header with `num_tables` and `rest` from offset 12, padded with NULs.
    fn header(version: &[u8; 4], num_tables: u16, rest: &[u8]) -> Vec<u8> {
        let mut bytes = version.to_vec();
        bytes.extend_from_slice(&num_tables.to_be_bytes());
        bytes.extend_from_slice(&[0; 6]);
        bytes.extend_from_slice(rest);
        bytes.extend_from_slice(&[0; 400]);
        bytes
    }

    // Expectations below were checked against `file -b` 5.47.
    #[test]
    fn magic_truetype_versions_and_table_count() {
        for version in [b"\x00\x01\x00\x00", b"true", b"typ1"] {
            assert_eq!(
                SfntMagic::from_bytes(&header(version, 3, b"FFTM")),
                Some(SfntMagic::TrueType)
            );
        }
        assert_eq!(
            SfntMagic::from_bytes(&header(b"\x00\x01\x00\x00", 47, b"FFTM")),
            Some(SfntMagic::TrueType)
        );
        assert_eq!(
            SfntMagic::from_bytes(&header(b"\x00\x01\x00\x00", 48, b"FFTM")),
            None
        );
        assert_eq!(
            SfntMagic::from_bytes(&header(b"\x00\x02\x00\x00", 3, b"FFTM")),
            None
        );
        assert_eq!(SfntMagic::from_bytes(b"\x00\x01\x00"), None);
    }

    #[test]
    fn magic_opentype_is_otto_prefix() {
        assert_eq!(
            SfntMagic::from_bytes(b"OTTOxxxx"),
            Some(SfntMagic::OpenType)
        );
        assert_eq!(
            SfntMagic::from_bytes(&header(b"OTTO", 3, b"FFTM")),
            Some(SfntMagic::OpenType)
        );
    }

    #[test]
    fn magic_first_tag_classes() {
        let tag = |rest: &[u8]| SfntMagic::from_bytes(&header(b"\x00\x01\x00\x00", 3, rest));
        for good in [&b"FFTM"[..], b"OS/2", b"cvt ", b"C2PA"] {
            assert_eq!(tag(good), Some(SfntMagic::TrueType), "{good:?}");
        }
        for bad in [&b"1FTM"[..], b"OS/3", b"O/S2", b"12FFTM"] {
            assert_eq!(tag(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn magic_regex_window() {
        let tag = |rest: &[u8]| SfntMagic::from_bytes(&header(b"\x00\x01\x00\x00", 3, rest));
        // `^` after a newline, but not after `\r` or past a NUL.
        assert_eq!(tag(b"1234\nFFTM"), Some(SfntMagic::TrueType));
        assert_eq!(tag(b"1\r\nFFTM"), Some(SfntMagic::TrueType));
        assert_eq!(tag(b"1234\rFFTM"), None);
        assert_eq!(tag(b"12\0\nFFTM"), None);
        // Four lines: the fifth is outside the window.
        assert_eq!(tag(b"1\n2\n3\nFFTM"), Some(SfntMagic::TrueType));
        assert_eq!(tag(b"1\n2\n3\nFFTM\n"), Some(SfntMagic::TrueType));
        assert_eq!(tag(b"1\n2\n3\n4\nFFTM"), None);
        // `\r` counts for the window only once no `\n` is left.
        assert_eq!(tag(b"1\r2\r3\r4\r5\nFFTM"), Some(SfntMagic::TrueType));
        // 320 bytes, minus the last one.
        let far = |ones: usize| {
            let mut rest = vec![b'1'; ones];
            rest.extend_from_slice(b"\nFFTM");
            tag(&rest)
        };
        assert_eq!(far(314), Some(SfntMagic::TrueType));
        assert_eq!(far(315), None);
        // The last byte of a short file is not searched.
        let mut short = header(b"\x00\x01\x00\x00", 3, b"");
        short.truncate(12);
        let mut four = short.clone();
        four.extend_from_slice(b"FFTM");
        assert_eq!(SfntMagic::from_bytes(&four), None);
        four.push(b'a');
        assert_eq!(SfntMagic::from_bytes(&four), Some(SfntMagic::TrueType));
    }

    #[test]
    fn fs_type_from_os2() -> Result<(), SfntError> {
        assert_eq!(Sfnt::parse(&truetype(0x0002))?.fs_type(), Some(0x0002));
        assert_eq!(
            Sfnt::parse(&sfnt(b"OTTO", &[(b"OS/2", &os2(3, 0x0008))]))?.fs_type(),
            Some(0x0008)
        );
        assert_eq!(
            Sfnt::parse(&sfnt(b"true", &[(b"OS/2", &os2(0, 0x0004))]))?.fs_type(),
            Some(0x0004)
        );
        assert_eq!(
            Sfnt::parse(&sfnt(b"\x00\x01\x00\x00", &[(b"cmap", &[0; 4])]))?.fs_type(),
            None
        );
        Ok(())
    }

    #[test]
    fn fs_type_follows_font_ttf_quirks() -> Result<(), SfntError> {
        // OS/2 version 5 has no field layout in Font::TTF 1.06.
        assert_eq!(
            Sfnt::parse(&sfnt(b"\x00\x01\x00\x00", &[(b"OS/2", &os2(5, 0x0002))]))?.fs_type(),
            None
        );
        // The table length is ignored; only the file end limits the read.
        let mut bytes = sfnt(b"\x00\x01\x00\x00", &[(b"OS/2", &os2(1, 0x0002)[..10])]);
        assert_eq!(Sfnt::parse(&bytes)?.fs_type(), Some(0x0002));
        bytes.pop();
        assert_eq!(Sfnt::parse(&bytes)?.fs_type(), None);
        // A later duplicate record wins.
        let first = os2(1, 0x0002);
        let second = os2(1, 0x0000);
        let bytes = sfnt(
            b"\x00\x01\x00\x00",
            &[(b"OS/2", &first), (b"OS/2", &second)],
        );
        assert_eq!(Sfnt::parse(&bytes)?.fs_type(), Some(0x0000));
        Ok(())
    }

    #[test]
    fn parse_errors() {
        assert_eq!(
            Sfnt::parse(&sfnt(b"typ1", &[(b"OS/2", &os2(1, 2))])),
            Err(SfntError::UnknownVersion)
        );
        assert_eq!(Sfnt::parse(b"\x00\x01"), Err(SfntError::UnknownVersion));
        let mut bytes = header(b"\x00\x01\x00\x00", 2, b"");
        bytes.truncate(12 + 16 + 8);
        assert_eq!(Sfnt::parse(&bytes), Err(SfntError::TruncatedTableDirectory));
        // No table count reads as no tables.
        assert_eq!(
            Sfnt::parse(b"\x00\x01\x00\x00\x00").map(|font| font.fs_type()),
            Ok(None)
        );
    }
}
