//! Plan command implementation.
//!
//! Implements the `plan movies` and `plan tv_series` subcommands.
//! Coordinates scanning, parsing, TMDB lookup, and plan generation.

use crate::core::planner::{self, Planner};
use crate::models::config::Config;
use crate::models::media::MediaType;
use crate::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};

/// Execute the plan command for movies.
pub async fn plan_movies(
    source: &Path,
    target: Option<&Path>,
    output: Option<&Path>,
    config: &Config,
) -> Result<()> {
    println!("{}", "[PLAN] Planning movies organization...".bold().cyan());
    println!();

    plan_media(source, target, output, MediaType::Movies, config).await
}

/// Execute the plan command for TV shows.
pub async fn plan_tv_series(
    source: &Path,
    target: Option<&Path>,
    output: Option<&Path>,
    config: &Config,
) -> Result<()> {
    println!(
        "{}",
        "[PLAN] Planning TV shows organization...".bold().cyan()
    );
    println!();

    plan_media(source, target, output, MediaType::TvSeries, config).await
}

/// Common planning logic for both movies and TV shows.
async fn plan_media(
    source: &Path,
    target: Option<&Path>,
    output: Option<&Path>,
    media_type: MediaType,
    config: &Config,
) -> Result<()> {
    // Validate source path
    if !source.exists() {
        return Err(crate::Error::PathNotFound(source.display().to_string()));
    }
    if !source.is_dir() {
        return Err(crate::Error::NotADirectory(source.display().to_string()));
    }

    // Determine target path
    let target_path = match target {
        Some(t) => t.to_path_buf(),
        None => {
            // Default: create _organized directory next to source
            let source_name = source
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("videos");
            let organized_name = format!("{}_organized", source_name);
            source
                .parent()
                .map(|p| p.join(organized_name))
                .unwrap_or_else(|| PathBuf::from(format!("{}_organized", source_name)))
        }
    };

    // Print configuration
    println!("  {} {}", "Source:".bold(), source.display());
    println!("  {} {}", "Target:".bold(), target_path.display());
    println!("  {} {}", "Type:".bold(), media_type);
    println!();

    // Create planner and generate plan
    let planner = Planner::with_application_config(config)?;
    let plan = planner.generate(source, &target_path, media_type).await?;

    // Print summary
    println!();
    println!("{}", "[Plan Summary]".bold().green());
    println!("  {} {}", "Videos to organize:".bold(), plan.items.len());
    println!("  {} {}", "Sample files:".bold(), plan.samples.len());
    println!("  {} {}", "Unknown/failed:".bold(), plan.unknown.len());

    // Calculate total operations
    let total_ops: usize = plan.items.iter().map(|i| i.operations.len()).sum();
    println!("  {} {}", "Total operations:".bold(), total_ops);
    println!();

    // Ensure target directory exists before saving plan
    if !target_path.exists() {
        std::fs::create_dir_all(&target_path)?;
    }

    // Determine output path (prefer target directory)
    let output_path = match output {
        Some(o) => o.to_path_buf(),
        None => planner::default_plan_path(source, Some(&target_path)),
    };

    // Save plan
    planner::save_plan(&plan, &output_path)?;
    println!(
        "{} {}",
        "[OK] Plan saved to:".bold().green(),
        output_path.display()
    );

    // Save to sessions
    match planner::save_to_sessions(&plan) {
        Ok(session_dir) => {
            println!(
                "{} {}",
                "[INFO] Session saved to:".bold(),
                session_dir.display()
            );
        }
        Err(e) => {
            tracing::warn!("Failed to save session: {}", e);
        }
    }

    // Print next steps with complete commands
    println!();
    println!("{}", "[Next Steps]".bold().cyan());
    println!("  1. Review the plan:");
    println!("     {}", format!("cat {}", output_path.display()).bold());
    println!("  2. Execute the plan:");
    println!(
        "     {}",
        format!("mediaorganizer execute {}", output_path.display()).bold()
    );

    // Warn about unknown files - group by error reason
    if !plan.unknown.is_empty() {
        println!();
        println!("{}", "[WARNING] Unknown Files:".bold().yellow());

        // Group files by error reason
        let mut grouped: std::collections::HashMap<String, Vec<&crate::models::plan::UnknownItem>> =
            std::collections::HashMap::new();
        for item in &plan.unknown {
            grouped.entry(item.reason.clone()).or_default().push(item);
        }

        // Sort groups by number of files (descending)
        let mut sorted_groups: Vec<_> = grouped.into_iter().collect();
        sorted_groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        for (reason, files) in sorted_groups {
            println!();
            println!("  {} ({} files):", reason.yellow(), files.len());
            for item in files {
                println!("    {}", item.source.path.display().to_string().red());
            }
        }
    }

    Ok(())
}
