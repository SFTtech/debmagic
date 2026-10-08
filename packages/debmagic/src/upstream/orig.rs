use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, bail};

use debmagic_common::debian::version::PackageVersion;
use debmagic_common::package::SourcePackage;

use crate::changes::find_orig_in_dir;

/// How to fetch the `orig` tarball a `3.0 (quilt)` source build needs
/// when no local copy exists.
#[derive(
    Debug,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    clap::ValueEnum,
)]
#[serde(rename_all = "kebab-case")]
pub enum OrigTarballMethod {
    /// The distro archives via Launchpad's API: Ubuntu first, then
    /// Debian (which Launchpad mirrors). No configuration needed.
    #[default]
    Launchpad,
    /// Debian's own archive pool (`deb.debian.org`), without Launchpad.
    Debian,
    /// Ubuntu's own archive pool (`archive.ubuntu.com`), without Launchpad.
    Ubuntu,
    /// A custom command from `orig_tarball.command`.
    Custom,
    /// Never fetch: build only with tarballs found locally.
    Disabled,
}

/// The `[orig_tarball]` config section.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct OrigTarballConfig {
    /// Which method to use; unset means `launchpad`.
    pub method: Option<OrigTarballMethod>,
    /// Command for `method = "custom"`, run via `sh -c` in the source
    /// dir with placeholders substituted (including `{output_dir}`).
    pub command: Option<String>,
    /// Mirror root for `method = "debian"`, like an apt sources entry
    /// (`https://deb.debian.org/debian` by default); `/pool` is appended.
    pub debian_mirror: Option<String>,
    /// Mirror root for `method = "ubuntu"` (`http://archive.ubuntu.com/ubuntu`
    /// by default); `/pool` is appended.
    pub ubuntu_mirror: Option<String>,
}

impl OrigTarballConfig {
    /// The resolved method: the configured one, or the `launchpad`
    /// default when unset.
    pub fn method(&self) -> OrigTarballMethod {
        self.method.unwrap_or_default()
    }

    /// The pool root on a mirror: the configured mirror (or the distro
    /// default), with `/pool` appended.
    fn pool_root(configured: Option<&str>, default: &str) -> String {
        format!(
            "{}/pool",
            configured.unwrap_or(default).trim_end_matches('/')
        )
    }
}

/// Substitute `{name}`, `{version}`, `{upstream_version}`, `{source_dir}`
/// and `{output_dir}` placeholders.
fn substitute_placeholders(
    s: &str,
    package: &SourcePackage,
    upstream_version: &str,
    output_dir: &Path,
) -> String {
    let version = package.version().to_string();
    let source_dir = package.source_dir().unwrap().to_string_lossy();
    let out = output_dir.to_string_lossy();
    crate::upload::target::substitute_placeholders(
        s,
        &[
            ("name", package.name()),
            ("version", version.as_str()),
            ("upstream_version", upstream_version),
            ("source_dir", &source_dir),
            ("output_dir", &out),
        ],
    )
}

/// The download cache dir for one package version:
/// `<cache>/debmagic/orig/<name>/<upstream_version>/`.
fn cache_dir(name: &str, upstream_version: &str) -> Option<PathBuf> {
    dirs::cache_dir().map(|cache| {
        cache
            .join("debmagic")
            .join("orig")
            .join(name)
            .join(upstream_version)
    })
}

/// Fetch the `orig` tarball for `package` into `output_dir`, unless a
/// suitable one is already there. Debian/Ubuntu developers conventionally
/// keep one next to the source tree (`../`), so an existing tarball there
/// is used as-is instead of fetching.
///
/// Returns the path of the tarball to build with, or `None` for native
/// packages that need no `orig` tarball at all.
pub async fn fetch_orig_tarball(
    config: &OrigTarballConfig,
    package: &SourcePackage,
    output_dir: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    fetch_orig_tarball_for_version(
        config,
        package,
        output_dir,
        package.version().upstream_version(),
        None,
    )
    .await
}

