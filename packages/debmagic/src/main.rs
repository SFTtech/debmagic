use std::process::ExitCode;

use anyhow::Context;
use clap::{CommandFactory, Parser};

use crate::cli::{
    BuildTarget, ChangelogCommands, Cli, Commands, ConfigCommands, EnvCommands, UpstreamCommands,
};

pub mod build;
pub mod build_intent;
pub mod changelog;
pub mod changes;
pub mod cli;
pub mod config;
pub mod control;
pub mod data_dir;
pub mod driver;
pub mod env;
pub mod environment;
pub mod output;
pub mod package;
pub mod requests;
pub mod sign;
pub mod subprocess;
pub mod test;
pub mod time;
pub mod upload;
pub mod upstream;

fn main() -> ExitCode {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build the tokio runtime");
    match runtime.block_on(run()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error:?}");
            ExitCode::FAILURE
        }
    }
}

fn upstream_fetch_command(
    current_dir: &std::path::Path,
    cli: &Cli,
    version: Option<String>,
    args: &cli::UpstreamFetchArgs,
) -> anyhow::Result<upstream::UpstreamFetchCommand> {
    Ok(upstream::UpstreamFetchCommand {
        source_dir: package::resolve_source_dir(current_dir, args.common.source_dir.as_deref())?,
        config_file: cli.config.clone(),
        version,
        from: args.from,
        repack: args.repack,
        signature_check: args.signature_check,
        verify_command: args.verify_command.clone(),
        force: args.force,
        dry_run: args.dry_run,
        output_dir: args.output_dir.clone(),
    })
}

