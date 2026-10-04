use std::path::PathBuf;

use crate::build::source::SourceSyncMode;
use crate::driver::DriverType;
use crate::driver::config::DriverOverrides;
use crate::driver::driver_bare::DriverBareConfigOverrides;
use crate::driver::driver_docker::DriverDockerConfigOverrides;
use crate::driver::driver_lxd::DriverLxdConfigOverrides;
use crate::sign::{SignMode, SignTool};
use crate::time::RefreshPolicy;
use crate::upload::UploadMethod;
use crate::upload::orig::IncludeOrig;
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
    #[command(
        name = "environment",
        alias = "env",
        about = "List, clean, or open a shell in a Driver Environment"
    )]
    Environment(EnvSubcommandArgs),
    #[command(about = "Run the package's declared Debian autopkgtest tests against a prior build")]
    Test(TestSubcommandArgs),
    #[command(about = "Check the project")]
    Check(CheckSubcommandArgs),
    #[command(about = "GPG-sign a .changes file (and its .dsc/.buildinfo) on the host")]
    Sign(SignSubcommandArgs),
    #[command(
        about = "Upload a .changes file (and everything it references) to an upload target, dput-style"
    )]
    Upload(UploadSubcommandArgs),
    #[command(about = "Inspect the debmagic configuration")]
    Config(ConfigSubcommandArgs),
    #[command(about = "Query and switch upstream versions")]
    Upstream(UpstreamSubcommandArgs),
    #[command(about = "Show version information")]
    Version {},
}

#[derive(Args, Debug)]
pub struct UpstreamSubcommandArgs {
    #[command(subcommand)]
    pub command: UpstreamCommands,
}

#[derive(Subcommand, Debug)]
pub enum UpstreamCommands {
    #[command(
        about = "List available upstream versions from debian/watch, newest eligible by default"
    )]
    List(UpstreamListArgs),
    #[command(
        about = "Switch the package tree to an upstream version: fetch, repack, replace the tree (keeping debian/)"
    )]
    Switch(UpstreamSwitchArgs),
}

#[derive(Args, Debug)]
pub struct UpstreamListArgs {
    #[arg(
        long,
        help = "Show all candidate versions, not just the newest newer than the changelog's"
    )]
    pub all: bool,

    #[arg(
        long,
        help = "Show the N versions newer than the changelog's, not just the newest"
    )]
    pub previous: Option<usize>,

    #[command(flatten)]
    pub common: CommonCli,
}

#[derive(Args, Debug)]
pub struct UpstreamSwitchArgs {
    #[arg(help = "The upstream version to switch to, or 'latest' for the newest eligible")]
    pub version: String,

    #[arg(
        long,
        help = "Only report what would happen, without touching anything"
    )]
    pub dry_run: bool,

    #[arg(
        long,
        help = "Skip verifying the upstream tarball signature against debian/upstream/signing-key.asc"
    )]
    pub no_signature_check: bool,

    #[arg(
        long = "verify-command",
        help = "Custom verification command for sign.tool = 'custom', run without a shell. Supports {file}, {signature} and {keyring} placeholders; without {signature} the signature path is appended. Defaults to the 'sign.verify_command' setting, falling back to 'sign.sign_command'."
    )]
    pub verify_command: Option<String>,

    #[command(flatten)]
    pub common: CommonCli,
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
        help = "Synchronize changed source inputs to preserve build outputs. Forces --persistent=always"
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
        default_missing_value = "always",
        action = clap::ArgAction::Set,
        help = "How long the build environment outlives this build: 'on-failure' (the default) keeps it when the build command fails, 'always' keeps it after every build, 'no' tears it down. A bare --persistent means 'always'. A failed build that keeps the environment prints `debmagic env shell <id>`."
    )]
    pub persistent: Option<crate::driver::Persistence>,

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
        long = "apt-update-age",
        help = "When a persistent build environment runs 'apt-get update' again: 'now' (every build), 'never' (only on first creation), or a maximum age of the apt index like '1d' (the default), '12h', '30m'. Fresh environments always update once. Defaults to the 'apt_update_age' setting in the config file. Ignored by the bare driver."
    )]
    pub apt_update_age: Option<RefreshPolicy>,

    #[arg(
        long,
        help = "Target distribution to build for, overriding the changelog's (e.g. 'trixie', 'noble', or a suite declared in base_images). If not provided, use the single distro from changelog."
    )]
    pub distro: Option<String>,

    #[arg(
        long = "bare-ignore-release",
        help = "With the bare driver, build even though the target distro differs from the host's os-release. The host must still provide the build dependencies itself."
    )]
    pub bare_ignore_release: bool,

    #[arg(
        long = "host-arch-variant",
        help = "Build for a dpkg architecture variant (e.g. 'amd64v3' on Ubuntu), like dpkg-buildpackage's --host-arch-variant. Sets DEB_HOST_ARCH_VARIANT for the build, which makes the Ubuntu vendor hook append the variant's -march= flags and names the .changes file after the variant. Defaults to the 'host_arch_variant' setting in the config file."
    )]
    pub host_arch_variant: Option<String>,

    #[arg(
        long,
        value_enum,
        num_args = 0..=1,
        default_missing_value = "auto",
        help = "Sign the resulting .changes/.dsc after building: 'auto' (the default when passed without a value) signs what is unsigned and skips what our key already signed, 'force' always re-signs, 'no' never signs. Defaults to the 'sign.source' setting in the config file (no if unset)."
    )]
    pub sign: Option<SignMode>,

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
        help = "Custom signing command for --sign-tool custom, run without a shell. Supports {file}, {key} and {email} placeholders; writes the clearsigned result to stdout. Defaults to the 'sign.sign_command' setting in the config file."
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

    #[command(flatten)]
    pub common: CommonCli,

    #[arg(short, long, help = "Output directory for the package artifacts")]
    pub output_dir: Option<PathBuf>,

    #[arg(
        long = "changes-option",
        help = "Extra field for the .changes file, passed to dpkg-buildpackage as-is, e.g. --changes-option=-DVcs-Git=https://... (repeatable)"
    )]
    pub changes_options: Vec<String>,

    #[arg(
        long,
        help = "After building (and signing, if enabled), upload the resulting .changes to this upload target ('name' or 'name:parameter', e.g. 'ppa:user/repo')"
    )]
    pub upload: Option<String>,
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

    #[arg(
        long = "test",
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = clap::value_parser!(bool),
        help = "Run the package's test suite during the build. Defaults to the 'run_test' setting in the config file (true if unset); --test=false exports DEB_BUILD_OPTIONS=nocheck so dpkg-buildpackage skips tests."
    )]
    pub test: Option<bool>,
}

