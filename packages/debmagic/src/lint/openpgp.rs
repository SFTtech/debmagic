//! A native reader of the OpenPGP packet stream of a key file (RFC 4880,
//! with the ASCII armor of section 6): what lintian learns from
//! `gpg --list-packets` about a key, without running gpg.
//!
//! Where a Rule needs gpg's answer it matches it: empty input, a byte that
//! starts no packet, an unknown packet tag, a bad armor checksum and a packet
//! that follows the last complete one are errors. A packet cut short by the
//! end of the data ends the stream without an error, as gpg does.
//!
//! Only tag 6 (public key) packets become [`PublicKey`]s. Subkeys, secret
//! keys and user IDs are read past. Signature packets are read only for the
//! key id of their issuer, which is kept on the public key they follow. Whether
//! a key can verify anything is not decided here.

use sha1::{Digest, Sha1};

/// A key file read as OpenPGP data. A parse error is kept on the object, and
/// then it has no public keys: the file is read as a whole or not at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenPgpFile {
    public_keys: Vec<PublicKey>,
    error: Option<String>,
}

/// A public key packet: its version and key id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublicKey {
    version: u8,
    key_id: Option<[u8; 8]>,
    signers: Vec<Option<[u8; 8]>>,
}

impl OpenPgpFile {
    /// The file's bytes, armored or binary. Never fails: an error is kept.
    pub(crate) fn parse(bytes: &[u8]) -> Self {
        match read(bytes) {
            Ok(packets) => Self {
                public_keys: public_keys(&packets),
                error: None,
            },
            Err(error) => Self {
                public_keys: Vec::new(),
                error: Some(error),
            },
        }
    }

    /// The public key packets, in file order. Empty after an error.
    pub(crate) fn public_keys(&self) -> &[PublicKey] {
        &self.public_keys
    }

    /// Why the file cannot be read as OpenPGP data, `None` when it can.
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[allow(dead_code)]
impl PublicKey {
    fn from_body(body: &[u8]) -> Self {
        let version = body.first().copied().unwrap_or(0);
        let key_id = if version == 4 { v4_key_id(body) } else { None };
        Self {
            version,
            key_id,
            signers: Vec::new(),
        }
    }

    /// The key packet's version number.
    pub(crate) fn version(&self) -> u8 {
        self.version
    }

    /// The last eight bytes of the fingerprint, for a version 4 key. `None`
    /// for other versions, which this reader does not identify.
    pub(crate) fn key_id(&self) -> Option<[u8; 8]> {
        self.key_id
    }

