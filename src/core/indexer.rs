//! Central index management - scanning, building, and searching.

use crate::models::index::{CentralIndex, CollectionInfo, DiskIndex, MovieEntry, TvSeriesEntry, VideoFileInfo};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use sha2::{Sha256, Digest};

/// Calculate a content hash for a directory based on NFO files.
/// This is used to detect if the directory content has changed.
pub fn calculate_directory_hash(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    
    let mut nfo_files: Vec<PathBuf> = WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_lowercase() == "nfo")
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();
    
    // Sort files by path to ensure consistent hash
    nfo_files.sort();
    
    for nfo_path in nfo_files {
        if let Ok(metadata) = nfo_path.metadata() {
            // Include filename, modification time, and size in hash
            hasher.update(nfo_path.to_string_lossy().as_bytes());
            if let Ok(mtime) = metadata.modified() {
                hasher.update(mtime.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs().to_string().as_bytes());
            }
            hasher.update(metadata.len().to_string().as_bytes());
        }
    }
    
    let hash = hasher.finalize();
    Ok(hex::encode(hash))
}

/// Configuration directory path.
fn config_dir() -> Result<PathBuf> {
    let config = dirs::config_dir()
        .context("Failed to get config directory")?
        .join("media_organizer");
    Ok(config)
}

/// Path to central index file.
pub fn central_index_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("central_index.json"))
}

/// Path to disk indexes directory.
pub fn disk_indexes_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("disk_indexes"))
}

/// Load central index from disk.
pub fn load_central_index() -> Result<CentralIndex> {
    let path = central_index_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read central index: {}", path.display()))?;
        let index: CentralIndex =
            serde_json::from_str(&content).with_context(|| "Failed to parse central index")?;
        Ok(index)
    } else {
        Ok(CentralIndex::default())
    }
}

/// Save central index to disk.
pub fn save_central_index(index: &CentralIndex) -> Result<()> {
    let path = central_index_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Backup existing file
    if path.exists() {
        let backup_path = path.with_extension("json.backup");
        fs::copy(&path, &backup_path)?;
    }

    let content = serde_json::to_string_pretty(index)?;
    fs::write(&path, content)?;

    tracing::info!("Central index saved to: {}", path.display());
    Ok(())
}

/// Load disk index for a specific disk.
pub fn load_disk_index(disk_label: &str) -> Result<Option<DiskIndex>> {
    let path = disk_indexes_dir()?.join(format!("{}.json", disk_label));
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let index: DiskIndex = serde_json::from_str(&content)?;
        Ok(Some(index))
    } else {
        Ok(None)
    }
}

/// Save disk index to disk.
pub fn save_disk_index(index: &DiskIndex) -> Result<()> {
    let dir = disk_indexes_dir()?;
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.json", index.disk.label));
    let content = serde_json::to_string_pretty(index)?;
    fs::write(&path, content)?;

    tracing::info!("Disk index saved to: {}", path.display());
    Ok(())
}

