use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::ValueEnum;
use debmagic_common::artifact::{component_orig_prefix, orig_filenames, orig_prefix};
use debmagic_common::package::SourcePackage;

use crate::changes::find_orig_in_dir;
use crate::config::SignConfig;
use crate::upstream::mangle::mangle;
use crate::upstream::orig::{
    OrigTarballConfig, OrigTarballMethod, fetch_with_method, link_or_copy,
};
use crate::upstream::query::{self, Candidate};
use crate::upstream::repack::{RepackConfig, apply_excludes, is_excluded, load_repack_config};
use crate::upstream::watch::WatchSource;

/// Where `upstream fetch` and `upstream switch` get the orig tarball from.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum FetchFrom {
    /// The configured `orig_tarball.method`, then upstream: a version the
    /// distros already have must be reused, since a re-created tarball
    /// would have different checksums and the upload would be rejected.
    #[default]
    Auto,
    /// The upstream release from `debian/watch`.
    Upstream,
    /// The distro archives via Launchpad.
    Launchpad,
    /// Ubuntu's archive pool.
    Ubuntu,
    /// Debian's archive pool.
    Debian,
    /// The `orig_tarball.command`.
    Custom,
}

impl FetchFrom {
    /// The orig tarball method to try before upstream, if any.
    fn method(self, config: &OrigTarballConfig) -> Option<OrigTarballMethod> {
        match self {
            Self::Auto => Some(config.method()),
            Self::Upstream => None,
            Self::Launchpad => Some(OrigTarballMethod::Launchpad),
            Self::Ubuntu => Some(OrigTarballMethod::Ubuntu),
            Self::Debian => Some(OrigTarballMethod::Debian),
            Self::Custom => Some(OrigTarballMethod::Custom),
        }
    }
}

/// How [`fetch`] gets and places one source's orig tarball.
pub struct FetchOptions<'a> {
    pub from: FetchFrom,
    pub orig_tarball: &'a OrigTarballConfig,
    /// Strip the `Files-Excluded` paths from upstream and custom
    /// tarballs; distro tarballs are immutable.
    pub repack: bool,
    pub signature_check: bool,
    pub sign: &'a SignConfig,
    pub output_dir: &'a Path,
    /// If upstream ships the output dir, use this instead (`None` fails)
    pub output_dir_fallback: Option<&'a Path>,
    /// Replace an existing orig tarball instead of keeping it.
    pub force: bool,
    /// Only report what would happen.
    pub dry_run: bool,
}

/// A fetched orig tarball.
#[derive(Debug)]
pub struct Fetched {
    /// The tarball in the output dir; in a dry run, where it would go.
    pub tarball: PathBuf,
    /// The upstream version, without a repack suffix.
    pub upstream_version: String,
}

/// A scratch dir, removed when dropped on success and error alike.
fn scratch_dir(name: &str) -> anyhow::Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix(&format!("debmagic-fetch-{name}-"))
        .tempdir()
        .context("failed to create the scratch dir")
}

