use deb822_lossless::Paragraph;

/// Filenames ending in `.<ext>` listed under `Files:` or any
/// `Checksums-*:` field.
pub fn child_filename(paragraph: &Paragraph, ext: &str) -> Option<String> {
    let suffix = format!(".{ext}");
    for key in paragraph.keys() {
        if key != "Files" && !key.starts_with("Checksums-") {
            continue;
        }
        for line in paragraph
            .get(&key)
            .into_iter()
            .flat_map(|v| v.lines().map(str::to_string).collect::<Vec<_>>())
        {
            if let Some(name) = line.split_whitespace().next_back()
                && name.ends_with(&suffix)
            {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// The hash algorithms used by the checksum fields of a control file.
#[derive(Clone, Copy)]
pub enum Hash {
    Md5,
    Sha1,
    Sha256,
}

impl Hash {
    pub fn hex(self, data: &[u8]) -> String {
        fn hex<D: sha2::Digest>(mut hasher: D, data: &[u8]) -> String {
            hasher.update(data);
            hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        }
        match self {
            Hash::Md5 => hex(md5::Md5::default(), data),
            Hash::Sha1 => hex(sha1::Sha1::default(), data),
            Hash::Sha256 => hex(sha2::Sha256::default(), data),
        }
    }
}

/// The checksum fields debmagic understands, mapped to their hash.
pub const CHECKSUM_FIELDS: &[(&str, Hash)] = &[
    ("Files", Hash::Md5),
    ("Checksums-Sha1", Hash::Sha1),
    ("Checksums-Sha256", Hash::Sha256),
];

/// Rewrite one `Files:`/`Checksums-*:` line: the first token is the
/// checksum, the second the size, the last the filename; entries for
/// other files pass through unchanged.
fn rewrite_checksum_line(line: &str, filename: &str, checksum: &str, size: usize) -> String {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    match tokens.as_slice() {
        [old_checksum, old_size, middle @ .., name] if *name == filename => {
            let middle = if middle.is_empty() {
                String::new()
            } else {
                format!(" {}", middle.join(" "))
            };
            format!("{checksum} {size}{middle} {name}")
        }
        _ => line.to_string(),
    }
}

/// Rewrite the size and checksum entries for the file listings in a control file
pub fn fixup_checksums(
    paragraph: &mut Paragraph,
    filename: &str,
    data: &[u8],
) -> anyhow::Result<()> {
    let size = data.len();

    for key in paragraph.keys() {
        if key.starts_with("Checksums-") && !CHECKSUM_FIELDS.iter().any(|(field, ..)| *field == key)
        {
            // An unknown checksum format would keep a stale checksum for a
            // re-signed file, producing an upload that fails verification
            // far away from here.
            anyhow::bail!("unknown checksum field '{key}:' in control file");
        }
    }

    for (key, hash) in CHECKSUM_FIELDS {
        let Some(value) = paragraph.get(key) else {
            continue;
        };
        let checksum = hash.hex(data);
        let updated = value
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| rewrite_checksum_line(line, filename, &checksum, size))
            .collect::<Vec<_>>()
            .join("\n");
        paragraph.set(key, &updated);
    }
    Ok(())
}

/// The digests of one file, for every algorithm a control file's
/// checksum fields may use.
#[derive(Debug, Clone, Default)]
pub struct Digests {
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
}

impl Digests {
    /// Digest `data` with every checksum-field algorithm at once.
    pub fn of(data: &[u8]) -> Self {
        Self {
            md5: Hash::Md5.hex(data),
            sha1: Hash::Sha1.hex(data),
            sha256: Hash::Sha256.hex(data),
        }
    }

    fn for_hash(&self, hash: Hash) -> &str {
        match hash {
            Hash::Md5 => &self.md5,
            Hash::Sha1 => &self.sha1,
            Hash::Sha256 => &self.sha256,
        }
    }
}

/// Verify that the checksum entries a paragraph records for `filename`
/// match `digests`. Returns whether any entry was checked; a paragraph
/// listing no entry for the file is not an error.
pub fn verify_checksums(
    paragraph: &Paragraph,
    filename: &str,
    digests: &Digests,
) -> anyhow::Result<bool> {
    let mut checked = false;
    for (field, hash) in CHECKSUM_FIELDS {
        let Some(value) = paragraph.get(field) else {
            continue;
        };
        let actual = digests.for_hash(*hash);
        for line in value.lines().filter(|l| !l.is_empty()) {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.last() != Some(&filename) {
                continue;
            }
            let Some(&expected) = tokens.first() else {
                continue;
            };
            checked = true;
            if actual != expected {
                anyhow::bail!("{filename} does not match the checksum {field} recorded for it");
            }
        }
    }
    Ok(checked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_checksums_matches_and_mismatches() {
        let data = b"tarball content";
        let digests = Digests::of(data);
        let control: Paragraph = format!(
            "Format: 3.0 (quilt)\nFiles:\n {} 15 debiandev optional pkg_1.0.orig.tar.xz\nChecksums-Sha256:\n {} 15 pkg_1.0.orig.tar.xz\n",
            digests.md5, digests.sha256
        )
        .parse()
        .unwrap();

        assert!(verify_checksums(&control, "pkg_1.0.orig.tar.xz", &digests).unwrap());

        // a stale tarball fails
        let stale = Digests::of(b"different content");
        assert!(verify_checksums(&control, "pkg_1.0.orig.tar.xz", &stale).is_err());

        // a file the paragraph does not list is not checked
        assert!(!verify_checksums(&control, "other.tar.xz", &digests).unwrap());
    }

    fn parse_control(content: &str) -> Paragraph {
        content.parse().unwrap()
    }

    #[test]
    fn child_filename_finds_dsc_and_buildinfo() {
        let control = parse_control(
            "Format: 1.8\nSource: pkg\nFiles:\n abc 123 pkg_1.0.dsc\n def 456 pkg_1.0.buildinfo\n ghi 789 other.txt\nChecksums-Sha256:\n xyz 123 pkg_1.0.dsc\n",
        );
        assert_eq!(
            child_filename(&control, "dsc").as_deref(),
            Some("pkg_1.0.dsc")
        );
        assert_eq!(
            child_filename(&control, "buildinfo").as_deref(),
            Some("pkg_1.0.buildinfo")
        );
        assert_eq!(child_filename(&control, "deb"), None);
    }

    #[test]
    fn fixup_rewrites_all_checksum_sections() {
        let mut control = parse_control(
            "Format: 1.8\nFiles:\n oldmd5 3 hash optional pkg_1.0.dsc\nChecksums-Sha1:\n oldsha1 3 pkg_1.0.dsc\nChecksums-Sha256:\n oldsha256 3 pkg_1.0.dsc\n",
        );
        let data = b"abc";
        fixup_checksums(&mut control, "pkg_1.0.dsc", data).unwrap();

        let md5 = Hash::Md5.hex(data);
        let joined = control.to_string();
        assert!(joined.contains(&format!("{md5} 3 hash optional pkg_1.0.dsc")));
        assert!(joined.contains(&format!(" {} 3 pkg_1.0.dsc", Hash::Sha1.hex(data))));
        assert!(joined.contains(&format!(" {} 3 pkg_1.0.dsc", Hash::Sha256.hex(data))));
    }
}
