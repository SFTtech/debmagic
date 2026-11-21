//! A minimal zip reader: the central directory and the members' bytes.
//!
//! Lintian reads jar files with `Archive::Zip`; this reads what lintian reads
//! from them (member names, directories, and the content of the members it
//! looks into) and fails with `Archive::Zip`'s error texts where they do not
//! depend on the host (`$!`, the temporary path). Zip64 archives, encryption,
//! and compression methods other than stored and deflate are not supported;
//! an archive that needs them fails like any other unreadable member.

use std::io::Read;

use flate2::Crc;
use flate2::read::DeflateDecoder;

const EOCD_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const EOCD_SIGNATURE_NUMBER: u32 = 0x0605_4b50;
/// Length of the end of central directory record without its signature.
const EOCD_LENGTH: usize = 18;
const CENTRAL_HEADER_LENGTH: usize = 42;
const LOCAL_HEADER_LENGTH: usize = 26;
/// `Archive::Zip` looks for the end of central directory in the last
/// `$Archive::Zip::ChunkSize` bytes.
const EOCD_SEARCH_WINDOW: usize = 32768;

/// Why a zip archive could not be read, in `Archive::Zip`'s words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipError(String);

impl ZipError {
    fn format(message: impl std::fmt::Display) -> Self {
        Self(format!("format error: {message}"))
    }

    /// `Archive::Zip`'s I/O error text, which ends in the host's `$!`; this is
    /// the part before it.
    fn io(message: impl std::fmt::Display) -> Self {
        Self(format!("IO error: {message} :"))
    }

    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ZipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One member as the central directory lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipMember {
    name: String,
    method: u16,
    crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
    local_header_offset: usize,
}

impl ZipMember {
    /// The member name, read lossily as UTF-8.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `Archive::Zip`'s `isDirectory`: the name ends in `/` and the member is
    /// empty.
    pub fn is_directory(&self) -> bool {
        self.name.ends_with('/') && self.uncompressed_size == 0
    }
}

/// A zip archive whose central directory has been read.
#[derive(Debug)]
pub struct ZipArchive<'a> {
    bytes: &'a [u8],
    members: Vec<ZipMember>,
    /// Bytes in front of the archive proper, which shift every offset.
    prefix: usize,
}

impl<'a> ZipArchive<'a> {
    /// Reads the central directory of `bytes`. `file_name` is the path
    /// `Archive::Zip` would name in a "bad signature" message.
    pub fn parse(bytes: &'a [u8], file_name: &str) -> Result<Self, ZipError> {
        if bytes.len() < EOCD_LENGTH + 4 {
            return Err(ZipError::format("file is too short"));
        }
        let window = bytes.len().saturating_sub(EOCD_SEARCH_WINDOW);
        let eocd = bytes[window..]
            .windows(4)
            .rposition(|candidate| candidate == EOCD_SIGNATURE)
            .map(|position| window + position)
            .ok_or_else(|| ZipError::format("can't find EOCD signature"))?;
        let record = bytes
            .get(eocd + 4..eocd + 4 + EOCD_LENGTH)
            .ok_or_else(|| ZipError::io("reading end of central directory"))?;
        let word =
            |offset: usize| u32::from(u16::from_le_bytes([record[offset], record[offset + 1]]));
        let double = |offset: usize| {
            u32::from_le_bytes([
                record[offset],
                record[offset + 1],
                record[offset + 2],
                record[offset + 3],
            ])
        };
        let (disk, cd_disk, entries_on_disk, entries, directory_size, directory_offset) =
            (word(0), word(2), word(4), word(6), double(8), double(12));
        if [disk, cd_disk, entries_on_disk, entries].contains(&0xffff)
            || directory_size == 0xffff_ffff
            || directory_offset == 0xffff_ffff
        {
            return Err(ZipError::format("unexpected zip64 marker values in EOCD"));
        }
        let directory_size = directory_size as usize;
        let start = eocd
            .checked_sub(directory_size)
            .ok_or_else(|| ZipError::io("Can't seek"))?;
        // Garbage in front of the archive moves every offset by this much.
        let prefix = start.saturating_sub(directory_offset as usize);

        let mut members = Vec::new();
        let mut position = start;
        loop {
            let signature = bytes
                .get(position..position + 4)
                .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
                .ok_or_else(|| ZipError::io("reading header signature"))?;
            if signature == EOCD_SIGNATURE_NUMBER {
                break;
            }
            if signature != CENTRAL_SIGNATURE {
                return Err(ZipError::format(format!(
                    "bad signature: 0x{signature:08x} at offset {position} in file {file_name}"
                )));
            }
            let header = bytes
                .get(position + 4..position + 4 + CENTRAL_HEADER_LENGTH)
                .ok_or_else(|| ZipError::io("reading central dir header"))?;
            let short = |offset: usize| {
                usize::from(u16::from_le_bytes([header[offset], header[offset + 1]]))
            };
            let long = |offset: usize| {
                u32::from_le_bytes([
                    header[offset],
                    header[offset + 1],
                    header[offset + 2],
                    header[offset + 3],
                ])
            };
            let (name_length, extra_length, comment_length) = (short(24), short(26), short(28));
            let method = short(6) as u16;
            let compressed_size = u64::from(long(16));
            let mut uncompressed_size = u64::from(long(20));
            // `Archive::Zip` trusts the compressed size of a stored member.
            if method == 0 && uncompressed_size != compressed_size {
                uncompressed_size = compressed_size;
            }
            let name_start = position + 4 + CENTRAL_HEADER_LENGTH;
            let name = bytes
                .get(name_start..name_start + name_length)
                .ok_or_else(|| ZipError::io("reading central dir filename"))?;
            let after = name_start + name_length;
            if after + extra_length > bytes.len() {
                return Err(ZipError::io("reading central dir extra field"));
            }
            if after + extra_length + comment_length > bytes.len() {
                return Err(ZipError::io("reading central dir file comment"));
            }
            members.push(ZipMember {
                name: String::from_utf8_lossy(name).into_owned(),
                method,
                crc32: long(12),
                compressed_size,
                uncompressed_size,
                local_header_offset: long(38) as usize,
            });
            position = after + extra_length + comment_length;
        }
        Ok(Self {
            bytes,
            members,
            prefix,
        })
    }

