use std::path::PathBuf;

use crate::build::common::BuildDriverType;
use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, help = "Path to config file")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Build a debian package")]
    Build(BuildSubcommandArgs),
    #[command(about = "Open an interactive shell to the currently active build environment")]
    Shell(ShellSubcommandArgs),
    #[command(about = "Run tests")]
    Test(TestSubcommandArgs),
    #[command(about = "Check the project")]
    Check(CheckSubcommandArgs),
    #[command(about = "Show version information")]
    Version {},
}

#[derive(Args, Debug)]
pub struct CommonCli {
    #[arg(
        short,
        long,
        help = "Path to the parent directory of the debian package. If not specified defaults to the current working directory"
    )]
    pub source_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct DockerArgs {
    #[arg(
        long = "driver-docker-base-image",
        help = "If passed will override the base image for the current build"
    )]
    pub base_image: Option<String>,
}

#[derive(Args, Debug)]
pub struct BuildSubcommandArgs {
    #[arg(short, long, help = "Build driver type")]
    pub driver: BuildDriverType,

    #[arg(long, action = clap::ArgAction::SetTrue, help = "Persist the build environment after the build finished")]
    pub persist_driver: Option<bool>,

    #[command(flatten)]
    pub docker: DockerArgs,

    #[arg(short, long, action = clap::ArgAction::SetTrue, help = "Enable incremental builds. This implies --persist-driver")]
    pub incremental: Option<bool>,

    #[arg(
        long,
        help = "Select the target distribution version, only required in the debian changelog specifies multiple versions"
    )]
    pub distro: Option<String>,

    #[command(flatten)]
    pub common: CommonCli,

    #[arg(short, long, help = "Output directory for the package artifacts")]
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
