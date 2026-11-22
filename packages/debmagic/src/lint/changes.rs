use std::path::Path;

use anyhow::Context;

use super::debian_control::ControlParagraph;
use super::file_list::FileList;

/// Parsed first Deb822 paragraph of a `.changes` file (Policy 5.5), and the
/// files it names. A national-encoding file is read lossily, as the text of a
/// news item is not what a Rule looks at. Absent from every Subject but a
/// Group's Changes file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    fields: ControlParagraph,
    files: FileList,
}

// The Changes file is an inspectable of its own; Rules for the Tags that read
// the Distribution, Urgency, Changed-By, Binary and Description fields are
// follow-on work (see TODO-lint.md), so those accessors have no caller yet.
#[allow(dead_code)]
impl Changes {
    /// Load from a `.changes` path. A missing file or a file that is not a
    /// Deb822 paragraph is a runtime error.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("reading {} failed", path.display()))?;
        Self::parse(&String::from_utf8_lossy(&bytes))
            .with_context(|| format!("{} is not a valid changes file", path.display()))
    }

    /// `Err` on PGP/Deb822 failure or a missing paragraph.
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let fields = ControlParagraph::parse_first_signed(text, "Changes")?;
        let files = FileList::parse(&fields, true);
        Ok(Self { fields, files })
    }

    #[cfg(test)]
    pub(crate) fn from_fields(
        fields: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        let fields = ControlParagraph::from_fields(fields);
        let files = FileList::parse(&fields, true);
        Self { fields, files }
    }

    pub fn fields(&self) -> &ControlParagraph {
        &self.fields
    }

    /// The `Files` and `Checksums-*` entries.
    pub fn files(&self) -> &FileList {
        &self.files
    }

    /// `Source`, trimmed. A binNMU changes file has `name (version)` here.
    pub fn source(&self) -> Option<&str> {
        self.fields.get("Source").map(str::trim)
    }

    pub fn version(&self) -> Option<&str> {
        self.fields.get("Version").map(str::trim)
    }

    pub fn architecture(&self) -> Option<&str> {
        self.fields.get("Architecture").map(str::trim)
    }

    /// `Distribution` as written.
    pub fn distribution(&self) -> Option<&str> {
        self.fields.get("Distribution").map(str::trim)
    }

    /// The words of `Distribution`.
    pub fn distributions(&self) -> Vec<&str> {
        self.distribution()
            .map(|value| value.split_whitespace().collect())
            .unwrap_or_default()
    }

    pub fn urgency(&self) -> Option<&str> {
        self.fields.get("Urgency").map(str::trim)
    }

    pub fn maintainer(&self) -> Option<&str> {
        self.fields.get("Maintainer").map(str::trim)
    }

    pub fn changed_by(&self) -> Option<&str> {
        self.fields.get("Changed-By").map(str::trim)
    }

    /// The names in `Binary`.
    pub fn binaries(&self) -> Vec<&str> {
        self.fields
            .get("Binary")
            .map(|value| value.split_whitespace().collect())
            .unwrap_or_default()
    }

    /// `Description`, one line per binary, lines as written.
    pub fn description(&self) -> Option<&str> {
        self.fields.get("Description")
    }

    pub fn format(&self) -> Option<&str> {
        self.fields.get("Format").map(str::trim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const SAMPLE: &str = "Format: 1.8\n\
        Source: example\n\
        Binary: example example-doc\n\
        Architecture: source all\n\
        Version: 1.0-1\n\
        Distribution: unstable experimental\n\
        Urgency: low\n\
        Maintainer: A <a@example.org>\n\
        Changed-By: B <b@example.org>\n\
        Description:\n example - one\n example-doc - two\n\
        Files:\n d41d8cd98f00b204e9800998ecf8427e 0 doc optional example_1.0-1.dsc\n\
        Checksums-Sha1:\n da39a3ee5e6b4b0d3255bfef95601890afd80709 0 example_1.0-1.dsc\n";

    #[test]
    fn parse_reads_fields_and_files() -> anyhow::Result<()> {
        let changes = Changes::parse(SAMPLE)?;
        assert_eq!(changes.source(), Some("example"));
        assert_eq!(changes.version(), Some("1.0-1"));
        assert_eq!(changes.distributions(), ["unstable", "experimental"]);
        assert_eq!(changes.urgency(), Some("low"));
        assert_eq!(changes.changed_by(), Some("B <b@example.org>"));
        assert_eq!(changes.binaries(), ["example", "example-doc"]);
        let file = changes.files().get("example_1.0-1.dsc").expect("dsc");
        assert_eq!(file.section(), Some("doc"));
        assert_eq!(file.size(), Some(0));
        assert_eq!(file.checksums().len(), 2);
        Ok(())
    }

    #[test]
    fn empty_is_not_a_paragraph() {
        assert!(Changes::parse("").is_err());
    }

    #[test]
    fn load_strips_pgp_and_reads_national_encoding() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let path = dir.path().join("example_1.0-1_source.changes");
        let bytes = b"-----BEGIN PGP SIGNED MESSAGE-----\nHash: SHA256\n\nSource: example\nChanges:\n example (1.0-1) unstable; urgency=low\n .\n   * Bokm\xe5l\n\n-----BEGIN PGP SIGNATURE-----\n\nfakesig\n-----END PGP SIGNATURE-----\n".to_vec();
        fs::write(&path, bytes)?;
        let changes = Changes::load(&path)?;
        assert_eq!(changes.source(), Some("example"));
        Ok(())
    }

    #[test]
    fn load_names_the_file_on_failure() {
        let error = Changes::load(Path::new("/nonexistent/x.changes")).unwrap_err();
        assert!(error.to_string().contains("x.changes"), "{error}");
    }
}
