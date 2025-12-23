use std::{
    env,
    path::{self, PathBuf},
};

use anyhow::Context;
use clap::{CommandFactory, Parser};

use crate::{
    build::{build_package, common::PackageDescription, get_shell_in_build},
    cli::{Cli, Commands},
    config::Config,
};

pub mod build;
pub mod cli;
pub mod config;

fn get_config_file_paths() -> Vec<PathBuf> {
    let mut config_file_paths = vec![];
    let xdg_config_file = dirs::config_dir().map(|p| p.join("debmagic").join("config.toml"));
    if let Some(xdg_config_file) = xdg_config_file {
        if xdg_config_file.is_file() {
            config_file_paths.push(xdg_config_file);
        }
    }

    config_file_paths
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut config_file_paths = get_config_file_paths();
    if let Some(config_override) = &cli.config {
        config_file_paths.push(config_override.clone());
    }
    let config = Config::new(&config_file_paths, &cli)?;

    let current_dir = env::current_dir()?;
    match &cli.command {
        Commands::Build(args) => {
            let source_dir = args.source_dir.as_deref().unwrap_or(&current_dir);
            let package = PackageDescription {
                name: "debmagic".to_string(),
                version: "0.1.0".to_string(),
                source_dir: path::absolute(source_dir).context("resolving source dir failed")?,
            };
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
            let package = PackageDescription {
                name: "debmagic".to_string(),
                version: "0.1.0".to_string(),
                source_dir: path::absolute(source_dir).context("resolving source dir failed")?,
            };
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
