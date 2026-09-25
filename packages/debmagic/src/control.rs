use std::path::Path;

use anyhow::Context;
use deb822_lossless::Paragraph;
use debian_control::lossless::changes::Changes;
use debian_control::pgp;
use sha2::Digest;

pub use debmagic_common::debian::control::{
    CHECKSUM_FIELDS, Digests, Hash, child_filename, fixup_checksums, verify_checksums,
};

/// Digest `path` streaming, with every algorithm a control file's
/// checksum fields may use.
pub(crate) fn digest_file(path: &Path) -> anyhow::Result<Digests> {
    let mut sha256 = sha2::Sha256::new();
    let mut sha1 = sha1::Sha1::new();
    let mut md5 = md5::Md5::new();
    let mut file =
        std::fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        let chunk = &buffer[..read];
        sha256.update(chunk);
        sha1.update(chunk);
        md5.update(chunk);
    }
    let to_hex = |digest: &[u8]| {
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    Ok(Digests {
        md5: to_hex(&md5.finalize()),
        sha1: to_hex(&sha1.finalize()),
        sha256: to_hex(&sha256.finalize()),
    })
}

/// Read the single deb822 paragraph of a `.changes`/`.dsc`/`.buildinfo`
/// control file, losslessly, so checksum rewrites preserve the original
/// formatting of untouched fields byte-for-byte. PGP-clearsigned files
/// (like archive `.dsc` files) have their armor stripped first.
pub(crate) fn read_control(path: &Path) -> anyhow::Result<Paragraph> {
    let content = read_control_content(path)?;
    let deb822 = content
        .parse::<deb822_lossless::Deb822>()
        .with_context(|| format!("failed to parse {}", path.display()))?;
    deb822
        .paragraphs()
        .next()
        .with_context(|| format!("{} contains no paragraph", path.display()))
}

/// Read a `.changes` file, losslessly, with PGP armor stripped first —
/// signed `.changes` files are the norm, not the exception.
pub(crate) fn read_changes(path: &Path) -> anyhow::Result<Changes> {
    let content = read_control_content(path)?;
    Changes::read(content.as_bytes()).with_context(|| format!("failed to parse {}", path.display()))
}

/// The file content with the PGP clearsign armor removed when present.
fn read_control_content(path: &Path) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if content.starts_with("-----BEGIN PGP SIGNED MESSAGE-----") {
        let (payload, _) = pgp::strip_pgp_signature(&content)
            .with_context(|| format!("failed to strip the signature from {}", path.display()))?;
        Ok(payload)
    } else {
        Ok(content)
    }
}

pub(crate) fn write_control(paragraph: &Paragraph, path: &Path) -> anyhow::Result<()> {
    std::fs::write(path, paragraph.to_string())
        .with_context(|| format!("failed to write {}", path.display()))
}
