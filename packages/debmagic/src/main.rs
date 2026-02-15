use std::{
    env,
    path::{self, PathBuf},
};

use anyhow::Context;
use clap::{CommandFactory, Parser};

use crate::{
    build::{build_package, get_shell_in_build},
    cli::{Cli, Commands},
    config::Config,
    package::PackageDescription,
};

pub mod build;
pub mod cli;
pub mod config;
pub mod package;

/// Precedence of config files is:
///
/// 1. explicit config file passed on the command line
/// 2. `<source_dir>/debian/debmagic.toml`
/// 3. `<XDG_CONFIG_HOME>/debmagic/config.toml`
///
fn get_config(cli: &Cli, source_dir: &Option<PathBuf>) -> anyhow::Result<Config> {
    let mut config_file_paths = vec![];
    let xdg_config_file = dirs::config_dir().map(|p| p.join("debmagic").join("config.toml"));
    if let Some(xdg_config_file) = xdg_config_file
        && xdg_config_file.is_file()
    {
        config_file_paths.push(xdg_config_file);
    }

    if let Some(source_dir) = &source_dir {
        config_file_paths.push(source_dir.join("debian").join("debmagic.toml"));
    }

    if let Some(config_file_override) = &cli.config {
        config_file_paths.push(config_file_override.clone());
    }

    let config = Config::new(&config_file_paths)?;
    Ok(config)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let current_dir = env::current_dir()?;
    match &cli.command {
        Commands::Build(args) => {
            let source_dir = args.common.source_dir.as_deref().unwrap_or(&current_dir);
            let mut config = get_config(&cli, &Some(source_dir.to_path_buf()))?;

            // TODO: figure out a better way to override config from CLI args - maybe more generic, if that is even possible since
            // we want a nice cli which somewhat matches the config structure
            // but some config options only make sense in some cli subcommands -> these flags don't make sense in all commands
            // and should only be used in some
            if let Some(persist_driver) = args.persist_driver {
                config.driver.persistent = persist_driver;
            }

            if let Some(incremental) = args.incremental {
                config.incremental = incremental;
            }
            if config.incremental {
                // TODO: investigate if this is actually needed
                config.driver.persistent = true;
            }
            if let Some(docker_base_image) = &args.docker.base_image {
                config.driver.docker.base_image = Some(docker_base_image.clone());
            }

            let package = PackageDescription::from_dir(
                &path::absolute(source_dir).context("resolving source dir failed")?,
            )?;
            let output_dir = args.output_dir.as_deref().unwrap_or(&current_dir);
            build_package(
                &config,
                &package,
                args.driver,
                &path::absolute(output_dir).context("resolving output dir failed")?,
            )
            .context("Building the package failed")?;
        }
        Commands::Shell(args) => {
            let source_dir = args.common.source_dir.as_deref().unwrap_or(&current_dir);
            let config = get_config(&cli, &Some(source_dir.to_path_buf()))?;
            let package = PackageDescription::from_dir(
                &path::absolute(source_dir).context("resolving source dir failed")?,
            )?;
            get_shell_in_build(&config, &package)?;
        }
        Commands::Test(_args) => {
            println!("Test subcommand! - not implemented");
        }
        Commands::Check(_args) => {
            println!("Check subcommand! - not implemented");
        }
        Commands::Version {} => {
            let cmd = Cli::command();
            println!("{}", cmd.render_version());
        }
    }

    Ok(())
}
