use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Context;

use crate::config::Config;
use crate::package::load_package;
use fetch::FetchFrom;

pub mod fetch;
pub mod mangle;
pub mod orig;
pub mod query;
pub mod repack;
pub mod switch;
pub mod watch;

/// Inputs for the `debmagic upstream list` command.
#[derive(Debug, Clone)]
pub struct UpstreamListCommand {
    pub source_dir: PathBuf,
    pub config_file: Option<PathBuf>,
    /// Show all candidate versions, not just the newest newer ones.
    pub all: bool,
    /// Show the N versions newer than the changelog's, not just the newest.
    pub previous: Option<usize>,
}

/// Run the `debmagic upstream list` command: query each watch source and
/// print its current and newer upstream versions.
pub async fn list(command: UpstreamListCommand) -> anyhow::Result<()> {
    let source_dir = command.source_dir;
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

/// Inputs shared by the `debmagic upstream fetch` and `switch` commands.
#[derive(Debug, Clone)]
pub struct UpstreamFetchCommand {
    pub source_dir: PathBuf,
    pub config_file: Option<PathBuf>,
    /// An upstream or Debian version, or `latest`; `None` means the
    /// changelog's upstream version.
    pub version: Option<String>,
    pub from: FetchFrom,
    /// Overrides `upstream.repack`.
    pub repack: Option<bool>,
    /// Overrides `upstream.signature_check`.
    pub signature_check: Option<bool>,
    /// Custom verification command overriding `sign.verify_command`.
    pub verify_command: Option<String>,
    pub force: bool,
    pub dry_run: bool,
    /// Overrides the configured output dir.
    pub output_dir: Option<PathBuf>,
}

/// Run `debmagic upstream fetch`: put the orig tarballs (main and MUT
/// components) of an upstream version into the output dir.
pub async fn fetch(command: UpstreamFetchCommand) -> anyhow::Result<()> {
    run_fetch(command, false).await
}

/// Run `debmagic upstream switch`: fetch like `upstream fetch`, then
/// replace the tree (and each component's subdir) with the tarballs'
/// contents.
pub async fn switch(command: UpstreamFetchCommand) -> anyhow::Result<()> {
    run_fetch(command, true).await
}

async fn run_fetch(command: UpstreamFetchCommand, switch_tree: bool) -> anyhow::Result<()> {
    let source_dir = command.source_dir;
    let package = load_package(&source_dir)?;
    let sources = query::load_watch(&source_dir, package.name())?;
    let mut config = Config::load(Some(&source_dir), command.config_file.as_deref())?;
    if let Some(verify_command) = command.verify_command {
        config.sign.verify_command = Some(verify_command);
    }
    let output_dir = std::path::absolute(
        command
            .output_dir
            .unwrap_or_else(|| source_dir.join(&config.output_dir)),
    )
    .context("resolving the output dir failed")?;
    let options = fetch::FetchOptions {
        from: command.from,
        orig_tarball: &config.orig_tarball,
        repack: command.repack.unwrap_or(config.upstream.repack),
        signature_check: command
            .signature_check
            .unwrap_or(config.upstream.signature_check),
        sign: &config.sign,
        output_dir: &output_dir,
        force: command.force,
        dry_run: command.dry_run,
    };

    // the main source (no Component field) drives the version;
    // component sources contribute their own tarballs
    let main_source = sources
        .iter()
        .find(|s| s.untrackable.is_none() && s.component.is_none())
        .context("no usable main watch source found")?;
    let components: Vec<String> = sources
        .iter()
        .filter_map(|source| source.component.clone())
        .collect();
    let requested = command
        .version
        .unwrap_or_else(|| package.version().upstream_version().to_string());

    let run = async |source: &watch::WatchSource, version: &str| {
        let what = match &source.component {
            Some(component) => format!("component {component} of {}", package.name()),
            None => package.name().to_string(),
        };
        let result = if switch_tree {
            switch::switch(&package, source, version, &options, &components).await
        } else {
            fetch::fetch(&package, source, version, &options).await
        };
        result.with_context(|| format!("fetching {what} {version} failed"))
    };

    let main = run(main_source, &requested).await?;
    // MUT: the components follow the main tarball's version
    for source in &sources {
        if source.untrackable.is_some() || source.component.is_none() {
            continue;
        }
        run(source, &main.upstream_version).await?;
    }
    Ok(())
}
