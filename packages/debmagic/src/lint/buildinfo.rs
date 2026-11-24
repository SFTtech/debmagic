use std::path::Path;

use anyhow::Context;

use super::debian_control::ControlParagraph;
use super::file_list::FileList;
use super::package_relation::PackageRelation;

/// Parsed first Deb822 paragraph of a `.buildinfo` file (deb-buildinfo(5)), and
/// the files it names. Absent from every Subject but a Group's Buildinfo file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Buildinfo {
    fields: ControlParagraph,
    files: FileList,
}

// The Buildinfo file is an inspectable of its own; the Tags that read it are
// follow-on work (see TODO-lint.md), so these accessors have no caller yet.
#[allow(dead_code)]
impl Buildinfo {
    /// Load from a `.buildinfo` path. A missing file or a file that is not a
    /// Deb822 paragraph is a runtime error.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("reading {} failed", path.display()))?;
        Self::parse(&String::from_utf8_lossy(&bytes))
            .with_context(|| format!("{} is not a valid buildinfo file", path.display()))
    }

    /// `Err` on PGP/Deb822 failure or a missing paragraph.
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let fields = ControlParagraph::parse_first_signed(text, "Buildinfo")?;
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

    pub fn source(&self) -> Option<&str> {
        self.fields.get("Source").map(str::trim)
    }

    pub fn version(&self) -> Option<&str> {
        self.fields.get("Version").map(str::trim)
    }

    pub fn architecture(&self) -> Option<&str> {
        self.fields.get("Architecture").map(str::trim)
    }

    pub fn format(&self) -> Option<&str> {
        self.fields.get("Format").map(str::trim)
    }

    /// The names in `Binary`.
    pub fn binaries(&self) -> Vec<&str> {
        self.fields
            .get("Binary")
            .map(|value| value.split_whitespace().collect())
            .unwrap_or_default()
    }

    pub fn build_architecture(&self) -> Option<&str> {
        self.fields.get("Build-Architecture").map(str::trim)
    }

    pub fn build_origin(&self) -> Option<&str> {
        self.fields.get("Build-Origin").map(str::trim)
    }

    /// `Installed-Build-Depends` as a Package relation.
    pub(crate) fn installed_build_depends(&self) -> Option<PackageRelation> {
        self.fields.relation("Installed-Build-Depends")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Format: 1.0\n\
        Source: example\n\
        Binary: example\n\
        Architecture: amd64 source\n\
        Version: 1.0-1\n\
        Build-Origin: Debian\n\
        Build-Architecture: amd64\n\
        Checksums-Sha256:\n e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 0 example_1.0-1.dsc\n\
        Installed-Build-Depends:\n debhelper (= 13),\n libc6 (= 2.36)\n";

    #[test]
    fn parse_reads_fields_and_files() -> anyhow::Result<()> {
        let buildinfo = Buildinfo::parse(SAMPLE)?;
        assert_eq!(buildinfo.source(), Some("example"));
        assert_eq!(buildinfo.build_architecture(), Some("amd64"));
        assert_eq!(buildinfo.binaries(), ["example"]);
        assert_eq!(buildinfo.files().files().len(), 1);
        let depends = buildinfo.installed_build_depends().expect("declared");
        assert_eq!(depends.alternatives().count(), 2);
        Ok(())
    }

    #[test]
    fn empty_is_not_a_paragraph() {
        assert!(Buildinfo::parse("").is_err());
    }
}
