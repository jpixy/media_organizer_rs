//! Poster download command implementation.

use anyhow::{Context, Result};
use crate::models::config::Config;
use crate::services::tmdb::{TmdbClient, TmdbConfig};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio;
use futures::future;

/// Record for failed downloads.
struct FailedItem {
    folder_name: String,
    reason: String,
    path: String,
}

/// Download posters for movies.
/// 
/// Scans the directory recursively for movie folders (identified by tmdb ID in folder name),
/// and downloads posters. If a poster already exists, it will be skipped.
pub async fn download_movie_posters(path: &Path, config: &Config) -> Result<()> {
    println!("Downloading movie posters for: {}", path.display());
    println!("Using poster size from config: {}", config.organize.poster_size);
    
    let api_key = config.tmdb.api_key.clone().ok_or_else(|| anyhow::anyhow!("TMDB API key not configured"))?;
    let tmdb_config = TmdbConfig {
        api_key: api_key.clone(),
        language: config.tmdb.language.clone(),
        use_bearer: false,
        proxy_enabled: config.network.proxy_enabled,
        proxy: config.network.proxy.clone(),
    };
    
    let mut downloaded_count = 0;
    let mut skipped_count = 0;
    let mut failed_count = 0;
    let mut total_size_bytes: u64 = 0;
    let mut failed_items: Vec<FailedItem> = Vec::new();
    let start_time = Instant::now();
    
    if !path.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
    }
    
    // Recursively find all directories containing tmdb ID (movies)
    let mut movie_dirs = Vec::new();
    find_movie_dirs(path, &mut movie_dirs);
    
    let total_folders = movie_dirs.len();
    
    if movie_dirs.is_empty() {
        tracing::warn!("No movie folders found (containing tmdb ID) in: {}", path.display());
        println!();
        println!("Download completed!");
        println!("  Total folders: {}", total_folders);
        println!("  Downloaded: {} posters", downloaded_count);
        println!("  Skipped: {} posters (already exist)", skipped_count);
        println!("  Failed: {} posters", failed_count);
        println!("  Total size: {} bytes", total_size_bytes);
        println!("  Time elapsed: {:?}", start_time.elapsed());
        return Ok(());
    }
    
    println!("Found {} movie folders", total_folders);
    
    // Prepare tasks for concurrent download
    let mut tasks = Vec::new();
    let poster_size = config.organize.poster_size.clone();
    let proxy_enabled = config.network.proxy_enabled;
    let proxy = config.network.proxy.clone();
    
    for entry_path in movie_dirs {
        // Find video files in the folder
        let mut video_files = Vec::new();
        if let Ok(entries) = fs::read_dir(&entry_path) {
            for e in entries.flatten() {
                if is_video_file(e.path()) {
                    video_files.push(e.path());
                }
            }
        }
        
        if video_files.is_empty() {
            continue;
        }
        
        // Get movie title from folder name or nfo
        let folder_name = entry_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        
        // Try to parse tmdb id from folder name
        let tmdb_id = match parse_tmdb_id_from_folder_name(&folder_name) {
            Some(id) => id as u64,
            None => {
                tracing::warn!("Could not parse TMDB ID from folder: {}", folder_name);
                continue;
            }
        };
        
        let video_path = video_files[0].clone();
        let tmdb_config_clone = tmdb_config.clone();
        let poster_size_clone = poster_size.clone();
        let proxy_clone = proxy.clone();
        
        // Spawn async task for concurrent download
        let task = tokio::spawn(async move {
            download_single_movie_poster(
                tmdb_id,
                &folder_name,
                &entry_path,
                &video_path,
                &tmdb_config_clone,
                &poster_size_clone,
                proxy_enabled,
                &proxy_clone,
            ).await
        });
        
        tasks.push(task);
    }
    
    // Wait for all tasks to complete
    let results = future::join_all(tasks).await;
    
    // Process results
    for result in results {
        match result {
            Ok(download_result) => match download_result {
                DownloadResult::Downloaded(size) => {
                    downloaded_count += 1;
                    total_size_bytes += size;
                }
                DownloadResult::Skipped => {
                    skipped_count += 1;
                }
                DownloadResult::Failed { folder_name, reason, path } => {
                    failed_count += 1;
                    failed_items.push(FailedItem {
                        folder_name,
                        reason,
                        path,
                    });
                }
            },
            Err(e) => {
                tracing::warn!("Task error: {}", e);
                failed_count += 1;
            }
        }
    }
    
    let elapsed_time = start_time.elapsed();
    
    // Print failed items list before statistics
    if !failed_items.is_empty() {
        println!();
        println!("Failed items ({}):", failed_count);
        for item in &failed_items {
            println!("  - {}: {}", item.folder_name, item.reason);
            println!("    Path: {}", item.path);
        }
    }
    
    println!();
    println!("Download completed!");
    println!("  Total folders: {}", total_folders);
    println!("  Downloaded: {} posters", downloaded_count);
    println!("  Skipped: {} posters (already exist)", skipped_count);
    println!("  Failed: {} posters", failed_count);
    println!("  Total size: {} bytes ({})", total_size_bytes, format_size(total_size_bytes));
    println!("  Time elapsed: {:?}", elapsed_time);
    
    Ok(())
}

