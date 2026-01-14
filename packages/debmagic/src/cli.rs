use std::path::PathBuf;

use crate::build::common::BuildDriverType;
use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Build(BuildSubcommandArgs),
    Shell(ShellSubcommandArgs),
    Test {},
    Check {},
    Version {},
}

#[derive(Args, Debug)]
pub struct BuildSubcommandArgs {
    #[arg(short, long)]
    pub driver: BuildDriverType,

    #[arg(long)]
    pub driver_docker_build_image: Option<String>,

    #[arg(long)]
    pub driver_persistent: Option<bool>,

    #[arg(short, long)]
    pub source_dir: Option<PathBuf>,
    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ShellSubcommandArgs {
    #[arg(short, long)]
    pub source_dir: Option<PathBuf>,
}
