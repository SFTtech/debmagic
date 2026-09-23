use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, anyhow, bail};
use debmagic_common::distro::{Distro, DistroVersion, get_distro_version};
use debmagic_common::package::{FileReader, Location, SourcePackage};

use crate::driver::DriverType;

/// A [`Package`] plus the chosen [`DistroVersion`] for a build run.
#[derive(Debug, Clone)]
pub struct PackageTarget {
    pub package: SourcePackage,
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
    /// Bare driver: the system running debmagic is used directly.
    Bare,
}

/// Read a source tree's `debian/` metadata into the common [`Package`] model.
/// The package reads its files lazily through a closure over the source
/// dir, so only the metadata actually requested is ever read.
pub fn load_package(dir: &Path) -> anyhow::Result<SourcePackage> {
    let source_dir = dir.to_path_buf();
    let reader_dir = source_dir.clone();
    let reader: FileReader = Box::new(move |name| {
        let path = reader_dir.join("debian").join(name);
        std::fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))
    });
    SourcePackage::from_reader(std::rc::Rc::new(reader), Location::SourceDir(source_dir))
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
    let package = load_package(dir)?;
    let distro = select_distro_version(package.distributions(), explicit_distro, mode)?;
    Ok(PackageTarget { package, distro })
}

/// Pick the resolve mode for the active Driver from its config maps.
pub fn distro_resolve_mode_for_driver<'a>(
    driver: DriverType,
    docker_base_images: &'a HashMap<String, String>,
    lxd_base_images: &'a HashMap<String, String>,
) -> DistroResolveMode<'a> {
    match driver {
        DriverType::Docker => DistroResolveMode::Container {
            base_images: docker_base_images,
            config_key: "driver.docker.base_images",
        },
        DriverType::Lxd | DriverType::Incus => DistroResolveMode::Container {
            base_images: lxd_base_images,
            config_key: "driver.lxd.base_images",
        },
        DriverType::Bare => DistroResolveMode::Bare,
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
        return Ok(builtin);
    }

    match mode {
        DistroResolveMode::Container {
            base_images,
            config_key,
        } => custom_distro_from_base_images(name, base_images, config_key),
        DistroResolveMode::Bare => Ok(DistroVersion::custom(Distro::from(name), name)),
    }
}

