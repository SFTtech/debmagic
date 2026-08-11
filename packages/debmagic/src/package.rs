use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use debmagic_common::debian::version::PackageVersion;
use debmagic_common::distro::{Distro, DistroVersion, get_distro_version};

use crate::build::common::BuildDriverType;

/// Who/what is being built, as read from the source tree changelog.
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

/// How to resolve non-built-in suites for the active Driver.
#[derive(Debug, Clone, Copy)]
pub enum DistroResolveMode<'a> {
    /// Container Drivers: custom suites come from this driver's `base_images`.
    Container {
        base_images: &'a HashMap<String, String>,
        /// e.g. `driver.docker.base_images` — used in error messages.
        config_key: &'a str,
    },
    /// Bare: custom suites require a host `/etc/os-release` codename match;
    /// built-in Debian/Ubuntu also require family (`ID`) match.
    Bare {
        /// Usually `/etc/os-release`; overridable in tests.
        os_release_path: &'a Path,
    },
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
/// Suite aliases (`stable`, `sid`, `devel`, …) resolve to the same concrete
/// [`DistroVersion`] as their canonical codename.
pub fn resolve_package_target(
    dir: &Path,
    explicit_distro: Option<&str>,
    mode: DistroResolveMode<'_>,
) -> anyhow::Result<PackageTarget> {
    let parsed = read_changelog_package(dir)?;
    let distro = select_distro_version(&parsed.changelog_distros, explicit_distro, mode)?;
    Ok(PackageTarget {
        identity: parsed.identity,
        distro,
    })
}

/// Pick the resolve mode for the active Driver from its config maps.
pub fn distro_resolve_mode_for_driver<'a>(
    driver: BuildDriverType,
    docker_base_images: &'a HashMap<String, String>,
    lxd_base_images: &'a HashMap<String, String>,
) -> DistroResolveMode<'a> {
    match driver {
        BuildDriverType::Docker => DistroResolveMode::Container {
            base_images: docker_base_images,
            config_key: "driver.docker.base_images",
        },
        BuildDriverType::Lxd | BuildDriverType::Incus => DistroResolveMode::Container {
            base_images: lxd_base_images,
            config_key: "driver.lxd.base_images",
        },
        BuildDriverType::Bare => DistroResolveMode::Bare {
            os_release_path: Path::new("/etc/os-release"),
        },
    }
}

/// Parse a `base_images` map key (`family:codename`).
fn parse_base_image_key(key: &str) -> Option<(&str, &str)> {
    let (family, codename) = key.split_once(':')?;
    if family.is_empty() || codename.is_empty() || codename.contains(':') {
        return None;
    }
    Some((family, codename))
}

/// Reject custom (non-debian/ubuntu family) map keys whose codename collides
/// with a built-in release or suite alias.
fn validate_base_images_keys<'a, I>(keys: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = &'a str>,
{
    for key in keys {
        let Some((family, codename)) = parse_base_image_key(key) else {
            return Err(anyhow!(
                "invalid base_images key '{key}'; expected 'family:codename'"
            ));
        };
        match Distro::parse(family) {
            Distro::Debian | Distro::Ubuntu => {}
            Distro::Custom(_) if get_distro_version(codename).is_some() => {
                return Err(anyhow!(
                    "base_images key '{key}' uses codename '{codename}' which is a built-in \
                     Debian/Ubuntu suite; use a different codename or a debian:/ubuntu: image override"
                ));
            }
            Distro::Custom(_) => {}
        }
    }
    Ok(())
}

