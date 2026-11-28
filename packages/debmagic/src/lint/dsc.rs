use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Context;

use super::debian_control::ControlParagraph;

/// Parsed single Deb822 paragraph of a Source package's `.dsc` (Policy 5.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dsc {
    fields: ControlParagraph,
}

impl Dsc {
    /// Load from a `.dsc` path. A missing paragraph or Deb822/PGP failure is
    /// a runtime error, not an empty Debian control.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {} failed", path.display()))?;
        Self::parse(&text).with_context(|| format!("{} is not a readable .dsc", path.display()))
    }

    /// `Err` on PGP/Deb822 failure or a missing paragraph.
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        Ok(Self {
            fields: ControlParagraph::parse_first_signed(text, "Dsc")?,
        })
    }

    pub fn fields(&self) -> &ControlParagraph {
        &self.fields
    }

    /// lintian's `files` keys (`Processable::Fields::Files`), sorted: the last
    /// word of each `Files` line and the third word of each `Checksums-Sha1`
    /// and `Checksums-Sha256` line, blank lines skipped, any name with a `/`
    /// left out. Not every file `dpkg-source` needs; see
    /// `debmagic_common::checksums::filenames`.
    pub fn files(&self) -> Vec<String> {
        let mut names = BTreeSet::new();
        for line in self.fields.get("Files").unwrap_or_default().lines() {
            if let Some(name) = line.split_whitespace().last() {
                names.insert(name);
            }
        }
        for field in ["Checksums-Sha1", "Checksums-Sha256"] {
            for line in self.fields.get(field).unwrap_or_default().lines() {
                if let Some(name) = line.split_whitespace().nth(2) {
                    names.insert(name);
                }
            }
        }
        names
            .into_iter()
            .filter(|name| !name.contains('/'))
            .map(str::to_string)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parse_reads_fields() -> anyhow::Result<()> {
        let dsc = Dsc::parse(
            "Format: 3.0 (native)\n\
             Source: example\n\
             Version: 1.0\n\
             Files:\n d41d8cd98f00b204e9800998ecf8427e 1 empty\n",
        )?;
        assert_eq!(dsc.fields().get("Source"), Some("example"));
        assert!(dsc.fields().declares("Files"));
        assert!(!dsc.fields().declares("Standards-Version"));
        Ok(())
    }

    #[test]
    fn files_are_lintians_listed_names() {
        let dsc = Dsc::parse(
            "Source: example\n\
             Files:\n 0 1 b.tar.xz\n 0 1 ../escape.tar.xz\n\
             Checksums-Sha256:\n 0 1 c.asc extra\n 0 1 b.tar.xz\n short line\n",
        )
        .expect("dsc");
        // Checksums lines give their third word, not their last.
        assert_eq!(dsc.files(), ["b.tar.xz", "c.asc"]);
    }

    #[test]
    fn empty_is_not_a_paragraph() {
        assert!(Dsc::parse("").is_err());
    }

    #[test]
    fn load_strips_pgp() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example.dsc");
        fs::write(
            &path,
            "-----BEGIN PGP SIGNED MESSAGE-----\n\
             Hash: SHA256\n\
             \n\
             Format: 3.0 (native)\n\
             Source: example\n\
             \n\
             -----BEGIN PGP SIGNATURE-----\n\
             \n\
             fakesig\n\
             -----END PGP SIGNATURE-----\n",
        )?;
        let dsc = Dsc::load(&path)?;
        assert_eq!(dsc.fields().get("Source"), Some("example"));
        Ok(())
    }
}
