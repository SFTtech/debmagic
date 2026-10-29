use std::path::{Path, PathBuf};

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

    let mut rows = vec![
        [
            "ID", "PURPOSE", "DRIVER", "PACKAGE", "DISTRO", "PERSIST", "STATUS", "ATTACH", "SOURCE",
        ]
        .map(String::from),
    ];
    for registered in environments {
        let assessed = assess_environment(&registry, &registered, &config.driver).await?;
        let env = &registered.environment;
        rows.push([
            env.id().to_string(),
            env.purpose.as_str().to_string(),
            env.driver.as_str().to_string(),
            env.package_identifier.to_string(),
            env.distro.codename.to_string(),
            env.persistence.as_str().to_string(),
            assessed.status.as_str().to_string(),
            assessed.attachments.to_string(),
            env.source_dir.display().to_string(),
        ]);
    }
    print!("{}", crate::output::table(&rows));
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

async fn selected_environment(
    source_dir: &std::path::Path,
    id: Option<&str>,
) -> anyhow::Result<(Registry, crate::environment::RegisteredEnvironment)> {
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
    Ok((registry, registered))
}

fn command_context(
    current_dir: &Path,
    config_file: Option<&Path>,
    id: Option<&str>,
    source_dir: Option<&Path>,
) -> anyhow::Result<(Config, PathBuf)> {
    if id.is_some() {
        return Ok((Config::load(None, config_file)?, current_dir.to_path_buf()));
    }

    let source_dir = resolve_shell_source_dir(Some(source_dir.unwrap_or(current_dir)), current_dir)
        .map_err(|_| {
            anyhow::anyhow!("not a Source tree; pass an Environment id (see `debmagic env list`)")
        })?;
    let config = Config::load(Some(&source_dir), config_file)?;
    Ok((config, source_dir))
}

/// Returns the login shell's exit code.
pub async fn shell_environment(
    current_dir: &Path,
    config_file: Option<&Path>,
    id: Option<&str>,
    source_dir: Option<&Path>,
    user: Option<&str>,
) -> anyhow::Result<i32> {
    let (config, source_dir) = command_context(current_dir, config_file, id, source_dir)?;
    let (registry, registered) = selected_environment(&source_dir, id).await?;
    crate::environment::shell_environment(&registry, &config.driver, &registered, user).await
}

/// Execute `command` directly, without a shell, and return its exit code.
pub async fn exec_environment(
    current_dir: &Path,
    config_file: Option<&Path>,
    id: Option<&str>,
    source_dir: Option<&Path>,
    user: Option<&str>,
    command: &[String],
) -> anyhow::Result<i32> {
    let (config, source_dir) = command_context(current_dir, config_file, id, source_dir)?;
    let (registry, registered) = selected_environment(&source_dir, id).await?;
    let command: Vec<&str> = command.iter().map(String::as_str).collect();
    crate::environment::exec_environment(&registry, &config.driver, &registered, user, &command)
        .await
}

/// When no explicit id is given, resolve the Source tree from cwd / `--source-dir`.
///
/// Canonicalized: Environment ids and registry lookups key on the `source_dir`
/// string, so all entry points must agree on one spelling per tree.
fn resolve_shell_source_dir(source_dir: Option<&Path>, cwd: &Path) -> anyhow::Result<PathBuf> {
    let dir = std::fs::canonicalize(source_dir.unwrap_or(cwd))?;
    load_package(&dir).map(|_| dir)
}
