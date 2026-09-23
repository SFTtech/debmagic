use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::upstream::mangle::mangle;
use crate::upstream::orig::{OrigTarballConfig, fetch_orig_tarball_for_version};
use crate::upstream::query::Candidate;
use crate::upstream::repack::{RepackConfig, apply_excludes};
use crate::upstream::watch::WatchSource;

/// Resolve the absolute download URL for a candidate: relative hrefs
/// resolve against the watch source's URL, then `Download-Url-Mangle`
/// applies.
pub fn download_url(source: &WatchSource, candidate: &Candidate) -> anyhow::Result<String> {
    let absolute = candidate.href.starts_with("http://")
        || candidate.href.starts_with("https://")
        || candidate.href.starts_with("ftp://")
        || candidate.href.starts_with("ftps://");
    let mut url = if absolute {
        candidate.href.clone()
    } else {
        let base = source.source.trim_end_matches('/');
        format!("{base}/{}", candidate.href.trim_start_matches('/'))
    };
    if let Some(rules) = &source.download_url_mangle {
        url = mangle(rules, &url)?;
    }
    Ok(url)
}

/// The local filename for a downloaded tarball: `Filename-Mangle` if
/// set, else the last URL component without any query fragment.
pub fn download_filename(source: &WatchSource, candidate: &Candidate) -> anyhow::Result<String> {
    if let Some(rules) = &source.filename_mangle {
        return mangle(rules, &candidate.href);
    }
    let url = download_url(source, candidate)?;
    let last = url.rsplit('/').next().unwrap_or_default();
    Ok(last
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_string())
}

