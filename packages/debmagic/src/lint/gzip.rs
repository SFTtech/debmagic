use std::io::Read;

/// Gzip payload. `None` when the bytes are not a gzip stream.
pub(crate) fn gunzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = flate2::read::GzDecoder::new(bytes);
    let mut plain = Vec::new();
    decoder.read_to_end(&mut plain).ok()?;
    Some(plain)
}
