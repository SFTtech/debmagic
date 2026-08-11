use std::env;

use anyhow::Context;
use clap::{CommandFactory, Parser};

use crate::{
    build::{
        build_package, build_source_package, common::BuildDriverType, config::DriverOverrides,
        driver_bare::DriverBareConfigOverrides, driver_docker::DriverDockerConfigOverrides,
        driver_lxd::DriverLxdConfigOverrides, get_shell_in_build,
    },
    build_intent::{BuildIntentInput, load_config, resolve_build_intent},
    cli::{BuildTarget, Cli, Commands},
    package::{distro_resolve_mode_for_driver, load_package_identity, resolve_package_target},
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
            let (build_args, debug_symbols, incremental, is_source) = match &args.target {
                BuildTarget::Binary(binary_args) => (
                    &binary_args.build,
                    binary_args.debug_symbols,
                    binary_args.incremental,
                    false,
                ),
                BuildTarget::Source(source_args) => (&source_args.build, None, None, true),
            };

            let driver = if is_source {
                build_args.driver.unwrap_or(BuildDriverType::Bare)
            } else {
                build_args.driver.context(
                    "--driver is required for binary builds (docker, bare, lxd or incus)",
                )?
            };

            let intent = resolve_build_intent(BuildIntentInput {
                fallback_dir: current_dir.clone(),
                source_dir: build_args.common.source_dir.clone(),
                output_dir: build_args.output_dir.clone(),
                config_file: cli.config.clone(),
                driver,
                persistent: build_args.persistent,
                incremental,
                disable_incremental: is_source,
                debug_symbols,
                sign: build_args.sign,
                no_sign: build_args.no_sign,
                sign_with: build_args.sign_with,
                sign_key: build_args.sign_key.clone(),
                clean: build_args.clean,
                no_clean: build_args.no_clean,
                source_sync: build_args.source_sync,
                driver_overrides: DriverOverrides {
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
                },
            })?;

            let target = resolve_package_target(
                &intent.source_dir,
                build_args.distro.as_deref(),
                distro_resolve_mode_for_driver(
                    intent.driver,
                    &intent.config.driver.docker.base_images,
                    &intent.config.driver.lxd.base_images,
                ),
            )
            .context("failed to determine package target")?;

            if is_source {
                build_source_package(&intent, &target)
                    .context("Building the source package failed")?;
            } else {
                build_package(&intent, &target).context("Building the package failed")?;
            }
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