/// Resolve a non-built-in suite from a Driver `base_images` map by unique
/// `*:codename` match.
///
/// `config_key` is the config path shown in errors (e.g. `driver.docker.base_images`).
fn custom_distro_from_base_images(
    codename: &str,
    base_images: &HashMap<String, String>,
    config_key: &str,
) -> anyhow::Result<DistroVersion> {
    let mut matches: Vec<(&str, &str)> = base_images
        .keys()
        .filter_map(|key| parse_base_image_key(key))
        .filter(|(_, key_codename)| *key_codename == codename)
        .collect();

    matches.sort_by_key(|(family, _)| *family);
    matches.dedup();

    match matches.as_slice() {
        [] => Err(anyhow!(
            "unknown distro codename '{codename}'. To use a non-Debian/Ubuntu suite, declare it \
             in the active driver's base_images map, e.g. \
             {config_key} = {{ \"yocto:{codename}\" = \"<image>\" }}"
        )),
        [(family, _)] => Ok(DistroVersion::custom(Distro::parse(family), codename)),
        many => {
            let keys: Vec<String> = many
                .iter()
                .map(|(family, c)| format!("{family}:{c}"))
                .collect();
            Err(anyhow!(
                "ambiguous distro codename '{codename}' matches multiple base_images keys: {}",
                keys.join(", ")
            ))
        }
    }
}

fn lookup_distro(name: &str, mode: DistroResolveMode<'_>) -> anyhow::Result<DistroVersion> {
    if let DistroResolveMode::Container { base_images, .. } = mode {
        validate_base_images_keys(base_images.keys().map(|k| k.as_str()))?;
    }

    if let Some(builtin) = get_distro_version(name) {
        if let DistroResolveMode::Bare { os_release_path } = mode {
            check_bare_os_release_for_builtin(os_release_path, &builtin)?;
        }
        return Ok(builtin);
    }

    match mode {
        DistroResolveMode::Container {
            base_images,
            config_key,
        } => custom_distro_from_base_images(name, base_images, config_key),
        DistroResolveMode::Bare { os_release_path } => {
            let os = read_os_release(os_release_path)?;
            let host_codename = os.codename().ok_or_else(|| {
                anyhow!("host {} has no VERSION_CODENAME", os_release_path.display())
            })?;
            if host_codename != name {
                return Err(anyhow!(
                    "unknown distro codename '{name}' does not match host VERSION_CODENAME \
                     '{host_codename}'. For Bare builds of non-Debian/Ubuntu suites, the host \
                     codename must match; for container Drivers, declare the suite in \
                     base_images (e.g. driver.docker.base_images = {{ \"yocto:{name}\" = \"<image>\" }})"
                ));
            }
            let family = os
                .id
                .ok_or_else(|| anyhow!("host {} has no ID", os_release_path.display()))?;
            Ok(DistroVersion::custom(Distro::from(family), name))
        }
    }
}

