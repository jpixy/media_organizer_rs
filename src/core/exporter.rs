//! Export and import functionality for configuration and indexes.

use crate::core::indexer;
use crate::models::index::{
    CentralIndex, ExportContents, ExportManifest, ExportStatistics, SourcePaths,
};
use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// Export options.
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    /// Include sensitive data (API keys)
    pub include_secrets: bool,
    /// Only export specific types
    pub only: Option<ExportType>,
    /// Exclude specific types
    pub exclude: Vec<ExportType>,
    /// Only export specific disk
    pub disk: Option<String>,
    /// Description for the export
    pub description: Option<String>,
}

/// Types that can be exported.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportType {
    Config,
    Indexes,
    Sessions,
}

/// Import options.
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Dry run - don't actually import
    pub dry_run: bool,
    /// Only import specific types
    pub only: Option<ExportType>,
    /// Merge with existing data
    pub merge: bool,
    /// Force overwrite without confirmation
    pub force: bool,
    /// Backup existing config before import
    pub backup_first: bool,
}

/// Import preview result.
#[derive(Debug)]
pub struct ImportPreview {
    pub manifest: ExportManifest,
    pub conflicts: Vec<String>,
    pub will_import: Vec<String>,
}

/// Configuration directory path.
fn config_dir() -> Result<PathBuf> {
    let config = dirs::config_dir()
        .context("Failed to get config directory")?
        .join("media_organizer");
    Ok(config)
}

/// Sessions directory path.
fn sessions_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("sessions"))
}

/// Export configuration and indexes to a zip file.
pub fn export_to_file(output_path: &Path, options: &ExportOptions) -> Result<ExportManifest> {
    let config_path = config_dir()?;

    if !config_path.exists() {
        anyhow::bail!(
            "No configuration directory found at {}",
            config_path.display()
        );
    }

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create export file: {}", output_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let zip_options =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut contents = ExportContents {
        config: false,
        central_index: false,
        disk_indexes: Vec::new(),
        sessions: 0,
        includes_secrets: options.include_secrets,
    };

    let mut stats = ExportStatistics {
        total_movies: 0,
        total_tv_series: 0,
        total_disks: 0,
        total_sessions: 0,
        export_size_bytes: 0,
    };

    // Determine what to export
    let export_config = options.only.is_none() || options.only == Some(ExportType::Config);
    let export_indexes = options.only.is_none() || options.only == Some(ExportType::Indexes);
    let export_sessions = options.only.is_none() || options.only == Some(ExportType::Sessions);

    let skip_config = options.exclude.contains(&ExportType::Config);
    let skip_indexes = options.exclude.contains(&ExportType::Indexes);
    let skip_sessions = options.exclude.contains(&ExportType::Sessions);

    // Export config
    if export_config && !skip_config {
        let config_file = config_path.join("config.toml");
        if config_file.exists() {
            let mut config_content = fs::read_to_string(&config_file)?;

            // Remove secrets if not included
            if !options.include_secrets {
                config_content = remove_secrets_from_config(&config_content);
            }

            zip.start_file("config/config.toml", zip_options)?;
            zip.write_all(config_content.as_bytes())?;
            contents.config = true;
            tracing::info!("Exported: config.toml");
        }
    }

    // Export indexes
    if export_indexes && !skip_indexes {
        // Central index
        let central_index_path = config_path.join("central_index.json");
        if central_index_path.exists() {
            let central_content = fs::read_to_string(&central_index_path)?;
            zip.start_file("indexes/central_index.json", zip_options)?;
            zip.write_all(central_content.as_bytes())?;
            contents.central_index = true;

            // Parse for statistics
            if let Ok(index) = serde_json::from_str::<CentralIndex>(&central_content) {
                stats.total_movies = index.movies.len();
                stats.total_tv_series = index.tv_series.len();
                stats.total_disks = index.disks.len();
            }

            tracing::info!("Exported: central_index.json");
        }

        // Disk indexes
        let disk_indexes_path = config_path.join("disk_indexes");
        if disk_indexes_path.exists() {
            for entry in fs::read_dir(&disk_indexes_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    let disk_label = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");

                    // Filter by disk if specified
                    if let Some(ref filter_disk) = options.disk {
                        if disk_label != filter_disk {
                            continue;
                        }
                    }

                    let content = fs::read_to_string(&path)?;
                    let zip_path = format!("indexes/disk_indexes/{}.json", disk_label);
                    zip.start_file(&zip_path, zip_options)?;
                    zip.write_all(content.as_bytes())?;
                    contents.disk_indexes.push(disk_label.to_string());
                    tracing::info!("Exported: {}", zip_path);
                }
            }
        }
    }

    // Export sessions
    if export_sessions && !skip_sessions {
        let sessions_path = sessions_dir()?;
        if sessions_path.exists() {
            let mut session_count = 0;
            for entry in fs::read_dir(&sessions_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    let session_name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");

                    // Export all files in session directory
                    for file_entry in fs::read_dir(&path)? {
                        let file_entry = file_entry?;
                        let file_path = file_entry.path();
                        if file_path.is_file() {
                            let file_name = file_path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown");

                            let content = fs::read(&file_path)?;
                            let zip_path = format!("sessions/{}/{}", session_name, file_name);
                            zip.start_file(&zip_path, zip_options)?;
                            zip.write_all(&content)?;
                        }
                    }
                    session_count += 1;
                }
            }
            contents.sessions = session_count;
            stats.total_sessions = session_count;
            if session_count > 0 {
                tracing::info!("Exported: {} sessions", session_count);
            }
        }
    }

    // Create manifest
    let manifest = ExportManifest {
        version: "1.0".to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        created_by: format!(
            "{}@{}",
            whoami::username(),
            whoami::fallible::hostname().unwrap_or_else(|_| "unknown".to_string())
        ),
        description: options.description.clone(),
        contents,
        statistics: stats,
        source_paths: SourcePaths {
            config_dir: config_path.to_string_lossy().to_string(),
            hostname: whoami::fallible::hostname().unwrap_or_else(|_| "unknown".to_string()),
        },
    };

    // Write manifest
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    zip.start_file("manifest.json", zip_options)?;
    zip.write_all(manifest_json.as_bytes())?;

    zip.finish()?;

    // Get final file size
    let file_size = fs::metadata(output_path)?.len();
    tracing::info!("Export complete: {} bytes", file_size);

    Ok(manifest)
}

