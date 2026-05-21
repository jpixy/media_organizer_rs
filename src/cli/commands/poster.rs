//! Poster download command implementation.

use anyhow::{Context, Result};
use crate::models::config::Config;
use crate::services::tmdb::{TmdbClient, TmdbConfig};
use std::fs;
use std::path::{Path, PathBuf};

/// Download posters for movies.
/// 
/// Scans the directory recursively for movie folders (identified by tmdb ID in folder name),
/// and downloads posters. If a poster already exists, it will be skipped.
pub async fn download_movie_posters(path: &Path, config: &Config) -> Result<()> {
    println!("Downloading movie posters for: {}", path.display());
    println!("Using poster size from config: {}", config.organize.poster_size);
    
    let api_key = config.tmdb.api_key.clone().ok_or_else(|| anyhow::anyhow!("TMDB API key not configured"))?;
    let tmdb_config = TmdbConfig {
        api_key,
        language: config.tmdb.language.clone(),
        use_bearer: false,
    };
    let tmdb = TmdbClient::new(tmdb_config);
    
    let mut downloaded_count = 0;
    let mut skipped_count = 0;
    
    if !path.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
    }
    
    // Recursively find all directories containing tmdb ID (movies)
    let mut movie_dirs = Vec::new();
    find_movie_dirs(path, &mut movie_dirs);
    
    if movie_dirs.is_empty() {
        tracing::warn!("No movie folders found (containing tmdb ID) in: {}", path.display());
        println!();
        println!("Download completed!");
        println!("  Downloaded: {} posters", downloaded_count);
        println!("  Skipped: {} posters (already exist)", skipped_count);
        return Ok(());
    }
    
    println!("Found {} movie folders", movie_dirs.len());
    
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
        let folder_name = entry_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        
        // Try to parse tmdb id from folder name
        let tmdb_id = parse_tmdb_id_from_folder_name(folder_name);
        
        if tmdb_id.is_none() {
            tracing::warn!("Could not parse TMDB ID from folder: {}", folder_name);
            continue;
        }
        
        let tmdb_id = tmdb_id.unwrap() as u64;
        
        // Fetch movie details from TMDB
        let movie_details = match tmdb.get_movie_details(tmdb_id).await {
            Ok(details) => details,
            Err(e) => {
                tracing::warn!("Failed to fetch movie details for tmdb{}: {}", tmdb_id, e);
                continue;
            }
        };
        
        // Get poster URL
        let poster_path = match movie_details.poster_path {
            Some(p) => p,
            None => {
                tracing::info!("No poster available for: {}", folder_name);
                continue;
            }
        };
        
        let poster_url = format!("https://image.tmdb.org/t/p/{}{}", config.organize.poster_size, poster_path);
        
        // Get the first video file to derive poster name
        let video_path = &video_files[0];
        let poster_name = format!("{}.jpg", video_path.file_stem().unwrap_or_default().to_string_lossy());
        let poster_path = entry_path.join(&poster_name);
        
        // Skip if poster already exists
        if poster_path.exists() {
            tracing::info!("Poster already exists, skipping: {}", poster_path.display());
            skipped_count += 1;
            continue;
        }
        
        // Download poster
        match download_file(&poster_url, &poster_path).await {
            Ok(_) => {
                tracing::info!("Downloaded poster: {}", poster_path.display());
                downloaded_count += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to download poster for {}: {}", folder_name, e);
            }
        }
    }
    
    println!();
    println!("Download completed!");
    println!("  Downloaded: {} posters", downloaded_count);
    println!("  Skipped: {} posters (already exist)", skipped_count);
    
    Ok(())
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
        api_key,
        language: config.tmdb.language.clone(),
        use_bearer: false,
    };
    let tmdb = TmdbClient::new(tmdb_config);
    
    let mut downloaded_count = 0;
    let mut skipped_count = 0;
    
    if !path.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
    }
    
    // Recursively find all directories containing tmdb ID (TV series)
    let mut tv_show_dirs = Vec::new();
    find_tv_show_dirs(path, &mut tv_show_dirs);
    
    if tv_show_dirs.is_empty() {
        tracing::warn!("No TV series folders found (containing tmdb ID) in: {}", path.display());
        println!();
        println!("Download completed!");
        println!("  Downloaded: {} season posters", downloaded_count);
        println!("  Skipped: {} season posters (already exist)", skipped_count);
        return Ok(());
    }
    
    for entry_path in tv_show_dirs {
        // Get TV show info
        let folder_name = entry_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let tmdb_id = parse_tmdb_id_from_folder_name(folder_name);
        
        if tmdb_id.is_none() {
            tracing::warn!("Could not parse TMDB ID from folder: {}", folder_name);
            continue;
        }
        
        let tmdb_id = tmdb_id.unwrap() as u64;
        
        // Get show details to know how many seasons
        let show_details = match tmdb.get_tv_details(tmdb_id).await {
            Ok(details) => details,
            Err(e) => {
                tracing::warn!("Failed to fetch TV show details for tmdb{}: {}", tmdb_id, e);
                continue;
            }
        };
        
        // Get season directories or iterate through all seasons
        let mut season_dirs = Vec::new();
        if let Ok(entries) = fs::read_dir(&entry_path) {
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
            for season_num in 1..=show_details.number_of_seasons as u32 {
                season_dirs.push((season_num, entry_path.join(format!("Season {:02}", season_num))));
            }
        }
        
        // Process each season
        for (season_num, season_path) in season_dirs {
            // Create season directory if it doesn't exist
            if !season_path.exists() {
                fs::create_dir_all(&season_path)?;
            }
            
            // Fetch season details
            let season_details = match tmdb.get_season_details(tmdb_id, season_num as u16).await {
                Ok(details) => details,
                Err(e) => {
                    tracing::warn!("Failed to fetch season {} details for tmdb{}: {}", season_num, tmdb_id, e);
                    continue;
                }
            };
            
            // Get poster URL
            let poster_path = match season_details.poster_path {
                Some(p) => p,
                None => {
                    tracing::info!("No poster available for {} - Season {}", folder_name, season_num);
                    continue;
                }
            };
            
            let poster_url = format!("https://image.tmdb.org/t/p/{}{}", config.organize.poster_size, poster_path);
            
            // Season poster name: [TV名称]-seasonXX.jpg (same naming as NFO)
            let show_title = folder_name.split('-').next().unwrap_or(folder_name).trim().trim_matches(|c| c == '[' || c == ']');
            let poster_name = format!("[{}]-season{:02}.jpg", show_title, season_num);
            let poster_full_path = season_path.join(&poster_name);
            
            // Skip if poster already exists
            if poster_full_path.exists() {
                tracing::info!("Poster already exists, skipping: {}", poster_full_path.display());
                skipped_count += 1;
                continue;
            }
            
            // Download poster
            match download_file(&poster_url, &poster_full_path).await {
                Ok(_) => {
                    tracing::info!("Downloaded poster: {}", poster_full_path.display());
                    downloaded_count += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to download poster for {} Season {}: {}", folder_name, season_num, e);
                }
            }
        }
    }
    
    println!();
    println!("Download completed!");
    println!("  Downloaded: {} season posters", downloaded_count);
    println!("  Skipped: {} season posters (already exist)", skipped_count);
    
    Ok(())
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
pub async fn download_file(url: &str, path: &Path) -> Result<()> {
    let response = reqwest::get(url).await.context("Failed to fetch URL")?;
    let bytes = response.bytes().await.context("Failed to read response bytes")?;
    fs::write(path, bytes).context("Failed to write file")?;
    Ok(())
}
