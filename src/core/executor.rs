//! Plan executor module.
//!
//! Executes operations defined in a plan:
//! - mkdir: Create directories
//! - move: Move video files
//! - create: Generate NFO files
//! - download: Download posters (parallel with retry)

use crate::generators::nfo;
use crate::models::media::MediaType;
use crate::models::plan::{Operation, OperationType, Plan, PlanItem, PlanItemStatus};
use crate::models::rollback::{
    Rollback, RollbackAction, RollbackActionType, RollbackOpType, RollbackOperation,
};
use crate::utils::download::DownloadConfig;
use crate::utils::hash;
use crate::Result;
use chrono::Utc;
use colored::Colorize;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Executor configuration.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Whether to verify checksums after moving files.
    pub verify_checksum: bool,
    /// Whether to create backup before overwriting.
    pub backup_on_overwrite: bool,
    /// Whether to enable proxy.
    pub proxy_enabled: bool,
    /// Proxy URL.
    pub proxy: Option<String>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            verify_checksum: true,
            backup_on_overwrite: true,
            proxy_enabled: false,
            proxy: None,
        }
    }
}

/// Plan executor.
pub struct Executor {
    config: ExecutorConfig,
}

impl Executor {
    /// Create a new executor with default configuration.
    pub fn new() -> Self {
        Self {
            config: ExecutorConfig::default(),
        }
    }

    /// Create a new executor with custom configuration.
    pub fn with_config(config: ExecutorConfig) -> Self {
        Self {
            config,
        }
    }

