//! Rollback command implementation.
//!
//! Reads a rollback.json file and reverses all operations
//! to restore the original state.

use crate::core::rollback::{self, RollbackExecutor};
use crate::Result;
use colored::Colorize;
use std::path::Path;

/// Execute a rollback.
pub async fn rollback(rollback_file: &Path, dry_run: bool) -> Result<()> {
    println!("{}", "[ROLLBACK] Rollback command".bold().cyan());
    println!();

    // Validate rollback file exists
    if !rollback_file.exists() {
        return Err(crate::Error::PathNotFound(
            rollback_file.display().to_string(),
        ));
    }

    // Load rollback
    println!("[INFO] Loading rollback: {}", rollback_file.display());
    let rb = rollback::load_rollback(rollback_file)?;

    // Print rollback info
    println!("  {} {}", "Plan ID:".bold(), rb.plan_id);
    println!("  {} {}", "Executed at:".bold(), rb.executed_at);
    println!("  {} {}", "Operations:".bold(), rb.operations.len());
    println!();

    if dry_run {
        println!(
            "{}",
            "[DRY-RUN] Showing what would be done:".bold().yellow()
        );
        println!();
    } else {
        println!(
            "{}",
            "[WARNING] This will reverse all previous operations!"
                .bold()
                .yellow()
        );
        println!();
    }

    // Execute rollback
    let executor = RollbackExecutor::new();
    let result = executor.execute(&rb, dry_run).await?;

    // Print summary
    result.print_summary();
    println!();

    if result.is_success() {
        if dry_run {
            println!("{}", "[OK] Dry run complete - no changes were made".green());
            println!();
            println!("{}", "[Next Steps]".bold().cyan());
            println!("  To actually execute the rollback:");
            println!(
                "     {}",
                format!("media-organizer rollback {}", rollback_file.display()).bold()
            );
        } else {
            println!("{}", "[OK] Rollback completed successfully!".green());
            println!();
            println!("{}", "[Next Steps]".bold().cyan());
            println!("  Files have been restored. To reorganize, run:");
            println!(
                "     {}",
                "media-organizer plan movies <source> -t <target>".bold()
            );
            println!("  or:");
            println!(
                "     {}",
                "media-organizer plan tv_series <source> -t <target>".bold()
            );
        }
    } else {
        println!("{}", "[WARNING] Rollback completed with errors".yellow());
    }

    Ok(())
}
