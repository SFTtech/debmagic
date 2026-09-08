use std::path::PathBuf;

use crate::build::source::SourceSyncMode;
use crate::driver::DriverType;
use crate::sign::SignTool;
use clap::{Args, Parser, Subcommand};

/// When to use colored output. Mirrors common CLI conventions; `auto` is the
/// default and respects the `NO_COLOR` environment variable.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorChoice {
    /// Color when stderr is a terminal and `NO_COLOR` is unset.
    #[default]
    Auto,
    /// Always color, even when piped or `NO_COLOR` is set.
    Always,
    /// Never color.
    Never,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, help = "Path to config file")]
    pub config: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_enum,
        default_value_t = ColorChoice::Auto,
        help = "When to colorize output: 'auto' (default) colors on a terminal and respects NO_COLOR, 'always' forces color, 'never' disables it"
    )]
    pub color: ColorChoice,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Build a debian package: 'binary' (.deb) or 'source' (.dsc) packages")]
    Build(Box<BuildSubcommandArgs>),
    #[command(about = "Open an interactive shell to the currently active build environment")]
    Shell(ShellSubcommandArgs),
    #[command(about = "Run the package's declared Debian autopkgtest tests against a prior build")]
    Test(TestSubcommandArgs),
    #[command(about = "Check the project")]
    Check(CheckSubcommandArgs),
    #[command(about = "GPG-sign a .changes file (and its .dsc/.buildinfo) on the host")]
    Sign(SignSubcommandArgs),
    #[command(about = "Inspect the debmagic configuration")]
    Config(ConfigSubcommandArgs),
    #[command(about = "Show version information")]
    Version {},
}

#[derive(Args, Debug)]
pub struct ConfigSubcommandArgs {
    #[command(subcommand)]
    pub command: ConfigCommands,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    #[command(
        about = "Print the effective config as TOML, and which config file paths were considered"
    )]
    Show(ConfigShowSubcommandArgs),
    #[command(about = "Print a single config value, addressed by dotted key (e.g. 'sign.key')")]
    Get(ConfigGetSubcommandArgs),
    #[command(about = "Set a single config value, addressed by dotted key (e.g. 'sign.key ROFL')")]
    Set(ConfigSetSubcommandArgs),
}

#[derive(Args, Debug)]
pub struct ConfigShowSubcommandArgs {
    #[command(flatten)]
    pub common: CommonCli,
}

#[derive(Args, Debug)]
pub struct ConfigGetSubcommandArgs {
    #[arg(help = "Config key, dotted path like 'sign.key' or 'driver.default'")]
    pub key: String,

    #[command(flatten)]
    pub common: CommonCli,
}

#[derive(Args, Debug)]
pub struct ConfigSetSubcommandArgs {
    #[arg(help = "Config key, dotted path like 'sign.key' or 'driver.default'")]
    pub key: String,