async fn run() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    output::init_color(cli.color);

    let current_dir = std::env::current_dir()?;
    match &cli.command {
        Commands::Build(args) => {
            let (build_args, debug_symbols, kind, include_orig, run_test) = match &args.target {
                BuildTarget::Binary(binary_args) => (
                    &binary_args.build,
                    binary_args.debug_symbols,
                    build_intent::BuildKind::Binary,
                    None,
                    binary_args.test,
                ),
                BuildTarget::Source(source_args) => (
                    &source_args.build,
                    None,
                    build_intent::BuildKind::Source,
                    Some(source_args.include_orig),
                    Some(false),
                ),
            };
            build::build(build::BuildCommand {
                intent: build_intent::BuildIntentInput {
                    source_dir: package::resolve_source_dir(
                        &current_dir,
                        build_args.common.source_dir.as_deref(),
                    )?,
                    output_dir: build_args.output_dir.clone(),
                    config_file: cli.config.clone(),
                    driver: build_args.driver,
                    kind,
                    persistent: build_args.persistent,
                    incremental: build_args.incremental,
                    debug_symbols,
                    test: run_test,
                    sign: build_args.sign,
                    sign_key: build_args.sign_key.clone(),
                    sign_tool: build_args.sign_tool,
                    sign_command: build_args.sign_command.clone(),
                    sign_notify: build_args.sign_notify,
                    notify: build_args.notify,
                    clean: build_args.clean,
                    source_sync: build_args.source_sync,
                    host_arch_variant: build_args.host_arch_variant.clone(),
                    driver_overrides: build_args.driver_overrides(),
                },
                distro: build_args.distro.clone(),
                include_orig,
                bare_ignore_release: build_args.bare_ignore_release,
                changes_options: build_args.changes_options.clone(),
                upload: build_args.upload.clone(),
            })
            .await?;
        }
        Commands::Environment(args) => match &args.command {
            EnvCommands::List => {
                let config = config::Config::load(None, cli.config.as_deref())?;
                env::list_environments(&config).await?;
            }
            EnvCommands::Clean => {
                let config = config::Config::load(None, cli.config.as_deref())?;
                env::clean_environments(&config).await?;
            }
            EnvCommands::Rm { ids, force } => {
                let config = config::Config::load(None, cli.config.as_deref())?;
                env::remove_environments(&config, ids, *force).await?;
            }
            EnvCommands::Shell { id, common } => {
                if let Some(id) = id {
                    let config = config::Config::load(None, cli.config.as_deref())?;
                    env::shell_environment(&config, &current_dir, Some(id)).await?;
                } else {
                    let source_dir = common.source_dir.as_deref().unwrap_or(&current_dir);
                    let source_dir = match env::resolve_shell_source_dir(
                        Some(source_dir),
                        &current_dir,
                    ) {
                        Ok(dir) => dir,
                        Err(_) => {
                            anyhow::bail!(
                                "not a Source tree; pass an Environment id (see `debmagic env list`)"
                            );
                        }
                    };
                    let config = config::Config::load(Some(&source_dir), cli.config.as_deref())?;
                    env::shell_environment(&config, &source_dir, None).await?;
                }
            }
        },
        Commands::Test(args) => {
            let intent = test::resolve_test_intent(test::TestIntentInput {
                source_dir: package::resolve_source_dir(
                    &current_dir,
                    args.common.source_dir.as_deref(),
                )?,
                config_file: cli.config.clone(),
                driver: args.driver,
                persistent: args.persistent,
                strict: args.strict,
                changes: args.changes.clone(),
                allow_host_test: args.allow_host_test,
                distro: args.distro.clone(),
                driver_overrides: args.driver_overrides(),
            })?;

            let outcome = test::run_test(&intent)
                .await
                .context("running tests failed")?;
            return Ok(match outcome {
                test::TestOutcome::Passed => ExitCode::SUCCESS,
                test::TestOutcome::Failed => ExitCode::from(1),
                test::TestOutcome::StrictFailure => ExitCode::from(2),
            });
        }
        Commands::Check(_args) => {
            println!("Check subcommand! - not implemented");
        }
        Commands::Upload(args) => {
            upload::upload(upload::UploadIntent {
                source_dir: package::resolve_source_dir(
                    &current_dir,
                    args.common.source_dir.as_deref(),
                )?,
                config_file: cli.config.clone(),
                target: args.target.clone(),
                method: args.method,
                server: args.server.clone(),
                incoming: args.incoming.clone(),
                login: args.login.clone(),
                port: args.port,
                hooks: args.hooks,
                force: args.force,
                sign: args.sign,
                include_orig: args.include_orig,
                changes: args.changes.clone(),
            })?;
        }
        Commands::Sign(args) => {
            sign::sign(sign::SignIntent {
                source_dir: package::resolve_source_dir(
                    &current_dir,
                    args.common.source_dir.as_deref(),
                )?,
                config_file: cli.config.clone(),
                mode: args.mode,
                sign_key: args.sign_key.clone(),
                sign_tool: args.sign_tool,
                sign_command: args.sign_command.clone(),
                sign_notify: args.sign_notify,
                output_dir: args.output_dir.clone(),
                file: args.file.clone(),
            })?;
        }
        Commands::Config(args) => {
            let (command, common) = match &args.command {
                ConfigCommands::Show(show_args) => {
                    (config::ConfigCommandKind::Show, &show_args.common)
                }
                ConfigCommands::Get(get_args) => (
                    config::ConfigCommandKind::Get {
                        key: get_args.key.clone(),
                    },
                    &get_args.common,
                ),
                ConfigCommands::Set(set_args) => (
                    config::ConfigCommandKind::Set {
                        key: set_args.key.clone(),
                        value: set_args.value.clone(),
                        global: set_args.global,
                    },
                    &set_args.common,
                ),
            };
            config::run_config(config::ConfigCommand {
                source_dir: package::resolve_source_dir(
                    &current_dir,
                    common.source_dir.as_deref(),
                )?,
                config_file: cli.config.clone(),
                command,
            })?;
        }
        Commands::Upstream(args) => match &args.command {
            UpstreamCommands::List(list_args) => {
                upstream::list(upstream::UpstreamListCommand {
                    source_dir: package::resolve_source_dir(
                        &current_dir,
                        list_args.common.source_dir.as_deref(),
                    )?,
                    config_file: cli.config.clone(),
                    all: list_args.all,
                    previous: list_args.previous,
                })
                .await?;
            }
            UpstreamCommands::Fetch(fetch_args) => {
                upstream::fetch(upstream_fetch_command(
                    &current_dir,
                    &cli,
                    fetch_args.version.clone(),
                    &fetch_args.fetch,
                )?)
                .await?;
            }
            UpstreamCommands::Switch(switch_args) => {
                upstream::switch(upstream_fetch_command(
                    &current_dir,
                    &cli,
                    Some(switch_args.version.clone()),
                    &switch_args.fetch,
                )?)
                .await?;
            }
        },
        Commands::Changelog(args) => match &args.command {
            ChangelogCommands::New(new_args) => {
                changelog::new_entry(changelog::ChangelogNewCommand {
                    source_dir: package::resolve_source_dir(
                        &current_dir,
                        new_args.common.source_dir.as_deref(),
                    )?,
                })?;
            }
        },
        Commands::Version {} => {
            let cmd = Cli::command();
            println!("{}", cmd.render_version());
        }
    }

    Ok(ExitCode::SUCCESS)
}
