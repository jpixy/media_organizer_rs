//! Execute command implementation.
//!
//! Reads a plan.json file and executes all operations,
//! generating a rollback.json for recovery.

use crate::core::executor::{self, Executor, ExecutorConfig};
use crate::core::planner;
use crate::models::config::Config;
use crate::Result;
use chrono::Utc;
use colored::Colorize;
use std::path::{Path, PathBuf};

/// Execute a plan file.
pub async fn execute_plan(plan_file: &Path, output: Option<&Path>, config: &Config) -> Result<()> {
    println!("{}", "[EXEC] Executing plan...".bold().cyan());
    println!();

    // Validate plan file exists
    if !plan_file.exists() {
        return Err(crate::Error::PathNotFound(plan_file.display().to_string()));
    }

    // Load plan
    println!("[INFO] Loading plan: {}", plan_file.display());
    let plan = planner::load_plan(plan_file)?;

    // Print plan info
    println!(
        "  {} {}",
        "Media type:".bold(),
        plan.media_type
            .map(|t| t.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!("  {} {}", "Source:".bold(), plan.source_path.display());
    println!("  {} {}", "Target:".bold(), plan.target_path.display());
    println!("  {} {}", "Items:".bold(), plan.items.len());
    println!();

    // Confirm execution
    println!(
        "{}",
        "[WARNING] This will move and modify files!".bold().yellow()
    );
    println!();

    // Create executor with config (including proxy)
    let mut executor_config = ExecutorConfig::default();
    executor_config.proxy_enabled = config.network.proxy_enabled;
    executor_config.proxy = config.network.proxy.clone();
    let executor = Executor::with_config(executor_config);
    let rollback = executor.execute(&plan).await?;

    // Determine rollback output path
    let rollback_path = match output {
        Some(p) => p.to_path_buf(),
        None => {
            // Default: same directory as plan file with rollback prefix
            let filename = format!("rollback_{}.json", Utc::now().format("%Y%m%d_%H%M%S"));
            plan_file
                .parent()
                .map(|p| p.join(filename.clone()))
                .unwrap_or_else(|| PathBuf::from(filename))
        }
    };

    // Save rollback
    executor::save_rollback(&rollback, &rollback_path)?;
    println!(
        "{} {}",
        "[OK] Rollback saved to:".bold().green(),
        rollback_path.display()
    );

    // Print next steps with complete commands
    println!();
    println!("{}", "[Next Steps]".bold().cyan());
    println!("  To undo all changes, run:");
    println!();
    println!(
        "    {}",
        format!("media-organizer rollback {}", rollback_path.display()).bold()
    );
    println!();

    Ok(())
}