    #[arg(
        help = "Value to set; parsed as TOML when valid (true, 3, [\"a\"]), else treated as a string"
    )]
    pub value: String,

    #[arg(
        long,
        help = "Write to the user-wide config file instead of the project's debian/debmagic.toml"
    )]
    pub global: bool,

    #[command(flatten)]
    pub common: CommonCli,
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
        num_args = 0..=1,
        default_missing_value = "true",
        action = clap::ArgAction::Set,
        help = "Synchronize changed source inputs while preserving build outputs. Implies --persistent"
    )]
    pub incremental: Option<bool>,

    #[arg(
        short,
        long,
        help = "Build driver type. Defaults to the 'driver' key in debmagic.toml; without either, source-only builds use 'bare', since those need no build-deps or compilation."
    )]
    pub driver: Option<DriverType>,

    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        action = clap::ArgAction::Set,
        help = "Keep the build environment for reuse after the build finishes"
    )]
    pub persistent: Option<bool>,

    #[arg(
        long = "source-sync",
        help = "Which source files are staged into the build tree: 'tracked' stages git-tracked files including uncommitted changes and warns about untracked files (default), 'committed' additionally fails if the worktree is dirty, 'worktree' stages everything that is not git-ignored. Defaults to the 'source_sync_mode' setting in the config file."
    )]
    pub source_sync: Option<SourceSyncMode>,

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
        num_args = 0..=1,
        default_missing_value = "true",
        action = clap::ArgAction::Set,
        help = "Also enable the '<release>-proposed' pocket in the build environment. Ignored by the bare driver."
    )]
    pub proposed: Option<bool>,

    #[arg(
        long,
        help = "Select the target distribution version, only required if the debian changelog specifies multiple versions"
    )]
    pub distro: Option<String>,
    #[arg(
        long = "host-arch-variant",
        help = "Build for a dpkg architecture variant (e.g. 'amd64v3' on Ubuntu), like dpkg-buildpackage's --host-arch-variant. Sets DEB_HOST_ARCH_VARIANT for the build, which makes the Ubuntu vendor hook append the variant's -march= flags and names the .changes file after the variant. Defaults to the 'host_arch_variant' setting in the config file."
    )]
    pub host_arch_variant: Option<String>,

    // NOTE: Option<bool> flags use ArgAction::Set with default_missing_value
    // for tri-state parsing (None when absent) — SetTrue/SetFalse force an
    // implicit Some(false)/Some(true) default that would always override the
    // config file.
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = clap::value_parser!(bool),
        help = "Sign the resulting .changes/.dsc after building. Defaults to the 'sign.source' setting in the config file (false if unset)."
    )]
    pub sign: Option<bool>,

    #[arg(
        long = "sign-key",
        help = "GPG key ID/email to sign with. Defaults to the 'sign.key' setting in the config file, or the Changed-By/Maintainer address of the file being signed if unset."
    )]
    pub sign_key: Option<String>,

    #[arg(
        long = "sign-tool",
        value_enum,
        help = "OpenPGP implementation to sign with: 'gpg' (default), 'sequoia' (sq), or 'custom' (uses --sign-command). Defaults to the 'sign.tool' setting in the config file."
    )]
    pub sign_tool: Option<SignTool>,

    #[arg(
        long = "sign-command",
        help = "Custom signing command for --sign-tool custom, run without a shell. Supports {file}, {key} and {email} placeholders; writes the clearsigned result to stdout. Defaults to the 'sign.command' setting in the config file."
    )]
    pub sign_command: Option<String>,

    #[arg(
        long = "sign-notify",
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = clap::value_parser!(bool),
        help = "Send a desktop notification via notify-send just before signing, so a hardware-key touch prompt isn't missed. Defaults to the 'sign.notify' setting in the config file (false if unset)."
    )]
    pub sign_notify: Option<bool>,

    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = clap::value_parser!(bool),
        help = "Run 'debian/rules clean' before building, like plain dpkg-buildpackage does unless passed -nc. Defaults to the 'clean' setting in the config file (false if unset); non-incremental builds already stage a clean source tree, while incremental builds preserve outputs by design. For source builds this also installs build-dependencies first, since a clean target usually needs its own tooling."
    )]
    pub clean: Option<bool>,

    #[arg(
        long = "shell-on-failure",
        action = clap::ArgAction::SetTrue,
        help = "On build failure, drop into an interactive shell in the build environment when stdout is a TTY. Defaults to the 'shell_on_failure' setting in the config file (false if unset)."
    )]
    pub shell_on_failure: Option<bool>,

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

    #[arg(
        long = "debug-symbols",
        num_args = 0..=1,
        default_missing_value = "true",
        action = clap::ArgAction::Set,
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
    #[arg(
        short,
        long,
        help = "Driver type for the test environment. Defaults to the driver recorded in the prior build's environment.json."
    )]
    pub driver: Option<DriverType>,

    #[arg(long, action = clap::ArgAction::SetTrue, help = "Keep the test environment for reuse after the test run finishes")]
    pub persistent: Option<bool>,

    #[command(flatten)]
    pub docker: DockerArgs,

    #[command(flatten)]
    pub lxd: LxdArgs,

    #[arg(
        long = "apt-mirror",
        help = "Apt mirror URL to use inside the test environment instead of the default archive mirrors. Ignored by the bare driver."
    )]
    pub apt_mirror: Option<String>,

    #[arg(
        long,
        action = clap::ArgAction::SetTrue,
        help = "Also enable the '<release>-proposed' pocket in the test environment. Ignored by the bare driver."
    )]
    pub proposed: Option<bool>,

    #[arg(
        long,
        help = "Override the target distribution for the test environment. Defaults to the distro recorded in the prior build's environment.json, not the changelog."
    )]
    pub distro: Option<String>,

    #[arg(
        long,
        action = clap::ArgAction::SetTrue,
        help = "Treat skipped tests and 'no tests declared' as failures (exit code 2)"
    )]
    pub strict: bool,

    #[arg(
        long,
        help = "Path to a .changes file whose directory supplies the built .debs (for pipeline use)"
    )]
    pub changes: Option<PathBuf>,

    #[arg(
        long,
        action = clap::ArgAction::SetTrue,
        help = "Allow running tests with the bare driver, which executes autopkgtest as root on the host"
    )]
    pub allow_host_test: bool,

    #[arg(
        long = "shell-on-failure",
        action = clap::ArgAction::SetTrue,
        help = "On test failure, drop into an interactive shell in the test environment when stdout is a TTY. Defaults to the 'shell_on_failure' setting in the config file (false if unset)."
    )]
    pub shell_on_failure: Option<bool>,

    #[command(flatten)]
    pub common: CommonCli,
}

#[derive(Args, Debug)]
pub struct CheckSubcommandArgs {
    #[command(flatten)]
    pub common: CommonCli,
}

#[derive(Args, Debug)]
pub struct SignSubcommandArgs {
    #[arg(
        long = "sign-key",
        help = "GPG key ID/email to sign with. Defaults to the 'sign.key' setting in the config file, or the Changed-By/Maintainer address of the file being signed if unset."
    )]
    pub sign_key: Option<String>,

    #[arg(
        long = "sign-tool",
        value_enum,
        help = "OpenPGP implementation to sign with: 'gpg' (default), 'sequoia' (sq), or 'custom' (uses --sign-command). Defaults to the 'sign.tool' setting in the config file."
    )]
    pub sign_tool: Option<SignTool>,

    #[arg(
        long = "sign-command",
        help = "Custom signing command for --sign-tool custom, run without a shell. Supports {file}, {key} and {email} placeholders; writes the clearsigned result to stdout. Defaults to the 'sign.command' setting in the config file."
    )]
    pub sign_command: Option<String>,

    #[arg(
        long = "sign-notify",
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = clap::value_parser!(bool),
        help = "Send a desktop notification via notify-send just before signing, so a hardware-key touch prompt isn't missed. Defaults to the 'sign.notify' setting in the config file (false if unset)."
    )]
    pub sign_notify: Option<bool>,

    #[arg(
        short,
        long,
        help = "Directory holding the .changes file when no file is given (default '..')."
    )]
    pub output_dir: Option<PathBuf>,

    #[command(flatten)]
    pub common: CommonCli,

    #[arg(
        help = "The .changes, .buildinfo or .dsc file to sign; when omitted, located via debian/changelog and --output"
    )]
    pub file: Option<PathBuf>,
}