#[derive(Args, Debug)]
pub struct SourceTargetArgs {
    #[command(flatten)]
    pub build: CommonBuildArgs,

    #[arg(
        long = "include-orig",
        value_enum,
        default_value_t = IncludeOrig::Auto,
        help = "Include the orig tarball in the source upload: 'auto' (default) includes it only when the archive cannot have it yet (a new upstream version or a deltarebase onto Debian), 'yes' always, 'no' never"
    )]
    pub include_orig: IncludeOrig,
}

#[derive(Args, Debug)]
pub struct EnvSubcommandArgs {
    #[command(subcommand)]
    pub command: EnvCommands,
}

#[derive(Subcommand, Debug)]
pub enum EnvCommands {
    #[command(about = "List Environments in the machine-local registry")]
    List,
    #[command(about = "Destroy Stale Environments, or a specific Environment when given its id")]
    Clean {
        #[arg(help = "Environment id to destroy, even if it is healthy")]
        id: Option<String>,
        #[arg(
            long,
            help = "Destroy even when the Environment is Unreachable, without Driver cooperation"
        )]
        force: bool,
    },
    #[command(about = "Open an interactive shell in an Environment")]
    Shell {
        #[arg(
            help = "Environment id; when omitted, unique Environment for the current Source tree"
        )]
        id: Option<String>,
        #[command(flatten)]
        common: CommonCli,
    },
}

#[derive(Args, Debug)]
pub struct TestSubcommandArgs {
    #[arg(
        short,
        long,
        help = "Driver type for the test environment. Defaults to the driver recorded on the prior binary-build Invocation."
    )]
    pub driver: Option<DriverType>,

    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "always",
        action = clap::ArgAction::Set,
        help = "How long the test environment outlives this run: 'on-failure' (the default) keeps it when the tests fail, 'always' keeps it after every run, 'no' tears it down. A bare --persistent means 'always'. A failed run that keeps the environment prints `debmagic env shell <id>`."
    )]
    pub persistent: Option<crate::driver::Persistence>,

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
        num_args = 0..=1,
        default_missing_value = "true",
        action = clap::ArgAction::Set,
        help = "Also enable the '<release>-proposed' pocket in the test environment. Ignored by the bare driver."
    )]
    pub proposed: Option<bool>,

    #[arg(
        long = "apt-update-age",
        help = "When a persistent test environment runs 'apt-get update' again: 'now' (every run), 'never' (only on first creation), or a maximum age of the apt index like '1d' (the default), '12h', '30m'. Fresh environments always update once. Defaults to the 'apt_update_age' setting in the config file. Ignored by the bare driver."
    )]
    pub apt_update_age: Option<RefreshPolicy>,

    #[arg(
        long,
        help = "Override the target distribution for the test environment. Defaults to the distro of the prior binary-build Invocation, not the changelog."
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
        long,
        value_enum,
        default_value = "auto",
        help = "'auto' skips files our key already signed, 'force' always re-signs."
    )]
    pub mode: SignMode,

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
        help = "Custom signing command for --sign-tool custom, run without a shell. Supports {file}, {key} and {email} placeholders; writes the clearsigned result to stdout. Defaults to the 'sign.sign_command' setting in the config file."
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