    /// The issuer key ids of the signature packets after this key and before
    /// the next public key packet, in file order. `None` for a signature that
    /// names no issuer.
    pub(crate) fn signature_key_ids(&self) -> &[Option<[u8; 8]>] {
        &self.signers
    }
}

const PUBLIC_KEY_TAG: u8 = 6;
const SIGNATURE_TAG: u8 = 2;

/// The public key packets, each with the signatures that follow it. Signatures
/// before the first key belong to no key and are dropped.
fn public_keys(packets: &[Packet]) -> Vec<PublicKey> {
    let mut keys: Vec<PublicKey> = Vec::new();
    for packet in packets {
        match packet.tag {
            PUBLIC_KEY_TAG => keys.push(PublicKey::from_body(&packet.body)),
            SIGNATURE_TAG => {
                if let Some(key) = keys.last_mut() {
                    key.signers.push(issuer_key_id(&packet.body));
                }
            }
            _ => {}
        }
    }
    keys
}

/// The key id of the issuer of a signature packet body (RFC 4880 section
/// 5.2): the version 3 key id field, or the Issuer subpacket of a version 4
/// signature, else the key id of its Issuer Fingerprint subpacket.
fn issuer_key_id(body: &[u8]) -> Option<[u8; 8]> {
    match *body.first()? {
        3 => <[u8; 8]>::try_from(body.get(7..15)?).ok(),
        4 => {
            let hashed = subpacket_area(body, 4)?;
            let unhashed = subpacket_area(body, 6 + hashed.len())?;
            issuer_subpacket(hashed).or_else(|| issuer_subpacket(unhashed))
        }
        _ => None,
    }
}

/// The subpacket area whose two-byte length is at `position`.
fn subpacket_area(body: &[u8], position: usize) -> Option<&[u8]> {
    let length = body.get(position..position + 2)?;
    let length = usize::from(u16::from_be_bytes([length[0], length[1]]));
    body.get(position + 2..position + 2 + length)
}

/// The key id named by the first Issuer (16) or Issuer Fingerprint (33)
/// subpacket of `area`. A version 4 fingerprint's key id is its last eight
/// bytes.
fn issuer_subpacket(mut area: &[u8]) -> Option<[u8; 8]> {
    let mut fingerprint = None;
    while let Some(&first) = area.first() {
        let (length, header): (usize, usize) = match first {
            0..=191 => (usize::from(first), 1),
            192..=254 => (
                (usize::from(first) - 192) * 256 + usize::from(*area.get(1)?) + 192,
                2,
            ),
            255 => {
                let four = area.get(1..5)?;
                let length = u32::from_be_bytes([four[0], four[1], four[2], four[3]]);
                (usize::try_from(length).unwrap_or(usize::MAX), 5)
            }
        };
        let subpacket = area.get(header..header.checked_add(length)?)?;
        match subpacket.split_first() {
            Some((&16, data)) => {
                if let Ok(key_id) = <[u8; 8]>::try_from(data) {
                    return Some(key_id);
                }
            }
            Some((&33, data)) if fingerprint.is_none() && data.len() == 21 && data[0] == 4 => {
                fingerprint = <[u8; 8]>::try_from(&data[13..21]).ok();
            }
            _ => {}
        }
        area = &area[header + length..];
    }
    fingerprint
}

/// The packets of the file, after the armor is removed. A packet cut short by
/// the end of the data ends the list.
fn read(bytes: &[u8]) -> Result<Vec<Packet>, String> {
    let data = dearmor(bytes)?;
    if data.is_empty() {
        return Err("no OpenPGP data found".to_string());
    }
    let mut packets = Vec::new();
    let mut rest = data.as_slice();
    while !rest.is_empty() {
        let Some((tag, body, used)) = packet(rest)? else {
            break;
        };
        packets.push(Packet { tag, body });
        rest = &rest[used..];
    }
    Ok(packets)
}

struct Packet {
    tag: u8,
    body: Vec<u8>,
}

/// The packet at the start of `data`: its tag, its body and the number of
/// bytes it takes. `Ok(None)` when the data ends inside the packet.
fn packet(data: &[u8]) -> Result<Option<(u8, Vec<u8>, usize)>, String> {
    let ctb = data[0];
    if ctb & 0x80 == 0 {
        return Err("invalid packet".to_string());
    }
    let mut body = Vec::new();
    if ctb & 0x40 != 0 {
        let tag = ctb & 0x3f;
        if !valid_tag(tag) {
            return Err("invalid packet".to_string());
        }
        // New format: one or more lengths; partial lengths continue the body.
        let mut position = 1;
        loop {
            let Some(&first) = data.get(position) else {
                return Ok(None);
            };
            let partial = (224..=254).contains(&first);
            let (length, header) = match first {
                0..=191 => (usize::from(first), 1),
                192..=223 => {
                    let Some(&second) = data.get(position + 1) else {
                        return Ok(None);
                    };
                    (
                        (usize::from(first) - 192) * 256 + usize::from(second) + 192,
                        2,
                    )
                }
                224..=254 => (1 << (first & 0x1f), 1),
                255 => {
                    let Some(four) = data.get(position + 1..position + 5) else {
                        return Ok(None);
                    };
                    let length = u32::from_be_bytes([four[0], four[1], four[2], four[3]]);
                    (usize::try_from(length).unwrap_or(usize::MAX), 5)
                }
            };
            position += header;
            let Some(chunk) = data.get(position..position.saturating_add(length)) else {
                return Ok(None);
            };
            body.extend_from_slice(chunk);
            position += length;
            if !partial {
                return Ok(Some((tag, body, position)));
            }
        }
    }
    // Old format: the tag is in the middle bits, the length in the last two.
    let tag = (ctb >> 2) & 0x0f;
    if !valid_tag(tag) {
        return Err("invalid packet".to_string());
    }
    let width = match ctb & 0x03 {
        0 => 1,
        1 => 2,
        2 => 4,
        _ => return Ok(Some((tag, data[1..].to_vec(), data.len()))),
    };
    let Some(length_bytes) = data.get(1..1 + width) else {
        return Ok(None);
    };
    let length = length_bytes
        .iter()
        .fold(0usize, |length, &byte| (length << 8) | usize::from(byte));
    let Some(chunk) = data.get(1 + width..(1 + width).saturating_add(length)) else {
        return Ok(None);
    };
    body.extend_from_slice(chunk);
    Ok(Some((tag, body, 1 + width + length)))
}

/// The tags gpg accepts: the RFC 4880 packets it knows, not the reserved ones
/// and not the GnuPG trust packet (12).
fn valid_tag(tag: u8) -> bool {
    matches!(tag, 1..=11 | 13 | 14 | 17..=19 | 60..=63)
}

/// Fingerprint of a version 4 key: SHA-1 over `0x99`, the body's length and
/// the body. The key id is its last eight bytes.
fn v4_key_id(body: &[u8]) -> Option<[u8; 8]> {
    let length = u16::try_from(body.len()).ok()?;
    let mut hasher = Sha1::new();
    hasher.update([0x99]);
    hasher.update(length.to_be_bytes());
    hasher.update(body);
    let fingerprint = hasher.finalize();
    fingerprint[12..20].try_into().ok()
}

/// The binary data of the file. Armor is removed when an armor header starts
/// a line anywhere in the file, as gpg finds it after leading prose; each
/// armor block's checksum must match its data. Text outside the blocks is
/// skipped.
fn dearmor(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let header = b"-----BEGIN PGP";
    let armored = bytes.starts_with(header)
        || bytes
            .windows(header.len() + 1)
            .any(|window| window[0] == b'\n' && &window[1..] == header);
    if !armored {
        return Ok(bytes.to_vec());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "armor is not text".to_string())?;
    let mut data = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if !line.starts_with("-----BEGIN PGP") {
            continue;
        }
        // Armor headers run to the first blank line.
        for header in lines.by_ref() {
            if header.trim().is_empty() {
                break;
            }
        }
        let mut body = String::new();
        let mut checksum = None;
        let mut ended = false;
        for line in lines.by_ref() {
            let line = line.trim();
            if line.starts_with("-----END PGP") {
                ended = true;
                break;
            }
            match line.strip_prefix('=') {
                Some(sum) => checksum = Some(sum),
                None => body.push_str(line),
            }
        }
        if !ended {
            return Err("armor has no END line".to_string());
        }
        let decoded = base64_decode(&body)?;
        if let Some(sum) = checksum {
            let expected = base64_decode(sum)?;
            let expected = expected
                .iter()
                .fold(0u32, |value, &byte| (value << 8) | u32::from(byte));
            if expected != crc24(&decoded) {
                return Err("armor checksum does not match".to_string());
            }
        }
        data.extend(decoded);
    }
    Ok(data)
}

