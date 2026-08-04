use std::env;

use anyhow::Context;
use clap::{CommandFactory, Parser};

use crate::{
    build::{build_package, get_shell_in_build},
    build_intent::{BuildIntentInput, load_config, resolve_build_intent},
    cli::{Cli, Commands},
    package::{load_package_identity, resolve_package_target},
};

pub mod build;
pub mod build_intent;
pub mod cli;
pub mod config;
pub mod package;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let current_dir = env::current_dir()?;
    match &cli.command {
        Commands::Build(args) => {
            let intent = resolve_build_intent(BuildIntentInput {
                fallback_dir: current_dir,
                source_dir: args.common.source_dir.clone(),
                output_dir: args.output_dir.clone(),
                config_file: cli.config.clone(),
                driver: args.driver,
                persist_driver: args.persist_driver,
                incremental: args.incremental,
                docker_base_image: args.docker.base_image.clone(),
            })?;

            let target = resolve_package_target(&intent.source_dir, args.distro.as_deref())
                .context("failed to resolve package target")?;
            build_package(&intent, &target).context("Building the package failed")?;
        }
        Commands::Shell(args) => {
            let source_dir = args.common.source_dir.as_deref().unwrap_or(&current_dir);
            let source_dir =
                std::path::absolute(source_dir).context("resolving source dir failed")?;
            let config = load_config(Some(&source_dir), cli.config.as_deref())?;
            let identity = load_package_identity(&source_dir)?;
            get_shell_in_build(&config, &identity)?;
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