/// [`fetch_orig_tarball`], for an explicit upstream version — used when
/// the version is not (yet) the changelog head, like `upstream switch`
/// operating on its candidate. `component` selects a MUT component's
/// tarball instead of the main one.
pub async fn fetch_orig_tarball_for_version(
    config: &OrigTarballConfig,
    package: &SourcePackage,
    output_dir: &Path,
    upstream_version: &str,
    component: Option<&str>,
) -> anyhow::Result<Option<PathBuf>> {
    if package.is_native() {
        return Ok(None);
    }

    let source_dir = package.source_dir()?;
    let name = package.name();

    if let Some(existing) = find_orig_in_dir(output_dir, name, upstream_version, component) {
        println!(
            "debmagic: using existing orig tarball {}",
            existing.display()
        );
        return Ok(Some(existing));
    }

    // the conventional location developers keep tarballs in
    if let Some(parent) = source_dir.parent()
        && let Some(existing) = find_orig_in_dir(parent, name, upstream_version, component)
    {
        println!(
            "debmagic: using existing orig tarball {} from {}",
            existing.display(),
            parent.display()
        );
        return Ok(Some(existing));
    }

    fetch_with_method(
        config,
        config.method(),
        package,
        output_dir,
        upstream_version,
        component,
    )
    .await
}

/// The MUT component names `debian/watch` declares; none without a
/// watch file.
pub fn mut_components(package: &SourcePackage) -> anyhow::Result<Vec<String>> {
    let source_dir = package.source_dir()?;
    if !source_dir.join("debian").join("watch").is_file() {
        return Ok(Vec::new());
    }
    let sources = crate::upstream::query::load_watch(source_dir, package.name())?;
    Ok(sources
        .into_iter()
        .filter_map(|source| source.component)
        .collect())
}

/// Fetch the `orig` tarball for `upstream_version` with an explicit
/// `method` into `output_dir`, without looking for local copies first.
/// `None` when the method does not have it.
pub async fn fetch_with_method(
    config: &OrigTarballConfig,
    method: OrigTarballMethod,
    package: &SourcePackage,
    output_dir: &Path,
    upstream_version: &str,
    component: Option<&str>,
) -> anyhow::Result<Option<PathBuf>> {
    if method == OrigTarballMethod::Disabled {
        return Ok(None);
    }
    let source_dir = package.source_dir()?;
    let name = package.name();

    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;

    match method {
        OrigTarballMethod::Launchpad => fetch_from_distro_archive(
            package,
            upstream_version,
            component,
            output_dir,
        )
        .await
        .with_context(|| {
            format!(
                "fetching {name} {upstream_version} from the distro archives via Launchpad failed"
            )
        }),
        OrigTarballMethod::Debian => fetch_from_pool(
            &OrigTarballConfig::pool_root(
                config.debian_mirror.as_deref(),
                "https://deb.debian.org/debian",
            ),
            &["main", "contrib", "non-free"],
            name,
            upstream_version,
            component,
            output_dir,
        )
        .await
        .with_context(|| format!("fetching {name} {upstream_version} from the Debian pool failed")),
        OrigTarballMethod::Ubuntu => fetch_from_pool(
            &OrigTarballConfig::pool_root(
                config.ubuntu_mirror.as_deref(),
                "http://archive.ubuntu.com/ubuntu",
            ),
            &["main", "universe", "multiverse", "restricted"],
            name,
            upstream_version,
            component,
            output_dir,
        )
        .await
        .with_context(|| format!("fetching {name} {upstream_version} from the Ubuntu pool failed")),
        OrigTarballMethod::Disabled => Ok(None),
        OrigTarballMethod::Custom => {
            let command = config.command.as_deref().context(
                "orig_tarball.method = \"custom\" requires orig_tarball.command to be set",
            )?;
            let command = substitute_placeholders(command, package, upstream_version, output_dir);
            println!("debmagic: running orig tarball command: {command}");
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .current_dir(source_dir)
                .status()
                .with_context(|| format!("failed to run orig tarball command: {command}"))?;
            if !status.success() {
                bail!("orig tarball command failed (exit status: {status}): {command}");
            }
            find_orig_in_dir(output_dir, name, upstream_version, component)
                .with_context(|| {
                    format!(
                        "orig tarball method did not produce an expected tarball in {}",
                        output_dir.display()
                    )
                })
                .map(Some)
        }
    }
}

