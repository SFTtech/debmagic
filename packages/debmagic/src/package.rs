use std::path::{Path, PathBuf};

use anyhow::anyhow;

use debmagic_common::debian::version::PackageVersion;

#[derive(Debug, Clone)]
pub struct PackageDescription {
    pub name: String,
    pub version: PackageVersion,
    pub source_dir: PathBuf,
    pub distro_versions: Vec<String>,
}

impl PackageDescription {
    pub fn from_dir(dir: &Path) -> anyhow::Result<Self> {
        let changelog_file = dir.join("debian").join("changelog");
        let changelog_contents = std::fs::read_to_string(changelog_file)?;
        let changelog: debian_changelog::ChangeLog = changelog_contents.parse()?;

        let first_entry = changelog
            .into_iter()
            .next()
            .ok_or(anyhow!("changelog is empty"))?;

        let name = first_entry
            .package()
            .ok_or(anyhow!("empty package name in changelog entry"))?;
        let version = first_entry
            .version()
            .ok_or(anyhow!("no package version in changelog entry"))
            .map(|v| PackageVersion::new(v.epoch, v.upstream_version, v.debian_revision))?;

        let distro_versions = first_entry
            .distributions()
            .ok_or(anyhow!("no distribution specified in changelog entry"))?;

        Ok(Self {
            name,
            version,
            source_dir: dir.to_path_buf(),
            distro_versions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_description_from_changelog() -> Result<(), anyhow::Error> {
        let test_asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join("test_package");

        let package = PackageDescription::from_dir(&test_asset_dir)?;

        assert_eq!(package.name, "test-package");
        assert_eq!(package.version.version(), "1.2.4-1");
        assert_eq!(package.distro_versions, vec!["stable"]);
        assert_eq!(package.source_dir, test_asset_dir);

        Ok(())
    }

    #[test]
    fn test_package_description_with_multiple_distros() -> Result<(), anyhow::Error> {
        let test_asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join("test_package_multi_distro");

        let package = PackageDescription::from_dir(&test_asset_dir)?;

        assert_eq!(package.name, "test-package");
        assert_eq!(package.version.version(), "1.2.4-1");
        assert_eq!(package.distro_versions, vec!["unstable", "testing"]);
        assert_eq!(package.source_dir, test_asset_dir);

        Ok(())
    }
}
