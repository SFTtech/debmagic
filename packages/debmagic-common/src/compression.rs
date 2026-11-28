//! Decompression of the formats Debian archives use, chosen by file name.

use std::io::{Cursor, Read};

use anyhow::bail;

/// A decoder for `reader`, picked by the extension of `name`: `.gz`, `.bz2`,
/// `.xz`, `.lzma` (legacy LZMA-alone), `.zst`, or a plain `.tar`. Each decoder reads one stream (no
/// multi-member gzip, no concatenated xz). `Err` for any other name.
pub fn decoder<'a>(name: &str, reader: impl Read + 'a) -> anyhow::Result<Box<dyn Read + 'a>> {
    if name.ends_with(".gz") {
        Ok(Box::new(flate2::read::GzDecoder::new(reader)))
    } else if name.ends_with(".bz2") {
        Ok(Box::new(bzip2::read::BzDecoder::new(reader)))
    } else if name.ends_with(".xz") {
        Ok(Box::new(xz2::read::XzDecoder::new(reader)))
    } else if name.ends_with(".lzma") {
        let stream = xz2::stream::Stream::new_lzma_decoder(u64::MAX)?;
        Ok(Box::new(xz2::read::XzDecoder::new_stream(reader, stream)))
    } else if name.ends_with(".zst") {
        Ok(Box::new(zstd::stream::read::Decoder::new(reader)?))
    } else if name.ends_with(".tar") {
        Ok(Box::new(reader))
    } else {
        bail!("{name} is compressed in a format debmagic does not read");
    }
}

/// The whole decompressed content of `bytes`; see [`decoder`].
pub fn decompress_by_name(name: &str, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut plain = Vec::new();
    decoder(name, Cursor::new(bytes))?.read_to_end(&mut plain)?;
    Ok(plain)
}

/// The payload of a gzip stream. `None` when the bytes are not one or it
/// ends early. Only the first member is read.
pub fn gunzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = flate2::read::GzDecoder::new(bytes);
    let mut plain = Vec::new();
    decoder.read_to_end(&mut plain).ok()?;
    Some(plain)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn decompresses_by_extension() {
        let xz = {
            let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 6);
            encoder.write_all(b"xz payload").unwrap();
            encoder.finish().unwrap()
        };
        let zst = zstd::stream::encode_all(&b"zst payload"[..], 0).unwrap();
        assert_eq!(
            decompress_by_name("a.tar.gz", &gzip(b"gz payload")).unwrap(),
            b"gz payload"
        );
        assert_eq!(decompress_by_name("a.tar.xz", &xz).unwrap(), b"xz payload");
        assert_eq!(
            decompress_by_name("a.tar.zst", &zst).unwrap(),
            b"zst payload"
        );
        assert_eq!(decompress_by_name("a.tar", b"plain").unwrap(), b"plain");
        let lzma = {
            let options = xz2::stream::LzmaOptions::new_preset(6).unwrap();
            let stream = xz2::stream::Stream::new_lzma_encoder(&options).unwrap();
            let mut encoder = xz2::write::XzEncoder::new_stream(Vec::new(), stream);
            encoder.write_all(b"lzma payload").unwrap();
            encoder.finish().unwrap()
        };
        assert_eq!(
            decompress_by_name("a.tar.lzma", &lzma).unwrap(),
            b"lzma payload"
        );
        assert!(decompress_by_name("a.tar.lz", b"").is_err());
    }

    #[test]
    fn gunzip_rejects_what_is_not_gzip() {
        assert_eq!(gunzip(&gzip(b"text")).as_deref(), Some(&b"text"[..]));
        assert_eq!(gunzip(b"not gzip"), None);
    }
}
