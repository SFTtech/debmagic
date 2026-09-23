use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, bail};

use debmagic_common::debian::version::PackageVersion;
use debmagic_common::package::SourcePackage;

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
    fn method(&self) -> OrigTarballMethod {
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
fn substitute_placeholders(s: &str, package: &SourcePackage, output_dir: &Path) -> String {
    let version = package.version().to_string();
    let source_dir = package.source_dir().unwrap().to_string_lossy();
    let out = output_dir.to_string_lossy();
    let vars = [
        ("name", package.name()),
        ("version", version.as_str()),
        ("upstream_version", package.version().upstream_version()),
        ("source_dir", &source_dir),
        ("output_dir", &out),
    ];
    let mut result = s.to_string();
    for (name, value) in vars {
        result = result.replace(&format!("{{{name}}}"), value);
    }
    result
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

/// Locate the wanted orig tarball in `dir`: the main one, or a MUT
/// component's when `component` is set.
fn find_in_dir(
    dir: &Path,
    name: &str,
    upstream_version: &str,
    component: Option<&str>,
) -> Option<PathBuf> {
    match component {
        Some(component) => {
            debmagic_common::changes::find_component_in_dir(dir, name, upstream_version, component)
        }
        None => debmagic_common::changes::find_orig_in_dir(dir, name, upstream_version),
    }
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

    if let Some(existing) = find_in_dir(output_dir, name, upstream_version, component) {
        println!(
            "debmagic: using existing orig tarball {}",
            existing.display()
        );
        return Ok(Some(existing));
    }

    // the conventional location developers keep tarballs in
    if let Some(parent) = source_dir.parent()
        && let Some(existing) = find_in_dir(parent, name, upstream_version, component)
    {
        println!(
            "debmagic: using existing orig tarball {} from {}",
            existing.display(),
            parent.display()
        );
        return Ok(Some(existing));
    }

    let method = config.method();
    if method == OrigTarballMethod::Disabled {
        return Ok(None);
    }

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
            let command = substitute_placeholders(command, package, output_dir);
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
            find_in_dir(output_dir, name, upstream_version, component)
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
    let prefix = match component {
        None => debmagic_common::changes::orig_prefix(name, upstream_version),
        Some(component) => {
            debmagic_common::changes::component_orig_prefix(name, upstream_version, component)
        }
    };
    // dpkg accepts exactly these compression extensions (its %COMP
    // table), so nothing else can appear in the archive
    for ext in ["gz", "xz", "bz2", "lzma"] {
        let url = format!("{base}{prefix}{ext}");
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
        && let Some(cached) = find_in_dir(cache_dir, name, upstream_version, component)
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
    if std::fs::hard_link(cached, &destination).is_ok() {
        return Ok(destination);
    }
    std::fs::copy(cached, &destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            cached.display(),
            destination.display()
        )
    })?;
    Ok(destination)
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
        && let Some(cached) = find_in_dir(cache_dir, name, upstream_version, component)
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
        let dsc_name = hrefs
            .iter()
            .map(String::as_str)
            .filter(|filename| filename.ends_with(".dsc"))
            .filter(|filename| {
                PackageVersion::from_str(filename.trim_end_matches(".dsc"))
                    .is_ok_and(|v| v.upstream_version() == upstream_version)
            })
            .max_by(|a, b| {
                let a = PackageVersion::from_str(a.trim_end_matches(".dsc"));
                let b = PackageVersion::from_str(b.trim_end_matches(".dsc"));
                match (a, b) {
                    (Ok(a), Ok(b)) => a.cmp(&b),
                    _ => std::cmp::Ordering::Equal,
                }
            })
            .map(str::to_string);
        if let Some(dsc_name) = dsc_name {
            let dsc_path = output_dir.join(format!("{name}.dsc"));
            crate::requests::http_download(&format!("{pool_url}{dsc_name}"), &dsc_path)
                .await
                .with_context(|| format!("downloading {dsc_name} from {pool_url} failed"))?;
            let dsc = crate::control::read_control(&dsc_path)?;
            let digests = crate::control::digest_file(&orig_path)?;
            debmagic_common::debian::control::verify_checksums(&dsc, &orig_name, &digests)
                .with_context(|| format!("the downloaded {orig_name} does not match its .dsc"))?;
            std::fs::remove_file(&dsc_path).ok();
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_substitute_placeholders() {
        let base = std::env::temp_dir().join(format!("debmagic-orig-subst-{}", std::process::id()));
        let source = base.join("postfix");
        let pkg = package(&source, None);
        let out = base.join("out");
        assert_eq!(
            substitute_placeholders(
                "fetch {name} {version} {upstream_version} from {source_dir} to {output_dir}",
                &pkg,
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