#[derive(Args, Debug)]
pub struct UploadSubcommandArgs {
    #[arg(
        help = "Upload target: 'name' or 'name:parameter' (e.g. 'ppa:user/repo'), resolved from [upload.targets] in the config, merging over the builtins (ppa, ubuntu, debian)"
    )]
    pub target: String,

    #[arg(
        long,
        value_enum,
        help = "Upload method: 'scp' or 'sftp'. Overrides the target's 'method'"
    )]
    pub method: Option<UploadMethod>,

    #[arg(long, help = "Server to upload to. Overrides the target's 'server'")]
    pub server: Option<String>,

    #[arg(
        long,
        help = "Remote directory to upload into. Overrides the target's 'incoming'"
    )]
    pub incoming: Option<String>,

    #[arg(
        long,
        help = "Login on the remote server. Overrides the target's 'login'"
    )]
    pub login: Option<String>,

    #[arg(long, help = "Remote port. Overrides the target's 'port'")]
    pub port: Option<u16>,

    #[arg(
        long,
        action = clap::ArgAction::SetTrue,
        help = "Skip the target's pre_upload_commands"
    )]
    pub no_hooks: bool,

    #[arg(
        long,
        action = clap::ArgAction::SetTrue,
        help = "Upload even if a successful upload to this target is already recorded"
    )]
    pub force: bool,

    #[arg(
        long,
        value_enum,
        num_args = 0..=1,
        default_missing_value = "auto",
        help = "Sign the .changes right before uploading: 'auto' (the default when passed without a value) signs what is unsigned and skips what our key already signed, 'force' always re-signs. Defaults to the 'sign.source' setting in the config file (no if unset)."
    )]
    pub sign: Option<SignMode>,

    #[arg(
        long = "include-orig",
        value_enum,
        help = "Include the orig tarball in the upload: 'auto' includes it only when the archive cannot have it yet (a new upstream version or a deltarebase onto Debian), 'yes' always, 'no' never. Rewrites and re-signs the .changes when it disagrees"
    )]
    pub include_orig: Option<IncludeOrig>,

    #[command(flatten)]
    pub common: CommonCli,

    #[arg(
        help = "The .changes file to upload; when omitted, located via debian/changelog and the output dir"
    )]
    pub changes: Option<PathBuf>,
}

impl CommonBuildArgs {
    /// Collect the driver-specific overrides from the CLI flags.
    pub fn driver_overrides(&self) -> DriverOverrides {
        DriverOverrides {
            apt_mirror: self.apt_mirror.clone(),
            proposed: self.proposed,
            apt_update_age: self.apt_update_age,
            docker: DriverDockerConfigOverrides {
                base_image: self.docker.base_image.clone(),
            },
            bare: DriverBareConfigOverrides {},
            lxd: DriverLxdConfigOverrides {
                base_image: self.lxd.base_image.clone(),
                project: self.lxd.project.clone(),
            },
        }
    }
}

impl TestSubcommandArgs {
    /// Collect the driver-specific overrides from the CLI flags.
    pub fn driver_overrides(&self) -> DriverOverrides {
        DriverOverrides {
            apt_mirror: self.apt_mirror.clone(),
            proposed: self.proposed,
            apt_update_age: self.apt_update_age,
            docker: DriverDockerConfigOverrides {
                base_image: self.docker.base_image.clone(),
            },
            bare: DriverBareConfigOverrides {},
            lxd: DriverLxdConfigOverrides {
                base_image: self.lxd.base_image.clone(),
                project: self.lxd.project.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::Persistence;
    use test_case::test_case;

    fn parse_test(args: &[&str]) -> TestSubcommandArgs {
        let cli = Cli::try_parse_from([&["debmagic", "test"], args].concat()).unwrap();
        match cli.command {
            Commands::Test(args) => args,
            other => panic!("unexpected command {other:?}"),
        }
    }

    #[test_case(&[], None; "absent")]
    #[test_case(&["--persistent"], Some(Persistence::Always); "bare flag")]
    #[test_case(&["--persistent=always"], Some(Persistence::Always); "always")]
    #[test_case(&["--persistent=on-failure"], Some(Persistence::OnFailure); "on failure")]
    #[test_case(&["--persistent=no"], Some(Persistence::No); "no")]
    fn persistent_flag(args: &[&str], expected: Option<Persistence>) {
        let args = parse_test(args);
        assert_eq!(args.persistent, expected);
    }

    #[test]
    fn persistent_rejects_a_boolean() {
        let error = Cli::try_parse_from(["debmagic", "test", "--persistent=true"]).unwrap_err();
        assert!(error.to_string().contains("always"), "{error}");
    }
}
