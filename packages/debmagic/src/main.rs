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

fn get_config(cli: &Cli, source_dir: &Option<PathBuf>) -> anyhow::Result<Config> {
    let mut config_file_paths = vec![];
    let xdg_config_file = dirs::config_dir().map(|p| p.join("debmagic").join("config.toml"));
    if let Some(xdg_config_file) = xdg_config_file
        && xdg_config_file.is_file()
    {
        config_file_paths.push(xdg_config_file);
    }

    if let Some(source_dir) = &source_dir {
        config_file_paths.push(source_dir.join(".debmagic.toml"));
        config_file_paths.push(source_dir.join("debmagic.toml"));
    }

    if let Some(config_file_override) = &cli.config {
        config_file_paths.push(config_file_override.clone());
    }

    let config = Config::new(&config_file_paths, cli)?;
    Ok(config)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let current_dir = env::current_dir()?;
    match &cli.command {
        Commands::Build(args) => {
            let source_dir = args.source_dir.as_deref().unwrap_or(&current_dir);
            let config = get_config(&cli, &Some(source_dir.to_path_buf()))?;
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
            let source_dir = args.source_dir.as_deref().unwrap_or(&current_dir);
            let config = get_config(&cli, &Some(source_dir.to_path_buf()))?;
            let package = PackageDescription::from_dir(
                &path::absolute(source_dir).context("resolving source dir failed")?,
            )?;
            get_shell_in_build(&config, &package)?;
        }
        Commands::Test {} => {
            println!("Test subcommand! - not implemented");
        }
        Commands::Check {} => {
            println!("Check subcommand! - not implemented");
        }
        Commands::Version {} => {
            let cmd = Cli::command();
            println!("{}", cmd.render_version());
        }
    }

    Ok(())
}