/// Download result enum.
enum DownloadResult {
    Downloaded(u64),
    Skipped,
    Failed { folder_name: String, reason: String, path: String },
}

/// Download a single movie poster.
async fn download_single_movie_poster(
    tmdb_id: u64,
    folder_name: &str,
    entry_path: &Path,
    video_path: &Path,
    tmdb_config: &TmdbConfig,
    poster_size: &str,
    proxy_enabled: bool,
    proxy: &Option<String>,
) -> DownloadResult {
    // Get the poster name from video file first
    let poster_name = format!("{}.jpg", video_path.file_stem().unwrap_or_default().to_string_lossy());
    let poster_full_path = entry_path.join(&poster_name);
    
    // Skip if poster already exists - check BEFORE calling TMDB API to save network requests
    if poster_full_path.exists() {
        tracing::info!("Poster already exists, skipping: {}", poster_full_path.display());
        return DownloadResult::Skipped;
    }
    
    let tmdb = TmdbClient::new(tmdb_config.clone());
    
    // Fetch movie details from TMDB
    let movie_details = match tmdb.get_movie_details(tmdb_id).await {
        Ok(details) => details,
        Err(e) => {
            tracing::warn!("Failed to fetch movie details for tmdb{}: {}", tmdb_id, e);
            return DownloadResult::Failed {
                folder_name: folder_name.to_string(),
                reason: format!("Failed to fetch movie details: {}", e),
                path: entry_path.display().to_string(),
            };
        }
    };
    
    // Get poster URL
    let poster_path = match movie_details.poster_path {
        Some(p) => p,
        None => {
            tracing::info!("No poster available for: {}", folder_name);
            return DownloadResult::Failed {
                folder_name: folder_name.to_string(),
                reason: "No poster available".to_string(),
                path: entry_path.display().to_string(),
            };
        }
    };
    
    let poster_url = format!("https://image.tmdb.org/t/p/{}{}", poster_size, poster_path);
    
    // Download poster
    match download_file_with_size(&poster_url, &poster_full_path, proxy_enabled, proxy).await {
        Ok(size) => {
            tracing::info!("Downloaded poster: {}", poster_full_path.display());
            DownloadResult::Downloaded(size)
        }
        Err(e) => {
            tracing::warn!("Failed to download poster for {}: {}", folder_name, e);
            DownloadResult::Failed {
                folder_name: folder_name.to_string(),
                reason: format!("Download failed: {}", e),
                path: entry_path.display().to_string(),
            }
        }
    }
}

/// Recursively find all directories containing tmdb ID (identified as movies)
fn find_movie_dirs(path: &Path, result: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                // Check if this directory name contains tmdb ID
                if let Some(dir_name) = entry_path.file_name().and_then(|n| n.to_str()) {
                    if parse_tmdb_id_from_folder_name(dir_name).is_some() {
                        // Check if it contains video files (to distinguish from TV series)
                        let mut has_video = false;
                        if let Ok(entries) = fs::read_dir(&entry_path) {
                            for e in entries.flatten() {
                                if is_video_file(e.path()) {
                                    has_video = true;
                                    break;
                                }
                            }
                        }
                        if has_video {
                            result.push(entry_path.clone());
                        }
                    }
                }
                // Recursively search subdirectories
                find_movie_dirs(&entry_path, result);
            }
        }
    }
}