/// Fetch the orig tarball of one watch source (the main tarball, or a
/// MUT component's) for `requested` into the output dir. `requested`
/// is an upstream or full Debian version, or `latest`.
///
/// An existing orig tarball for the version is kept unless forced:
/// re-creating it would change its checksums and break uploads.
pub async fn fetch(
    package: &SourcePackage,
    source: &WatchSource,
    requested: &str,
    options: &FetchOptions<'_>,
) -> anyhow::Result<Fetched> {
    let name = package.name();
    let component = source.component.as_deref();
    let output_dir = options.output_dir;
    let repack = load_repack_config(package, source)?;
    let suffix = repack
        .suffix
        .as_deref()
        .filter(|_| options.repack)
        .unwrap_or_default();

    // a concrete version is known up front, so the distro archives can
    // be asked without contacting upstream; `latest` needs the listing
    let mut candidate = None;
    let upstream_version = if requested == "latest" {
        let latest = resolve_candidate(package, source, requested, output_dir).await?;
        let version = latest.version.clone();
        candidate = Some(latest);
        version
    } else {
        let version = query::current_upstream_version(source, requested)?;
        match repack
            .suffix
            .as_deref()
            .and_then(|s| version.strip_suffix(s))
        {
            Some(base) => base.to_string(),
            None => version,
        }
    };
    let orig_base = match &source.oversion_mangle {
        Some(rules) => mangle(rules, &upstream_version)?,
        None => upstream_version.clone(),
    };
    let orig_version = format!("{orig_base}{suffix}");
    let fetched = |tarball| Fetched {
        tarball,
        upstream_version: upstream_version.clone(),
    };

    let changelog_upstream = package.version().upstream_version();
    if !options.repack
        && let Some(repack_suffix) = &repack.suffix
        && changelog_upstream.ends_with(repack_suffix.as_str())
    {
        println!(
            "debmagic: warning: the changelog's upstream version {changelog_upstream} carries the repack suffix {repack_suffix}, but without repacking the orig tarball is for {orig_version}"
        );
    }

    if options.force {
        if !options.dry_run {
            remove_existing(output_dir, name, &orig_version, component)?;
        }
    } else if let Some(existing) = use_existing(package, &orig_version, component, options)? {
        return Ok(fetched(existing));
    }

    if let Some(method) = options.from.method(options.orig_tarball)
        && method != OrigTarballMethod::Disabled
    {
        let method_name = method
            .to_possible_value()
            .expect("no orig tarball method is skipped")
            .get_name()
            .to_string();
        // a custom command's output is pristine like an upstream
        // download and repacked like one, so it is for the bare version
        let custom = method == OrigTarballMethod::Custom;
        let version = if custom {
            &upstream_version
        } else {
            &orig_version
        };
        if options.dry_run {
            println!("debmagic: would fetch {name} {version} via the {method_name} method");
            if options.from != FetchFrom::Auto {
                let prefix = orig_name_prefix(name, &orig_version, component);
                return Ok(fetched(output_dir.join(format!("{prefix}xz"))));
            }
        } else {
            match fetch_with_method(
                options.orig_tarball,
                method,
                package,
                output_dir,
                version,
                component,
            )
            .await
            {
                Ok(Some(tarball)) if custom => {
                    let work_dir = scratch_dir(name)?;
                    return place_orig(
                        &tarball,
                        work_dir.path(),
                        name,
                        &orig_version,
                        component,
                        &repack,
                        options,
                    )
                    .map(fetched);
                }
                Ok(Some(tarball)) => {
                    println!("debmagic: fetched {}", tarball.display());
                    return Ok(fetched(tarball));
                }
                Ok(None) if options.from == FetchFrom::Auto => {}
                Ok(None) => {
                    bail!("the {method_name} method has no orig tarball for {name} {version}")
                }
                Err(error) if options.from == FetchFrom::Auto => {
                    println!(
                        "debmagic: no orig tarball via the {method_name} method ({error:#}); fetching from upstream"
                    );
                }
                Err(error) => return Err(error),
            }
        }
    }

    let candidate = match candidate {
        Some(candidate) => candidate,
        None => match resolve_candidate(package, source, &upstream_version, output_dir).await {
            // its orig tarball is still named after the main version
            Err(error)
                if let Some(component) = component
                    && source.version_unrestricted =>
            {
                println!(
                    "debmagic: {error:#}; the watch file doesn't tie component {component} to the main version, using its newest release"
                );
                resolve_candidate(package, source, "latest", output_dir).await?
            }
            result => result?,
        },
    };
    let url = download_url(source, &candidate)?;
    if options.dry_run {
        println!("debmagic: would download {url}");
        let repacking = options.repack && !repack.excludes.is_empty();
        if repacking {
            println!("debmagic: would repack, excluding:");
            for pattern in &repack.excludes {
                println!("  {pattern}");
            }
        }
        let compression = if repacking {
            Some("xz")
        } else {
            orig_compression(&download_filename(source, &candidate)?)
        };
        let prefix = orig_name_prefix(name, &orig_version, component);
        return Ok(fetched(
            output_dir.join(format!("{prefix}{}", compression.unwrap_or("xz"))),
        ));
    }

    let work_dir = scratch_dir(name)?;
    let download_dir = work_dir.path().join("download");
    std::fs::create_dir_all(&download_dir)
        .with_context(|| format!("failed to create {}", download_dir.display()))?;
    let tarball = download_tarball(source, &candidate, &download_dir).await?;
    if options.signature_check {
        verify_tarball_signature(
            source,
            package.source_dir()?,
            &candidate,
            &tarball,
            options.sign,
        )
        .await?;
    }
    place_orig(
        &tarball,
        work_dir.path(),
        name,
        &orig_version,
        component,
        &repack,
        options,
    )
    .map(fetched)
}

