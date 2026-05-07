//! Export and import command implementations.

use crate::core::exporter::{self, ExportOptions, ExportType, ImportOptions};
use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

/// Execute export command.
pub async fn execute_export(
    output: Option<PathBuf>,
    include_secrets: bool,
    only: Option<String>,
    exclude: Option<Vec<String>>,
    disk: Option<String>,
    description: Option<String>,
    auto_name: bool,
) -> Result<()> {
    // Determine output path
    let output_path = match (auto_name, output) {
        (true, _) | (_, None) => {
            let filename = exporter::auto_filename();
            PathBuf::from(&filename)
        }
        (false, Some(path)) => path,
    };

    println!("{}", "[EXPORT] Collecting data...".bold().cyan());

    // Parse options
    let only_type = only.as_deref().and_then(parse_export_type);
    let exclude_types: Vec<ExportType> = exclude
        .unwrap_or_default()
        .iter()
        .filter_map(|s| parse_export_type(s))
        .collect();

    let options = ExportOptions {
        include_secrets,
        only: only_type,
        exclude: exclude_types,
        disk,
        description,
    };

    // Show what will be exported
    println!();
    println!("Export contents:");
    if (options.only.is_none() || options.only == Some(ExportType::Config))
        && !options.exclude.contains(&ExportType::Config)
    {
        println!("  [x] App configuration (config.toml)");
    }
    if (options.only.is_none() || options.only == Some(ExportType::Indexes))
        && !options.exclude.contains(&ExportType::Indexes)
    {
        println!("  [x] Central index");
        println!("  [x] Disk indexes");
    }
    if (options.only.is_none() || options.only == Some(ExportType::Sessions))
        && !options.exclude.contains(&ExportType::Sessions)
    {
        println!("  [x] Session history");
    }
    if include_secrets {
        println!("  [x] Sensitive data (API keys)");
    } else {
        println!("  [ ] Sensitive data (use --include-secrets to include)");
    }
    println!();

    // Execute export
    println!("{}", "[EXPORT] Creating archive...".cyan());
    let manifest = exporter::export_to_file(&output_path, &options)?;

    // Print summary
    println!();
    println!("{}", "[OK] Export successful!".bold().green());
    println!("  File: {}", output_path.display());
    println!(
        "  Size: {:.2} MB",
        std::fs::metadata(&output_path)?.len() as f64 / 1_048_576.0
    );
    println!(
        "  Contents: {} movies, {} TV shows, {} disks, {} sessions",
        manifest.statistics.total_movies,
        manifest.statistics.total_tv_series,
        manifest.statistics.total_disks,
        manifest.statistics.total_sessions
    );
    println!();
    println!(
        "Tip: Import command: media-organizer import {}",
        output_path.display()
    );

    Ok(())
}

/// Execute import command.
pub async fn execute_import(
    backup_file: PathBuf,
    dry_run: bool,
    only: Option<String>,
    merge: bool,
    force: bool,
    backup_first: bool,
) -> Result<()> {
    println!("{}", "[IMPORT] Analyzing backup file...".bold().cyan());

    // Preview import
    let preview = exporter::preview_import(&backup_file)?;

    println!();
    println!("Backup information:");
    println!("  Created: {}", preview.manifest.created_at);
    println!("  Creator: {}", preview.manifest.created_by);
    if let Some(ref desc) = preview.manifest.description {
        println!("  Description: {}", desc);
    }
    println!("  App version: {}", preview.manifest.app_version);
    println!();

    println!("Will import:");
    for item in &preview.will_import {
        println!("  {}", item);
    }
    println!();

    // Check for conflicts
    if !preview.conflicts.is_empty() {
        println!("{}", "Conflict detection:".bold().yellow());
        for conflict in &preview.conflicts {
            println!("  [!] {}", conflict);
        }
        if merge {
            println!("      Using --merge to merge data");
        } else if force {
            println!("      Using --force to overwrite");
        } else {
            println!("      Use --merge to merge or --force to overwrite");
        }
        println!();
    }

    if dry_run {
        println!(
            "{}",
            "[DRY-RUN] No actions performed. Remove --dry-run to execute import."
                .bold()
                .yellow()
        );
        return Ok(());
    }

    // Confirm if not forced and has conflicts
    if !force && !preview.conflicts.is_empty() && !merge {
        println!(
            "{}",
            "[WARN] Use --force to overwrite or --merge to merge data"
                .bold()
                .yellow()
        );
        return Ok(());
    }

    // Parse options
    let options = ImportOptions {
        dry_run,
        only: only.as_deref().and_then(parse_export_type),
        merge,
        force,
        backup_first,
    };

    // Execute import
    println!("{}", "[IMPORT] Importing...".cyan());
    let result = exporter::import_from_file(&backup_file, &options)?;

    // Print summary
    println!();
    println!("{}", "[OK] Import successful!".bold().green());
    println!("  Imported: {} files", result.imported);
    if result.skipped > 0 {
        println!("  Skipped: {} files", result.skipped);
    }
    if result.new_movies > 0 {
        println!("  New movies: {}", result.new_movies);
    }
    if result.new_tv_series > 0 {
        println!("  New TV shows: {}", result.new_tv_series);
    }

    Ok(())
}

/// Parse export type from string.
fn parse_export_type(s: &str) -> Option<ExportType> {
    match s.to_lowercase().as_str() {
        "config" => Some(ExportType::Config),
        "indexes" | "index" => Some(ExportType::Indexes),
        "sessions" | "session" => Some(ExportType::Sessions),
        _ => None,
    }
}
