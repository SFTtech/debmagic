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
    Test(TestSubcommandArgs),
    Check(CheckSubcommandArgs),
    Version {},
}

#[derive(Args, Debug)]
pub struct CommonCli {
    #[arg(short, long)]
    pub source_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct DockerArgs {
    #[arg(long("driver-docker-base-image"))]
    pub base_image: Option<String>,
}

#[derive(Args, Debug)]
pub struct BuildSubcommandArgs {
    #[arg(short, long)]
    pub driver: BuildDriverType,

    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub persist_driver: Option<bool>,

    #[command(flatten)]
    pub docker: DockerArgs,

    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    pub incremental: Option<bool>,

    #[command(flatten)]
    pub common: CommonCli,

    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ShellSubcommandArgs {
    #[command(flatten)]
    pub common: CommonCli,
}

#[derive(Args, Debug)]
pub struct TestSubcommandArgs {
    #[command(flatten)]
    pub common: CommonCli,
}

#[derive(Args, Debug)]
pub struct CheckSubcommandArgs {
    #[command(flatten)]
    pub common: CommonCli,
}
