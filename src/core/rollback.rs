//! Rollback execution module.
//!
//! Reverses operations performed by the executor:
//! - Move files back to original locations
//! - Delete created files (NFO, posters)
//! - Remove created directories

use crate::models::rollback::{Rollback, RollbackActionType, RollbackOperation};
use crate::utils::hash;
use crate::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::Write;
use std::path::Path;

/// Rollback executor.
pub struct RollbackExecutor {
    /// Whether to verify checksums before rollback.
    verify_checksum: bool,
}

impl RollbackExecutor {
    /// Create a new rollback executor.
    pub fn new() -> Self {
        Self {
            verify_checksum: true,
        }
    }

    /// Execute a rollback.
    pub async fn execute(&self, rollback: &Rollback, dry_run: bool) -> Result<RollbackResult> {
        if dry_run {
            println!("{}", "[DRY-RUN] No changes will be made".bold().yellow());
        } else {
            println!("{}", "[ROLLBACK] Executing rollback...".bold().cyan());
        }
        println!();

        // Check for conflicts first
        let conflicts = self.check_conflicts(rollback)?;
        if !conflicts.is_empty() {
            println!("{}", "[WARNING] Conflicts detected:".bold().yellow());
            for conflict in &conflicts {
                println!("  - {}", conflict);
            }
            if !dry_run {
                println!();
                println!("{}", "Proceeding with rollback anyway...".yellow());
            }
        }

        let mut result = RollbackResult {
            success_count: 0,
            skip_count: 0,
            error_count: 0,
            errors: Vec::new(),
        };

        // Execute operations in reverse order
        let operations: Vec<_> = rollback.operations.iter().rev().collect();

        let pb = ProgressBar::new(operations.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        for (idx, op) in operations.iter().enumerate() {
            let progress_msg = format!(
                "[{}/{}] {:?}: {}",
                idx + 1,
                operations.len(),
                op.rollback.op,
                op.rollback
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            pb.set_message(progress_msg.clone());
            pb.inc(1);

            // Log detailed info for each operation
            tracing::info!(
                "Rollback [{}/{}]: {:?} - {}",
                idx + 1,
                operations.len(),
                op.rollback.op,
                op.rollback.path.display()
            );

            if dry_run {
                println!(
                    "  {} {:?} {}",
                    "[DRY RUN]".yellow(),
                    op.rollback.op,
                    op.rollback.path.display()
                );
                result.success_count += 1;
                continue;
            }

            match self.execute_rollback_op(op) {
                Ok(executed) => {
                    if executed {
                        tracing::debug!("  [OK] Success");
                        result.success_count += 1;
                    } else {
                        tracing::debug!("  [SKIP] Skipped");
                        result.skip_count += 1;
                    }
                }
                Err(e) => {
                    let error_msg = format!("{}: {}", op.rollback.path.display(), e);
                    tracing::error!("Rollback operation failed: {}", error_msg);
                    result.errors.push(error_msg);
                    result.error_count += 1;
                }
            }
        }

        pb.finish_with_message("Done!");
        println!();

        Ok(result)
    }

    /// Check for conflicts before rollback.
    fn check_conflicts(&self, rollback: &Rollback) -> Result<Vec<String>> {
        let mut conflicts = Vec::new();

        for op in &rollback.operations {
            match op.rollback.op {
                RollbackActionType::Move => {
                    // Check if source file still exists at target location
                    if !op.rollback.path.exists() {
                        conflicts.push(format!(
                            "File not found at target: {}",
                            op.rollback.path.display()
                        ));
                    }

                    // Check if original location is occupied
                    if let Some(ref original_path) = op.rollback.to {
                        if original_path.exists() {
                            conflicts.push(format!(
                                "Original location occupied: {}",
                                original_path.display()
                            ));
                        }
                    }

                    // Check checksum if available
                    if self.verify_checksum {
                        if let Some(ref expected_checksum) = op.checksum {
                            if op.rollback.path.exists() {
                                if let Ok(current_checksum) = hash::sha256_file(&op.rollback.path) {
                                    if &current_checksum != expected_checksum {
                                        conflicts.push(format!(
                                            "File modified since execution: {}",
                                            op.rollback.path.display()
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                RollbackActionType::Delete => {
                    // Check if file still exists
                    if !op.rollback.path.exists() {
                        // Not a conflict, just skip
                    }
                }
                RollbackActionType::Rmdir => {
                    // Directory not empty is not a conflict - we simply skip removing it
                    // The files were moved back, directory cleanup is optional
                }
            }
        }

        Ok(conflicts)
    }

    /// Execute a single rollback operation.
    fn execute_rollback_op(&self, op: &RollbackOperation) -> Result<bool> {
        match op.rollback.op {
            RollbackActionType::Move => {
                let from = &op.rollback.path;
                let to = op.rollback.to.as_ref().ok_or_else(|| {
                    crate::Error::RollbackConflict("Move rollback missing 'to' path".to_string())
                })?;

                if !from.exists() {
                    tracing::warn!("Source file not found, skipping: {:?}", from);
                    return Ok(false);
                }

                // Create parent directory if needed
                if let Some(parent) = to.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                }

                // Try atomic rename first (same filesystem)
                match fs::rename(from, to) {
                    Ok(()) => {
                        tracing::debug!("Moved back (rename): {:?} -> {:?}", from, to);
                        Ok(true)
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                        // Cross-filesystem: use copy + delete
                        tracing::debug!(
                            "Cross-filesystem rollback, using copy+delete: {:?} -> {:?}",
                            from,
                            to
                        );
                        fs::copy(from, to)?;
                        fs::remove_file(from)?;
                        tracing::debug!("Moved back (copy+delete): {:?} -> {:?}", from, to);
                        Ok(true)
                    }
                    Err(e) => Err(crate::Error::RollbackConflict(format!(
                        "Failed to move back {:?} -> {:?}: {}",
                        from, to, e
                    ))),
                }
            }
            RollbackActionType::Delete => {
                let path = &op.rollback.path;

                if !path.exists() {
                    tracing::debug!("File already deleted, skipping: {:?}", path);
                    return Ok(false);
                }

                fs::remove_file(path)?;
                tracing::debug!("Deleted: {:?}", path);
                Ok(true)
            }
            RollbackActionType::Rmdir => {
                let path = &op.rollback.path;

                if !path.exists() {
                    tracing::debug!("Directory already removed, skipping: {:?}", path);
                    return Ok(false);
                }

                // Only remove if empty - silently skip non-empty directories
                // This is expected behavior: files were moved back, other files may remain
                if let Ok(mut entries) = fs::read_dir(path) {
                    if entries.next().is_some() {
                        tracing::debug!("Directory not empty, keeping: {:?}", path);
                        return Ok(false);
                    }
                }

                fs::remove_dir(path)?;
                tracing::debug!("Removed directory: {:?}", path);
                Ok(true)
            }
        }
    }
}

impl Default for RollbackExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a rollback execution.
#[derive(Debug, Default)]
pub struct RollbackResult {
    /// Number of successful operations.
    pub success_count: usize,
    /// Number of skipped operations.
    pub skip_count: usize,
    /// Number of failed operations.
    pub error_count: usize,
    /// Error messages.
    pub errors: Vec<String>,
}

impl RollbackResult {
    /// Check if rollback was successful.
    pub fn is_success(&self) -> bool {
        self.error_count == 0
    }

    /// Print summary.
    pub fn print_summary(&self) {
        println!("{}", "[Rollback Summary]".bold().green());
        println!("  {} {}", "Successful:".bold(), self.success_count);
        println!("  {} {}", "Skipped:".bold(), self.skip_count);
        println!("  {} {}", "Failed:".bold(), self.error_count);

        if !self.errors.is_empty() {
            println!();
            println!("{}", "[ERRORS]:".bold().red());
            for error in &self.errors {
                println!("  - {}", error);
            }
        }
    }
}

/// Execute a rollback (convenience function).
pub async fn execute_rollback(rollback: &Rollback, dry_run: bool) -> Result<RollbackResult> {
    let executor = RollbackExecutor::new();
    executor.execute(rollback, dry_run).await
}

/// Load a rollback from a JSON file.
pub fn load_rollback(path: &Path) -> Result<Rollback> {
    let content = fs::read_to_string(path)?;
    let rollback: Rollback = serde_json::from_str(&content)?;
    Ok(rollback)
}

/// Save a rollback to a JSON file.
pub fn save_rollback(rollback: &Rollback, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(rollback)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::File::create(path)?;
    file.write_all(json.as_bytes())?;

    tracing::info!("Rollback saved to {:?}", path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollback_result_default() {
        let result = RollbackResult::default();
        assert!(result.is_success());
        assert_eq!(result.success_count, 0);
        assert_eq!(result.error_count, 0);
    }

    #[test]
    fn test_rollback_result_with_errors() {
        let result = RollbackResult {
            success_count: 5,
            skip_count: 1,
            error_count: 2,
            errors: vec!["error1".to_string(), "error2".to_string()],
        };
        assert!(!result.is_success());
    }

    // test_load_save_rollback moved to tests/io_tests.rs
}
