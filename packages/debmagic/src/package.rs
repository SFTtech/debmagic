use std::path::{Path, PathBuf};

use anyhow::anyhow;

use debmagic_common::debian::version::PackageVersion;

#[derive(Debug, Clone)]
pub struct PackageDescription {
    pub name: String,
    pub version: PackageVersion,
    pub source_dir: PathBuf,
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

        Ok(Self {
            name,
            version,
            source_dir: dir.to_path_buf(),
        })
    }
}
