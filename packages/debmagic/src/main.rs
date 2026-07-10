use std::{
    env,
    path::{self, PathBuf},
};

use anyhow::Context;
use clap::{CommandFactory, Parser};

use crate::{
    build::{
        BuildRequest, build_package, build_source_package, common::BuildDriverType,
        config::DriverOverrides, driver_bare::DriverBareConfigOverrides,
        driver_docker::DriverDockerConfigOverrides, driver_lxd::DriverLxdConfigOverrides,
        get_shell_in_build,
    },
    cli::{BuildTarget, Cli, Commands, CommonBuildArgs},
    config::Config,
    package::PackageDescription,
};

pub mod build;
pub mod cli;
pub mod config;
pub mod package;

/// Precedence of config files is:
///
/// 1. explicit config file passed on the command line
/// 2. `<source_dir>/debian/debmagic.toml`
/// 3. `<XDG_CONFIG_HOME>/debmagic/config.toml`
///
fn get_config(cli: &Cli, source_dir: &Option<PathBuf>) -> anyhow::Result<Config> {
    let mut config_file_paths = vec![];
    let xdg_config_file = dirs::config_dir().map(|p| p.join("debmagic").join("config.toml"));
    if let Some(xdg_config_file) = xdg_config_file
        && xdg_config_file.is_file()
    {
        config_file_paths.push(xdg_config_file);
    }

    if let Some(source_dir) = &source_dir {
        config_file_paths.push(source_dir.join("debian").join("debmagic.toml"));
    }

    if let Some(config_file_override) = &cli.config {
        config_file_paths.push(config_file_override.clone());
    }

    let config = Config::new(&config_file_paths)?;
    Ok(config)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let current_dir = env::current_dir()?;
    match &cli.command {
        Commands::Build(args) => {
            let (build_args, debug_symbols, incremental, is_source): (
                &CommonBuildArgs,
                Option<bool>,
                Option<bool>,
                bool,
            ) = match &args.target {
                BuildTarget::Binary(binary_args) => (
                    &binary_args.build,
                    binary_args.debug_symbols,
                    binary_args.incremental,
                    false,
                ),
                BuildTarget::Source(source_args) => (&source_args.build, None, None, true),
            };

            let source_dir = build_args
                .common
                .source_dir
                .as_deref()
                .unwrap_or(&current_dir);
            let mut config = get_config(&cli, &Some(source_dir.to_path_buf()))?;

            // TODO: figure out a better way to override config from CLI args - maybe more generic, if that is even possible since
            // we want a nice cli which somewhat matches the config structure
            // but some config options only make sense in some cli subcommands -> these flags don't make sense in all commands
            // and should only be used in some
            if let Some(persistent) = build_args.persistent {
                config.driver.persistent = persistent;
            }

            if is_source {
                config.incremental = false;
            } else if let Some(incremental) = incremental {
                config.incremental = incremental;
            }

            if let Some(debug_symbols) = debug_symbols {
                config.build_debug_symbols = debug_symbols;
            }
            if let Some(sign) = build_args.sign {
                config.sign_package = sign;
            }
            if let Some(sign_key) = build_args.sign_key.clone() {
                config.sign_key = Some(sign_key);
            }
            if let Some(clean) = build_args.clean {
                config.clean = clean;
            }
            if let Some(no_clean) = build_args.no_clean {
                config.clean = !no_clean;
            }
            if config.incremental {
                if config.clean {
                    anyhow::bail!("incremental builds are incompatible with clean builds");
                }
                config.driver.persistent = true;
            }
            let driver_overrides = DriverOverrides {
                apt_mirror: build_args.apt_mirror.clone(),
                proposed: build_args.proposed,
                docker: DriverDockerConfigOverrides {
                    base_image: build_args.docker.base_image.clone(),
                },
                bare: DriverBareConfigOverrides {},
                lxd: DriverLxdConfigOverrides {
                    base_image: build_args.lxd.base_image.clone(),
                    project: build_args.lxd.project.clone(),
                },
            };

            let package = PackageDescription::from_dir(
                &path::absolute(source_dir).context("resolving source dir failed")?,
            )?;
            let output_dir = build_args.output_dir.as_deref().unwrap_or(&current_dir);
            let output_dir = path::absolute(output_dir).context("resolving output dir failed")?;

            let request = BuildRequest {
                config: &config,
                package: &package,
                driver_type: if is_source {
                    build_args.driver.unwrap_or(BuildDriverType::Bare)
                } else {
                    build_args.driver.context(
                        "--driver is required for binary builds (docker, bare, lxd or incus)",
                    )?
                },
                driver_overrides: &driver_overrides,
                output_dir: &output_dir,
                explicit_distro_version: build_args.distro.as_deref(),
            };
            if is_source {
                build_source_package(&request).context("Building the source package failed")?;
            } else {
                build_package(&request).context("Building the package failed")?;
            }
        }
        Commands::Shell(args) => {
            let source_dir = args.common.source_dir.as_deref().unwrap_or(&current_dir);
            let config = get_config(&cli, &Some(source_dir.to_path_buf()))?;
            let package = PackageDescription::from_dir(
                &path::absolute(source_dir).context("resolving source dir failed")?,
            )?;
            get_shell_in_build(&config, &package)?;
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
