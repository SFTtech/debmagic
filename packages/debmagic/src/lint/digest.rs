//! Digests of file contents, by algorithm name: the MD5 of an Entry (lintian's
//! `$item->md5sum`), and the digests of the files a `.changes` names in its
//! Checksums fields.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, bail};
use md5::Digest;

/// The digest algorithms lint Rules compare file contents with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DigestAlgorithm {
    Md5,
    Sha1,
    Sha256,
}

impl DigestAlgorithm {
    /// The algorithm named `name` (`md5`, `sha1`, `sha256`), in any letter case.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "md5" => Some(Self::Md5),
            "sha1" => Some(Self::Sha1),
            "sha256" => Some(Self::Sha256),
            _ => None,
        }
    }

    /// The name as lintian spells it in hints and tables: `md5`, `sha1`, `sha256`.
    #[allow(dead_code)]
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }
}

/// Lowercase hex digest of `bytes`.
#[allow(dead_code)]
pub(crate) fn hex_digest(algorithm: DigestAlgorithm, bytes: &[u8]) -> String {
    match algorithm {
        DigestAlgorithm::Md5 => hex(&md5::Md5::digest(bytes)),
        DigestAlgorithm::Sha1 => hex(&sha1::Sha1::digest(bytes)),
        DigestAlgorithm::Sha256 => hex(&sha2::Sha256::digest(bytes)),
    }
}

/// Lowercase hex digest of the file at `path`, read in chunks. `Err` when the
/// file cannot be read.
#[allow(dead_code)]
pub(crate) fn file_digest(path: &Path, algorithm: DigestAlgorithm) -> anyhow::Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    Ok(match algorithm {
        DigestAlgorithm::Md5 => stream(&mut file, md5::Md5::new(), path)?,
        DigestAlgorithm::Sha1 => stream(&mut file, sha1::Sha1::new(), path)?,
        DigestAlgorithm::Sha256 => stream(&mut file, sha2::Sha256::new(), path)?,
    })
}

/// Digest of the file at `path` by algorithm name (`md5`, `sha1`, `sha256`),
/// for the sibling files a `.changes` Checksums field names. `Err` for an
/// unknown name or an unreadable file.
#[allow(dead_code)]
pub(crate) fn sibling_digest(path: &Path, algorithm: &str) -> anyhow::Result<String> {
    let Some(algorithm) = DigestAlgorithm::from_name(algorithm) else {
        bail!("unknown digest algorithm `{algorithm}`");
    };
    file_digest(path, algorithm)
}

fn stream<D: Digest>(reader: &mut impl Read, mut hasher: D, path: &Path) -> anyhow::Result<String> {
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("reading {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(sum: &[u8]) -> String {
    sum.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_MD5: &str = "d41d8cd98f00b204e9800998ecf8427e";
    const EMPTY_SHA1: &str = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn names_resolve_in_any_case() {
        assert_eq!(
            DigestAlgorithm::from_name("MD5"),
            Some(DigestAlgorithm::Md5)
        );
        assert_eq!(
            DigestAlgorithm::from_name("sha1"),
            Some(DigestAlgorithm::Sha1)
        );
        assert_eq!(
            DigestAlgorithm::from_name("Sha256"),
            Some(DigestAlgorithm::Sha256)
        );
        assert_eq!(DigestAlgorithm::from_name("sha512"), None);
        assert_eq!(DigestAlgorithm::Sha256.name(), "sha256");
    }

    #[test]
    fn hex_digests_of_empty_input() {
        assert_eq!(hex_digest(DigestAlgorithm::Md5, b""), EMPTY_MD5);
        assert_eq!(hex_digest(DigestAlgorithm::Sha1, b""), EMPTY_SHA1);
        assert_eq!(hex_digest(DigestAlgorithm::Sha256, b""), EMPTY_SHA256);
    }

    #[test]
    fn sibling_file_digests_by_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("empty.bin");
        std::fs::write(&path, b"").expect("write");

        assert_eq!(sibling_digest(&path, "md5").expect("md5"), EMPTY_MD5);
        assert_eq!(sibling_digest(&path, "SHA1").expect("sha1"), EMPTY_SHA1);
        assert_eq!(
            sibling_digest(&path, "sha256").expect("sha256"),
            EMPTY_SHA256
        );
        assert!(sibling_digest(&path, "sha512").is_err());
        assert!(sibling_digest(&dir.path().join("missing"), "md5").is_err());
    }

    #[test]
    fn chunked_file_digest_matches_in_memory_digest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("big.bin");
        let content: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
        std::fs::write(&path, &content).expect("write");

        assert_eq!(
            file_digest(&path, DigestAlgorithm::Sha256).expect("digest"),
            hex_digest(DigestAlgorithm::Sha256, &content)
        );
    }
}
