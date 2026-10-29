use std::process::ExitCode;

use anyhow::Context;
use clap::{CommandFactory, Parser};

use crate::cli::{
    BuildTarget, ChangelogCommands, Cli, Commands, ConfigCommands, EnvCommands, UpstreamCommands,
};

pub mod apt;
pub mod build;
pub mod build_intent;
pub mod changelog;
pub mod changes;
pub mod cli;
pub mod config;
pub mod control;
pub mod data_dir;
pub mod distro_info;
pub mod driver;
pub mod env;
pub mod environment;
pub mod output;
pub mod output_dir;
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
                    persistent: build_args.environment.persistent,
                    incremental: build_args.incremental,
                    debug_symbols,
                    test: run_test,
                    sign: build_args.sign,
                    sign_overrides: build_args.signing.overrides(),
                    notify: build_args.notify.notify,
                    clean: build_args.clean,
                    source_sync: build_args.source_sync,
                    host_arch_variant: build_args.host_arch_variant.clone(),
                    apt_overrides: build_args.environment.apt_overrides(),
                    driver_overrides: build_args.environment.driver_overrides(),
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
            EnvCommands::Shell { id, user, common } => {
                let code = env::shell_environment(
                    &current_dir,
                    cli.config.as_deref(),
                    id.as_deref(),
                    common.source_dir.as_deref(),
                    user.as_deref(),
                )
                .await?;
                return Ok(ExitCode::from(u8::try_from(code).unwrap_or(u8::MAX)));
            }
            EnvCommands::Exec {
                id,
                user,
                command,
                common,
            } => {
                let code = env::exec_environment(
                    &current_dir,
                    cli.config.as_deref(),
                    id.as_deref(),
                    common.source_dir.as_deref(),
                    user.as_deref(),
                    command,
                )
                .await?;
                return Ok(ExitCode::from(u8::try_from(code).unwrap_or(u8::MAX)));
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
                persistent: args.environment.persistent,
                notify: args.notify.notify,
                strict: args.strict,
                changes: args.changes.clone(),
                allow_host_test: args.allow_host_test,
                distro: args.distro.clone(),
                apt_overrides: args.environment.apt_overrides(),
                driver_overrides: args.environment.driver_overrides(),
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
                notify: args.notify.notify,
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
                overrides: args.signing.overrides(),
                notify: args.notify.notify,
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