/// Download posters for TV series seasons.
/// 
/// Scans the directory recursively for TV series folders (identified by tmdb ID in folder name),
/// and downloads season posters. If a poster already exists, it will be skipped.
pub async fn download_tv_season_posters(path: &Path, config: &Config) -> Result<()> {
    println!("Downloading TV season posters for: {}", path.display());
    println!("Using poster size from config: {}", config.organize.poster_size);
    
    let api_key = config.tmdb.api_key.clone().ok_or_else(|| anyhow::anyhow!("TMDB API key not configured"))?;
    let tmdb_config = TmdbConfig {
        api_key: api_key.clone(),
        language: config.tmdb.language.clone(),
        use_bearer: false,
        proxy_enabled: config.network.proxy_enabled,
        proxy: config.network.proxy.clone(),
    };
    
    let mut downloaded_count = 0;
    let mut skipped_count = 0;
    let mut failed_count = 0;
    let mut total_size_bytes: u64 = 0;
    let mut all_failed_items: Vec<FailedItem> = Vec::new();
    let start_time = Instant::now();
    
    if !path.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
    }
    
    // Recursively find all directories containing tmdb ID (TV series)
    let mut tv_show_dirs = Vec::new();
    find_tv_show_dirs(path, &mut tv_show_dirs);
    
    let total_shows = tv_show_dirs.len();
    
    if tv_show_dirs.is_empty() {
        tracing::warn!("No TV series folders found (containing tmdb ID) in: {}", path.display());
        println!();
        println!("Download completed!");
        println!("  Total shows: {}", total_shows);
        println!("  Total seasons: {}", 0);
        println!("  Downloaded: {} season posters", downloaded_count);
        println!("  Skipped: {} season posters (already exist)", skipped_count);
        println!("  Failed: {} season posters", failed_count);
        println!("  Total size: {} bytes", total_size_bytes);
        println!("  Time elapsed: {:?}", start_time.elapsed());
        return Ok(());
    }
    
    // Prepare tasks for concurrent download
    let mut tasks = Vec::new();
    let poster_size = config.organize.poster_size.clone();
    let proxy_enabled = config.network.proxy_enabled;
    let proxy = config.network.proxy.clone();
    
    let mut total_seasons = 0;
    
    for entry_path in tv_show_dirs {
        // Get TV show info
        let folder_name = entry_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let tmdb_id = match parse_tmdb_id_from_folder_name(&folder_name) {
            Some(id) => id as u64,
            None => {
                tracing::warn!("Could not parse TMDB ID from folder: {}", folder_name);
                continue;
            }
        };
        
        let tmdb_config_clone = tmdb_config.clone();
        let poster_size_clone = poster_size.clone();
        let proxy_clone = proxy.clone();
        
        // Spawn async task to fetch show details and process seasons
        let task = tokio::spawn(async move {
            process_tv_show_seasons(
                tmdb_id,
                &folder_name,
                &entry_path,
                tmdb_config_clone,
                poster_size_clone,
                proxy_enabled,
                proxy_clone,
            ).await
        });
        
        tasks.push(task);
    }
    
    // Wait for all tasks to complete
    let results = future::join_all(tasks).await;
    
    // Process results
    for result in results {
        match result {
            Ok(show_result) => {
                total_seasons += show_result.total_seasons;
                downloaded_count += show_result.downloaded;
                skipped_count += show_result.skipped;
                failed_count += show_result.failed;
                total_size_bytes += show_result.total_size;
                all_failed_items.extend(show_result.failed_items);
            },
            Err(e) => {
                tracing::warn!("Task error: {}", e);
            }
        }
    }
    
    let elapsed_time = start_time.elapsed();
    
    // Print failed items list before statistics
    if !all_failed_items.is_empty() {
        println!();
        println!("Failed items ({}):", failed_count);
        for item in &all_failed_items {
            println!("  - {}: {}", item.folder_name, item.reason);
            println!("    Path: {}", item.path);
        }
    }
    
    println!();
    println!("Download completed!");
    println!("  Total shows: {}", total_shows);
    println!("  Total seasons: {}", total_seasons);
    println!("  Downloaded: {} season posters", downloaded_count);
    println!("  Skipped: {} season posters (already exist)", skipped_count);
    println!("  Failed: {} season posters", failed_count);
    println!("  Total size: {} bytes ({})", total_size_bytes, format_size(total_size_bytes));
    println!("  Time elapsed: {:?}", elapsed_time);
    
    Ok(())
}

/// Result struct for TV show season processing.
struct TvShowResult {
    total_seasons: usize,
    downloaded: usize,
    skipped: usize,
    failed: usize,
    total_size: u64,
    failed_items: Vec<FailedItem>,
}