/// The orig tarball filename up to the compression extension.
fn orig_name_prefix(name: &str, version: &str, component: Option<&str>) -> String {
    match component {
        None => orig_prefix(name, version),
        Some(component) => component_orig_prefix(name, version, component),
    }
}

/// An existing orig tarball for `orig_version`: in the output dir, or
/// next to the source tree (`../`), where developers conventionally keep
/// them; that one is linked into the output dir.
fn use_existing(
    package: &SourcePackage,
    orig_version: &str,
    component: Option<&str>,
    options: &FetchOptions<'_>,
) -> anyhow::Result<Option<PathBuf>> {
    let name = package.name();
    let output_dir = options.output_dir;
    if let Some(existing) = find_orig_in_dir(output_dir, name, orig_version, component) {
        println!(
            "debmagic: keeping existing orig tarball {} (--force replaces it)",
            existing.display()
        );
        return Ok(Some(existing));
    }
    let Some(existing) = package
        .source_dir()?
        .parent()
        .and_then(|parent| find_orig_in_dir(parent, name, orig_version, component))
    else {
        return Ok(None);
    };
    let destination = output_dir.join(existing.file_name().expect("orig tarball has a file name"));
    println!(
        "debmagic: using existing orig tarball {}",
        existing.display()
    );
    if !options.dry_run {
        std::fs::create_dir_all(output_dir)
            .with_context(|| format!("failed to create {}", output_dir.display()))?;
        link_or_copy(&existing, &destination)?;
    }
    Ok(Some(destination))
}

