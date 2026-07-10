use std::path::PathBuf;

use crate::build::common::BuildDriverType;
use clap::{Args, Parser, Subcommand, builder::BoolishValueParser};

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
    #[command(about = "Build a debian package: 'binary' (.deb) or 'source' (.dsc) packages")]
    Build(Box<BuildSubcommandArgs>),
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
        id = "docker-base-image",
        long = "driver-docker-base-image",
        help = "If passed will override the base image for the current build"
    )]
    pub base_image: Option<String>,
}

#[derive(Args, Debug)]
pub struct LxdArgs {
    #[arg(
        id = "lxd-base-image",
        long = "driver-lxd-base-image",
        help = "Override the base image (image alias) for the LXD/Incus container"
    )]
    pub base_image: Option<String>,

    #[arg(
        long = "driver-lxd-project",
        help = "LXD/Incus project to register the container in"
    )]
    pub project: Option<String>,
}

/// Flags shared between `debmagic build binary` and `debmagic build source`.
#[derive(Args, Debug)]
pub struct CommonBuildArgs {
    #[arg(
        short,
        long,
        help = "Build driver type. Required for binary builds; source-only builds default to 'bare', since those need no build-deps or compilation."
    )]
    pub driver: Option<BuildDriverType>,

    #[arg(long, action = clap::ArgAction::SetTrue, help = "Keep the build environment for reuse after the build finishes")]
    pub persistent: Option<bool>,

    #[command(flatten)]
    pub docker: DockerArgs,

    #[command(flatten)]
    pub lxd: LxdArgs,

    #[arg(
        long = "apt-mirror",
        help = "Apt mirror URL to use inside the build environment instead of the default archive.ubuntu.com/security.ubuntu.com/deb.debian.org, e.g. http://my-mirror.example/ubuntu. Ignored by the bare driver."
    )]
    pub apt_mirror: Option<String>,

    #[arg(
        long,
        action = clap::ArgAction::SetTrue,
        help = "Also enable the '<release>-proposed' pocket in the build environment. Ignored by the bare driver."
    )]
    pub proposed: Option<bool>,

    #[arg(
        long,
        help = "Select the target distribution version, only required if the debian changelog specifies multiple versions"
    )]
    pub distro: Option<String>,

    #[arg(
        long,
        value_parser = BoolishValueParser::new(),
        help = "Sign the resulting .changes/.dsc with debsign after building (yes/no). Defaults to the 'sign_package' setting in the config file (false if unset). Always runs on the host, using your own gpg keyring, regardless of --driver."
    )]
    pub sign: Option<bool>,

    #[arg(
        long = "sign-key",
        help = "GPG key ID/email to sign with, passed to debsign's -k option. Defaults to the 'sign_key' setting in the config file, or debsign's own maintainer-based key lookup if unset."
    )]
    pub sign_key: Option<String>,

    #[arg(
        long,
        action = clap::ArgAction::SetTrue,
        help = "Run 'debian/rules clean' before building, like plain dpkg-buildpackage does unless passed -nc. Defaults to the 'clean' setting in the config file (false if unset); non-incremental builds already stage a clean source tree, while incremental builds preserve outputs by design. For source builds this also installs build-dependencies first, since a clean target usually needs its own tooling."
    )]
    pub clean: Option<bool>,

    #[arg(
        long,
        action = clap::ArgAction::SetFalse,
        help = "Do not run 'debian/rules clean' before building, overriding a 'clean = true' default in the config file."
    )]
    pub no_clean: Option<bool>,

    #[command(flatten)]
    pub common: CommonCli,

    #[arg(short, long, help = "Output directory for the package artifacts")]
    pub output_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct BuildSubcommandArgs {
    #[command(subcommand)]
    pub target: BuildTarget,
}

#[derive(Subcommand, Debug)]
pub enum BuildTarget {
    #[command(about = "Build binary .deb packages")]
    Binary(BinaryTargetArgs),
    #[command(
        about = "Build a source package only (.dsc + tarball, plus .buildinfo/.changes), no build-deps or compilation required"
    )]
    Source(SourceTargetArgs),
}

#[derive(Args, Debug)]
pub struct BinaryTargetArgs {
    #[command(flatten)]
    pub build: CommonBuildArgs,

    #[arg(short, long, action = clap::ArgAction::SetTrue, help = "Synchronize changed source inputs while preserving build outputs. Implies --persistent")]
    pub incremental: Option<bool>,

    #[arg(
        long = "debug-symbols",
        action = clap::ArgAction::SetTrue,
        help = "Also build the automatic '-dbgsym' debug symbol package"
    )]
    pub debug_symbols: Option<bool>,
}

#[derive(Args, Debug)]
pub struct SourceTargetArgs {
    #[command(flatten)]
    pub build: CommonBuildArgs,
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