fn check_bare_os_release_for_builtin(
    os_release_path: &Path,
    target: &DistroVersion,
) -> anyhow::Result<()> {
    if matches!(target.distro, Distro::Custom(_)) {
        return Ok(());
    }
    let os = read_os_release(os_release_path)?;
    let host_codename = os
        .codename()
        .ok_or_else(|| anyhow!("host {} has no VERSION_CODENAME", os_release_path.display()))?;
    let host_id = os
        .id
        .as_deref()
        .ok_or_else(|| anyhow!("host {} has no ID", os_release_path.display()))?;

    if host_id != target.distro.as_str() {
        return Err(anyhow!(
            "Bare build targets {} but host {} ID is '{}'",
            target.distro,
            os_release_path.display(),
            host_id
        ));
    }
    if host_codename != target.codename {
        return Err(anyhow!(
            "Bare build targets {} {} but host VERSION_CODENAME is '{}'",
            target.distro,
            target.codename,
            host_codename
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OsRelease {
    id: Option<String>,
    version_codename: Option<String>,
    ubuntu_codename: Option<String>,
}

impl OsRelease {
    fn codename(&self) -> Option<&str> {
        self.version_codename
            .as_deref()
            .or(self.ubuntu_codename.as_deref())
    }
}

fn read_os_release(path: &Path) -> anyhow::Result<OsRelease> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("failed to read {}: {e}", path.display()))?;
    Ok(parse_os_release(&contents))
}

fn parse_os_release(contents: &str) -> OsRelease {
    let mut id = None;
    let mut version_codename = None;
    let mut ubuntu_codename = None;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        match key {
            "ID" => id = Some(value.to_string()),
            "VERSION_CODENAME" => version_codename = Some(value.to_string()),
            "UBUNTU_CODENAME" => ubuntu_codename = Some(value.to_string()),
            _ => {}
        }
    }
    OsRelease {
        id,
        version_codename,
        ubuntu_codename,
    }
}

fn select_distro_version(
    changelog_distros: &[String],
    explicit_distro: Option<&str>,
    mode: DistroResolveMode<'_>,
) -> anyhow::Result<DistroVersion> {
    match (changelog_distros.len(), explicit_distro) {
        (0, _) => Err(anyhow!("changelog contains no distributions")),
        (1, None) => lookup_distro(&changelog_distros[0], mode),
        (1, Some(explicit)) => {
            let from_changelog = lookup_distro(&changelog_distros[0], mode)?;
            let from_explicit = lookup_distro(explicit, mode)?;
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
            let from_explicit = lookup_distro(explicit, mode)?;
            let matched = changelog_distros.iter().any(|name| {
                lookup_distro(name, mode)
                    .is_ok_and(|from_changelog| from_changelog == from_explicit)
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

    fn docker_mode(map: &HashMap<String, String>) -> DistroResolveMode<'_> {
        DistroResolveMode::Container {
            base_images: map,
            config_key: "driver.docker.base_images",
        }
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
        let empty = HashMap::new();
        let target = resolve_package_target(&test_package_dir(), None, docker_mode(&empty))?;
        assert_eq!(target.distro.codename, "trixie");
        assert_eq!(target.distro.distro, Distro::Debian);
        Ok(())
    }

    #[test]
    fn select_distro_version_alias_matches_canonical_explicit() -> anyhow::Result<()> {
        let empty = HashMap::new();
        let distro =
            select_distro_version(&["stable".to_string()], Some("trixie"), docker_mode(&empty))?;
        assert_eq!(distro.codename, "trixie");
        Ok(())
    }

    #[test]
    fn select_distro_version_sid_matches_unstable_explicit() -> anyhow::Result<()> {
        let empty = HashMap::new();
        let distro =
            select_distro_version(&["sid".to_string()], Some("unstable"), docker_mode(&empty))?;
        assert_eq!(distro.codename, "unstable");
        Ok(())
    }

    #[test]
    fn resolve_package_target_multiple_distros_requires_explicit() {
        let empty = HashMap::new();
        let result =
            resolve_package_target(&test_package_multi_distro_dir(), None, docker_mode(&empty));
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
        let empty = HashMap::new();
        let target = resolve_package_target(
            &test_package_multi_distro_dir(),
            Some("unstable"),
            docker_mode(&empty),
        )?;
        assert_eq!(target.identity.name, "test-package");
        assert_eq!(target.distro.codename, "unstable");
        assert_eq!(target.distro.distro, Distro::Debian);
        Ok(())
    }

    #[test]
    fn select_distro_version_single_no_explicit() -> anyhow::Result<()> {
        let empty = HashMap::new();
        let distro = select_distro_version(&["forky".to_string()], None, docker_mode(&empty))?;
        assert_eq!(distro.codename, "forky");
        assert_eq!(distro.distro, Distro::Debian);
        Ok(())
    }

    #[test]
    fn select_distro_version_single_matching_explicit() -> anyhow::Result<()> {
        let empty = HashMap::new();
        let distro =
            select_distro_version(&["forky".to_string()], Some("forky"), docker_mode(&empty))?;
        assert_eq!(distro.codename, "forky");
        Ok(())
    }

    #[test]
    fn select_distro_version_single_conflicting_explicit() {
        let empty = HashMap::new();
        let result =
            select_distro_version(&["forky".to_string()], Some("duke"), docker_mode(&empty));
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
        let empty = HashMap::new();
        let result = select_distro_version(
            &["forky".to_string(), "duke".to_string()],
            None,
            docker_mode(&empty),
        );
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
        let empty = HashMap::new();
        let distro = select_distro_version(
            &["forky".to_string(), "duke".to_string()],
            Some("duke"),
            docker_mode(&empty),
        )?;
        assert_eq!(distro.codename, "duke");
        Ok(())
    }

    #[test]
    fn select_distro_version_multiple_explicit_invalid() {
        let empty = HashMap::new();
        let result = select_distro_version(
            &["forky".to_string(), "duke".to_string()],
            Some("trixie"),
            docker_mode(&empty),
        );
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
        let empty = HashMap::new();
        let result = select_distro_version(&[], None, docker_mode(&empty));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("changelog contains no distributions")
        );
    }

    #[test]
    fn select_custom_distro_from_docker_base_images() -> anyhow::Result<()> {
        let mut map = HashMap::new();
        map.insert(
            "yocto:kirkstone".to_string(),
            "registry.example/yocto-kirkstone:latest".to_string(),
        );
        let distro = select_distro_version(&["kirkstone".to_string()], None, docker_mode(&map))?;
        assert_eq!(distro.distro, Distro::Custom("yocto".into()));
        assert_eq!(distro.codename, "kirkstone");
        assert_eq!(distro.version, "");
        Ok(())
    }

    #[test]
    fn select_unknown_without_base_images_errors_with_hint() {
        let empty = HashMap::new();
        let err = select_distro_version(&["kirkstone".to_string()], None, docker_mode(&empty))
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown distro codename 'kirkstone'"));
        assert!(err.contains("driver.docker.base_images"));
        assert!(err.contains("yocto:kirkstone"));
    }

    #[test]
    fn select_rejects_colliding_custom_base_images_key() {
        let mut map = HashMap::new();
        map.insert("yocto:trixie".to_string(), "img".to_string());
        let err =
            select_distro_version(&["trixie".to_string()], None, docker_mode(&map)).unwrap_err();
        assert!(err.to_string().contains("built-in"));
    }

    #[test]
    fn select_allows_debian_base_image_override() -> anyhow::Result<()> {
        let mut map = HashMap::new();
        map.insert("debian:trixie".to_string(), "my-trixie:latest".to_string());
        let distro = select_distro_version(&["trixie".to_string()], None, docker_mode(&map))?;
        assert_eq!(distro.distro, Distro::Debian);
        assert_eq!(distro.codename, "trixie");
        Ok(())
    }

    #[test]
    fn parse_os_release_prefers_version_codename() {
        let os = parse_os_release(
            r#"
ID=debian
VERSION_CODENAME=trixie
UBUNTU_CODENAME=ignored
"#,
        );
        assert_eq!(os.id.as_deref(), Some("debian"));
        assert_eq!(os.codename(), Some("trixie"));
    }

    #[test]
    fn bare_builtin_requires_id_and_codename_match() -> anyhow::Result<()> {
        let os_path = std::env::temp_dir().join(format!(
            "debmagic-os-release-builtin-{}",
            std::process::id()
        ));
        std::fs::write(&os_path, "ID=debian\nVERSION_CODENAME=bookworm\n")?;
        let mode = DistroResolveMode::Bare {
            os_release_path: &os_path,
        };
        let err = select_distro_version(&["trixie".to_string()], None, mode).unwrap_err();
        assert!(err.to_string().contains("VERSION_CODENAME"));

        std::fs::write(&os_path, "ID=ubuntu\nVERSION_CODENAME=trixie\n")?;
        let err = select_distro_version(&["trixie".to_string()], None, mode).unwrap_err();
        assert!(err.to_string().contains("ID is 'ubuntu'"));

        std::fs::write(&os_path, "ID=debian\nVERSION_CODENAME=trixie\n")?;
        let distro = select_distro_version(&["trixie".to_string()], None, mode)?;
        assert_eq!(distro.distro, Distro::Debian);
        assert_eq!(distro.codename, "trixie");
        let _ = std::fs::remove_file(&os_path);
        Ok(())
    }

    #[test]
    fn bare_custom_matches_codename_only() -> anyhow::Result<()> {
        let os_path =
            std::env::temp_dir().join(format!("debmagic-os-release-custom-{}", std::process::id()));
        std::fs::write(&os_path, "ID=yocto\nVERSION_CODENAME=kirkstone\n")?;
        let mode = DistroResolveMode::Bare {
            os_release_path: &os_path,
        };
        let distro = select_distro_version(&["kirkstone".to_string()], None, mode)?;
        assert_eq!(distro.distro, Distro::Custom("yocto".into()));
        assert_eq!(distro.codename, "kirkstone");
        assert_eq!(distro.version, "");
        let _ = std::fs::remove_file(&os_path);
        Ok(())
    }
}
