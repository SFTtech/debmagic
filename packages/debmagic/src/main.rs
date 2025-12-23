use std::{env, path};

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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::new(&cli)?;

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
