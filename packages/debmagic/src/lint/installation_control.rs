use anyhow::{Context, bail};
use debian_control::lossless::Control;

use super::binary_package_reader::BinaryPackageReader;
use super::debian_control::ControlParagraph;

/// Parsed Installation control: the single Deb822 paragraph in a Binary package.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallationControl {
    fields: ControlParagraph,
}

impl InstallationControl {
    /// Load from a Binary package reader. Not one Deb822 paragraph is a
    /// runtime failure, not a missing Debian control.
    pub fn from_reader(reader: &BinaryPackageReader) -> anyhow::Result<Self> {
        let path = reader.path();
        let Some(bytes) = reader.read_control_file("control")? else {
            bail!("installation control in {} is missing", path.display());
        };
        let text = std::str::from_utf8(bytes)
            .with_context(|| format!("installation control in {} is not UTF-8", path.display()))?;
        let control: Control = text.parse().with_context(|| {
            format!(
                "installation control in {} is not valid Deb822",
                path.display()
            )
        })?;
        let mut paragraphs = control.as_deb822().paragraphs();
        let Some(first) = paragraphs.next() else {
            bail!(
                "installation control in {} has no Deb822 paragraph",
                path.display()
            );
        };
        if paragraphs.next().is_some() {
            bail!(
                "installation control in {} is not exactly one Deb822 paragraph",
                path.display()
            );
        }
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
    use crate::lint::binary_package_reader::{BinaryPackageReader, write_test_deb};
    use tempfile::TempDir;

    #[test]
    fn loads_single_paragraph() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        write_test_deb(
            &path,
            "Package: example\n\
             Version: 1.0\n\
             Architecture: all\n\
             Maintainer: Example <ex@example.com>\n\
             Description: example\n extra\n",
        )?;
        let reader = BinaryPackageReader::from_path(&path)?;
        let control = InstallationControl::from_reader(&reader)?;
        assert_eq!(control.fields().get("Package"), Some("example"));
        assert!(control.fields().declares("Description"));
        Ok(())
    }

    #[test]
    fn two_paragraphs_are_a_runtime_failure() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example.deb");
        write_test_deb(&path, "Package: example\n\nPackage: other\n")?;
        let reader = BinaryPackageReader::from_path(&path)?;
        let error = InstallationControl::from_reader(&reader).unwrap_err();
        assert!(error.to_string().contains("exactly one"), "{error}");
        Ok(())
    }
}