/// Standard base64, whitespace skipped, data ends at the first `=`.
fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for byte in text.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return Err("armor is not base64".to_string()),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from(buffer >> bits).unwrap_or(0));
            buffer &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

/// The armor checksum (RFC 4880 section 6.1): CRC-24 with its init and
/// polynomial.
fn crc24(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0x00b7_04ce;
    for &byte in bytes {
        crc ^= u32::from(byte) << 16;
        for _ in 0..8 {
            crc <<= 1;
            if crc & 0x0100_0000 != 0 {
                crc ^= 0x0186_4cfb;
            }
        }
    }
    crc & 0x00ff_ffff
}

/// A throwaway ed25519 key (one user ID, one self-signature), armored as
/// `gpg --armor --export` writes it. Shared with the Rule tests.
#[cfg(test)]
pub(crate) const TINY_KEY: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\n\nmDMEawPkthYJKwYBBAHaRw8BAQdA2ySxbMJVLIUjgqLec5y9Q2J4BGh6Q4xyNyNa\nNkVZ/sq0H1Rlc3QgS2V5IDx0ZXN0QGV4YW1wbGUuaW52YWxpZD6IkAQTFgoAOBYh\nBPRtFFCzVZNPAmGLUbR4vIAGBWu/BQJrA+S2AhsDBQsJCAcCBhUKCQgLAgQWAgMB\nAh4BAheAAAoJELR4vIAGBWu/yAkA/1UOVH5XQRZKCfVWbFDguEtcfyYjjVK4tSWc\nMJVXSFZOAQDbfsRVJYjPeRltN3QSfRQ6GLV7AyJYQxNanebj0fuNBA==\n=11jc\n-----END PGP PUBLIC KEY BLOCK-----\n";

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_ID: [u8; 8] = [0xb4, 0x78, 0xbc, 0x80, 0x06, 0x05, 0x6b, 0xbf];

    #[test]
    fn armored_key_has_one_public_key_with_its_key_id() {
        let file = OpenPgpFile::parse(TINY_KEY.as_bytes());
        assert_eq!(file.error(), None);
        assert_eq!(file.public_keys().len(), 1);
        assert_eq!(file.public_keys()[0].version(), 4);
        assert_eq!(file.public_keys()[0].key_id(), Some(KEY_ID));
    }

    #[test]
    fn partial_body_lengths_read_the_same_key() -> Result<(), String> {
        // The key packet of the armored key, as binary, then with its body
        // split into a partial length (32 bytes) and a final one (19 bytes).
        let binary = dearmor(TINY_KEY.as_bytes())?;
        let body = &binary[2..53];
        let mut partial = vec![0xc6, 0xe5];
        partial.extend_from_slice(&body[..32]);
        partial.push(0x13);
        partial.extend_from_slice(&body[32..]);
        let file = OpenPgpFile::parse(&partial);
        assert_eq!(file.error(), None);
        assert_eq!(file.public_keys()[0].key_id(), Some(KEY_ID));
        Ok(())
    }

    #[test]
    fn a_key_then_an_invalid_byte_is_an_error() -> Result<(), String> {
        let mut data = dearmor(TINY_KEY.as_bytes())?;
        data.push(0x54);
        let file = OpenPgpFile::parse(&data);
        assert_eq!(file.error(), Some("invalid packet"));
        assert!(file.public_keys().is_empty());
        Ok(())
    }

    #[test]
    fn empty_and_non_packet_input_cannot_be_processed() {
        assert!(OpenPgpFile::parse(b"").error().is_some());
        let nonsense = b"Testing an empty file would require source format 3.0\n";
        assert_eq!(OpenPgpFile::parse(nonsense).error(), Some("invalid packet"));
        // An armor block with no data.
        let armor = "-----BEGIN PGP PUBLIC KEY BLOCK-----\n\n-----END PGP PUBLIC KEY BLOCK-----\n";
        assert!(OpenPgpFile::parse(armor.as_bytes()).error().is_some());
    }

    #[test]
    fn a_bad_armor_checksum_cannot_be_processed() {
        let bad = TINY_KEY.replace("=11jc", "=11jd");
        assert_eq!(
            OpenPgpFile::parse(bad.as_bytes()).error(),
            Some("armor checksum does not match")
        );
    }

    #[test]
    fn packets_without_a_public_key_have_no_keys() {
        // A user ID packet with a one-byte length.
        let file = OpenPgpFile::parse(b"\xb4\x04Test");
        assert_eq!(file.error(), None);
        assert!(file.public_keys().is_empty());
        // The same packet with a four-byte length.
        let file = OpenPgpFile::parse(b"\xb4\xff\x00\x00\x00\x04Test");
        assert_eq!(file.error(), None);
        assert!(file.public_keys().is_empty());
    }

    #[test]
    fn a_packet_cut_short_ends_the_stream_without_error() {
        let file = OpenPgpFile::parse(b"\xb4\x09Test");
        assert_eq!(file.error(), None);
        assert!(file.public_keys().is_empty());
    }

    /// A version 4 signature packet (old format) whose hashed area names
    /// `issuer` in an Issuer subpacket.
    fn signature_by(issuer: [u8; 8]) -> Vec<u8> {
        let mut body = vec![4, 0x10, 1, 8, 0, 10, 9, 16];
        body.extend_from_slice(&issuer);
        body.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        let mut packet = vec![0x88, u8::try_from(body.len()).unwrap_or(0)];
        packet.extend(body);
        packet
    }

    #[test]
    fn signatures_after_a_key_give_their_issuers() -> Result<(), String> {
        let binary = dearmor(TINY_KEY.as_bytes())?;
        let other = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut data = binary[..53].to_vec();
        data.extend(signature_by(KEY_ID));
        data.extend(signature_by(other));
        let file = OpenPgpFile::parse(&data);
        assert_eq!(file.error(), None);
        assert_eq!(
            file.public_keys()[0].signature_key_ids(),
            &[Some(KEY_ID), Some(other)]
        );
        Ok(())
    }

    #[test]
    fn a_signature_before_any_key_belongs_to_no_key() -> Result<(), String> {
        let binary = dearmor(TINY_KEY.as_bytes())?;
        let mut data = signature_by([1; 8]);
        data.extend_from_slice(&binary[..53]);
        let file = OpenPgpFile::parse(&data);
        assert_eq!(file.public_keys().len(), 1);
        assert!(file.public_keys()[0].signature_key_ids().is_empty());
        Ok(())
    }

    #[test]
    fn a_signature_without_an_issuer_has_none() -> Result<(), String> {
        // Version 4 with empty hashed and unhashed areas: no Issuer subpacket.
        let binary = dearmor(TINY_KEY.as_bytes())?;
        let mut data = binary[..53].to_vec();
        data.extend([0x88, 0x0a, 4, 0x10, 1, 8, 0, 0, 0, 0, 0, 0]);
        let file = OpenPgpFile::parse(&data);
        assert_eq!(file.error(), None);
        assert_eq!(file.public_keys()[0].signature_key_ids(), &[None]);
        Ok(())
    }

    #[test]
    fn an_unknown_tag_is_an_error() {
        assert_eq!(
            OpenPgpFile::parse(b"\x80\x00").error(),
            Some("invalid packet")
        );
    }
}
