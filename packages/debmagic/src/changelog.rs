use std::path::Path;

use anyhow::Context;

use debmagic_common::debian::source::SourceFormat;

/// Read `debian/source/format`; a missing file is `1.0`, like dpkg-source.
pub fn load_source_format(dir: &Path) -> SourceFormat {
    std::fs::read_to_string(dir.join("debian").join("source").join("format"))
        .map(|content| SourceFormat::parse(&content))
        .unwrap_or_default()
}

/// Read and parse `debian/changelog`.
pub fn load_changelog(dir: &Path) -> anyhow::Result<debian_changelog::ChangeLog> {
    let path = dir.join("debian").join("changelog");
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    content
        .parse()
        .with_context(|| format!("failed to parse {}", path.display()))
}

/// The head entry of `debian/changelog`, as the package identity.
pub fn load_changelog_head(
    dir: &Path,
) -> anyhow::Result<debmagic_common::debian::changelog::ChangelogHead> {
    let changelog = load_changelog(dir)?;
    changelog_head(&changelog).with_context(|| {
        format!(
            "failed to read the head entry of {}",
            dir.join("debian").join("changelog").display()
        )
    })
}

/// Extract the head entry of a parsed changelog.
pub fn changelog_head(
    changelog: &debian_changelog::ChangeLog,
) -> anyhow::Result<debmagic_common::debian::changelog::ChangelogHead> {
    use anyhow::bail;

    if changelog.iter().next().is_none() {
        bail!("changelog is empty");
    }
    debmagic_common::debian::changelog::ChangelogHead::from_changelog(changelog).ok_or_else(|| {
        anyhow::anyhow!("changelog head entry has no package, version or distribution")
    })
}
