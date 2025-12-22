use std::{env, fs};

use clap::{CommandFactory, Parser};

use crate::{
    build::{build_package, common::PackageDescription, get_shell_in_build},
    cli::{Cli, Commands},
    config::Config,
};

pub mod build;
pub mod cli;
pub mod config;

fn cli() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::new(&cli)?;

    let current_dir = env::current_dir()?;
    match &cli.command {
        Commands::Build(args) => {
            let source_dir = args.source_dir.as_deref().unwrap_or(&current_dir);
            let package = PackageDescription {
                name: "debmagic".to_string(),
                version: "0.1.0".to_string(),
                source_dir: fs::canonicalize(source_dir)?,
            };
            let output_dir = args.output_dir.as_deref().unwrap_or(&current_dir);
            build_package(
                &config,
                &package,
                args.driver,
                &fs::canonicalize(output_dir)?,
            )?;
        }
        Commands::Shell(args) => {
            let source_dir = args.source_dir.as_deref().unwrap_or(&current_dir);
            let package = PackageDescription {
                name: "debmagic".to_string(),
                version: "0.1.0".to_string(),
                source_dir: fs::canonicalize(source_dir)?,
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

fn main() {
    let result = cli();
    if let Err(err) = result {
        eprintln!("Error: {err}");
    }
}