/// The `launchpad` method: the download cache, then the distro
/// archives via Launchpad's `+files` download URLs, populating the
/// cache.
///
/// The orig filename is deterministic — only the compression
/// extension varies — so the tarball URL is constructed directly and
/// probed, like the pool methods probe their listing. No API query is
/// needed, so an unpublished changelog head (the usual case: the
/// version being built is new) is not a problem: the orig tarball
/// only depends on the upstream part, and Launchpad serves it under
/// the upstream-only filename.
/// The Launchpad download URL of a source package's orig tarball in a
/// distro archive, trying the compression extensions dpkg accepts.
/// `None` when the archive does not serve it.
async fn launchpad_orig_url(
    distro: &str,
    name: &str,
    upstream_version: &str,
    component: Option<&str>,
) -> Option<String> {
    let base = format!("https://launchpad.net/{distro}/+archive/primary/+files/");
    for filename in debmagic_common::changes::orig_filenames(name, upstream_version, component) {
        let url = format!("{base}{filename}");
        if crate::requests::http_exists(&url).await {
            return Some(url);
        }
    }
    None
}

async fn fetch_from_distro_archive(
    package: &SourcePackage,
    upstream_version: &str,
    component: Option<&str>,
    output_dir: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let name = package.name();
    let cache = cache_dir(name, upstream_version);
    if let Some(cache_dir) = &cache
        && let Some(cached) = find_orig_in_dir(cache_dir, name, upstream_version, component)
    {
        println!("debmagic: using cached orig tarball {}", cached.display());
        return Ok(Some(stage_from_cache(&cached, output_dir)?));
    }

    // Ubuntu first, then Debian: Launchpad mirrors Debian, so a
    // Debian-only version is found too.
    let mut orig_url = None;
    for distro in ["ubuntu", "debian"] {
        if let Some(url) = launchpad_orig_url(distro, name, upstream_version, component).await {
            orig_url = Some(url);
            break;
        }
    }
    let Some(orig_url) = orig_url else {
        return Ok(None);
    };
    let orig_name = orig_url.rsplit('/').next().unwrap_or_default();
    let orig_path = output_dir.join(orig_name);
    println!("debmagic: downloading {orig_url}");
    crate::requests::http_download(&orig_url, &orig_path)
        .await
        .with_context(|| format!("downloading {orig_name} failed"))?;

    if let Some(cache_dir) = &cache {
        let cached = cache_dir.join(orig_name);
        if let Err(error) =
            std::fs::create_dir_all(cache_dir).and_then(|()| std::fs::copy(&orig_path, &cached))
        {
            // a failing cache must never fail the build
            println!("debmagic: populating the orig cache failed: {error}");
        }
    }

    Ok(Some(orig_path))
}

/// Make the cached tarball available in `output_dir` without copying
/// bytes when avoidable: a hardlink when both are on the same
/// filesystem, a copy otherwise.
fn stage_from_cache(cached: &Path, output_dir: &Path) -> anyhow::Result<PathBuf> {
    let destination = output_dir.join(cached.file_name().unwrap_or_default());
    link_or_copy(cached, &destination)?;
    Ok(destination)
}