/// Process all seasons for a single TV show.
async fn process_tv_show_seasons(
    tmdb_id: u64,
    folder_name: &str,
    entry_path: &Path,
    tmdb_config: TmdbConfig,
    poster_size: String,
    proxy_enabled: bool,
    proxy: Option<String>,
) -> TvShowResult {
    let tmdb = TmdbClient::new(tmdb_config.clone());
    
    let mut downloaded = 0;
    let mut skipped = 0;
    let mut failed = 0;
    let mut total_size: u64 = 0;
    let mut failed_items: Vec<FailedItem> = Vec::new();
    
    // Get show details to know how many seasons
    let show_details = match tmdb.get_tv_details(tmdb_id).await {
        Ok(details) => details,
        Err(e) => {
            tracing::warn!("Failed to fetch TV show details for tmdb{}: {}", tmdb_id, e);
            return TvShowResult {
                total_seasons: 0,
                downloaded,
                skipped,
                failed,
                total_size,
                failed_items,
            };
        }
    };
    
    // Get season directories or iterate through all seasons
    let mut season_dirs = Vec::new();
    if let Ok(entries) = fs::read_dir(entry_path) {
        for e in entries.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Some(dir_name) = e.file_name().to_str() {
                    if let Some(season_num) = extract_season_from_dirname(dir_name) {
                        season_dirs.push((season_num, e.path()));
                    }
                }
            }
        }
    }
    
    // If no season directories found, create entries for all seasons
    if season_dirs.is_empty() {
        for season_num in 1..=show_details.number_of_seasons.unwrap_or_default() as u32 {
            season_dirs.push((season_num, entry_path.join(format!("Season {:02}", season_num))));
        }
    }
    
    let total_seasons = season_dirs.len();
    
    // Prepare season download tasks
    let mut season_tasks = Vec::new();
    
    for (season_num, season_path) in season_dirs {
        let folder_name_clone = folder_name.to_string();
        let poster_size_clone = poster_size.clone();
        let proxy_clone = proxy.clone();
        let tmdb_config_clone = tmdb_config.clone();
        
        let season_task = tokio::spawn(async move {
            download_single_tv_season_poster(
                tmdb_id,
                &folder_name_clone,
                season_num,
                &season_path,
                tmdb_config_clone,
                poster_size_clone,
                proxy_enabled,
                proxy_clone,
            ).await
        });
        
        season_tasks.push(season_task);
    }
    
    // Wait for all season tasks
    let season_results = future::join_all(season_tasks).await;
    
    // Process season results
    for season_result in season_results {
        match season_result {
            Ok(result) => match result {
                DownloadResult::Downloaded(size) => {
                    downloaded += 1;
                    total_size += size;
                }
                DownloadResult::Skipped => {
                    skipped += 1;
                }
                DownloadResult::Failed { folder_name, reason, path } => {
                    failed += 1;
                    failed_items.push(FailedItem {
                        folder_name,
                        reason,
                        path,
                    });
                }
            },
            Err(e) => {
                tracing::warn!("Season task error for {}: {}", folder_name, e);
                failed += 1;
                failed_items.push(FailedItem {
                    folder_name: folder_name.to_string(),
                    reason: format!("Task error: {}", e),
                    path: entry_path.display().to_string(),
                });
            }
        }
    }
    
    TvShowResult {
        total_seasons,
        downloaded,
        skipped,
        failed,
        total_size,
        failed_items,
    }
}

/// Download a single TV season poster.
async fn download_single_tv_season_poster(
    tmdb_id: u64,
    folder_name: &str,
    season_num: u32,
    season_path: &Path,
    tmdb_config: TmdbConfig,
    poster_size: String,
    proxy_enabled: bool,
    proxy: Option<String>,
) -> DownloadResult {
    // Create season directory if it doesn't exist
    if !season_path.exists() {
        if let Err(e) = fs::create_dir_all(season_path) {
            tracing::warn!("Failed to create season directory {}: {}", season_path.display(), e);
            return DownloadResult::Failed {
                folder_name: folder_name.to_string(),
                reason: format!("Failed to create season directory: {}", e),
                path: season_path.display().to_string(),
            };
        }
    }
    
    // Season poster name: [TV名称]-seasonXX.jpg (same naming as NFO)
    let show_title = folder_name.split('-').next().unwrap_or(folder_name).trim().trim_matches(|c| c == '[' || c == ']');
    let poster_name = format!("[{}]-season{:02}.jpg", show_title, season_num);
    let poster_full_path = season_path.join(&poster_name);
    
    // Skip if poster already exists - check BEFORE calling TMDB API to save network requests
    if poster_full_path.exists() {
        tracing::info!("Poster already exists, skipping: {}", poster_full_path.display());
        return DownloadResult::Skipped;
    }
    
    let tmdb = TmdbClient::new(tmdb_config);
    
    // Fetch season details
    let season_details = match tmdb.get_season_details(tmdb_id, season_num as u16).await {
        Ok(details) => details,
        Err(e) => {
            tracing::warn!("Failed to fetch season {} details for tmdb{}: {}", season_num, tmdb_id, e);
            return DownloadResult::Failed {
                folder_name: folder_name.to_string(),
                reason: format!("Failed to fetch season details: {}", e),
                path: season_path.display().to_string(),
            };
        }
    };
    
    // Get poster URL
    let poster_path = match season_details.poster_path {
        Some(p) => p,
        None => {
            tracing::info!("No poster available for {} - Season {}", folder_name, season_num);
            return DownloadResult::Failed {
                folder_name: folder_name.to_string(),
                reason: "No poster available".to_string(),
                path: season_path.display().to_string(),
            };
        }
    };
    
    let poster_url = format!("https://image.tmdb.org/t/p/{}{}", poster_size, poster_path);
    
    // Download poster
    match download_file_with_size(&poster_url, &poster_full_path, proxy_enabled, &proxy).await {
        Ok(size) => {
            tracing::info!("Downloaded poster: {}", poster_full_path.display());
            DownloadResult::Downloaded(size)
        }
        Err(e) => {
            tracing::warn!("Failed to download poster for {} Season {}: {}", folder_name, season_num, e);
            DownloadResult::Failed {
                folder_name: folder_name.to_string(),
                reason: format!("Download failed: {}", e),
                path: season_path.display().to_string(),
            }
        }
    }
}