    /// The members in central directory order.
    pub fn members(&self) -> &[ZipMember] {
        &self.members
    }

    /// The uncompressed content of `member`. Fails on a bad local header,
    /// an unsupported compression method, a damaged stream, or a CRC that
    /// does not match the central directory.
    pub fn contents(&self, member: &ZipMember) -> Result<Vec<u8>, ZipError> {
        let offset = member.local_header_offset + self.prefix;
        let signature = self
            .bytes
            .get(offset..offset + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
            .ok_or_else(|| ZipError::io("reading header signature"))?;
        if signature != LOCAL_SIGNATURE {
            return Err(ZipError::format(format!(
                "bad signature: 0x{signature:08x} at offset {offset}"
            )));
        }
        let header = self
            .bytes
            .get(offset + 4..offset + 4 + LOCAL_HEADER_LENGTH)
            .ok_or_else(|| ZipError::io("reading local header"))?;
        let name_length = usize::from(u16::from_le_bytes([header[22], header[23]]));
        let extra_length = usize::from(u16::from_le_bytes([header[24], header[25]]));
        let data_start = offset + 4 + LOCAL_HEADER_LENGTH + name_length + extra_length;
        let data = usize::try_from(member.compressed_size)
            .ok()
            .and_then(|size| self.bytes.get(data_start..data_start.checked_add(size)?))
            .ok_or_else(|| ZipError::io("reading file data"))?;
        let contents = match member.method {
            0 => data.to_vec(),
            8 => {
                let mut contents = Vec::new();
                DeflateDecoder::new(data)
                    .read_to_end(&mut contents)
                    .map_err(|_| ZipError::format("error inflating"))?;
                contents
            }
            method => {
                return Err(ZipError::format(format!(
                    "unsupported compression method {method}"
                )));
            }
        };
        let mut crc = Crc::new();
        crc.update(&contents);
        if crc.sum() != member.crc32 {
            return Err(ZipError::format("CRC mismatch"));
        }
        Ok(contents)
    }
}

/// Builds small zip archives for tests.
#[cfg(test)]
pub(crate) struct TestZip {
    members: Vec<(String, Vec<u8>, bool)>,
}

#[cfg(test)]
impl TestZip {
    pub fn new() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    /// A deflated member.
    pub fn member(mut self, name: &str, contents: &[u8]) -> Self {
        self.members
            .push((name.to_string(), contents.to_vec(), true));
        self
    }

    /// A stored member.
    pub fn stored(mut self, name: &str, contents: &[u8]) -> Self {
        self.members
            .push((name.to_string(), contents.to_vec(), false));
        self
    }