/// A binary build on the Bare driver runs directly on the host, so the host
/// must provide the target environment: built-in Debian/Ubuntu targets need
/// a matching `ID` and codename in the host `os-release`, custom suites a
/// matching codename (their placeholder family is anchored to the host `ID`).
/// Source builds skip this check — their artifacts are distro-independent.
pub fn validate_bare_host_target(
    target: &DistroVersion,
    os_release_path: &Path,
) -> anyhow::Result<DistroVersion> {
    let os = read_os_release(os_release_path)?;
    let host_id = os
        .id
        .as_deref()
        .ok_or_else(|| anyhow!("host {} has no ID", os_release_path.display()))?;
    let host_codename = os.codename().ok_or_else(|| {
        anyhow!(
            "host {} has no VERSION_CODENAME, cannot verify it for a bare binary build",
            os_release_path.display()
        )
    })?;

    if let Distro::Custom(_) = target.distro {
        if target.codename != host_codename {
            bail!(
                "bare binary build targets custom suite '{}' but host VERSION_CODENAME is \
                 '{host_codename}'; the host must match, or use a container driver with the \
                 suite declared in base_images",
                target.codename
            );
        }
        return Ok(DistroVersion::custom(
            Distro::from(host_id),
            &target.codename,
        ));
    }

    if host_id != target.distro.as_str() {
        bail!(
            "bare binary build targets {} but host {} ID is '{host_id}'",
            target.distro,
            os_release_path.display()
        );
    }
    if target.codename != host_codename {
        bail!(
            "bare binary build targets {} {} but host VERSION_CODENAME is '{host_codename}'",
            target.distro,
            target.codename
        );
    }
    Ok(target.clone())
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
    let Some(explicit) = explicit_distro else {
        return match changelog_distros {
            [] => bail!("changelog contains no distributions"),
            [single] => lookup_distro(single, mode),
            many => bail!(
                "changelog contains multiple distributions ({}), please specify which one to build for with --distro",
                many.join(", ")
            ),
        };
    };

    let resolved = lookup_distro(explicit, mode)?;
    let in_changelog = changelog_distros
        .iter()
        .any(|name| name == explicit || lookup_distro(name, mode).is_ok_and(|d| d == resolved));
    if !in_changelog {
        println!(
            "debmagic: building for '{explicit}', changelog targets {}",
            changelog_distros.join(", ")
        );
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
    fn load_package_from_changelog() -> anyhow::Result<()> {
        let dir = test_package_dir();
        let package = load_package(&dir)?;

        assert_eq!(package.name(), "test-package");
        assert_eq!(package.version().version(), "1.2.4-1");
        assert_eq!(package.source_dir()?, dir);
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
        assert_eq!(target.package.name(), "test-package");
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
    fn select_distro_version_single_conflicting_explicit_overrides() -> anyhow::Result<()> {
        let empty = HashMap::new();
        let distro =
            select_distro_version(&["forky".to_string()], Some("duke"), docker_mode(&empty))?;
        assert_eq!(distro.codename, "duke");
        Ok(())
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
    fn select_distro_version_multiple_explicit_not_in_changelog_overrides() -> anyhow::Result<()> {
        let empty = HashMap::new();
        let distro = select_distro_version(
            &["forky".to_string(), "duke".to_string()],
            Some("trixie"),
            docker_mode(&empty),
        )?;
        assert_eq!(distro.codename, "trixie");
        Ok(())
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
    fn bare_mode_resolves_any_suite_without_host_check() -> anyhow::Result<()> {
        let distro = select_distro_version(&["trixie".to_string()], None, DistroResolveMode::Bare)?;
        assert_eq!(distro.distro, Distro::Debian);
        assert_eq!(distro.codename, "trixie");

        let distro =
            select_distro_version(&["kirkstone".to_string()], None, DistroResolveMode::Bare)?;
        assert_eq!(distro.distro, Distro::Custom("kirkstone".into()));
        assert_eq!(distro.codename, "kirkstone");
        Ok(())
    }

    #[test]
    fn bare_binary_build_requires_id_and_codename_match() -> anyhow::Result<()> {
        let os_path = std::env::temp_dir().join(format!(
            "debmagic-os-release-builtin-{}",
            std::process::id()
        ));
        let target = DistroVersion::new(Distro::Debian, "trixie", "13");

        std::fs::write(&os_path, "ID=debian\nVERSION_CODENAME=bookworm\n")?;
        let err = validate_bare_host_target(&target, &os_path).unwrap_err();
        assert!(err.to_string().contains("VERSION_CODENAME is 'bookworm'"));

        std::fs::write(&os_path, "ID=ubuntu\nVERSION_CODENAME=trixie\n")?;
        let err = validate_bare_host_target(&target, &os_path).unwrap_err();
        assert!(err.to_string().contains("ID is 'ubuntu'"));

        std::fs::write(&os_path, "ID=debian\nVERSION_CODENAME=trixie\n")?;
        let distro = validate_bare_host_target(&target, &os_path)?;
        assert_eq!(distro.distro, Distro::Debian);
        assert_eq!(distro.codename, "trixie");

        std::fs::write(&os_path, "ID=gentoo\n")?;
        let err = validate_bare_host_target(&target, &os_path).unwrap_err();
        assert!(err.to_string().contains("no VERSION_CODENAME"));
        let _ = std::fs::remove_file(&os_path);
        Ok(())
    }

    #[test]
    fn bare_binary_build_custom_suite_anchors_family_to_host() -> anyhow::Result<()> {
        let os_path =
            std::env::temp_dir().join(format!("debmagic-os-release-custom-{}", std::process::id()));
        let target = DistroVersion::custom(Distro::from("kirkstone"), "kirkstone");

        std::fs::write(&os_path, "ID=yocto\nVERSION_CODENAME=kirkstone\n")?;
        let distro = validate_bare_host_target(&target, &os_path)?;
        assert_eq!(distro.distro, Distro::Custom("yocto".into()));
        assert_eq!(distro.codename, "kirkstone");
        assert_eq!(distro.version, "");

        std::fs::write(&os_path, "ID=yocto\nVERSION_CODENAME=other\n")?;
        let err = validate_bare_host_target(&target, &os_path).unwrap_err();
        assert!(err.to_string().contains("custom suite 'kirkstone'"));
        let _ = std::fs::remove_file(&os_path);
        Ok(())
    }
}