/// Recursively find all directories containing tmdb ID (identified as TV series)
fn find_tv_show_dirs(path: &Path, result: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                // Check if this directory name contains tmdb ID
                if let Some(dir_name) = entry_path.file_name().and_then(|n| n.to_str()) {
                    if parse_tmdb_id_from_folder_name(dir_name).is_some() {
                        result.push(entry_path.clone());
                    }
                }
                // Recursively search subdirectories
                find_tv_show_dirs(&entry_path, result);
            }
        }
    }
}

/// Check if a file is a video file.
pub fn is_video_file(path: PathBuf) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v")
}

/// Parse TMDB ID from folder name.
pub fn parse_tmdb_id_from_folder_name(folder_name: &str) -> Option<u32> {
    // Look for patterns like "tmdb123456"
    let re = regex::Regex::new(r"tmdb(\d+)").unwrap();
    if let Some(captures) = re.captures(folder_name) {
        captures[1].parse().ok()
    } else {
        None
    }
}

/// Extract season number from directory name.
pub fn extract_season_from_dirname(dir_name: &str) -> Option<u32> {
    let re = regex::Regex::new(r"(?i)season\s*(\d+)").unwrap();
    if let Some(captures) = re.captures(dir_name) {
        captures[1].parse().ok()
    } else {
        None
    }
}

/// Download a file from URL to path.
pub async fn download_file(url: &str, path: &Path, proxy_enabled: bool, proxy: &Option<String>) -> Result<()> {
    let mut client_builder = reqwest::Client::builder();
    
    if proxy_enabled {
        if let Some(proxy_url) = proxy {
            if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                client_builder = client_builder.proxy(proxy);
            }
        }
    }
    
    let client = client_builder.build().unwrap_or_else(|_| reqwest::Client::new());
    
    let response = client.get(url).send().await.context("Failed to fetch URL")?;
    let bytes = response.bytes().await.context("Failed to read response bytes")?;
    fs::write(path, bytes).context("Failed to write file")?;
    Ok(())
}

/// Download a file from URL to path and return the size in bytes.
pub async fn download_file_with_size(url: &str, path: &Path, proxy_enabled: bool, proxy: &Option<String>) -> Result<u64> {
    let mut client_builder = reqwest::Client::builder();
    
    if proxy_enabled {
        if let Some(proxy_url) = proxy {
            if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                client_builder = client_builder.proxy(proxy);
            }
        }
    }
    
    let client = client_builder.build().unwrap_or_else(|_| reqwest::Client::new());
    
    let response = client.get(url).send().await.context("Failed to fetch URL")?;
    let bytes = response.bytes().await.context("Failed to read response bytes")?;
    let size = bytes.len() as u64;
    fs::write(path, bytes).context("Failed to write file")?;
    Ok(size)
}

/// Format byte size to human readable string.
pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        let kb = bytes / 1024;
        let rem = bytes % 1024;
        let frac = (rem * 100) / 1024;
        format!("{}.{:02} KB", kb, frac)
    } else if bytes < 1024 * 1024 * 1024 {
        let mb = bytes / (1024 * 1024);
        let rem = bytes % (1024 * 1024);
        let frac = (rem * 100) / (1024 * 1024);
        format!("{}.{:02} MB", mb, frac)
    } else {
        let gb = bytes / (1024 * 1024 * 1024);
        let rem = bytes % (1024 * 1024 * 1024);
        let frac = (rem * 100) / (1024 * 1024 * 1024);
        format!("{}.{:02} GB", gb, frac)
    }
}