    /// Execute a plan with optimized parallel downloads.
    pub async fn execute(&self, plan: &Plan) -> Result<Rollback> {
        let total_start = Instant::now();
        println!("{}", "[EXEC] Executing plan...".bold().cyan());
        println!();

        // Validate plan first
        self.validate(plan)?;

        // Initialize rollback structure
        let rollback = Arc::new(Mutex::new(Rollback {
            version: "1.0".to_string(),
            plan_id: Uuid::new_v4().to_string(),
            executed_at: Utc::now().to_rfc3339(),
            operations: Vec::new(),
        }));

        let seq = Arc::new(Mutex::new(0u32));
        let mut success_count = 0;
        let mut error_count = 0;

        // Collect all operations, separating downloads for parallel execution
        let mut non_download_ops: Vec<(&Operation, &PlanItem)> = Vec::new();
        let mut download_ops: Vec<(&Operation, &PlanItem)> = Vec::new();

        for item in &plan.items {
            if item.status != PlanItemStatus::Pending {
                continue;
            }
            for op in &item.operations {
                if op.op == OperationType::Download {
                    download_ops.push((op, item));
                } else {
                    non_download_ops.push((op, item));
                }
            }
        }

        let total_ops = non_download_ops.len() + download_ops.len();
        tracing::info!(
            "Executing {} operations ({} sequential, {} parallel downloads)",
            total_ops,
            non_download_ops.len(),
            download_ops.len()
        );

        // Create progress bar
        let pb = ProgressBar::new(total_ops as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        // Phase 1: Execute non-download operations sequentially
        let mut op_idx = 0;
        for (op, item) in &non_download_ops {
            op_idx += 1;
            pb.set_message(format!(
                "[{}/{}] {:?}: {}",
                op_idx,
                total_ops,
                op.op,
                op.to.file_name().unwrap_or_default().to_string_lossy()
            ));
            pb.inc(1);

            tracing::info!(
                "Execute [{}/{}]: {:?} - {}",
                op_idx,
                total_ops,
                op.op,
                op.to.display()
            );

            match self.execute_operation(op, item, plan).await {
                Ok(rollback_op) => {
                    if let Some(mut rb_op) = rollback_op {
                        let mut seq_guard = seq.lock().await;
                        *seq_guard += 1;
                        rb_op.seq = *seq_guard;
                        rb_op.executed = true;
                        rollback.lock().await.operations.push(rb_op);
                    }
                    success_count += 1;
                }
                Err(e) => {
                    tracing::error!("Operation failed: {} - {}", op.to.display(), e);
                    error_count += 1;
                }
            }
        }

        // Phase 2: Execute downloads in parallel (up to 2 concurrent with retry)
        if !download_ops.is_empty() {
            tracing::info!("Downloading {} posters in parallel...", download_ops.len());

            // Reduced concurrency to avoid TMDB rate limiting
            const DOWNLOAD_CONCURRENCY: usize = 2;

            // Download configuration with retry mechanism
            let download_config = DownloadConfig {
                max_retries: 3,
                retry_delay_ms: 1000,
                exponential_backoff: true,
                timeout_secs: 30,
            };

            // Statistics for poster downloads
            let mut downloaded_count = 0;
            let mut skipped_count = 0;
            let mut failed_count = 0;
            let mut total_size: u64 = 0;
            let mut failed_items: Vec<(PathBuf, String)> = Vec::new();

            let download_results: Vec<_> = stream::iter(download_ops.iter())
                .map(|(op, _item)| {
                    let op_to = op.to.clone();
                    let op_url = op.url.clone();
                    let config = download_config.clone();
                    let proxy_enabled = self.config.proxy_enabled;
                    let proxy = self.config.proxy.clone();
                    async move {
                        let result = Self::execute_download_static_with_retry(
                            &op_url,
                            &op_to,
                            &config,
                            proxy_enabled,
                            &proxy,
                        ).await;
                        (op_to, result)
                    }
                })
                .buffer_unordered(DOWNLOAD_CONCURRENCY)
                .collect()
                .await;

            for (path, result) in download_results {
                op_idx += 1;
                pb.set_message(format!(
                    "[{}/{}] Downloaded: {}",
                    op_idx,
                    total_ops,
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
                pb.inc(1);

                match result {
                    Ok(Some(rb_op)) => {
                        // Downloaded successfully
                        let mut seq_guard = seq.lock().await;
                        *seq_guard += 1;
                        let mut rb_op = rb_op;
                        rb_op.seq = *seq_guard;
                        rb_op.executed = true;
                        rollback.lock().await.operations.push(rb_op);
                        success_count += 1;
                        downloaded_count += 1;
                        // Get file size
                        if let Ok(metadata) = fs::metadata(&path) {
                            total_size += metadata.len();
                        }
                    }
                    Ok(None) => {
                        // Skipped (already exists)
                        success_count += 1;
                        skipped_count += 1;
                    }
                    Err(e) => {
                        // Failed
                        tracing::warn!("Download failed: {} - {}", path.display(), e);
                        error_count += 1;
                        failed_count += 1;
                        failed_items.push((path, e.to_string()));
                    }
                }
            }

            // Print poster download summary
            if downloaded_count > 0 || skipped_count > 0 || failed_count > 0 {
                println!();
                println!("{}", "[Poster Download Summary]".bold().green());
                println!("  {} {}", "Downloaded:".bold(), downloaded_count);
                println!("  {} {}", "Skipped (already exist):".bold(), skipped_count);
                println!("  {} {}", "Failed:".bold(), failed_count);
                println!(
                    "  {} {} ({})",
                    "Total size:".bold(),
                    total_size,
                    format_size(total_size)
                );

                // Print failed items list
                if !failed_items.is_empty() {
                    println!();
                    println!("{} {}", "Failed items:".bold().red(), failed_count);
                    for (path, error) in &failed_items {
                        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                        println!("  - {}: {}", file_name, error);
                    }
                }
                println!();
            }
        }

        pb.finish_with_message("Done!");
        println!();

        let total_time = total_start.elapsed();

        // Print poster download summary from plan stats
        if let Some(ref poster_stats) = plan.poster_stats {
            println!("{}", "[Poster Download Summary]".bold().green());
            println!("  {} {}", "Will download:".bold(), poster_stats.download_count);
            println!("  {} {}", "Skipped (local image exists):".bold(), poster_stats.skipped_count);
            println!();
        }

        // Print summary
        println!("{}", "[Execution Summary]".bold().green());
        println!("  {} {}", "Successful operations:".bold(), success_count);
        println!("  {} {}", "Failed operations:".bold(), error_count);
        println!("  {} {:.2}s", "Total time:".bold(), total_time.as_secs_f64());
        println!();

        // Extract rollback from Arc<Mutex>
        let final_rollback = Arc::try_unwrap(rollback)
            .map_err(|_| crate::Error::ExecuteError("Failed to unwrap rollback".to_string()))?
            .into_inner();

        Ok(final_rollback)
    }

    /// Static download function with retry mechanism for parallel execution.
    async fn execute_download_static_with_retry(
        url: &Option<String>,
        path: &Path,
        config: &DownloadConfig,
        proxy_enabled: bool,
        proxy: &Option<String>,
    ) -> Result<Option<RollbackOperation>> {
        let url = url.as_ref().ok_or_else(|| {
            crate::Error::ExecuteError("Download operation missing 'url'".to_string())
        })?;

        // Skip if file already exists
        if path.exists() {
            tracing::debug!("File already exists, skipping: {:?}", path);
            return Ok(None);
        }

        // Download with retry mechanism
        let size = crate::utils::download::download_file_with_retry(
            url,
            path,
            config,
            proxy_enabled,
            proxy,
        ).await.map_err(|e| crate::Error::ExecuteError(e.to_string()))?;

        tracing::debug!("Downloaded: {:?} ({} bytes)", path, size);

        Ok(Some(RollbackOperation {
            seq: 0,
            op_type: RollbackOpType::Download,
            from: path.to_path_buf(),
            to: path.to_path_buf(),
            checksum: None,
            rollback: RollbackAction {
                op: RollbackActionType::Delete,
                path: path.to_path_buf(),
                to: None,
            },
            executed: false,
        }))
    }

    /// Validate a plan before execution.
    ///
    /// Supports resuming interrupted executions:
    /// - If source missing but target exists → already completed, will skip
    /// - If source missing and target missing → real error
    pub fn validate(&self, plan: &Plan) -> Result<()> {
        println!("[INFO] Validating plan...");

        let mut errors = Vec::new();
        let mut already_done = 0;

        for item in &plan.items {
            if item.status != PlanItemStatus::Pending {
                continue;
            }

            let source_exists = item.source.path.exists();
            let target_exists = item.target.full_path.exists();

            // Check if this item was already processed (interrupted execution)
            if !source_exists && target_exists {
                // Source moved to target - already completed, will be skipped
                already_done += 1;
                continue;
            }

            // Check if source file exists
            if !source_exists {
                errors.push(format!(
                    "Source file not found: {}",
                    item.source.path.display()
                ));
            }

            // Check for target conflicts (source exists but target also exists)
            if source_exists && target_exists && !self.config.backup_on_overwrite {
                errors.push(format!(
                    "Target file already exists: {}",
                    item.target.full_path.display()
                ));
            }
        }

        if already_done > 0 {
            println!(
                "[INFO] {} items already completed (will be skipped)",
                already_done
            );
        }

        if !errors.is_empty() {
            println!("{}", "[FAILED] Validation failed:".bold().red());
            for error in &errors {
                println!("  - {}", error);
            }
            return Err(crate::Error::PlanValidationError(format!(
                "{} errors found",
                errors.len()
            )));
        }

        println!("{}", "[OK] Validation passed".green());
        Ok(())
    }

    /// Execute a single operation.
    async fn execute_operation(
        &self,
        op: &Operation,
        item: &PlanItem,
        plan: &Plan,
    ) -> Result<Option<RollbackOperation>> {
        match op.op {
            OperationType::Mkdir => self.execute_mkdir(op),
            OperationType::Move => self.execute_move(op),
            OperationType::Create => self.execute_create(op, item, plan),
            OperationType::Download => self.execute_download(op).await,
        }
    }

    /// Execute mkdir operation.
    fn execute_mkdir(&self, op: &Operation) -> Result<Option<RollbackOperation>> {
        let path = &op.to;

        if path.exists() {
            tracing::debug!("Directory already exists: {:?}", path);
            return Ok(None);
        }

        fs::create_dir_all(path)?;
        tracing::debug!("Created directory: {:?}", path);

        Ok(Some(RollbackOperation {
            seq: 0,
            op_type: RollbackOpType::Mkdir,
            from: path.clone(),
            to: path.clone(),
            checksum: None,
            rollback: RollbackAction {
                op: RollbackActionType::Rmdir,
                path: path.clone(),
                to: None,
            },
            executed: false,
        }))
    }

    /// Execute move operation.
    ///
    /// Optimization: For same-filesystem moves (rename), skip checksum verification
    /// since rename is atomic and doesn't copy data. Only verify for cross-filesystem
    /// moves which require actual data copy.
    ///
    /// Supports resume after interruption with proper state detection:
    /// - (from=no, to=yes): Already completed → skip
    /// - (from=yes, to=yes): Interrupted cross-fs copy → verify and complete
    /// - (from=no, to=no): Source lost → error
    /// - (from=yes, to=no): Normal case → proceed
    fn execute_move(&self, op: &Operation) -> Result<Option<RollbackOperation>> {
        let from = op.from.as_ref().ok_or_else(|| {
            crate::Error::ExecuteError("Move operation missing 'from' path".to_string())
        })?;
        let to = &op.to;

        let from_exists = from.exists();
        let to_exists = to.exists();

        // State machine for move operation
        match (from_exists, to_exists) {
            (false, true) => {
                // Already completed (source moved to target)
                tracing::debug!("Move already completed, skipping: {:?} -> {:?}", from, to);
                return Ok(None);
            }
            (true, true) => {
                // Both exist: likely interrupted cross-filesystem copy
                // Compare file sizes to determine if copy was complete
                let from_size = fs::metadata(from).map(|m| m.len()).unwrap_or(0);
                let to_size = fs::metadata(to).map(|m| m.len()).unwrap_or(0);

                if from_size == to_size {
                    // Sizes match - copy was complete, just need to delete source
                    tracing::info!("Resuming interrupted move (deleting source): {:?}", from);
                    fs::remove_file(from)?;
                    return Ok(Some(RollbackOperation {
                        seq: 0,
                        op_type: RollbackOpType::Move,
                        from: from.clone(),
                        to: to.clone(),
                        checksum: None,
                        rollback: RollbackAction {
                            op: RollbackActionType::Move,
                            path: to.clone(),
                            to: Some(from.clone()),
                        },
                        executed: false,
                    }));
                } else {
                    // Sizes don't match - incomplete copy, remove and retry
                    tracing::warn!("Incomplete copy detected, removing and retrying: {:?}", to);
                    fs::remove_file(to)?;
                    // Fall through to normal processing
                }
            }
            (false, false) => {
                // Source lost and target doesn't exist
                return Err(crate::Error::ExecuteError(format!(
                    "Source file not found: {:?}",
                    from
                )));
            }
            (true, false) => {
                // Normal case: source exists, target doesn't
                // Fall through to normal processing
            }
        }

        // Create parent directory if needed
        if let Some(parent) = to.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        // Try atomic rename first (same filesystem, instant)
        match fs::rename(from, to) {
            Ok(()) => {
                // Rename succeeded - same filesystem, no checksum needed
                tracing::debug!("Moved (rename): {:?} -> {:?}", from, to);
                return Ok(Some(RollbackOperation {
                    seq: 0,
                    op_type: RollbackOpType::Move,
                    from: from.clone(),
                    to: to.clone(),
                    checksum: None, // No checksum for atomic rename
                    rollback: RollbackAction {
                        op: RollbackActionType::Move,
                        path: to.clone(),
                        to: Some(from.clone()),
                    },
                    executed: false,
                }));
            }
            Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                // Cross-filesystem move: need to copy + delete
                tracing::debug!("Cross-filesystem move detected, using copy+delete");
            }
            Err(e) => {
                return Err(crate::Error::ExecuteError(format!(
                    "Failed to move {:?}: {}",
                    from, e
                )));
            }
        }

        // Cross-filesystem move: copy with optional checksum verification
        let checksum = if self.config.verify_checksum {
            Some(hash::sha256_file(from)?)
        } else {
            None
        };

        // Copy file
        fs::copy(from, to)?;

        // Verify checksum after copy (only for cross-filesystem)
        if self.config.verify_checksum {
            if let Some(ref original_checksum) = checksum {
                let new_checksum = hash::sha256_file(to)?;
                if original_checksum != &new_checksum {
                    // Remove incomplete copy
                    let _ = fs::remove_file(to);
                    return Err(crate::Error::ExecuteError(format!(
                        "Checksum mismatch after copying: {:?}",
                        to
                    )));
                }
            }
        }

        // Delete original after successful copy
        fs::remove_file(from)?;
        tracing::debug!("Moved (copy+delete): {:?} -> {:?}", from, to);

        Ok(Some(RollbackOperation {
            seq: 0,
            op_type: RollbackOpType::Move,
            from: from.clone(),
            to: to.clone(),
            checksum,
            rollback: RollbackAction {
                op: RollbackActionType::Move,
                path: to.clone(),
                to: Some(from.clone()),
            },
            executed: false,
        }))
    }

    /// Execute create operation (NFO file).
    fn execute_create(
        &self,
        op: &Operation,
        item: &PlanItem,
        plan: &Plan,
    ) -> Result<Option<RollbackOperation>> {
        let path = &op.to;

        // Generate content based on content_ref
        let content = match op.content_ref.as_deref() {
            Some("nfo") => {
                match plan.media_type {
                    Some(MediaType::Movies) => {
                        if let Some(ref metadata) = item.movie_metadata {
                            nfo::generate_movie_nfo(metadata)
                        } else {
                            return Err(crate::Error::ExecuteError(
                                "Missing movie metadata for NFO generation".to_string(),
                            ));
                        }
                    }
                    Some(MediaType::TvSeries) => {
                        // Check if this is tvshow.nfo (show-level), seasonXX.nfo (season-level), or episode.nfo
                        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        // Support multiple tvshow NFO naming formats:
                        // - tvshow.nfo (old format)
                        // - [S][名称][原名]-tvshow.nfo (new format)
                        let is_tv_series_nfo = file_name == "tvshow.nfo" || file_name.ends_with("-tvshow.nfo");
                        // Support multiple season NFO naming formats:
                        // - seasonXX.nfo (old format)
                        // - [season]-seasonXX.nfo (intermediate format)
                        // - [TV名称]-seasonXX.nfo (old new format)
                        // - [S][名称][原名]-seasonXX.nfo (new format)
                        let is_season_nfo = (file_name.starts_with("season") || 
                                             file_name.starts_with("[season]-") ||
                                             (file_name.contains("-season") && file_name.ends_with(".nfo"))) 
                                            && file_name.ends_with(".nfo");

                        if is_tv_series_nfo {
                            // Generate show-level NFO
                            if let Some(ref show) = item.tv_series_metadata {
                                nfo::generate_tv_series_nfo(show)
                            } else {
                                return Err(crate::Error::ExecuteError(
                                    "Missing TV show metadata for NFO generation".to_string(),
                                ));
                            }
                        } else if is_season_nfo {
                            // Generate season-level NFO
                            if let (Some(ref show), Some(ref season)) =
                                (&item.tv_series_metadata, &item.season_metadata)
                            {
                                nfo::generate_season_nfo(show, season)
                            } else {
                                return Err(crate::Error::ExecuteError(
                                    "Missing TV show or season metadata for NFO generation".to_string(),
                                ));
                            }
                        } else {
                            // Generate episode-level NFO
                            if let (Some(ref show), Some(ref episode)) =
                                (&item.tv_series_metadata, &item.episode_metadata)
                            {
                                nfo::generate_episode_nfo(show, episode)
                            } else if let Some(ref show) = item.tv_series_metadata {
                                nfo::generate_tv_series_nfo(show)
                            } else {
                                return Err(crate::Error::ExecuteError(
                                    "Missing TV show metadata for NFO generation".to_string(),
                                ));
                            }
                        }
                    }
                    None => {
                        return Err(crate::Error::ExecuteError(
                            "Unknown media type for NFO generation".to_string(),
                        ));
                    }
                }
            }
            _ => {
                return Err(crate::Error::ExecuteError(format!(
                    "Unknown content_ref: {:?}",
                    op.content_ref
                )));
            }
        };

        // Skip if file already exists (for TV show NFO deduplication)
        if path.exists() {
            tracing::debug!("File already exists, skipping: {:?}", path);
            return Ok(None);
        }

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        // Write file
        let mut file = fs::File::create(path)?;
        file.write_all(content.as_bytes())?;
        tracing::debug!("Created file: {:?}", path);

        Ok(Some(RollbackOperation {
            seq: 0,
            op_type: RollbackOpType::Create,
            from: path.clone(),
            to: path.clone(),
            checksum: None,
            rollback: RollbackAction {
                op: RollbackActionType::Delete,
                path: path.clone(),
                to: None,
            },
            executed: false,
        }))
    }

    /// Execute download operation (poster) with retry mechanism.
    async fn execute_download(&self, op: &Operation) -> Result<Option<RollbackOperation>> {
        let url = op.url.as_ref().ok_or_else(|| {
            crate::Error::ExecuteError("Download operation missing 'url'".to_string())
        })?;
        let path = &op.to;

        // Skip if file already exists (optimization)
        if path.exists() {
            tracing::debug!("Poster already exists, skipping: {:?}", path);
            return Ok(None);
        }

        // Download with retry mechanism
        let config = DownloadConfig {
            max_retries: 3,
            retry_delay_ms: 1000,
            exponential_backoff: true,
            timeout_secs: 30,
        };

        let size = crate::utils::download::download_file_with_retry(
            url,
            path,
            &config,
            self.config.proxy_enabled,
            &self.config.proxy,
        ).await.map_err(|e| crate::Error::ExecuteError(e.to_string()))?;

        tracing::debug!("Downloaded: {} -> {:?} ({} bytes)", url, path, size);

        Ok(Some(RollbackOperation {
            seq: 0,
            op_type: RollbackOpType::Download,
            from: path.clone(),
            to: path.clone(),
            checksum: None,
            rollback: RollbackAction {
                op: RollbackActionType::Delete,
                path: path.clone(),
                to: None,
            },
            executed: false,
        }))
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a plan (convenience function).
pub async fn execute_plan(plan: &Plan) -> Result<Rollback> {
    let executor = Executor::new();
    executor.execute(plan).await
}

/// Validate a plan before execution (convenience function).
pub fn validate_plan(plan: &Plan) -> Result<()> {
    let executor = Executor::new();
    executor.validate(plan)
}

/// Save rollback to a JSON file.
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
    fn test_executor_config_default() {
        let config = ExecutorConfig::default();
        assert!(config.verify_checksum);
        assert!(config.backup_on_overwrite);
    }

    #[test]
    fn test_validate_empty_plan() {
        let plan = Plan::default();
        let executor = Executor::new();
        let result = executor.validate(&plan);
        assert!(result.is_ok());
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1073741824), "1.00 GB");
    }
}

/// Format byte size to human readable string.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