    pub fn build(&self) -> Vec<u8> {
        use std::io::Write;

        let mut out = Vec::new();
        let mut directory = Vec::new();
        for (name, contents, deflate) in &self.members {
            let mut crc = Crc::new();
            crc.update(contents);
            let data = if *deflate {
                let mut encoder =
                    flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(contents).expect("in-memory write");
                encoder.finish().expect("in-memory finish")
            } else {
                contents.clone()
            };
            let method: u16 = if *deflate { 8 } else { 0 };
            let offset = out.len() as u32;
            out.extend_from_slice(&LOCAL_SIGNATURE.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&crc.sum().to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(contents.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&data);

            directory.extend_from_slice(&CENTRAL_SIGNATURE.to_le_bytes());
            directory.extend_from_slice(&20u16.to_le_bytes());
            directory.extend_from_slice(&20u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&method.to_le_bytes());
            directory.extend_from_slice(&0u32.to_le_bytes());
            directory.extend_from_slice(&crc.sum().to_le_bytes());
            directory.extend_from_slice(&(data.len() as u32).to_le_bytes());
            directory.extend_from_slice(&(contents.len() as u32).to_le_bytes());
            directory.extend_from_slice(&(name.len() as u16).to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u32.to_le_bytes());
            directory.extend_from_slice(&offset.to_le_bytes());
            directory.extend_from_slice(name.as_bytes());
        }
        let directory_offset = out.len() as u32;
        out.extend_from_slice(&directory);
        out.extend_from_slice(EOCD_SIGNATURE);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(self.members.len() as u16).to_le_bytes());
        out.extend_from_slice(&(self.members.len() as u16).to_le_bytes());
        out.extend_from_slice(&(directory.len() as u32).to_le_bytes());
        out.extend_from_slice(&directory_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_member_names_directories_and_contents() {
        let bytes = TestZip::new()
            .stored("META-INF/", b"")
            .member("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\n")
            .stored("a/B.class", b"\xca\xfe\xba\xbe\0\0\0\x34")
            .build();
        let zip = ZipArchive::parse(&bytes, "x.jar").expect("zip");
        let names: Vec<_> = zip.members().iter().map(ZipMember::name).collect();
        assert_eq!(names, ["META-INF/", "META-INF/MANIFEST.MF", "a/B.class"]);
        assert!(zip.members()[0].is_directory());
        assert!(!zip.members()[1].is_directory());
        assert_eq!(
            zip.contents(&zip.members()[1]).expect("deflated"),
            b"Manifest-Version: 1.0\n"
        );
        assert_eq!(
            zip.contents(&zip.members()[2]).expect("stored"),
            b"\xca\xfe\xba\xbe\0\0\0\x34"
        );
    }

    #[test]
    fn an_empty_archive_has_no_members() {
        let bytes = TestZip::new().build();
        assert!(
            ZipArchive::parse(&bytes, "x.jar")
                .expect("zip")
                .members()
                .is_empty()
        );
    }

    #[test]
    fn a_truncated_archive_has_no_eocd() {
        let bytes = TestZip::new().member("a", b"hello hello hello").build();
        let error = ZipArchive::parse(&bytes[..bytes.len() - 30], "x.jar").unwrap_err();
        assert_eq!(error.message(), "format error: can't find EOCD signature");
        let error = ZipArchive::parse(b"PK\x03\x04short", "x.jar").unwrap_err();
        assert_eq!(error.message(), "format error: file is too short");
    }

    #[test]
    fn a_bad_central_directory_signature_names_the_offset_and_file() {
        let mut bytes = TestZip::new().stored("a", b"x").build();
        let central = bytes
            .windows(4)
            .position(|w| w == CENTRAL_SIGNATURE.to_le_bytes())
            .expect("central directory");
        bytes[central] = b'X';
        let error = ZipArchive::parse(&bytes, "usr/share/java/x.jar").unwrap_err();
        assert_eq!(
            error.message(),
            format!(
                "format error: bad signature: 0x{:08x} at offset {central} in file usr/share/java/x.jar",
                u32::from_le_bytes([b'X', 0x4b, 0x01, 0x02])
            )
        );
    }

    #[test]
    fn a_damaged_member_fails_to_read_but_the_archive_parses() {
        let mut bytes = TestZip::new().stored("a", b"hello").build();
        let data = bytes.windows(5).position(|w| w == b"hello").expect("data");
        bytes[data] = b'j';
        let zip = ZipArchive::parse(&bytes, "x.jar").expect("zip");
        assert_eq!(
            zip.contents(&zip.members()[0]).unwrap_err().message(),
            "format error: CRC mismatch"
        );
    }

    #[test]
    fn garbage_in_front_of_the_archive_shifts_the_offsets() {
        let mut bytes = b"#!/bin/sh\nexit\n".to_vec();
        bytes.extend(TestZip::new().member("a", b"hello").build());
        let zip = ZipArchive::parse(&bytes, "x.jar").expect("zip");
        assert_eq!(zip.contents(&zip.members()[0]).expect("contents"), b"hello");
    }
}
