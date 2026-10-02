use anyhow::{Context, bail};

use crate::config::Config;
use crate::environment::{
    DestroyWhen, EnvironmentStatus, Registry, assess_environment, claim_and_destroy,
    select_unique_environment,
};
use crate::package::load_package;

pub async fn list_environments(config: &Config) -> anyhow::Result<()> {
    let Some(registry) = Registry::open_default_if_exists().await? else {
        println!("No Environments in the registry.");
        return Ok(());
    };
    let environments = registry.list().await?;
    if environments.is_empty() {
        println!("No Environments in the registry.");
        return Ok(());
    }

    println!(
        "{:<16} {:<6} {:<8} {:<18} {:<10} {:<10} {:<6} {:<11} SOURCE",
        "ID", "PURPOSE", "DRIVER", "PACKAGE", "DISTRO", "PERSIST", "STATUS", "ATTACH"
    );
    for registered in environments {
        let assessed = assess_environment(&registry, &registered, &config.driver).await?;
        let env = &registered.environment;
        println!(
            "{:<16} {:<6} {:<8} {:<18} {:<10} {:<10} {:<6} {:<11} {}",
            env.id(),
            env.purpose.as_str(),
            env.driver.as_str(),
            env.package_identifier,
            env.distro.codename,
            env.persistence.as_str(),
            assessed.status.as_str(),
            assessed.attachments,
            env.source_dir.display(),
        );
    }
    Ok(())
}

/// Destroy every Stale Environment on the machine.
pub async fn clean_environments(config: &Config) -> anyhow::Result<()> {
    let Some(registry) = Registry::open_default_if_exists().await? else {
        println!("Cleaned 0 Stale Environment(s); skipped 0.");
        return Ok(());
    };
    let environments = registry.list().await?;
    let mut cleaned = 0;
    let mut skipped = 0;
    for registered in environments {
        let id = registered.environment.id();
        let status = match assess_environment(&registry, &registered, &config.driver).await {
            Ok(assessed) => assessed.status,
            Err(error) => {
                // Fail closed: never destroy what we cannot assess.
                eprintln!("skipping Environment {id}: {error}");
                skipped += 1;
                continue;
            }
        };
        match status {
            EnvironmentStatus::Unreachable => {
                eprintln!("skipping unreachable Environment {id}");
                skipped += 1;
            }
            EnvironmentStatus::Stale => {
                match claim_and_destroy(&registry, &config.driver, &id, DestroyWhen::StillStale)
                    .await
                {
                    Ok(true) => cleaned += 1,
                    Ok(false) => {
                        eprintln!("skipping Environment {id}: no longer Stale");
                        skipped += 1;
                    }
                    Err(error) => {
                        eprintln!("skipping Environment {id}: {error}");
                        skipped += 1;
                    }
                }
            }
            EnvironmentStatus::Live => {}
        }
    }
    println!("Cleaned {cleaned} Stale Environment(s); skipped {skipped}.");
    Ok(())
}

/// Destroy the given Environments, even healthy ones. Like `docker rm`, a
/// failure does not stop the others.
pub async fn remove_environments(
    config: &Config,
    ids: &[String],
    force: bool,
) -> anyhow::Result<()> {
    let Some(registry) = Registry::open_default_if_exists().await? else {
        bail!("no Environment with id {}", ids.join(", "));
    };
    let mut failed = 0;
    for id in ids {
        match remove_one(&registry, config, id, force).await {
            Ok(()) => println!("Destroyed Environment {id}"),
            Err(error) => {
                eprintln!("Error: {error:#}");
                failed += 1;
            }
        }
    }
    if failed > 0 {
        bail!("{failed} of {} Environment(s) not removed", ids.len());
    }
    Ok(())
}

async fn remove_one(
    registry: &Registry,
    config: &Config,
    id: &str,
    force: bool,
) -> anyhow::Result<()> {
    if registry.get(id).await?.is_none() {
        bail!("no Environment with id {id}");
    }
    let when = if force {
        DestroyWhen::Always
    } else {
        DestroyWhen::Reachable
    };
    if !claim_and_destroy(registry, &config.driver, id, when).await? {
        bail!(
            "Environment {id} is Unreachable; make the Driver queryable and retry, \
             or pass --force to destroy it without Driver cooperation"
        );
    }
    Ok(())
}

pub async fn shell_environment(
    config: &Config,
    source_dir: &std::path::Path,
    id: Option<&str>,
) -> anyhow::Result<()> {
    let Some(registry) = Registry::open_default_if_exists().await? else {
        match id {
            Some(id) => bail!("no Environment with id {id}"),
            None => bail!("no Environment found for this Source tree"),
        }
    };
    let registered = match id {
        Some(id) => registry
            .get(id)
            .await?
            .with_context(|| format!("no Environment with id {id}"))?,
        None => {
            let package = load_package(source_dir)?;
            let candidates = registry.list_for_source_tree(source_dir).await?;
            let package_identifier = format!("{}-{}", package.name(), package.version());
            select_unique_environment(&candidates, &package_identifier)?.clone()
        }
    };

    crate::environment::shell_environment(&registry, &config.driver, &registered).await
}

/// When no explicit id is given, resolve the Source tree from cwd / `--source-dir`.
///
/// Canonicalized: Environment ids and registry lookups key on the `source_dir`
/// string, so all entry points must agree on one spelling per tree.
pub fn resolve_shell_source_dir(
    source_dir: Option<&std::path::Path>,
    cwd: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    let dir = std::fs::canonicalize(source_dir.unwrap_or(cwd))?;
    load_package(&dir).map(|_| dir)
}