/// Remove sensitive data from config content.
fn remove_secrets_from_config(content: &str) -> String {
    let mut result = String::new();
    for line in content.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("api_key")
            || line_lower.contains("token")
            || line_lower.contains("secret")
        {
            // Comment out the line
            result.push_str("# [REMOVED] ");
            result.push_str(line);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    result
}

/// Preview what will be imported from a backup file.
pub fn preview_import(backup_path: &Path) -> Result<ImportPreview> {
    let file = File::open(backup_path)
        .with_context(|| format!("Failed to open backup file: {}", backup_path.display()))?;
    let mut archive = ZipArchive::new(file)?;

    // Read manifest
    let manifest_content = {
        let mut manifest_file = archive
            .by_name("manifest.json")
            .context("Backup file does not contain manifest.json")?;
        let mut content = String::new();
        manifest_file.read_to_string(&mut content)?;
        content
    };

    let manifest: ExportManifest =
        serde_json::from_str(&manifest_content).context("Failed to parse manifest.json")?;

    // Check for conflicts
    let mut conflicts = Vec::new();
    let mut will_import = Vec::new();

    let config_path = config_dir()?;

    if manifest.contents.config {
        let local_config = config_path.join("config.toml");
        if local_config.exists() {
            conflicts.push("Config file exists".to_string());
        }
        will_import.push("config.toml".to_string());
    }

    if manifest.contents.central_index {
        let local_index = config_path.join("central_index.json");
        if local_index.exists() {
            conflicts.push(format!(
                "Central index exists (current: {} movies)",
                indexer::load_central_index()
                    .map(|i| i.movies.len())
                    .unwrap_or(0)
            ));
        }
        will_import.push(format!(
            "Central index ({} movies, {} TV shows)",
            manifest.statistics.total_movies, manifest.statistics.total_tv_series
        ));
    }

    for disk in &manifest.contents.disk_indexes {
        will_import.push(format!("Disk index: {}", disk));
    }

    if manifest.contents.sessions > 0 {
        will_import.push(format!("{} sessions", manifest.contents.sessions));
    }

    Ok(ImportPreview {
        manifest,
        conflicts,
        will_import,
    })
}

/// Import configuration and indexes from a backup file.
pub fn import_from_file(backup_path: &Path, options: &ImportOptions) -> Result<ImportResult> {
    let file = File::open(backup_path)
        .with_context(|| format!("Failed to open backup file: {}", backup_path.display()))?;
    let mut archive = ZipArchive::new(file)?;

    let config_path = config_dir()?;
    fs::create_dir_all(&config_path)?;

    // Backup existing config if requested
    if options.backup_first {
        let backup_dir = config_path.with_file_name(format!(
            "media_organizer.backup.{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        ));
        if config_path.exists() {
            fs::rename(&config_path, &backup_dir)?;
            fs::create_dir_all(&config_path)?;
            tracing::info!("Existing config backed up to: {}", backup_dir.display());
        }
    }

    let mut result = ImportResult::default();

    // Read manifest first
    let manifest: ExportManifest = {
        let mut manifest_file = archive.by_name("manifest.json")?;
        let mut content = String::new();
        manifest_file.read_to_string(&mut content)?;
        serde_json::from_str(&content)?
    };

    // Re-open archive (ZipArchive doesn't allow random access after sequential read)
    let file = File::open(backup_path)?;
    let mut archive = ZipArchive::new(file)?;

    // Determine what to import
    let import_config = options.only.is_none() || options.only == Some(ExportType::Config);
    let import_indexes = options.only.is_none() || options.only == Some(ExportType::Indexes);
    let import_sessions = options.only.is_none() || options.only == Some(ExportType::Sessions);

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        let path_str = outpath.to_string_lossy();

        // Skip manifest
        if path_str == "manifest.json" {
            continue;
        }

        // Determine target path based on file location in archive
        let target_path = if path_str.starts_with("config/") {
            if !import_config {
                continue;
            }
            config_path.join(path_str.strip_prefix("config/").unwrap())
        } else if path_str.starts_with("indexes/") {
            if !import_indexes {
                continue;
            }
            config_path.join(path_str.strip_prefix("indexes/").unwrap())
        } else if path_str.starts_with("sessions/") {
            if !import_sessions {
                continue;
            }
            sessions_dir()?.join(path_str.strip_prefix("sessions/").unwrap())
        } else {
            continue;
        };

        if options.dry_run {
            tracing::info!("[DRY-RUN] Would import: {}", path_str);
            continue;
        }

        // Handle merge mode for central index
        if path_str == "indexes/central_index.json" && options.merge {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            let imported_index: CentralIndex = serde_json::from_str(&content)?;

            let mut current_index = indexer::load_central_index()?;
            let movies_before = current_index.movies.len();
            current_index.merge(imported_index);
            result.new_movies = current_index.movies.len() - movies_before;

            indexer::save_central_index(&current_index)?;
            tracing::info!("Merged central index: {} new entries", result.new_movies);
            continue;
        }

        // Create parent directory
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Check for existing file
        if target_path.exists() && !options.force && !options.merge {
            tracing::warn!("Skipping existing file: {}", target_path.display());
            result.skipped += 1;
            continue;
        }

        // Extract file
        let mut outfile = File::create(&target_path)?;
        std::io::copy(&mut file, &mut outfile)?;

        tracing::info!("Imported: {}", path_str);
        result.imported += 1;
    }

    result.manifest = Some(manifest);
    Ok(result)
}

/// Import result.
#[derive(Debug, Default)]
pub struct ImportResult {
    pub manifest: Option<ExportManifest>,
    pub imported: usize,
    pub skipped: usize,
    pub new_movies: usize,
    pub new_tv_series: usize,
}

/// Generate auto filename with timestamp.
pub fn auto_filename() -> String {
    format!(
        "media_organizer_backup_{}.zip",
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    )
}