/// Make `source` available at `destination`: a hardlink when both are on
/// the same filesystem, a copy otherwise (a symlink would point at a
/// host path that may not exist where the tarball is used).
///
/// A symlinked `source` is resolved first, as `link(2)` would hardlink
/// the symlink itself, whose relative target dangles from any other
/// directory. An existing `destination`, a dangling symlink included, is
/// replaced instead of written through.
pub fn link_or_copy(source: &Path, destination: &Path) -> anyhow::Result<()> {
    let source = source
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", source.display()))?;
    // nothing to do, and removing it would delete the source
    let is_symlink = destination
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink());
    if !is_symlink
        && destination
            .canonicalize()
            .is_ok_and(|existing| existing == source)
    {
        return Ok(());
    }
    match std::fs::remove_file(destination) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to remove {}", destination.display()));
        }
    }
    if std::fs::hard_link(&source, destination).is_ok() {
        return Ok(());
    }
    std::fs::copy(&source, destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

/// The `debian`/`ubuntu` methods: a distro archive pool directly,
/// without Launchpad. The pool directory is deterministic
/// (`<pool>/<section>/<prefix>/<name>/`); its listing reveals the orig
/// tarball and the `.dsc` for verification. Sections are tried in
/// order until one lists the package.
async fn fetch_from_pool(
    pool_root: &str,
    sections: &[&str],
    name: &str,
    upstream_version: &str,
    component: Option<&str>,
    output_dir: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let cache = cache_dir(name, upstream_version);
    if let Some(cache_dir) = &cache
        && let Some(cached) = find_orig_in_dir(cache_dir, name, upstream_version, component)
    {
        println!("debmagic: using cached orig tarball {}", cached.display());
        return Ok(Some(stage_from_cache(&cached, output_dir)?));
    }

    // the pool groups packages by source name prefix: `libfoo` under
    // `libf`, everything else under its first letter
    let prefix = if name.starts_with("lib") {
        name.chars().take(4).collect::<String>()
    } else {
        name.chars().take(1).collect::<String>()
    };

    // the pool holds origs of many versions; the filename must carry
    // the wanted one
    let orig_prefix = match component {
        None => debmagic_common::changes::orig_prefix(name, upstream_version),
        Some(component) => {
            debmagic_common::changes::component_orig_prefix(name, upstream_version, component)
        }
    };
    let wanted = |filename: &str| -> bool {
        filename.starts_with(&orig_prefix) && debmagic_common::changes::is_orig_tarball(filename)
    };

    for section in sections {
        let pool_url = format!("{pool_root}/{section}/{prefix}/{name}/");
        let Ok(listing) = crate::requests::http_get(&pool_url).await else {
            continue;
        };
        let hrefs = crate::upstream::query::extract_hrefs(&listing);
        let Some(orig_name) = hrefs
            .iter()
            .map(String::as_str)
            .find(|filename| wanted(filename))
            .map(str::to_string)
        else {
            continue;
        };
        let orig_path = output_dir.join(&orig_name);
        println!("debmagic: downloading {pool_url}{orig_name}");
        crate::requests::http_download(&format!("{pool_url}{orig_name}"), &orig_path)
            .await
            .with_context(|| format!("downloading {orig_name} from {pool_url} failed"))?;

        // verify against the .dsc of the newest matching publication
        if let Some(dsc_name) = newest_dsc(&hrefs, name, upstream_version) {
            let dsc = crate::requests::http_get(&format!("{pool_url}{dsc_name}"))
                .await
                .with_context(|| format!("downloading {dsc_name} from {pool_url} failed"))?;
            let dsc = crate::control::parse_control(&dsc)
                .with_context(|| format!("parsing {dsc_name} from {pool_url} failed"))?;
            let digests = crate::control::digest_file(&orig_path)?;
            let verified = debmagic_common::checksums::verify_checksums(&dsc, &orig_name, &digests);
            if let Err(error) = verified {
                // never leave a tarball behind that the archive would reject
                std::fs::remove_file(&orig_path).ok();
                return Err(error).with_context(|| {
                    format!("the downloaded {orig_name} does not match {dsc_name}")
                });
            }
        }

        if let Some(cache_dir) = &cache {
            let cached = cache_dir.join(&orig_name);
            if let Err(error) =
                std::fs::create_dir_all(cache_dir).and_then(|()| std::fs::copy(&orig_path, &cached))
            {
                // a failing cache must never fail the build
                println!("debmagic: populating the orig cache failed: {error}");
            }
        }

        return Ok(Some(orig_path));
    }
    Ok(None)
}

/// The newest `<name>_<version>.dsc` in a pool listing whose upstream
/// version is `upstream_version`. Pool filenames carry no epoch.
fn newest_dsc(hrefs: &[String], name: &str, upstream_version: &str) -> Option<String> {
    let prefix = format!("{name}_");
    hrefs
        .iter()
        .filter_map(|filename| {
            let version = filename.strip_prefix(&prefix)?.strip_suffix(".dsc")?;
            let version = PackageVersion::from_str(version).ok()?;
            (version.upstream_version() == upstream_version).then_some((version, filename))
        })
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, filename)| filename.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_newest_dsc() {
        let hrefs: Vec<String> = [
            "foo_1.0-1.dsc",
            "foo_1.0-10.dsc",
            "foo_1.0-2.dsc",
            "foo_1.1-1.dsc",
            "foo-bar_1.0-99.dsc",
            "foo_1.0.orig.tar.xz",
        ]
        .map(str::to_string)
        .to_vec();
        assert_eq!(
            newest_dsc(&hrefs, "foo", "1.0").as_deref(),
            Some("foo_1.0-10.dsc")
        );
        assert_eq!(newest_dsc(&hrefs, "foo", "2.0"), None);
    }

    /// A package over a real temp dir, so the reader exercises the
    /// on-disk path the production code uses.
    fn package(dir: &Path, format: Option<&str>) -> SourcePackage {
        std::fs::create_dir_all(dir.join("debian").join("source")).unwrap();
        std::fs::write(
            dir.join("debian").join("changelog"),
            "postfix (3.11.7-1ubuntu2) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
        )
        .unwrap();
        if let Some(format) = format {
            std::fs::write(dir.join("debian").join("source").join("format"), format).unwrap();
        }
        crate::package::load_package(dir).unwrap()
    }

    #[tokio::test]
    async fn test_native_package_needs_no_orig() {
        let base = std::env::temp_dir().join(format!("debmagic-native-{}", std::process::id()));
        let source = base.join("pkg");
        let pkg = package(&source, Some("3.0 (native)\n"));
        // a stale orig tarball next to the source must not be picked up
        std::fs::write(base.join("pkg_1.0.orig.tar.gz"), "stale").unwrap();

        let config = OrigTarballConfig::default();
        assert_eq!(
            fetch_orig_tarball(&config, &pkg, &base).await.unwrap(),
            None
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn test_fetch_prefers_output_dir() {
        let base = std::env::temp_dir().join(format!("debmagic-orig-{}", std::process::id()));
        let source = base.join("postfix");
        let out = base.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let pkg = package(&source, None);
        // a tarball in the output dir wins over the parent dir one
        std::fs::write(base.join("postfix_3.11.7.orig.tar.xz"), "parent").unwrap();
        std::fs::write(out.join("postfix_3.11.7.orig.tar.gz"), "output").unwrap();

        let config = OrigTarballConfig::default();
        let found = fetch_orig_tarball(&config, &pkg, &out).await.unwrap();
        assert_eq!(found, Some(out.join("postfix_3.11.7.orig.tar.gz")));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn test_fetch_uses_existing_in_parent() {
        let base =
            std::env::temp_dir().join(format!("debmagic-orig-parent-{}", std::process::id()));
        let source = base.join("postfix");
        let out = base.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let pkg = package(&source, None);
        // tarball next to the source tree, like developers keep it
        std::fs::write(base.join("postfix_3.11.7.orig.tar.xz"), "x").unwrap();

        let config = OrigTarballConfig::default();
        let found = fetch_orig_tarball(&config, &pkg, &out).await.unwrap();
        assert_eq!(found, Some(base.join("postfix_3.11.7.orig.tar.xz")));
        // nothing was copied into the output dir
        assert!(out.read_dir().unwrap().next().is_none());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn test_fetch_no_method_no_tarball() {
        let base = std::env::temp_dir().join(format!("debmagic-orig-none-{}", std::process::id()));
        let source = base.join("postfix");
        let pkg = package(&source, None);
        // fetching disabled explicitly: no method runs, no tarball appears
        let config = OrigTarballConfig {
            method: Some(OrigTarballMethod::Disabled),
            ..Default::default()
        };
        assert_eq!(
            fetch_orig_tarball(&config, &pkg, &base).await.unwrap(),
            None
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_mut_components_from_watch() {
        let base = std::env::temp_dir().join(format!("debmagic-mut-{}", std::process::id()));
        let source = base.join("postfix");
        let pkg = package(&source, None);
        assert!(mut_components(&pkg).unwrap().is_empty());

        std::fs::write(
            source.join("debian").join("watch"),
            "version=4\nhttps://example.com/ postfix-(.*)\\.tar\\.gz\n\
             opts=component=vendor https://example.com/ vendor-(.*)\\.tar\\.gz same\n",
        )
        .unwrap();
        assert_eq!(mut_components(&pkg).unwrap(), ["vendor"]);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn test_fetch_component_uses_component_tarball() {
        let base = std::env::temp_dir().join(format!("debmagic-orig-comp-{}", std::process::id()));
        let source = base.join("postfix");
        let out = base.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let pkg = package(&source, None);
        // only a component orig exists; the main lookup must not find it
        std::fs::write(out.join("postfix_3.11.7.orig-bar.tar.xz"), "x").unwrap();

        let config = OrigTarballConfig::default();
        let found = fetch_orig_tarball_for_version(&config, &pkg, &out, "3.11.7", Some("bar"))
            .await
            .unwrap();
        assert_eq!(found, Some(out.join("postfix_3.11.7.orig-bar.tar.xz")));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_stage_from_cache_hardlinks() {
        let base = std::env::temp_dir().join(format!("debmagic-orig-link-{}", std::process::id()));
        let cache = base.join("cache");
        let out = base.join("out");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        let cached = cache.join("postfix_3.11.7.orig.tar.xz");
        std::fs::write(&cached, "x").unwrap();

        let staged = stage_from_cache(&cached, &out).unwrap();
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            std::fs::metadata(&cached).unwrap().ino(),
            std::fs::metadata(&staged).unwrap().ino()
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_link_or_copy_resolves_a_relative_symlink_source() {
        use std::os::unix::fs::MetadataExt;
        let base =
            std::env::temp_dir().join(format!("debmagic-orig-symlink-{}", std::process::id()));
        let cache = base.join("pkg").join("cache");
        let build = base.join("pkg").join("build");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::create_dir_all(&build).unwrap();
        let real = cache.join("foo_1.0.orig.tar.xz");
        std::fs::write(&real, "x").unwrap();
        // like a tarball kept in `..` as a link into the git cache
        let link = base.join("foo_1.0.orig.tar.xz");
        std::os::unix::fs::symlink("pkg/cache/foo_1.0.orig.tar.xz", &link).unwrap();

        let staged = build.join("foo_1.0.orig.tar.xz");
        link_or_copy(&link, &staged).unwrap();
        assert!(
            !std::fs::symlink_metadata(&staged)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::metadata(&real).unwrap().ino(),
            std::fs::metadata(&staged).unwrap().ino()
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_link_or_copy_replaces_the_destination() {
        let base =
            std::env::temp_dir().join(format!("debmagic-orig-replace-{}", std::process::id()));
        let out = base.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let source = base.join("foo_1.0.orig.tar.xz");
        std::fs::write(&source, "new").unwrap();
        let destination = out.join("foo_1.0.orig.tar.xz");

        // a stale file
        std::fs::write(&destination, "old").unwrap();
        link_or_copy(&source, &destination).unwrap();
        assert_eq!(std::fs::read_to_string(&destination).unwrap(), "new");

        // a dangling link must not be written through
        std::fs::remove_file(&destination).unwrap();
        std::os::unix::fs::symlink("missing/dir/file", &destination).unwrap();
        link_or_copy(&source, &destination).unwrap();
        assert_eq!(std::fs::read_to_string(&destination).unwrap(), "new");
        assert!(!out.join("missing").exists());

        // the source itself stays intact when it already is the destination
        link_or_copy(&source, &source).unwrap();
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "new");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_substitute_placeholders() {
        let base = std::env::temp_dir().join(format!("debmagic-orig-subst-{}", std::process::id()));
        let source = base.join("postfix");
        let pkg = package(&source, None);
        let out = base.join("out");
        assert_eq!(
            substitute_placeholders(
                "fetch {name} {version} {upstream_version} from {source_dir} to {output_dir}",
                &pkg,
                "3.11.7",
                &out
            ),
            format!(
                "fetch postfix 3.11.7-1ubuntu2 3.11.7 from {} to {}",
                source.display(),
                out.display()
            )
        );
        std::fs::remove_dir_all(&base).unwrap();
    }
}
