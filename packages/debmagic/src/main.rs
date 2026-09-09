use std::env;
use std::process::ExitCode;

use anyhow::Context;
use clap::{CommandFactory, Parser};

use crate::{
    build::{build_package, build_source_package, get_shell_in_build},
    build_intent::{BuildIntentInput, resolve_build_intent},
    cli::{BuildTarget, Cli, Commands, ConfigCommands},
    config::{Config, ConfigPathStatus, resolve_set_target},
    driver::{
        DriverType, config::DriverOverrides, driver_bare::DriverBareConfigOverrides,
        driver_docker::DriverDockerConfigOverrides, driver_lxd::DriverLxdConfigOverrides,
    },
    package::{distro_resolve_mode_for_driver, load_package_identity, resolve_package_target},
    test::{TestIntentInput, TestOutcome, resolve_test_intent, run_test},
};

pub mod build;
pub mod build_intent;
pub mod cli;
pub mod config;
pub mod driver;
pub mod output;
pub mod package;
pub mod sign;
pub mod test;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error:?}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    output::init_color(cli.color);

    let current_dir = env::current_dir()?;
    match &cli.command {
        Commands::Build(args) => {
            let (build_args, debug_symbols, is_source) = match &args.target {
                BuildTarget::Binary(binary_args) => {
                    (&binary_args.build, binary_args.debug_symbols, false)
                }
                BuildTarget::Source(source_args) => (&source_args.build, None, true),
            };

            let config_driver = Config::load(
                build_args.common.source_dir.as_deref(),
                cli.config.as_deref(),
            )?
            .driver
            .default;

            let driver = if is_source {
                build_args
                    .driver
                    .or(config_driver)
                    .unwrap_or(DriverType::Bare)
            } else {
                build_args.driver.or(config_driver).context(
                    "no build driver selected: pass --driver or set 'driver' in debmagic.toml (docker, bare, lxd or incus)",
                )?
            };

            let intent = resolve_build_intent(BuildIntentInput {
                fallback_dir: current_dir.clone(),
                source_dir: build_args.common.source_dir.clone(),
                output_dir: build_args.output_dir.clone(),
                config_file: cli.config.clone(),
                driver,
                persistent: build_args.persistent,
                incremental: build_args.incremental,
                debug_symbols,
                sign: build_args.sign,
                sign_key: build_args.sign_key.clone(),
                sign_tool: build_args.sign_tool,
                sign_command: build_args.sign_command.clone(),
                sign_notify: build_args.sign_notify,
                clean: build_args.clean,
                source_sync: build_args.source_sync,
                host_arch_variant: build_args.host_arch_variant.clone(),
                shell_on_failure: build_args.shell_on_failure,
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
            let config = Config::load(Some(&source_dir), cli.config.as_deref())?;
            let identity = load_package_identity(&source_dir)?;
            get_shell_in_build(&config, &identity)?;
        }
        Commands::Test(args) => {
            let intent = resolve_test_intent(TestIntentInput {
                fallback_dir: current_dir.clone(),
                source_dir: args.common.source_dir.clone(),
                config_file: cli.config.clone(),
                driver: args.driver,
                persistent: args.persistent,
                strict: args.strict,
                changes: args.changes.clone(),
                allow_host_test: args.allow_host_test,
                shell_on_failure: args.shell_on_failure,
                distro: args.distro.clone(),
                driver_overrides: DriverOverrides {
                    apt_mirror: args.apt_mirror.clone(),
                    proposed: args.proposed,
                    docker: DriverDockerConfigOverrides {
                        base_image: args.docker.base_image.clone(),
                    },
                    bare: DriverBareConfigOverrides {},
                    lxd: DriverLxdConfigOverrides {
                        base_image: args.lxd.base_image.clone(),
                        project: args.lxd.project.clone(),
                    },
                },
            })?;

            let outcome = run_test(&intent).context("running tests failed")?;
            return Ok(match outcome {
                TestOutcome::Passed => ExitCode::SUCCESS,
                TestOutcome::Failed => ExitCode::from(1),
                TestOutcome::StrictFailure => ExitCode::from(2),
            });
        }
        Commands::Check(_args) => {
            println!("Check subcommand! - not implemented");
        }
        Commands::Sign(args) => {
            let source_dir = args.common.source_dir.as_deref().unwrap_or(&current_dir);
            let source_dir =
                std::path::absolute(source_dir).context("resolving source dir failed")?;
            let mut config = Config::load(Some(&source_dir), cli.config.as_deref())?;

            if let Some(key) = &args.sign_key {
                config.sign.key = Some(key.clone());
            }
            if let Some(tool) = args.sign_tool {
                config.sign.tool = tool;
            }
            if let Some(command) = &args.sign_command {
                config.sign.command = Some(command.clone());
            }
            if let Some(notify) = args.sign_notify {
                config.sign.notify = notify;
            }

            let options = sign::SignOptions {
                key: config.sign.key.clone(),
                tool: config.sign.tool,
                command: config.sign.command.clone(),
            };

            let file = match &args.file {
                Some(file) => {
                    std::path::absolute(file).context("resolving the file to sign failed")?
                }
                None => {
                    let identity = load_package_identity(&source_dir)?;
                    // -o wins; else the config value, relative to the package root.
                    let output_dir = match &args.output_dir {
                        Some(dir) => {
                            std::path::absolute(dir).context("resolving output dir failed")?
                        }
                        None => std::path::absolute(source_dir.join(&config.output_dir))
                            .context("resolving output dir failed")?,
                    };
                    sign::find_changes_file(
                        &identity.name,
                        &identity.version.to_string(),
                        &output_dir,
                    )?
                }
            };

            let package = file
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            sign::sign_file(&file, &options, config.sign.notify, &package)?;
        }
        Commands::Config(args) => match &args.command {
            ConfigCommands::Show(show_args) => {
                let source_dir = show_args
                    .common
                    .source_dir
                    .as_deref()
                    .unwrap_or(&current_dir);
                let source_dir =
                    std::path::absolute(source_dir).context("resolving source dir failed")?;
                let paths = Config::resolve_paths(Some(&source_dir), cli.config.as_deref())?;

                eprintln!("debmagic: config files (highest precedence first):");
                for entry in &paths {
                    let status = match entry.status {
                        ConfigPathStatus::Used => "used",
                        ConfigPathStatus::NotFound => "not found",
                    };
                    eprintln!("debmagic:   {status:<10} {}", entry.path.display());
                }

                let config = Config::new(&paths)?;
                print!("{}", toml::to_string_pretty(&config)?);
            }
            ConfigCommands::Get(get_args) => {
                let source_dir = get_args
                    .common
                    .source_dir
                    .as_deref()
                    .unwrap_or(&current_dir);
                let source_dir =
                    std::path::absolute(source_dir).context("resolving source dir failed")?;
                let config = Config::load(Some(&source_dir), cli.config.as_deref())?;
                println!("{}", config.get_value(&get_args.key)?);
            }
            ConfigCommands::Set(set_args) => {
                let source_dir = set_args
                    .common
                    .source_dir
                    .as_deref()
                    .unwrap_or(&current_dir);
                let source_dir =
                    std::path::absolute(source_dir).context("resolving source dir failed")?;
                let target =
                    resolve_set_target(Some(&source_dir), cli.config.as_deref(), set_args.global)?;

                if let Some(parent) = target.path.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("creating {} failed", parent.display()))?;
                }

                target.set_value(&set_args.key, &set_args.value)?;

                // validate the result parses and the key landed
                let config = Config::load(Some(&source_dir), cli.config.as_deref())?;
                let effective = config.get_value(&set_args.key)?;
                println!("debmagic: {} = {}", set_args.key, effective.trim_end());
                eprintln!("debmagic: written to {}", target.path.display());
            }
        },
        Commands::Version {} => {
            let cmd = Cli::command();
            println!("{}", cmd.render_version());
        }
    }

    Ok(ExitCode::SUCCESS)
}