/// Detect disk label from mount path.
///
/// For paths like `/run/media/johnny/JMedia_M05/Movies`, returns "JMedia_M05".
pub fn detect_disk_label(path: &Path) -> Option<String> {
    let path_str = path.to_string_lossy();

    // Pattern: /run/media/<user>/<label>/...
    if path_str.starts_with("/run/media/") {
        let parts: Vec<&str> = path_str.split('/').collect();
        if parts.len() >= 5 {
            return Some(parts[4].to_string());
        }
    }

    // Pattern: /media/<user>/<label>/...
    if path_str.starts_with("/media/") {
        let parts: Vec<&str> = path_str.split('/').collect();
        if parts.len() >= 4 {
            return Some(parts[3].to_string());
        }
    }

    // Pattern: /mnt/<label>/...
    if path_str.starts_with("/mnt/") {
        let parts: Vec<&str> = path_str.split('/').collect();
        if parts.len() >= 3 {
            return Some(parts[2].to_string());
        }
    }

    // Fallback: use directory name
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Get disk UUID using lsblk command.
pub fn get_disk_uuid(path: &Path) -> Option<String> {
    // Try to get UUID using df and blkid
    let output = std::process::Command::new("df").arg(path).output().ok()?;

    let df_output = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = df_output.lines().collect();
    if lines.len() < 2 {
        return None;
    }

    let device = lines[1].split_whitespace().next()?;

    let blkid_output = std::process::Command::new("blkid")
        .arg("-s")
        .arg("UUID")
        .arg("-o")
        .arg("value")
        .arg(device)
        .output()
        .ok()?;

    let uuid = String::from_utf8_lossy(&blkid_output.stdout)
        .trim()
        .to_string();

    if uuid.is_empty() {
        None
    } else {
        Some(uuid)
    }
}

/// Check if a disk is currently mounted/online.
pub fn is_disk_online(disk_label: &str) -> bool {
    // Check common mount points
    let paths = [
        format!("/run/media/{}/{}", whoami::username(), disk_label),
        format!("/media/{}/{}", whoami::username(), disk_label),
        format!("/mnt/{}", disk_label),
    ];

    for path in &paths {
        if Path::new(path).exists() {
            return true;
        }
    }

    false
}

/// Scan a directory for NFO files and build index entries.
/// 
/// Supports idempotency: if the content hash hasn't changed since last scan,
/// returns the existing index without re-scanning.
pub fn scan_directory(
    path: &Path,
    disk_label: &str,
    disk_uuid: Option<String>,
    media_type: &str,
    force: bool,
) -> Result<(DiskIndex, bool)> {
    tracing::info!("Scanning directory: {}", path.display());

    // Calculate content hash for idempotency check
    let content_hash = calculate_directory_hash(path)?;
    tracing::debug!("Directory content hash: {}", content_hash);

    // Check if we have an existing index with the same hash (skip if force=true)
    if !force {
        if let Ok(Some(existing)) = load_disk_index(disk_label) {
            if existing.disk.content_hash == content_hash {
                tracing::info!("Content unchanged (hash match), returning cached index");
                return Ok((existing, false));
            }
            tracing::info!("Content changed (hash mismatch), re-scanning...");
        }
    } else {
        tracing::info!("Force re-index requested, skipping hash check");
    }

    let mut index = DiskIndex::default();
    index.disk.label = disk_label.to_string();
    index.disk.uuid = disk_uuid.clone();
    index.disk.base_path = path.to_string_lossy().to_string();
    index.disk.last_indexed = chrono::Utc::now().to_rfc3339();
    index.disk.content_hash = content_hash;

    let mut total_size: u64 = 0;
    
    // Track already processed paths to avoid duplicates
    let mut processed_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Scan all .nfo files recursively
    for entry in WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let entry_path = entry.path();

        if entry_path.is_file() {
            // Check if it's an NFO file by extension
            if let Some(ext) = entry_path.extension() {
                if ext.to_ascii_lowercase() == "nfo" {
                    // Get the parent directory as the unique identifier
                    let nfo_dir = match entry_path.parent() {
                        Some(dir) => dir,
                        None => {
                            tracing::warn!("Skipping NFO without parent directory: {}", entry_path.display());
                            continue;
                        }
                    };
                    
                    let relative_path = nfo_dir
                        .strip_prefix(path)
                        .unwrap_or(nfo_dir)
                        .to_string_lossy()
                        .to_string();
                    
                    // Skip if already processed this path
                    if processed_paths.contains(&relative_path) {
                        tracing::debug!("Skipping duplicate entry: {}", relative_path);
                        continue;
                    }
                    processed_paths.insert(relative_path);
                    
                    match parse_nfo_file(entry_path, disk_label, &index.disk.uuid, path) {
                        Ok(ParsedNfo::Movie(movie)) => {
                            if media_type == "movies" {
                                total_size += movie.size_bytes;
                                index.movies.push(movie);
                            } else {
                                tracing::warn!(
                                    "Skipping movie NFO (type mismatch): {} (expected tv_series)",
                                    entry_path.display()
                                );
                            }
                        }
                        Ok(ParsedNfo::TvSeries(tvshow)) => {
                            if media_type == "tv_series" {
                                total_size += tvshow.size_bytes;
                                index.tv_series.push(tvshow);
                            } else {
                                tracing::warn!(
                                    "Skipping TV show NFO (type mismatch): {} (expected movies)",
                                    entry_path.display()
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse NFO {}: {}", entry_path.display(), e);
                        }
                    }
                }
            }
        }
    }

    // Store path by media type for composite storage support
    index
        .disk
        .paths
        .insert(media_type.to_string(), path.to_string_lossy().to_string());

    index.disk.movie_count = index.movies.len();
    index.disk.tv_series_count = index.tv_series.len();
    index.disk.total_size_bytes = total_size;

    tracing::info!(
        "Scan complete: {} movies, {} TV shows",
        index.movies.len(),
        index.tv_series.len()
    );

    Ok((index, true))
}

/// Parsed NFO result.
enum ParsedNfo {
    Movie(MovieEntry),
    TvSeries(TvSeriesEntry),
}

/// Parse a movie.nfo or tvshow.nfo file.
fn parse_nfo_file(
    nfo_path: &Path,
    disk_label: &str,
    disk_uuid: &Option<String>,
    base_path: &Path,
) -> Result<ParsedNfo> {
    let content = fs::read_to_string(nfo_path)?;
    let nfo_dir = nfo_path.parent().context("NFO has no parent directory")?;

    // Calculate relative path
    let relative_path = nfo_dir
        .strip_prefix(base_path)
        .unwrap_or(nfo_dir)
        .to_string_lossy()
        .to_string();

    // Collect video files and calculate total size
    let (video_files, size_bytes) = collect_video_files(nfo_dir);

    // Determine if movie or tvshow based on root element
    if content.contains("<movie>") {
        let movie = parse_movie_nfo(&content, disk_label, disk_uuid, &relative_path, size_bytes, video_files)?;
        Ok(ParsedNfo::Movie(movie))
    } else if content.contains("<tvshow>") {
        let tvshow = parse_tv_series_nfo(&content, disk_label, disk_uuid, &relative_path, size_bytes, nfo_dir)?;
        Ok(ParsedNfo::TvSeries(tvshow))
    } else {
        anyhow::bail!("Unknown NFO format");
    }
}

/// Parse movie NFO content.
fn parse_movie_nfo(
    content: &str,
    disk_label: &str,
    disk_uuid: &Option<String>,
    relative_path: &str,
    size_bytes: u64,
    video_files: Vec<VideoFileInfo>,
) -> Result<MovieEntry> {
    // Simple XML parsing using regex (for robustness with malformed XML)
    // Use (?s) flag to make . match newlines for multi-line tags
    let get_tag = |tag: &str| -> Option<String> {
        let pattern = format!(r"(?s)<{}>(.*?)</{}>", tag, tag);
        regex::Regex::new(&pattern)
            .ok()?
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
    };

    let get_all_tags = |tag: &str| -> Vec<String> {
        let pattern = format!(r"(?s)<{}>(.*?)</{}>", tag, tag);
        regex::Regex::new(&pattern)
            .map(|re| {
                re.captures_iter(content)
                    .filter_map(|c| c.get(1))
                    .map(|m| m.as_str().trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };

    let title = get_tag("title").unwrap_or_else(|| "Unknown".to_string());
    let original_title = get_tag("originaltitle");
    let year = get_tag("year").and_then(|y| y.parse().ok());

    // TMDB ID from uniqueid or tmdbid tag
    let tmdb_id = get_tag("tmdbid")
        .or_else(|| {
            // Try to find <uniqueid type="tmdb">
            let pattern = r#"<uniqueid[^>]*type="tmdb"[^>]*>(\d+)</uniqueid>"#;
            regex::Regex::new(pattern)
                .ok()?
                .captures(content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        })
        .and_then(|id| id.parse().ok());

    // IMDB ID
    let imdb_id = get_tag("imdbid").or_else(|| {
        let pattern = r#"<uniqueid[^>]*type="imdb"[^>]*>(tt\d+)</uniqueid>"#;
        regex::Regex::new(pattern)
            .ok()?
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
    });

    // Collection info
    let collection_id = get_tag("tmdbcollectionid").and_then(|id| id.parse().ok());
    // First try <set><name>...</name></set> format (nested structure)
    let collection_name = {
        let pattern = r"(?s)<set>\s*<name>(.*?)</name>";
        regex::Regex::new(pattern)
            .ok()
            .and_then(|re| re.captures(content))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .filter(|s| !s.is_empty())
    }
    .or_else(|| {
        // Fallback: simple <set>name</set> format (flat structure)
        get_tag("set").filter(|s| !s.contains('<') && !s.is_empty())
    });

    // Collection total movies (from <set><totalmovies>N</totalmovies></set>)
    let collection_total_movies = {
        let pattern = r"(?s)<set>.*?<totalmovies>(\d+)</totalmovies>";
        regex::Regex::new(pattern)
            .ok()
            .and_then(|re| re.captures(content))
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().trim().parse().ok())
    };

    // Country
    let country = get_tag("country").map(|c| {
        // Convert full country name to code if needed
        country_name_to_code(&c)
    });

    let genres = get_all_tags("genre");
    let actors = get_all_tags("actor")
        .into_iter()
        .flat_map(|a| {
            // Try to extract name from <actor><name>...</name></actor>
            let pattern = r"<name>(.*?)</name>";
            regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(&a))
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .or(Some(a))
        })
        .collect();

    let directors = get_all_tags("director");
    let runtime = get_tag("runtime").and_then(|r| r.parse().ok());
    let rating = get_tag("rating").and_then(|r| r.parse().ok());

    // Resolution from video info or filename
    let resolution = get_tag("resolution");

    Ok(MovieEntry {
        id: uuid::Uuid::new_v4().to_string(),
        disk: disk_label.to_string(),
        disk_uuid: disk_uuid.clone(),
        relative_path: relative_path.to_string(),
        title,
        original_title,
        year,
        tmdb_id,
        imdb_id,
        collection_id,
        collection_name,
        collection_total_movies,
        country,
        genres,
        actors,
        directors,
        runtime,
        rating,
        size_bytes,
        resolution,
        video_files,
        indexed_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Parse tvshow NFO content.
fn parse_tv_series_nfo(
    content: &str,
    disk_label: &str,
    disk_uuid: &Option<String>,
    relative_path: &str,
    size_bytes: u64,
    tvshow_dir: &Path,
) -> Result<TvSeriesEntry> {
    // Use (?s) flag to make . match newlines for multi-line tags
    let get_tag = |tag: &str| -> Option<String> {
        let pattern = format!(r"(?s)<{}>(.*?)</{}>", tag, tag);
        regex::Regex::new(&pattern)
            .ok()?
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
    };

    let get_all_tags = |tag: &str| -> Vec<String> {
        let pattern = format!(r"(?s)<{}>(.*?)</{}>", tag, tag);
        regex::Regex::new(&pattern)
            .map(|re| {
                re.captures_iter(content)
                    .filter_map(|c| c.get(1))
                    .map(|m| m.as_str().trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };

    let title = get_tag("title").unwrap_or_else(|| "Unknown".to_string());
    let original_title = get_tag("originaltitle");
    let year = get_tag("year")
        .or_else(|| get_tag("premiered").map(|p| p[..4].to_string()))
        .and_then(|y| y.parse().ok());

    let tmdb_id = get_tag("tmdbid")
        .or_else(|| {
            let pattern = r#"<uniqueid[^>]*type="tmdb"[^>]*>(\d+)</uniqueid>"#;
            regex::Regex::new(pattern)
                .ok()?
                .captures(content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        })
        .and_then(|id| id.parse().ok());

    let imdb_id = get_tag("imdbid").or_else(|| {
        let pattern = r#"<uniqueid[^>]*type="imdb"[^>]*>(tt\d+)</uniqueid>"#;
        regex::Regex::new(pattern)
            .ok()?
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
    });

    let country = get_tag("country").map(|c| country_name_to_code(&c));
    let genres = get_all_tags("genre");

    let actors: Vec<String> = get_all_tags("actor")
        .into_iter()
        .flat_map(|a| {
            let pattern = r"<name>(.*?)</name>";
            regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(&a))
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .or(Some(a))
        })
        .collect();

    let seasons = get_tag("season").and_then(|s| s.parse().ok()).unwrap_or(1);
    let episodes = get_tag("episode").and_then(|e| e.parse().ok()).unwrap_or(0);

    // Calculate owned seasons: count season directories like "Season 01", "Season 1", etc.
    let owned_seasons = count_owned_seasons(tvshow_dir);
    let owned_episodes = count_owned_episodes(tvshow_dir);

    tracing::debug!(
        "Parsed TV show '{}': seasons={}, episodes={}, owned_seasons={}, owned_episodes={}, tvshow_dir={}",
        title, seasons, episodes, owned_seasons, owned_episodes, tvshow_dir.display()
    );

    Ok(TvSeriesEntry {
        id: uuid::Uuid::new_v4().to_string(),
        disk: disk_label.to_string(),
        disk_uuid: disk_uuid.clone(),
        relative_path: relative_path.to_string(),
        title,
        original_title,
        year,
        tmdb_id,
        imdb_id,
        country,
        genres,
        actors,
        seasons,
        episodes,
        owned_seasons,
        owned_episodes,
        size_bytes,
        indexed_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Collect video file information from a directory.
fn collect_video_files(dir: &Path) -> (Vec<VideoFileInfo>, u64) {
    let video_extensions = [
        "mkv", "mp4", "avi", "mov", "wmv", "m4v", "ts", "m2ts", "flv", "webm",
        "mpg", "mpeg", "vob", "ogv", "ogm", "divx", "xvid", "3gp", "3g2", "mts", "rm", "rmvb", "asf",
        "f4v",
    ];

    let mut video_files = Vec::new();
    let mut total_size: u64 = 0;

    for entry in WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().is_file() {
            if let Some(ext) = entry.path().extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if video_extensions.contains(&ext_str.as_str()) {
                    if let Ok(metadata) = entry.metadata() {
                        let file_size = metadata.len();
                        total_size += file_size;
                        
                        let file_name = entry.file_name().to_string_lossy().to_string();
                        let file_path = entry.path().to_string_lossy().to_string();
                        
                        let format = Some(ext_str.clone());
                        
                        let resolution = detect_resolution_from_filename(&file_name);
                        let codec = detect_codec_from_filename(&file_name);
                        
                        video_files.push(VideoFileInfo {
                            file_name,
                            file_path,
                            size_bytes: file_size,
                            resolution,
                            format,
                            codec,
                        });
                    }
                }
            }
        }
    }

    (video_files, total_size)
}

/// Detect resolution from filename.
fn detect_resolution_from_filename(filename: &str) -> Option<String> {
    let patterns = [
        ("2160", "4K"),
        ("3840", "4K"),
        ("1080", "1080p"),
        ("1920", "1080p"),
        ("720", "720p"),
        ("1280", "720p"),
        ("480", "480p"),
        ("576", "576p"),
        ("4K", "4K"),
        ("8K", "8K"),
    ];
    
    for (pattern, resolution) in patterns.iter() {
        if filename.to_lowercase().contains(*pattern) {
            return Some(resolution.to_string());
        }
    }
    None
}

/// Detect video codec from filename.
fn detect_codec_from_filename(filename: &str) -> Option<String> {
    let filename_lower = filename.to_lowercase();
    let codecs = [
        ("hevc", "HEVC"),
        ("h265", "H.265"),
        ("h.265", "H.265"),
        ("av1", "AV1"),
        ("vp9", "VP9"),
        ("vp10", "VP10"),
        ("x264", "H.264"),
        ("x265", "H.265"),
        ("h264", "H.264"),
        ("h.264", "H.264"),
    ];
    
    for (pattern, codec) in codecs.iter() {
        if filename_lower.contains(*pattern) {
            return Some(codec.to_string());
        }
    }
    None
}

/// Count number of owned seasons by counting season directories like "Season 01", "Season 1", etc.
fn count_owned_seasons(tvshow_dir: &Path) -> u16 {
    let season_patterns = [
        regex::Regex::new(r"^Season\s+(\d+)$").ok(),
        regex::Regex::new(r"^Season\s*(\d+)$").ok(),
        regex::Regex::new(r"^S(\d+)$").ok(),
        regex::Regex::new(r"^s(\d+)$").ok(),
    ];

    let mut season_numbers = std::collections::HashSet::new();

    if let Ok(entries) = std::fs::read_dir(tvshow_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    for pattern in &season_patterns {
                        if let Some(p) = pattern {
                            if let Some(captures) = p.captures(name) {
                                if let Some(num_str) = captures.get(1) {
                                    if let Ok(num) = num_str.as_str().parse::<u16>() {
                                        season_numbers.insert(num);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    season_numbers.len() as u16
}

/// Count total number of owned episodes by counting video files.
fn count_owned_episodes(tvshow_dir: &Path) -> u32 {
    let video_extensions = [
        "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts",
    ];

    WalkDir::new(tvshow_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| video_extensions.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .count() as u32
}

/// Convert country name to ISO 3166-1 alpha-2 code.
fn country_name_to_code(name: &str) -> String {
    let name_lower = name.to_lowercase();
    match name_lower.as_str() {
        "united states" | "usa" | "united states of america" => "US".to_string(),
        "china" | "中国" => "CN".to_string(),
        "united kingdom" | "uk" | "great britain" => "GB".to_string(),
        "japan" | "日本" => "JP".to_string(),
        "korea" | "south korea" | "韩国" => "KR".to_string(),
        "france" | "法国" => "FR".to_string(),
        "germany" | "德国" => "DE".to_string(),
        "india" | "印度" => "IN".to_string(),
        "italy" | "意大利" => "IT".to_string(),
        "spain" | "西班牙" => "ES".to_string(),
        "canada" | "加拿大" => "CA".to_string(),
        "australia" | "澳大利亚" => "AU".to_string(),
        "russia" | "俄罗斯" => "RU".to_string(),
        "hong kong" | "香港" => "HK".to_string(),
        "taiwan" | "台湾" => "TW".to_string(),
        // Unknown country name or already a 2-letter code - return as-is
        _ => name.to_string(),
    }
}

/// Merge disk index into central index.
///
/// Supports composite storage: if a disk already exists in the central index,
/// the new scan is merged by media type instead of completely replacing it.
/// This allows one disk label to have both movies and tv_series with different paths.
pub fn merge_disk_into_central(central: &mut CentralIndex, disk: DiskIndex) {
    let label = disk.disk.label.clone();

    // Determine what media types are being added in this scan
    let has_movies = !disk.movies.is_empty();
    let has_tv_series = !disk.tv_series.is_empty();

    // Update or merge disk info
    if let Some(existing_disk) = central.disks.get_mut(&label) {
        // Merge: keep existing paths, add new ones
        for (media_type, path) in &disk.disk.paths {
            existing_disk.paths.insert(media_type.clone(), path.clone());
        }
        // Update timestamp
        existing_disk.last_indexed = disk.disk.last_indexed.clone();
        // Update UUID if provided
        if disk.disk.uuid.is_some() {
            existing_disk.uuid = disk.disk.uuid.clone();
        }

        tracing::info!(
            "Merging into existing disk '{}': movies={}, tv_series={}",
            label,
            has_movies,
            has_tv_series
        );
    } else {
        // New disk: insert directly
        central.disks.insert(label.clone(), disk.disk.clone());
        tracing::info!(
            "Adding new disk '{}': movies={}, tv_series={}",
            label,
            has_movies,
            has_tv_series
        );
    }

    // Remove old entries ONLY for the media types being updated
    // This is the key change: we don't remove all entries, just the ones being replaced
    if has_movies {
        central.movies.retain(|m| m.disk != label);
        central.movies.extend(disk.movies);
    }

    if has_tv_series {
        central.tv_series.retain(|t| t.disk != label);
        central.tv_series.extend(disk.tv_series);
    }

    // Update disk counts in the disk info
    if let Some(disk_info) = central.disks.get_mut(&label) {
        disk_info.movie_count = central.movies.iter().filter(|m| m.disk == label).count();
        disk_info.tv_series_count = central.tv_series.iter().filter(|t| t.disk == label).count();
        disk_info.total_size_bytes = central
            .movies
            .iter()
            .filter(|m| m.disk == label)
            .map(|m| m.size_bytes)
            .sum::<u64>()
            + central
                .tv_series
                .iter()
                .filter(|t| t.disk == label)
                .map(|t| t.size_bytes)
                .sum::<u64>();
    }

    // Rebuild indexes and update statistics
    central.rebuild_indexes();
    central.update_statistics();
    central.updated_at = chrono::Utc::now().to_rfc3339();
}

/// Search results container.
#[derive(Debug)]
pub struct SearchResults {
    pub movies: Vec<MovieEntry>,
    pub tv_series: Vec<TvSeriesEntry>,
    pub collections: Vec<CollectionInfo>,
}

/// Search the central index.
#[allow(clippy::too_many_arguments)]
pub fn search(
    index: &CentralIndex,
    title: Option<&str>,
    actor: Option<&str>,
    director: Option<&str>,
    collection: Option<&str>,
    year: Option<u16>,
    year_range: Option<(u16, u16)>,
    genre: Option<&str>,
    country: Option<&str>,
) -> SearchResults {
    let mut movie_ids: Option<std::collections::HashSet<String>> = None;
    let mut tv_series_ids: Option<std::collections::HashSet<String>> = None;

    // Helper to intersect sets
    fn intersect(
        existing: &mut Option<std::collections::HashSet<String>>,
        new: std::collections::HashSet<String>,
    ) {
        match existing {
            Some(set) => {
                set.retain(|id| new.contains(id));
            }
            None => {
                *existing = Some(new);
            }
        }
    }

    // Search by actor
    if let Some(actor_name) = actor {
        let actor_lower = actor_name.to_lowercase();
        let ids: std::collections::HashSet<String> = index
            .indexes
            .by_actor
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&actor_lower))
            .flat_map(|(_, ids)| ids.clone())
            .collect();
        intersect(&mut movie_ids, ids.clone());
        intersect(&mut tv_series_ids, ids);
    }

    // Search by director
    if let Some(director_name) = director {
        let director_lower = director_name.to_lowercase();
        let ids: std::collections::HashSet<String> = index
            .indexes
            .by_director
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&director_lower))
            .flat_map(|(_, ids)| ids.clone())
            .collect();
        intersect(&mut movie_ids, ids);
    }

    // Search by genre
    if let Some(genre_name) = genre {
        let genre_lower = genre_name.to_lowercase();
        let ids: std::collections::HashSet<String> = index
            .indexes
            .by_genre
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&genre_lower))
            .flat_map(|(_, ids)| ids.clone())
            .collect();
        intersect(&mut movie_ids, ids.clone());
        intersect(&mut tv_series_ids, ids);
    }

    // Search by country
    if let Some(country_code) = country {
        let country_upper = country_code.to_uppercase();
        if let Some(ids) = index.indexes.by_country.get(&country_upper) {
            let id_set: std::collections::HashSet<String> = ids.iter().cloned().collect();
            intersect(&mut movie_ids, id_set.clone());
            intersect(&mut tv_series_ids, id_set);
        } else {
            movie_ids = Some(std::collections::HashSet::new());
            tv_series_ids = Some(std::collections::HashSet::new());
        }
    }

    // Search by year or year range
    if let Some(y) = year {
        if let Some(ids) = index.indexes.by_year.get(&y) {
            let id_set: std::collections::HashSet<String> = ids.iter().cloned().collect();
            intersect(&mut movie_ids, id_set.clone());
            intersect(&mut tv_series_ids, id_set);
        } else {
            movie_ids = Some(std::collections::HashSet::new());
            tv_series_ids = Some(std::collections::HashSet::new());
        }
    } else if let Some((start, end)) = year_range {
        let ids: std::collections::HashSet<String> = (start..=end)
            .flat_map(|y| index.indexes.by_year.get(&y).cloned().unwrap_or_default())
            .collect();
        intersect(&mut movie_ids, ids.clone());
        intersect(&mut tv_series_ids, ids);
    }

    // Get movies
    let mut movies: Vec<MovieEntry> = if let Some(ref ids) = movie_ids {
        index
            .movies
            .iter()
            .filter(|m| ids.contains(&m.id))
            .cloned()
            .collect()
    } else {
        index.movies.clone()
    };

    // Get TV shows
    let mut tv_series: Vec<TvSeriesEntry> = if let Some(ref ids) = tv_series_ids {
        index
            .tv_series
            .iter()
            .filter(|t| ids.contains(&t.id))
            .cloned()
            .collect()
    } else {
        index.tv_series.clone()
    };

    // Filter by title
    if let Some(title_query) = title {
        let query_lower = title_query.to_lowercase();
        movies.retain(|m| {
            m.title.to_lowercase().contains(&query_lower)
                || m.original_title
                    .as_ref()
                    .map(|t| t.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
        });
        tv_series.retain(|t| {
            t.title.to_lowercase().contains(&query_lower)
                || t.original_title
                    .as_ref()
                    .map(|title| title.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
        });
    }

    // Sort by year descending
    movies.sort_by(|a, b| b.year.cmp(&a.year));
    tv_series.sort_by(|a, b| b.year.cmp(&a.year));

    // Search collections
    let collections: Vec<CollectionInfo> = if let Some(collection_query) = collection {
        let query_lower = collection_query.to_lowercase();
        index
            .collections
            .values()
            .filter(|c| c.name.to_lowercase().contains(&query_lower))
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    SearchResults {
        movies,
        tv_series,
        collections,
    }
}

// Unit tests moved to tests/indexer_tests.rs for better code organization
