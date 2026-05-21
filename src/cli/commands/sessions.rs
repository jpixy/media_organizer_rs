//! Sessions command implementation.
//!
//! Manages historical sessions stored in ~/.config/mediaorganizer/sessions/

use crate::core::planner;
use crate::Result;
use colored::Colorize;
use std::fs;

/// List all sessions.
pub async fn list_sessions() -> Result<()> {
    println!("{}", "[Sessions]".bold().cyan());
    println!();

    let sessions_dir = planner::sessions_dir()?;

    if !sessions_dir.exists() {
        println!("No sessions found.");
        return Ok(());
    }

    let mut sessions: Vec<_> = fs::read_dir(&sessions_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();

    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }

    // Sort by name (which includes timestamp)
    sessions.sort_by_key(|e| e.file_name());
    sessions.reverse(); // Most recent first

    println!(
        "{:<25} {:<10} {:<10} {}",
        "Session ID".bold(),
        "Items".bold(),
        "Unknown".bold(),
        "Source".bold()
    );
    println!("{}", "-".repeat(80));

    for entry in sessions {
        let session_id = entry.file_name().to_string_lossy().to_string();
        let plan_path = entry.path().join("plan.json");

        if plan_path.exists() {
            match planner::load_plan(&plan_path) {
                Ok(plan) => {
                    println!(
                        "{:<25} {:<10} {:<10} {}",
                        session_id,
                        plan.items.len(),
                        plan.unknown.len(),
                        plan.source_path.display()
                    );
                }
                Err(_) => {
                    println!("{:<25} {}", session_id, "(corrupted)".red());
                }
            }
        } else {
            println!("{:<25} {}", session_id, "(no plan.json)".yellow());
        }
    }

    println!();
    println!("Sessions directory: {}", sessions_dir.display());

    Ok(())
}

/// Show details of a specific session.
pub async fn show_session(session_id: &str) -> Result<()> {
    println!("{} {}", "[Session]".bold().cyan(), session_id);
    println!();

    let sessions_dir = planner::sessions_dir()?;
    let session_dir = sessions_dir.join(session_id);

    if !session_dir.exists() {
        return Err(crate::Error::PathNotFound(format!(
            "Session not found: {}",
            session_id
        )));
    }

    let plan_path = session_dir.join("plan.json");
    let rollback_path = session_dir.join("rollback.json");

    // Load and display plan
    if plan_path.exists() {
        let plan = planner::load_plan(&plan_path)?;

        println!("{}", "Plan Details:".bold());
        println!("  {} {}", "Created:".bold(), plan.created_at);
        println!(
            "  {} {}",
            "Media Type:".bold(),
            plan.media_type
                .map(|t| t.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );
        println!("  {} {}", "Source:".bold(), plan.source_path.display());
        println!("  {} {}", "Target:".bold(), plan.target_path.display());
        println!("  {} {}", "Items:".bold(), plan.items.len());
        println!("  {} {}", "Samples:".bold(), plan.samples.len());
        println!("  {} {}", "Unknown:".bold(), plan.unknown.len());
        println!();

        // Show items
        if !plan.items.is_empty() {
            println!("{}", "Items:".bold());
            for (i, item) in plan.items.iter().take(10).enumerate() {
                println!(
                    "  {}. {} -> {}",
                    i + 1,
                    item.source.filename,
                    item.target.filename
                );
            }
            if plan.items.len() > 10 {
                println!("  ... and {} more", plan.items.len() - 10);
            }
            println!();
        }

        // Show unknown files
        if !plan.unknown.is_empty() {
            println!("{}", "Unknown Files:".bold().yellow());
            for item in &plan.unknown {
                println!("  - {} ({})", item.source.filename, item.reason);
            }
            println!();
        }
    } else {
        println!("{}", "No plan.json found".yellow());
    }

    // Check for rollback
    if rollback_path.exists() {
        println!("{} {}", "Rollback:".bold(), "Available".green());
        println!("  {}", rollback_path.display());
    }

    println!();
    println!("Session directory: {}", session_dir.display());

    Ok(())
}