/// Remove the output dir's orig tarballs for `orig_version`, in any
/// compression.
fn remove_existing(
    output_dir: &Path,
    name: &str,
    orig_version: &str,
    component: Option<&str>,
) -> anyhow::Result<()> {
    for filename in orig_filenames(name, orig_version, component) {
        let path = output_dir.join(filename);
        // a dangling symlink is leftover too
        if path.symlink_metadata().is_ok() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

/// Resolve `requested` (a version or `latest`) to a downloadable
/// candidate of the watch source.
async fn resolve_candidate(
    package: &SourcePackage,
    source: &WatchSource,
    requested: &str,
    output_dir: &Path,
) -> anyhow::Result<Candidate> {
    let name = package.name();
    if requested == "latest" {
        return query::query_source(source, name)
            .await?
            .into_iter()
            .next()
            .with_context(|| format!("no upstream candidates found for {name}"));
    }

    // a concrete version: construct the URL from the watch pattern
    // directly, the extension of an existing orig tarball first; only
    // fall back to scraping when that fails
    let component = source.component.as_deref();
    let current = package.version().upstream_version();
    let existing_orig = find_orig_in_dir(output_dir, name, current, component).or_else(|| {
        package
            .source_dir()
            .ok()?
            .parent()
            .and_then(|parent| find_orig_in_dir(parent, name, current, component))
    });
    if let Ok(Some(candidate)) =
        query::resolve_concrete(source, name, requested, existing_orig.as_deref()).await
    {
        return Ok(candidate);
    }
    let candidates = query::query_source(source, name).await?;
    query::find_candidate(&candidates, requested)
        .cloned()
        .with_context(|| {
            format!(
                "upstream version {requested} not found for {name}; run 'debmagic upstream list' to see the available versions"
            )
        })
}

/// Turn a pristine upstream tarball into the orig tarball in the output
/// dir: repacked with `options.repack` when excluded files are in it (or
/// dpkg-source can't use its compression), else moved as-is, with a
/// warning listing every file `debian/copyright` excludes.
fn place_orig(
    pristine: &Path,
    work_dir: &Path,
    name: &str,
    orig_version: &str,
    component: Option<&str>,
    repack: &RepackConfig,
    options: &FetchOptions<'_>,
) -> anyhow::Result<PathBuf> {
    let output_dir = options.output_dir;
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let prefix = orig_name_prefix(name, orig_version, component);
    let pristine_name = pristine
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let compression = orig_compression(&pristine_name);

    let excluded: Vec<String> = match compression {
        Some(_) if !repack.excludes.is_empty() => list_tarball(pristine)?
            .into_iter()
            .filter(|path| is_excluded(path, &repack.excludes))
            .collect(),
        _ => Vec::new(),
    };

    if options.repack && (compression.is_none() || !excluded.is_empty()) {
        let tree = work_dir.join("tree");
        extract_tarball(pristine, &tree)?;
        let removed = apply_excludes(&tree, &repack.excludes)?;
        let repacked = work_dir.join(format!("{prefix}xz"));
        create_tarball(&tree, &format!("{name}-{orig_version}"), &repacked)?;
        let destination = output_dir.join(format!("{prefix}xz"));
        move_file(&repacked, &destination)?;
        // a custom command leaves its pristine tarball in the output dir
        if pristine != destination && pristine.starts_with(output_dir) {
            std::fs::remove_file(pristine)
                .with_context(|| format!("failed to remove {}", pristine.display()))?;
        }
        println!(
            "debmagic: repacked {pristine_name} as {} ({removed} excluded paths removed)",
            destination.display()
        );
        return Ok(destination);
    }

    let Some(compression) = compression else {
        bail!(
            "dpkg-source only accepts .tar.{{gz,xz,bz2,lzma}} orig tarballs, not {pristine_name}; pass --repack=yes to re-create it"
        );
    };
    if !excluded.is_empty() {
        let field = match component {
            Some(component) => format!("Files-Excluded-{component}"),
            None => "Files-Excluded".to_string(),
        };
        println!(
            "debmagic: warning: {} file(s) matching {field} in debian/copyright kept without repacking:",
            excluded.len()
        );
        for path in &excluded {
            println!("  {path}");
        }
    }
    let destination = output_dir.join(format!("{prefix}{compression}"));
    if pristine != destination {
        move_file(pristine, &destination)?;
    }
    println!("debmagic: fetched {}", destination.display());
    Ok(destination)
}

/// The orig tarball compression for an upstream tarball's filename, if
/// dpkg-source accepts it; maps the short `.tgz`-style forms.
pub fn orig_compression(filename: &str) -> Option<&'static str> {
    [
        (".tar.gz", "gz"),
        (".tgz", "gz"),
        (".tar.xz", "xz"),
        (".txz", "xz"),
        (".tar.bz2", "bz2"),
        (".tbz2", "bz2"),
        (".tbz", "bz2"),
        (".tar.lzma", "lzma"),
    ]
    .into_iter()
    .find(|(suffix, _)| filename.ends_with(suffix))
    .map(|(_, compression)| compression)
}

/// The file paths in a tarball, without the leading `<name>-<version>/`
/// component, like [`extract_tarball`] unpacks them.
fn list_tarball(tarball: &Path) -> anyhow::Result<Vec<String>> {
    let output = std::process::Command::new("tar")
        .arg("--list")
        .arg("--file")
        .arg(tarball)
        .output()
        .with_context(|| format!("failed to run tar for {}", tarball.display()))?;
    if !output.status.success() {
        bail!(
            "listing {} failed: {}",
            tarball.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mut paths: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once('/').map(|(_, path)| path))
        .filter(|path| !path.is_empty() && !path.ends_with('/'))
        .map(str::to_string)
        .collect();
    paths.sort();
    Ok(paths)
}

/// Create an xz tarball of `tree` with the canonical Debian layout: a
/// single top-level `top_level/` dir. Moves `tree` into place.
fn create_tarball(tree: &Path, top_level: &str, destination: &Path) -> anyhow::Result<()> {
    let stage = tree.with_file_name("stage");
    std::fs::create_dir_all(&stage)
        .with_context(|| format!("failed to create {}", stage.display()))?;
    std::fs::rename(tree, stage.join(top_level))
        .with_context(|| format!("failed to stage {}", tree.display()))?;
    let output = std::process::Command::new("tar")
        .current_dir(&stage)
        .args(["--create", "--xz", "--file"])
        .arg(destination)
        .arg(top_level)
        .output()
        .with_context(|| format!("failed to run tar for {}", destination.display()))?;
    if !output.status.success() {
        bail!(
            "creating {} failed: {}",
            destination.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Move a file, copying when the rename crosses filesystems.
fn move_file(from: &Path, to: &Path) -> anyhow::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)
        .and_then(|_| std::fs::remove_file(from))
        .with_context(|| format!("failed to move {} to {}", from.display(), to.display()))
}

/// Resolve the absolute download URL for a candidate: hrefs resolve
/// against the watch source's URL like a browser (RFC 3986, as uscan),
/// then `Download-Url-Mangle` applies.
pub fn download_url(source: &WatchSource, candidate: &Candidate) -> anyhow::Result<String> {
    let base = reqwest::Url::parse(&source.source)
        .with_context(|| format!("invalid watch source URL {}", source.source))?;
    let mut url = base
        .join(&candidate.href)
        .with_context(|| format!("resolving {} against {base} failed", candidate.href))?
        .to_string();
    if let Some(rules) = &source.download_url_mangle {
        url = mangle(rules, &url)?;
    }
    Ok(url)
}

/// The local filename for a downloaded tarball: `Filename-Mangle` if
/// set, else the last URL component without any query fragment. Always
/// a bare filename: a mangle rule that doesn't match leaves a path.
fn download_filename(source: &WatchSource, candidate: &Candidate) -> anyhow::Result<String> {
    let name = match &source.filename_mangle {
        Some(rules) => mangle(rules, &candidate.href)?,
        None => download_url(source, candidate)?,
    };
    let last = name.rsplit('/').next().unwrap_or_default();
    Ok(last
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_string())
}

/// Download a candidate's tarball into `download_dir`, returning its path.
/// Uses curl for ftp, the requests module for http(s).
async fn download_tarball(
    source: &WatchSource,
    candidate: &Candidate,
    download_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let url = download_url(source, candidate)?;
    let filename = download_filename(source, candidate)?;
    if filename.is_empty() {
        bail!("could not determine a filename for {url}");
    }
    let destination = download_dir.join(filename);
    println!("debmagic: downloading {url}");
    if url.starts_with("ftp://") || url.starts_with("ftps://") {
        crate::requests::ftp_download(&url, &destination).await?;
    } else {
        crate::requests::http_download(&url, &destination).await?;
    }
    Ok(destination)
}

/// Download and verify the signature of a downloaded tarball.
/// The signature URL comes from `Pgp-Sig-Url-Mangle` or the common
/// suffixes (`.asc`, `.sig`, ...); verification runs against
/// `debian/upstream/signing-key.asc` when that file exists.
/// Returns Ok(()) when no keyring or no signature is available.
async fn verify_tarball_signature(
    source: &WatchSource,
    source_dir: &Path,
    candidate: &Candidate,
    tarball: &Path,
    sign_options: &SignConfig,
) -> anyhow::Result<()> {
    let keyring = source_dir
        .join("debian")
        .join("upstream")
        .join("signing-key.asc");
    if !keyring.is_file() {
        return Ok(());
    }

    let url = download_url(source, candidate)?;
    let sig_url = match &source.pgp_sig_url_mangle {
        Some(rules) => mangle(rules, &url)?,
        None => {
            let mut found = None;
            for suffix in ["asc", "sig", "sign", "pgp", "gpg"] {
                let candidate_url = format!("{url}.{suffix}");
                if crate::requests::http_exists(&candidate_url).await {
                    found = Some(candidate_url);
                    break;
                }
            }
            match found {
                Some(url) => url,
                None => return Ok(()),
            }
        }
    };

    let sig_name = sig_url.rsplit('/').next().unwrap_or_default();
    let sig_path = tarball
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(sig_name);
    println!("debmagic: downloading signature {sig_url}");
    crate::requests::http_download(&sig_url, &sig_path).await?;

    crate::sign::verify_signature(sign_options, tarball, &sig_path, &keyring)?;
    println!(
        "debmagic: verified upstream signature of {}",
        tarball.display()
    );
    Ok(())
}

/// Extract a tarball into `extract_dir`, stripping the leading
/// `<name>-<version>/` component so the tree sits directly in
/// `extract_dir`.
pub fn extract_tarball(tarball: &Path, extract_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(extract_dir)
        .with_context(|| format!("failed to create {}", extract_dir.display()))?;
    let output = std::process::Command::new("tar")
        .arg("--extract")
        .arg("--file")
        .arg(tarball)
        .arg("--directory")
        .arg(extract_dir)
        .arg("--strip-components=1")
        .output()
        .with_context(|| format!("failed to run tar for {}", tarball.display()))?;
    if !output.status.success() {
        bail!(
            "extracting {} failed: {}",
            tarball.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> WatchSource {
        WatchSource {
            source: "https://example.com/releases/".to_string(),
            matching_pattern: "foo-@ANY_VERSION@@ARCHIVE_EXT@".to_string(),
            ..Default::default()
        }
    }

    #[test_case::test_case("foo-1.0.tar.gz", "https://example.com/releases/foo-1.0.tar.gz"; "relative")]
    #[test_case::test_case("/dl/foo-1.0.tar.gz", "https://example.com/dl/foo-1.0.tar.gz"; "host absolute")]
    #[test_case::test_case("../old/foo-1.0.tar.gz", "https://example.com/old/foo-1.0.tar.gz"; "parent")]
    #[test_case::test_case("//cdn.example.org/foo-1.0.tar.gz", "https://cdn.example.org/foo-1.0.tar.gz"; "scheme relative")]
    fn test_download_url_relative(href: &str, expected: &str) {
        let source = source();
        let candidate = Candidate {
            version: "1.0".to_string(),
            href: href.to_string(),
        };
        assert_eq!(download_url(&source, &candidate).unwrap(), expected);
    }

    #[test]
    fn test_download_url_absolute() {
        let source = source();
        let candidate = Candidate {
            version: "1.0".to_string(),
            href: "https://mirror.example.com/foo-1.0.tar.gz".to_string(),
        };
        assert_eq!(
            download_url(&source, &candidate).unwrap(),
            "https://mirror.example.com/foo-1.0.tar.gz"
        );
    }

    #[test]
    fn test_download_url_mangle() {
        let mut source = source();
        source.download_url_mangle = Some("s/prdownload/download/".to_string());
        let candidate = Candidate {
            version: "1.0".to_string(),
            href: "https://prdownload.example.com/foo-1.0.tar.gz".to_string(),
        };
        assert_eq!(
            download_url(&source, &candidate).unwrap(),
            "https://download.example.com/foo-1.0.tar.gz"
        );
    }

    #[test]
    fn test_download_filename() {
        let source = source();
        let candidate = Candidate {
            version: "1.0".to_string(),
            href: "foo-1.0.tar.gz".to_string(),
        };
        assert_eq!(
            download_filename(&source, &candidate).unwrap(),
            "foo-1.0.tar.gz"
        );
    }

    #[test_case::test_case("foo-1.0.tar.gz", Some("gz"); "tar gz")]
    #[test_case::test_case("foo-1.0.tgz", Some("gz"); "tgz")]
    #[test_case::test_case("foo-1.0.tar.xz", Some("xz"); "tar xz")]
    #[test_case::test_case("foo-1.0.tbz2", Some("bz2"); "tbz2")]
    #[test_case::test_case("foo-1.0.tar.lzma", Some("lzma"); "lzma")]
    #[test_case::test_case("foo-1.0.tar.zst", None; "zstd")]
    #[test_case::test_case("foo-1.0.zip", None; "zip")]
    fn test_orig_compression(filename: &str, expected: Option<&str>) {
        assert_eq!(orig_compression(filename), expected);
    }

    /// A `pkg-1.0/` upstream tarball with a `win32/` dir to exclude.
    fn pristine_tarball(base: &Path) -> PathBuf {
        let tree = base.join("upstream").join("pkg-1.0");
        std::fs::create_dir_all(tree.join("win32")).unwrap();
        std::fs::create_dir_all(tree.join("src")).unwrap();
        std::fs::write(tree.join("win32").join("a.c"), "x").unwrap();
        std::fs::write(tree.join("src").join("main.c"), "x").unwrap();
        let tarball = base.join("pkg-1.0.tgz");
        let status = std::process::Command::new("tar")
            .current_dir(base.join("upstream"))
            .args(["--create", "--gzip", "--file"])
            .arg(&tarball)
            .arg("pkg-1.0")
            .status()
            .unwrap();
        assert!(status.success());
        tarball
    }

    fn place(base: &Path, pristine: &Path, repack: bool, suffix: Option<&str>) -> PathBuf {
        let work_dir = base.join("work");
        std::fs::create_dir_all(&work_dir).unwrap();
        let config = RepackConfig {
            excludes: vec!["win32/*".to_string()],
            suffix: suffix.map(str::to_string),
        };
        let orig_tarball = OrigTarballConfig::default();
        let sign = SignConfig::default();
        let output_dir = base.join("out");
        let options = FetchOptions {
            from: FetchFrom::Upstream,
            orig_tarball: &orig_tarball,
            repack,
            signature_check: false,
            sign: &sign,
            output_dir: &output_dir,
            output_dir_fallback: None,
            force: false,
            dry_run: false,
        };
        let version = format!("1.0{}", suffix.unwrap_or_default());
        place_orig(
            pristine, &work_dir, "pkg", &version, None, &config, &options,
        )
        .unwrap()
    }

    #[test]
    fn test_place_orig_repacks_excluded_files() {
        let _base_tmp = tempfile::Builder::new()
            .prefix("debmagic-repack-")
            .tempdir()
            .unwrap();
        let base = _base_tmp.path().to_path_buf();
        let orig = place(&base, &pristine_tarball(&base), true, Some("+dfsg"));
        assert_eq!(orig, base.join("out").join("pkg_1.0+dfsg.orig.tar.xz"));
        assert_eq!(list_tarball(&orig).unwrap(), vec!["src/main.c"]);
    }

    #[test]
    fn test_place_orig_without_repack_keeps_tarball() {
        let _base_tmp = tempfile::Builder::new()
            .prefix("debmagic-pristine-")
            .tempdir()
            .unwrap();
        let base = _base_tmp.path().to_path_buf();
        let tarball = pristine_tarball(&base);
        let pristine = std::fs::read(&tarball).unwrap();
        let orig = place(&base, &tarball, false, None);
        // the upstream tarball, byte for byte, under the orig name
        assert_eq!(orig, base.join("out").join("pkg_1.0.orig.tar.gz"));
        assert_eq!(std::fs::read(&orig).unwrap(), pristine);
        assert_eq!(
            list_tarball(&orig).unwrap(),
            vec!["src/main.c", "win32/a.c"]
        );
    }
}