/// Download a candidate's tarball into `download_dir`, returning its path.
/// Uses curl for ftp, the requests module for http(s).
pub async fn download_tarball(
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
    if destination.exists() {
        println!(
            "debmagic: using already-downloaded {}",
            destination.display()
        );
        return Ok(destination);
    }
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
pub async fn verify_tarball_signature(
    source: &WatchSource,
    source_dir: &Path,
    candidate: &Candidate,
    tarball: &Path,
    sign_options: &crate::sign::SignOptions,
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

/// The result of a switch: what was done and to which version.
#[derive(Debug)]
pub struct SwitchResult {
    pub version: String,
    pub orig_tarball: PathBuf,
    pub removed_files: usize,
}

/// Switch the package tree to `candidate`'s upstream version:
pub struct SwitchOptions<'a> {
    pub repack: &'a RepackConfig,
    pub output_dir: &'a Path,
    /// How to fetch an orig tarball the distros may already have;
    /// `None` skips the distro lookup.
    pub orig_tarball_config: Option<&'a OrigTarballConfig>,
    pub verify_signatures: bool,
    pub sign_options: &'a crate::sign::SignOptions,
    pub dry_run: bool,
}

/// download, extract, apply the repack excludes, write the orig
/// tarball next to the source tree, and swap the tree contents
/// (keeping `debian/`). With `dry_run`, only report what would happen.
pub async fn switch(
    source_dir: &Path,
    source: &WatchSource,
    candidate: &Candidate,
    options: &SwitchOptions<'_>,
) -> anyhow::Result<SwitchResult> {
    let package = crate::package::load_package(source_dir)?;
    let name = package.name().to_string();
    let repack = options.repack;
    let output_dir = options.output_dir;
    let dry_run = options.dry_run;
    let version = &candidate.version;
    let oversion = match &repack.suffix {
        Some(suffix) => format!("{version}{suffix}"),
        None => version.clone(),
    };

    let work_dir =
        std::env::temp_dir().join(format!("debmagic-switch-{}-{}", name, std::process::id()));
    let download_dir = work_dir.join("download");
    let extract_dir = work_dir.join("tree");

    if dry_run {
        let url = download_url(source, candidate)?;
        println!("debmagic: would switch {} to upstream {version}", name);
        println!("debmagic: tarball: {url}");
        if !repack.excludes.is_empty() {
            println!("debmagic: would repack, excluding:");
            for pattern in &repack.excludes {
                println!("  {pattern}");
            }
        }
        if let Some(suffix) = &repack.suffix {
            println!("debmagic: version suffix: {suffix}");
        }
        println!("debmagic: would replace the source tree, keeping debian/");
        return Ok(SwitchResult {
            version: oversion,
            orig_tarball: download_dir.join(download_filename(source, candidate)?),
            removed_files: 0,
        });
    }

    std::fs::create_dir_all(&download_dir)
        .with_context(|| format!("failed to create {}", download_dir.display()))?;

    // the distros may already have this version (a re-switch, or a
    // version Debian/Ubuntu published): their orig tarball must be
    // reused, since a re-created one would have different checksums
    // and the upload would be rejected. Only when they don't have it
    // is the tarball fetched from upstream and repacked.
    let mut distro_orig = None;
    if let Some(orig_config) = options.orig_tarball_config {
        match fetch_orig_tarball_for_version(
            orig_config,
            &package,
            options.output_dir,
            &oversion,
            source.component.as_deref(),
        )
        .await
        {
            Ok(Some(tarball)) => {
                println!(
                    "debmagic: using orig tarball {} (already available; \
                     skipping the upstream download and repack)",
                    tarball.display()
                );
                distro_orig = Some(tarball);
            }
            Ok(None) => {}
            Err(error) => {
                println!(
                    "debmagic: no orig tarball from the configured method ({error}); \
                     fetching from upstream"
                );
            }
        }
    }

    let tarball = match &distro_orig {
        Some(tarball) => tarball.clone(),
        None => {
            let tarball = download_tarball(source, candidate, &download_dir).await?;

            if options.verify_signatures {
                verify_tarball_signature(
                    source,
                    source_dir,
                    candidate,
                    &tarball,
                    options.sign_options,
                )
                .await?;
            }
            tarball
        }
    };

    extract_tarball(&tarball, &extract_dir)?;
    let removed = apply_excludes(&extract_dir, &repack.excludes)?;

    // canonical Debian layout: one top-level `<name>-<oversion>/` dir
    let top_level = format!("{}-{}", name, oversion);
    let stage_dir = work_dir.join("stage").join(&top_level);
    std::fs::create_dir_all(&stage_dir)
        .with_context(|| format!("failed to create {}", stage_dir.display()))?;
    move_tree_contents(&extract_dir, &stage_dir)?;

    // swap the tree first: it removes stale entries, which must not
    // touch the freshly written orig in the output dir
    swap_tree(source_dir, &stage_dir)?;

    // an orig tarball for this version may already exist — from a prior
    // switch or a developer-provided one. Re-creating it would produce
    // different checksums than the archive already holds, and the upload
    // would be rejected, so the existing one is kept as-is.
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let orig_path =
        match find_existing_orig(output_dir, &name, &oversion, source.component.as_deref()) {
            Some(existing) => {
                println!(
                    "debmagic: keeping existing orig tarball {} (re-creating it would \
                 change its checksums and break uploads)",
                    existing.display()
                );
                existing
            }
            None => {
                let orig_name = match &source.component {
                    Some(component) => format!(
                        "{}xz",
                        debmagic_common::changes::component_orig_prefix(
                            &name, &oversion, component
                        )
                    ),
                    None => format!(
                        "{}xz",
                        debmagic_common::changes::orig_prefix(&name, &oversion)
                    ),
                };
                let orig_path = output_dir.join(&orig_name);
                let output = std::process::Command::new("tar")
                    .current_dir(stage_dir.parent().expect("stage dir has a parent"))
                    .args(["--create", "--xz", "--file"])
                    .arg(&orig_path)
                    .arg(&top_level)
                    .output()
                    .with_context(|| format!("failed to run tar for {}", orig_path.display()))?;
                if !output.status.success() {
                    bail!(
                        "creating {} failed: {}",
                        orig_path.display(),
                        String::from_utf8_lossy(&output.stderr).trim()
                    );
                }
                orig_path
            }
        };

    std::fs::remove_dir_all(&work_dir).ok();
    println!(
        "debmagic: switched {} to upstream {oversion} (orig: {}, {} files excluded)",
        name,
        orig_path.display(),
        removed
    );
    Ok(SwitchResult {
        version: oversion,
        orig_tarball: orig_path,
        removed_files: removed,
    })
}

/// An orig tarball for this version that already exists in the output
/// dir — from a prior switch or a developer-provided one. Re-creating
/// it would produce different checksums than the archive already
/// holds, and the upload would be rejected, so it is kept as-is.
fn find_existing_orig(
    output_dir: &Path,
    name: &str,
    oversion: &str,
    component: Option<&str>,
) -> Option<PathBuf> {
    match component {
        Some(component) => {
            debmagic_common::changes::find_component_in_dir(output_dir, name, oversion, component)
        }
        None => debmagic_common::changes::find_orig_in_dir(output_dir, name, oversion),
    }
}

/// Move every entry of `from_dir` into `to_dir`.
fn move_tree_contents(from_dir: &Path, to_dir: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(from_dir)
        .with_context(|| format!("failed to read {}", from_dir.display()))?
        .flatten()
    {
        let destination = to_dir.join(entry.file_name());
        move_entry(&entry.path(), &destination)?;
    }
    Ok(())
}

/// Move one entry, falling back to copy+remove when a rename crosses
/// filesystems (the work dir is in /tmp, the source tree may not be).
fn move_entry(from: &Path, to: &Path) -> anyhow::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if from.is_dir() {
        copy_dir(from, to)?;
        std::fs::remove_dir_all(from)
    } else {
        std::fs::copy(from, to)
            .map(|_| ())
            .and_then(|()| std::fs::remove_file(from))
    }
    .with_context(|| format!("failed to move {} to {}", from.display(), to.display()))
}

