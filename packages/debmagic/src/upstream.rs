use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, bail};

use crate::config::Config;
use crate::package::{load_package, resolve_source_dir};

pub mod mangle;
pub mod orig;
pub mod query;
pub mod repack;
pub mod switch;
pub mod watch;

/// Inputs for the `debmagic upstream list` command.
#[derive(Debug, Clone)]
pub struct UpstreamListCommand {
    /// Directory used when `source_dir` is unset (typically cwd).
    pub fallback_dir: PathBuf,
    pub source_dir: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    /// Show all candidate versions, not just the newest newer ones.
    pub all: bool,
    /// Show the N versions newer than the changelog's, not just the newest.
    pub previous: Option<usize>,
}

/// Inputs for the `debmagic upstream switch` command.
#[derive(Debug, Clone)]
pub struct UpstreamSwitchCommand {
    /// Directory used when `source_dir` is unset (typically cwd).
    pub fallback_dir: PathBuf,
    pub source_dir: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    /// The upstream version to switch to, or "latest" for the newest eligible.
    pub version: String,
    /// Only report what would happen, without touching anything.
    pub dry_run: bool,
    /// Skip verifying the upstream tarball signature.
    pub no_signature_check: bool,
    /// Custom verification command overriding `sign.verify_command`.
    pub verify_command: Option<String>,
}

/// Run the `debmagic upstream list` command: query each watch source and
/// print its current and newer upstream versions.
pub async fn list(command: UpstreamListCommand) -> anyhow::Result<()> {
    let source_dir = resolve_source_dir(&command.fallback_dir, command.source_dir.as_deref())?;
    let identity = load_package(&source_dir)?;
    let sources = query::load_watch(&source_dir, identity.name())?;

    let mut newest: Option<String> = None;
    for source in &sources {
        if let Some(reason) = &source.untrackable {
            println!("debmagic: skipping untrackable source: {reason}");
            continue;
        }
        let candidates = query::query_source(source, identity.name()).await?;
        let current = query::current_upstream_version(source, &identity.version().to_string())?;
        let current_version = debmagic_common::debian::version::PackageVersion::from_str(&current)
            .map_err(|_| anyhow::anyhow!("invalid current version: {current}"))?;
        let newer: Vec<_> = candidates
            .iter()
            .filter(|c| {
                debmagic_common::debian::version::PackageVersion::from_str(&c.version)
                    .is_ok_and(|v| v > current_version)
            })
            .collect();
        println!("debmagic: current upstream version: {current}");
        if command.all {
            for candidate in &candidates {
                println!("  {}", candidate.version);
            }
        } else {
            let limit = command.previous.unwrap_or(1);
            for candidate in newer.iter().take(limit) {
                println!("  {} (newer)", candidate.version);
            }
        }
        newest = candidates.first().map(|c| c.version.clone());
    }
    if newest.is_none() {
        println!("debmagic: no candidates found");
    }
    Ok(())
}

/// Run the `debmagic upstream switch` command: resolve the candidate
/// version, then switch the main source and every component source to it.
pub async fn switch(command: UpstreamSwitchCommand) -> anyhow::Result<()> {
    let source_dir = resolve_source_dir(&command.fallback_dir, command.source_dir.as_deref())?;
    let identity = load_package(&source_dir)?;
    let sources = query::load_watch(&source_dir, identity.name())?;
    let mut config = Config::load(Some(&source_dir), command.config_file.as_deref())?;
    let output_dir = source_dir.join(&config.output_dir);

    // the main source (no Component field) drives the version;
    // component sources contribute their own tarballs
    let main_source = sources
        .iter()
        .find(|s| s.untrackable.is_none() && s.component.is_none())
        .context("no usable main watch source found")?;
    let candidate = if command.version == "latest" {
        // discovery needs the listing
        let candidates = query::query_source(main_source, identity.name()).await?;
        candidates.first().cloned()
    } else {
        // a concrete version: construct the URL from the watch
        // pattern directly; only fall back to scraping when the
        // pattern is not invertible or the URL does not exist.
        // an existing orig tarball hints at the extension first.
        let existing_orig = crate::changes::find_orig_in_dir(
            &output_dir,
            identity.name(),
            identity.version().upstream_version(),
            None,
        )
        .or_else(|| {
            source_dir.parent().and_then(|parent| {
                crate::changes::find_orig_in_dir(
                    parent,
                    identity.name(),
                    identity.version().upstream_version(),
                    None,
                )
            })
        });
        match query::resolve_concrete(
            main_source,
            identity.name(),
            &command.version,
            existing_orig.as_deref(),
        )
        .await
        {
            Ok(Some(candidate)) => Some(candidate),
            Ok(None) | Err(_) => {
                let candidates = query::query_source(main_source, identity.name()).await?;
                query::find_candidate(&candidates, &command.version).cloned()
            }
        }
    };
    let Some(candidate) = candidate else {
        if command.version == "latest" {
            bail!("no upstream candidates found for {}", identity.name());
        }
        bail!(
            "upstream version {} not found for {}; run 'debmagic upstream list' to see the available versions",
            command.version,
            identity.name()
        );
    };

    let package = load_package(&source_dir)?;
    let repack = repack::load_repack_config(&package, main_source)?;
    let verify = config.upstream.verify_signatures && !command.no_signature_check;
    if let Some(verify_command) = &command.verify_command {
        config.sign.verify_command = Some(verify_command.clone());
    }
    let components: Vec<String> = sources
        .iter()
        .filter_map(|source| source.component.clone())
        .collect();
    let options = switch::SwitchOptions {
        repack: &repack,
        output_dir: &output_dir,
        orig_tarball_config: Some(&config.orig_tarball),
        verify_signatures: verify,
        sign_options: &config.sign,
        dry_run: command.dry_run,
        components: &components,
    };
    switch::switch(&source_dir, main_source, &candidate, &options).await?;

    // MUT: switch each component source to the same version
    for source in &sources {
        if source.untrackable.is_some() || source.component.is_none() {
            continue;
        }
        let candidates = query::query_source(source, identity.name()).await?;
        let Some(component_candidate) = candidates
            .iter()
            .find(|c| c.version == candidate.version)
            .or_else(|| candidates.first())
        else {
            bail!(
                "no candidates for component {} at version {}",
                source.component.as_deref().unwrap_or_default(),
                candidate.version
            );
        };
        let repack = repack::load_repack_config(&package, source)?;
        let options = switch::SwitchOptions {
            repack: &repack,
            output_dir: &output_dir,
            orig_tarball_config: Some(&config.orig_tarball),
            verify_signatures: verify,
            sign_options: &config.sign,
            dry_run: command.dry_run,
            components: &components,
        };
        switch::switch(&source_dir, source, component_candidate, &options).await?;
    }
    Ok(())
}
