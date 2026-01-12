use std::{path::PathBuf, str::FromStr};

use anyhow::anyhow;

use debian_changelog::ChangeLog;
use debmagic_common::debian::version::PackageVersion;

pub struct PackageDescription {
    pub name: String,
    pub version: PackageVersion,
}

impl PackageDescription {
    pub fn from_dir(dir: &PathBuf) -> anyhow::Result<Self> {
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
        let version: PackageVersion = first_entry
            .version()
            .ok_or(anyhow!("no package version in changelog entry"))
            .map(|v| PackageVersion::from_str(v))
            .map_err(|| anyhow!("invalid package version in changelog entry"))?;

        Self { name, version }
    }
}
