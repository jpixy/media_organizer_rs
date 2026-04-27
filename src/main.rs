//! Media Organizer CLI
//!
//! A command-line tool for organizing video files (movies and TV shows) using AI and TMDB.

use clap::Parser;
use media_organizer::cli::{
    args::{Cli, Commands, PlanType, SessionsAction},
    commands::{execute, export_import, index, plan, rollback, search, sessions, verify},
};
use media_organizer::models::config::load_config;
use media_organizer::preflight;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose);

    // Load configuration (config.toml + environment variables)
    let config = load_config();
    tracing::debug!("Configuration loaded: {:?}", config);

    // Run the appropriate command
    match cli.command {
        Commands::Plan { media_type } => {
            // Run preflight checks unless skipped
            if !cli.skip_preflight {
                run_preflight_checks(&config).await?;
            }

            match media_type {
                PlanType::Movies {
                    source,
                    target,
                    output,
                } => {
                    plan::plan_movies(&source, target.as_deref(), output.as_deref(), &config).await?;
                }
                PlanType::Tvshows {
                    source,
                    target,
                    output,
                } => {
                    plan::plan_tvshows(&source, target.as_deref(), output.as_deref(), &config).await?;
                }
            }
        }

        Commands::Execute { plan_file, output } => {
            execute::execute_plan(&plan_file, output.as_deref()).await?;
        }

        Commands::Rollback {
            rollback_file,
            dry_run,
        } => {
            rollback::rollback(&rollback_file, dry_run).await?;
        }

        Commands::Sessions { action } => match action {
            SessionsAction::List => {
                sessions::list_sessions().await?;
            }
            SessionsAction::Show { session_id } => {
                sessions::show_session(&session_id).await?;
            }
        },

        Commands::Verify { path } => {
            verify::verify(&path).await?;
        }

        Commands::Index { action } => {
            index::execute_index(action).await?;
        }

        Commands::Search {
            title,
            actor,
            director,
            collection,
            year,
            genre,
            country,
            show_status,
            format,
        } => {
            search::execute_search(
                title,
                actor,
                director,
                collection,
                year,
                genre,
                country,
                show_status,
                format,
            )
            .await?;
        }

        Commands::Export {
            output,
            include_secrets,
            only,
            exclude,
            disk,
            description,
            auto_name,
        } => {
            export_import::execute_export(
                output,
                include_secrets,
                only,
                exclude,
                disk,
                description,
                auto_name,
            )
            .await?;
        }

        Commands::Import {
            backup_file,
            dry_run,
            only,
            merge,
            force,
            backup_first,
        } => {
            export_import::execute_import(backup_file, dry_run, only, merge, force, backup_first)
                .await?;
        }
    }

    Ok(())
}

/// Initialize the logging system.
fn init_logging(verbose: bool) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = if verbose {
        EnvFilter::new("media_organizer=debug")
    } else {
        EnvFilter::new("media_organizer=info")
    };

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).without_time())
        .with(filter)
        .init();
}

/// Run preflight checks and exit if any fail.
async fn run_preflight_checks(config: &media_organizer::models::config::Config) -> anyhow::Result<()> {
    use colored::Colorize;

    println!("{}", "Running preflight checks...".bold());
    println!();

    let results = preflight::run_preflight_checks(config).await?;
    preflight::print_results(&results);

    println!();

    if !preflight::all_required_passed(&results) {
        anyhow::bail!("Preflight checks failed. Fix the issues above and try again.");
    }

    Ok(())
}
