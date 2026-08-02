use std::path::{Path, PathBuf};

use anyhow::anyhow;
use debmagic_common::debian::version::PackageVersion;
use debmagic_common::distro::DistroVersion;

/// Who/what is being built, without a chosen target distro.
#[derive(Debug, Clone)]
pub struct PackageIdentity {
    pub name: String,
    pub version: PackageVersion,
    pub source_dir: PathBuf,
}

/// A [`PackageIdentity`] plus the chosen [`DistroVersion`] for a build run.
#[derive(Debug, Clone)]
pub struct PackageTarget {
    pub identity: PackageIdentity,
    pub distro: DistroVersion,
}

struct ChangelogPackage {
    identity: PackageIdentity,
    /// Raw distribution names from the changelog entry (not looked up yet).
    changelog_distros: Vec<String>,
}

fn read_changelog_package(dir: &Path) -> anyhow::Result<ChangelogPackage> {
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

    let changelog_distros = first_entry
        .distributions()
        .ok_or(anyhow!("no distribution specified in changelog entry"))?;

    Ok(ChangelogPackage {
        identity: PackageIdentity {
            name,
            version,
            source_dir: dir.to_path_buf(),
        },
        changelog_distros,
    })
}

pub fn load_package_identity(dir: &Path) -> anyhow::Result<PackageIdentity> {
    Ok(read_changelog_package(dir)?.identity)
}

/// Resolve package identity and target distro from a source tree.
///
/// If only one distribution is listed in the changelog, it is used automatically.
/// If multiple are listed, an explicit `--distro` is required and must match one of them.
/// The chosen name must be known to distro knowledge (suite aliases are not resolved here).
pub fn resolve_package_target(
    dir: &Path,
    explicit_distro: Option<&str>,
) -> anyhow::Result<PackageTarget> {
    let parsed = read_changelog_package(dir)?;
    let distro = select_distro_version(&parsed.changelog_distros, explicit_distro)?;
    Ok(PackageTarget {
        identity: parsed.identity,
        distro,
    })
}

fn lookup_distro(name: &str) -> anyhow::Result<DistroVersion> {
    debmagic_common::distro::get_distro_version(name)
        .ok_or_else(|| anyhow!("unknown distro codename '{}'", name))
}

fn select_distro_version(
    changelog_distros: &[String],
    explicit_distro: Option<&str>,
) -> anyhow::Result<DistroVersion> {
    match (changelog_distros.len(), explicit_distro) {
        (0, _) => Err(anyhow!("changelog contains no distributions")),
        (1, None) => lookup_distro(&changelog_distros[0]),
        (1, Some(explicit)) => {
            let from_changelog = lookup_distro(&changelog_distros[0])?;
            let from_explicit = lookup_distro(explicit)?;
            if from_changelog == from_explicit {
                Ok(from_explicit)
            } else {
                Err(anyhow!(
                    "explicit distro version '{}' conflicts with distribution specified in changelog '{}'",
                    explicit,
                    changelog_distros[0]
                ))
            }
        }
        (_, None) => Err(anyhow!(
            "changelog contains multiple distributions ({}), please specify which one to build for with --distro",
            changelog_distros.join(", ")
        )),
        (_, Some(explicit)) => {
            let from_explicit = lookup_distro(explicit)?;
            let matched = changelog_distros.iter().any(|name| {
                lookup_distro(name).is_ok_and(|from_changelog| from_changelog == from_explicit)
            });
            if matched {
                Ok(from_explicit)
            } else {
                Err(anyhow!(
                    "explicit distro version '{}' not found in changelog distributions: {}",
                    explicit,
                    changelog_distros.join(", ")
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use debmagic_common::distro::Distro;

    use super::*;

    fn test_package_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join("test_package")
    }

    fn test_package_multi_distro_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join("test_package_multi_distro")
    }

    #[test]
    fn load_package_identity_from_changelog() -> anyhow::Result<()> {
        let dir = test_package_dir();
        let identity = load_package_identity(&dir)?;

        assert_eq!(identity.name, "test-package");
        assert_eq!(identity.version.version(), "1.2.4-1");
        assert_eq!(identity.source_dir, dir);
        Ok(())
    }

    #[test]
    fn resolve_package_target_stable_aliases_to_trixie() -> anyhow::Result<()> {
        let target = resolve_package_target(&test_package_dir(), None)?;
        assert_eq!(target.distro.codename, "trixie");
        assert_eq!(target.distro.distro, Distro::Debian);
        Ok(())
    }

    #[test]
    fn select_distro_version_alias_matches_canonical_explicit() -> anyhow::Result<()> {
        let distro = select_distro_version(&["stable".to_string()], Some("trixie"))?;
        assert_eq!(distro.codename, "trixie");
        Ok(())
    }

    #[test]
    fn select_distro_version_sid_matches_unstable_explicit() -> anyhow::Result<()> {
        let distro = select_distro_version(&["sid".to_string()], Some("unstable"))?;
        assert_eq!(distro.codename, "unstable");
        Ok(())
    }

    #[test]
    fn resolve_package_target_multiple_distros_requires_explicit() {
        let result = resolve_package_target(&test_package_multi_distro_dir(), None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("multiple distributions")
        );
    }

    #[test]
    fn resolve_package_target_multiple_distros_with_explicit() -> anyhow::Result<()> {
        let target = resolve_package_target(&test_package_multi_distro_dir(), Some("unstable"))?;
        assert_eq!(target.identity.name, "test-package");
        assert_eq!(target.distro.codename, "unstable");
        assert_eq!(target.distro.distro, Distro::Debian);
        Ok(())
    }

    #[test]
    fn select_distro_version_single_no_explicit() -> anyhow::Result<()> {
        let distro = select_distro_version(&["forky".to_string()], None)?;
        assert_eq!(distro.codename, "forky");
        assert_eq!(distro.distro, Distro::Debian);
        Ok(())
    }

    #[test]
    fn select_distro_version_single_matching_explicit() -> anyhow::Result<()> {
        let distro = select_distro_version(&["forky".to_string()], Some("forky"))?;
        assert_eq!(distro.codename, "forky");
        Ok(())
    }

    #[test]
    fn select_distro_version_single_conflicting_explicit() {
        let result = select_distro_version(&["forky".to_string()], Some("duke"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("conflicts with distribution specified in changelog")
        );
    }

    #[test]
    fn select_distro_version_multiple_no_explicit() {
        let result = select_distro_version(&["forky".to_string(), "duke".to_string()], None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("multiple distributions")
        );
    }

    #[test]
    fn select_distro_version_multiple_explicit_valid() -> anyhow::Result<()> {
        let distro =
            select_distro_version(&["forky".to_string(), "duke".to_string()], Some("duke"))?;
        assert_eq!(distro.codename, "duke");
        Ok(())
    }

    #[test]
    fn select_distro_version_multiple_explicit_invalid() {
        let result =
            select_distro_version(&["forky".to_string(), "duke".to_string()], Some("trixie"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not found in changelog distributions")
        );
    }

    #[test]
    fn select_distro_version_empty_distros() {
        let result = select_distro_version(&[], None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("changelog contains no distributions")
        );
    }
}
