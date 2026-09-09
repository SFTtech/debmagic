use std::path::Path;

use anyhow::{Context, bail};
use debian_control::lossless::Control;

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
        let (payload, _) =
            debian_control::pgp::strip_pgp_signature(text).context("Dsc is not readable Deb822")?;
        let control: Control = payload.parse().context("Dsc is not valid Deb822")?;
        let mut paragraphs = control.as_deb822().paragraphs();
        let Some(first) = paragraphs.next() else {
            bail!("Dsc has no Deb822 paragraph");
        };
        Ok(Self {
            fields: ControlParagraph::from_items(first.items()),
        })
    }

    pub fn fields(&self) -> &ControlParagraph {
        &self.fields
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