/// Recursively copy a directory. Symlinks are recreated as symlinks —
/// a tarball may contain dangling ones, and following them would fail.
fn copy_dir(from: &Path, to: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(to).with_context(|| format!("failed to create {}", to.display()))?;
    for entry in std::fs::read_dir(from)
        .with_context(|| format!("failed to read {}", from.display()))?
        .flatten()
    {
        let destination = to.join(entry.file_name());
        let path = entry.path();
        if entry.file_type()?.is_symlink() {
            let target = std::fs::read_link(&path)
                .with_context(|| format!("failed to read link {}", path.display()))?;
            // a leftover entry from an interrupted run must not block the swap
            if std::fs::symlink_metadata(&destination).is_ok() {
                if destination.is_dir() {
                    std::fs::remove_dir_all(&destination)
                        .with_context(|| format!("failed to remove {}", destination.display()))?;
                } else {
                    std::fs::remove_file(&destination)
                        .with_context(|| format!("failed to remove {}", destination.display()))?;
                }
            }
            std::os::unix::fs::symlink(&target, &destination).with_context(|| {
                format!(
                    "failed to create symlink {} -> {}",
                    destination.display(),
                    target.display()
                )
            })?;
        } else if path.is_dir() {
            copy_dir(&path, &destination)?;
        } else {
            std::fs::copy(&path, &destination).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    path.display(),
                    destination.display()
                )
            })?;
        }
    }
    Ok(())
}

/// Replace the source tree's contents with the extracted tree's,
/// keeping `debian/` and any VCS metadata dir (`.git`, ...) intact.
fn swap_tree(source_dir: &Path, extract_dir: &Path) -> anyhow::Result<()> {
    let new_names: Vec<String> = std::fs::read_dir(extract_dir)
        .with_context(|| format!("failed to read {}", extract_dir.display()))?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    // stale entries from the old tree that the new one doesn't have
    for entry in std::fs::read_dir(source_dir)
        .with_context(|| format!("failed to read {}", source_dir.display()))?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_kept(&name) || new_names.contains(&name) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        } else {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }

    // move the new tree in
    for name in &new_names {
        if is_kept(name) {
            continue;
        }
        move_entry(&extract_dir.join(name), &source_dir.join(name))?;
    }
    Ok(())
}

/// Entries that always survive a tree swap: the packaging dir and
/// VCS storage (`.git`)
fn is_kept(name: &str) -> bool {
    name == "debian" || name == ".git"
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

    #[test]
    fn test_download_url_relative() {
        let source = source();
        let candidate = Candidate {
            version: "1.0".to_string(),
            href: "foo-1.0.tar.gz".to_string(),
        };
        assert_eq!(
            download_url(&source, &candidate).unwrap(),
            "https://example.com/releases/foo-1.0.tar.gz"
        );
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

    #[test]
    fn test_find_existing_orig() {
        let dir = std::env::temp_dir().join(format!("debmagic-switch-orig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // nothing there yet
        assert_eq!(find_existing_orig(&dir, "pkg", "1.0", None), None);

        // a main orig for the version is found, any compression
        std::fs::write(dir.join("pkg_1.0.orig.tar.xz"), "x").unwrap();
        assert_eq!(
            find_existing_orig(&dir, "pkg", "1.0", None),
            Some(dir.join("pkg_1.0.orig.tar.xz"))
        );
        // a different version is not
        assert_eq!(find_existing_orig(&dir, "pkg", "1.1", None), None);

        // component origs are only found for their component
        std::fs::write(dir.join("pkg_1.0.orig-bar.tar.xz"), "x").unwrap();
        assert_eq!(
            find_existing_orig(&dir, "pkg", "1.0", Some("bar")),
            Some(dir.join("pkg_1.0.orig-bar.tar.xz"))
        );
        assert_eq!(find_existing_orig(&dir, "pkg", "1.0", Some("baz")), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_extract_and_swap() {
        let base = std::env::temp_dir().join(format!("debmagic-switch-{}", std::process::id()));
        let source_dir = base.join("pkg");
        let extract_dir = base.join("tree");
        std::fs::create_dir_all(source_dir.join("debian")).unwrap();
        std::fs::create_dir_all(&extract_dir).unwrap();
        // old tree: a file that the new tree lacks, plus debian/
        std::fs::write(source_dir.join("old.txt"), "old").unwrap();
        std::fs::write(source_dir.join("debian").join("changelog"), "keep").unwrap();
        // new tree
        std::fs::write(extract_dir.join("new.txt"), "new").unwrap();
        std::fs::create_dir_all(extract_dir.join("src")).unwrap();
        std::fs::write(extract_dir.join("src").join("main.c"), "x").unwrap();

        swap_tree(&source_dir, &extract_dir).unwrap();

        assert!(source_dir.join("new.txt").exists());
        assert!(source_dir.join("src").join("main.c").exists());
        assert!(!source_dir.join("old.txt").exists());
        assert!(source_dir.join("debian").join("changelog").exists());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
