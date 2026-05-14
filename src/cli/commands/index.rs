//! Index command implementation.

use crate::cli::args::IndexAction;
use crate::core::indexer;
use crate::models::config::Config;
use crate::models::index::{VolumeGroupInfo, MovieEntry, TvSeriesEntry};
use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;

/// Execute index subcommand.
pub async fn execute_index(action: IndexAction, config: &Config) -> Result<()> {
    match action {
        IndexAction::Scan {
            path,
            media_type,
            volume_label,
            force,
        } => scan_directory(&path, &media_type, volume_label, force).await,
        IndexAction::Stats => show_stats().await,
        IndexAction::List {
            volume_label,
            media_type,
        } => list_volume(&volume_label, &media_type).await,
        IndexAction::Verify { path } => verify_index(&path).await,
        IndexAction::Remove {
            volume_label,
            confirm,
        } => remove_volume(&volume_label, confirm).await,
        IndexAction::Duplicates { media_type, format } => {
            find_duplicates(&media_type, &format).await
        }
        IndexAction::Collections {
            filter,
            format,
            hide_paths,
            update,
        } => {
            if update {
                update_collections(config).await?;
            }
            list_collections(&filter, &format, hide_paths).await
        }
        IndexAction::Tv {
            filter,
            format,
            hide_paths,
            update,
        } => {
            if update {
                update_tv(config).await?;
            }
            list_tv(&filter, &format, hide_paths).await
        }
        IndexAction::Rebuild { skip_preflight: _ } => rebuild_index().await,
    }
}

/// Scan and index a directory.
async fn scan_directory(
    path: &Path,
    media_type: &str,
    volume_label: Option<String>,
    force: bool,
) -> Result<()> {
    println!("{}", "[INDEX] Scanning directory...".bold().cyan());
    println!("  Path: {}", path.display());
    println!("  Media type: {}", media_type);

    // Detect or use provided volume label
    let label = volume_label.unwrap_or_else(|| {
        indexer::detect_disk_label(path).unwrap_or_else(|| "unknown".to_string())
    });
    println!("  Volume: {}", label);

    // Get disk UUID
    let uuid = indexer::get_disk_uuid(path);
    if let Some(ref u) = uuid {
        println!("  Disk UUID: {}", u);
    }

    // Check if already indexed
    if !force {
        if let Ok(Some(existing)) = indexer::load_disk_index(&label) {
            println!(
                "{}",
                format!(
                    "[WARN] Volume '{}' already indexed ({} movies, {} TV shows)",
                    label, existing.disk.movie_count, existing.disk.tv_series_count
                )
                .yellow()
            );
            println!("  Use --force to re-index");
        }
    }

    println!();

    // Scan directory
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message("Scanning for NFO files...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let (disk_index, was_updated) = indexer::scan_directory(path, &label, uuid, media_type, force)?;

    pb.finish_with_message(if was_updated { "Scan complete" } else { "Content unchanged - using cached index" });

    if was_updated {
        // Save disk index only if content changed
        indexer::save_disk_index(&disk_index)?;

        // Update central index
        let mut central = indexer::load_central_index()?;
        indexer::merge_disk_into_central(&mut central, disk_index.clone());
        indexer::save_central_index(&central)?;

        // Print summary
        println!();
        println!("{}", "[INDEX] Complete!".bold().green());
        println!("  Movies indexed: {}", disk_index.disk.movie_count);
        println!("  TV shows indexed: {}", disk_index.disk.tv_series_count);
        println!(
            "  Total size: {:.2} GB",
            disk_index.disk.total_size_bytes as f64 / 1_073_741_824.0
        );
        println!();
        println!(
            "  Central index: {} movies, {} TV shows across {} disks",
            central.statistics.total_movies,
            central.statistics.total_tv_series,
            central.statistics.total_disks
        );
    } else {
        // Content unchanged, just show the cached results
        println!();
        println!("{}", "[INDEX] Content unchanged.".bold().blue());
        println!("  Using cached index ({} movies, {} TV shows)", disk_index.disk.movie_count, disk_index.disk.tv_series_count);
    }

    Ok(())
}

/// Rebuild indexes and recalculate all statistics.
async fn rebuild_index() -> Result<()> {
    println!("{}", "[INDEX] Rebuilding indexes and recalculating statistics...".bold().cyan());

    let mut index = indexer::load_central_index()?;

    let before_movies = index.movies.len();
    let before_tv = index.tv_series.len();
    let before_collections = index.collections.len();

    println!("  Before: {} movies, {} TV shows, {} collections", before_movies, before_tv, before_collections);

    // Rebuild all indexes (including collections)
    index.rebuild_indexes();
    index.update_statistics();

    // Save the updated index
    indexer::save_central_index(&index)?;

    let after_collections = index.collections.len();
    let after_complete = index.statistics.complete_collections;
    let after_incomplete = index.statistics.incomplete_collections;
    let after_tv_complete = index.statistics.complete_tv_series;
    let after_tv_incomplete = index.statistics.incomplete_tv_series;

    println!();
    println!("{}", "[INDEX] Rebuild complete!".bold().green());
    println!("  Collections: {} total ({} complete, {} incomplete)", after_collections, after_complete, after_incomplete);
    println!("  TV Series: {} total ({} complete, {} incomplete)", after_tv_complete + after_tv_incomplete, after_tv_complete, after_tv_incomplete);

    Ok(())
}

/// Show collection statistics.
async fn show_stats() -> Result<()> {
    let mut index = indexer::load_central_index()?;
    // Ensure statistics are up-to-date (in case collection info was updated)
    index.update_statistics();

    println!("{}", "Media Collection Statistics".bold().cyan());
    println!("{}", "=".repeat(50));
    println!();

    // Volume Groups (formerly Disks)
    println!("{}", "Volume Groups:".bold());
    for (label, disk) in &index.disks {
        println!(
            "  {} | {} movies | {} TV shows | {:.1} GB",
            label.bold(),
            disk.movie_count,
            disk.tv_series_count,
            disk.total_size_bytes as f64 / 1_073_741_824.0,
        );
        // Show primary path (the path where media was indexed from)
        if !disk.base_path.is_empty() {
            println!("      Path: {}", disk.base_path.dimmed());
        } else if !disk.paths.is_empty() {
            // Show first path as primary location
            if let Some((_, path)) = disk.paths.iter().next() {
                println!("      Path: {}", path.dimmed());
            }
        }
    }
    println!("{}", "-".repeat(50));
    println!(
        "  {} | {} movies | {} TV shows | {:.1} GB",
        "Total".bold(),
        index.statistics.total_movies,
        index.statistics.total_tv_series,
        index.statistics.total_size_bytes as f64 / 1_073_741_824.0
    );
    println!();

    // Collections
    println!("{}", "Collections:".bold());
    println!(
        "  Complete: {} collections",
        index.statistics.complete_collections
    );
    println!(
        "  Incomplete: {} collections",
        index.statistics.incomplete_collections
    );
    println!();

    // TV Shows
    println!("{}", "TV Shows:".bold());
    println!(
        "  Complete: {} series",
        index.statistics.complete_tv_series
    );
    println!(
        "  Incomplete: {} series",
        index.statistics.incomplete_tv_series
    );

    Ok(())
}

/// List contents of a specific disk.
async fn list_volume(volume_label: &str, media_type: &str) -> Result<()> {
    let index = indexer::load_central_index()?;

    let show_movies = media_type == "all" || media_type == "movies";
    let show_tv_series = media_type == "all" || media_type == "tv_series";

    if show_movies {
        let movies: Vec<_> = index
            .movies
            .iter()
            .filter(|m| m.disk == volume_label)
            .collect();

        if !movies.is_empty() {
            println!(
                "{}",
                format!("Movies on {} ({}):", volume_label, movies.len()).bold()
            );
            for movie in movies {
                println!(
                    "  [{}] {} ({})",
                    movie.year.map(|y| y.to_string()).unwrap_or_default(),
                    movie.title,
                    movie.country.as_deref().unwrap_or("??")
                );
            }
            println!();
        }
    }

    if show_tv_series {
        let tv_series: Vec<_> = index
            .tv_series
            .iter()
            .filter(|t| t.disk == volume_label)
            .collect();

        if !tv_series.is_empty() {
            println!(
                "{}",
                format!("TV Shows on {} ({}):", volume_label, tv_series.len()).bold()
            );
            for tvshow in tv_series {
                println!(
                    "  [{}] {} - {} episodes",
                    tvshow.year.map(|y| y.to_string()).unwrap_or_default(),
                    tvshow.title,
                    tvshow.episodes
                );
            }
        }
    }

    Ok(())
}

/// Verify index against actual files.
async fn verify_index(path: &Path) -> Result<()> {
    println!("{}", "[VERIFY] Verifying index...".bold().cyan());

    let label = indexer::detect_disk_label(path).unwrap_or_else(|| "unknown".to_string());
    println!("  Disk: {}", label);

    let index = indexer::load_central_index()?;
    let movies: Vec<_> = index.movies.iter().filter(|m| m.disk == label).collect();
    let tv_series: Vec<_> = index.tv_series.iter().filter(|t| t.disk == label).collect();

    let mut valid = 0;
    let mut missing = 0;

    for movie in &movies {
        let movie_path = path.join(&movie.relative_path);
        if movie_path.exists() {
            valid += 1;
        } else {
            missing += 1;
            println!("  [MISSING] {}", movie.title);
        }
    }

    for tvshow in &tv_series {
        let tv_series_path = path.join(&tvshow.relative_path);
        if tv_series_path.exists() {
            valid += 1;
        } else {
            missing += 1;
            println!("  [MISSING] {}", tvshow.title);
        }
    }

    println!();
    if missing == 0 {
        println!(
            "{}",
            format!("[OK] All {} entries valid", valid).bold().green()
        );
    } else {
        println!(
            "{}",
            format!("[WARN] {} valid, {} missing", valid, missing)
                .bold()
                .yellow()
        );
    }

    Ok(())
}

/// Remove a volume group from the index.
async fn remove_volume(volume_label: &str, confirm: bool) -> Result<()> {
    if !confirm {
        println!(
            "{}",
            format!(
                "[WARN] This will remove all entries for volume '{}' from the index.",
                volume_label
            )
            .yellow()
        );
        println!("  Use --confirm to proceed");
        return Ok(());
    }

    let mut index = indexer::load_central_index()?;

    let movies_before = index.movies.len();
    let tv_series_before = index.tv_series.len();

    index.movies.retain(|m| m.disk != volume_label);
    index.tv_series.retain(|t| t.disk != volume_label);
    index.disks.remove(volume_label);

    let movies_removed = movies_before - index.movies.len();
    let tv_series_removed = tv_series_before - index.tv_series.len();

    index.rebuild_indexes();
    index.update_statistics();
    indexer::save_central_index(&index)?;

    // Remove disk index file
    let disk_index_path = indexer::disk_indexes_dir()?.join(format!("{}.json", volume_label));
    if disk_index_path.exists() {
        std::fs::remove_file(&disk_index_path)?;
    }

    println!(
        "{}",
        format!(
            "[OK] Removed volume '{}': {} movies, {} TV shows",
            volume_label, movies_removed, tv_series_removed
        )
        .bold()
        .green()
    );

    Ok(())
}

/// Data structure for duplicate entry info.
#[derive(Debug, Clone, serde::Serialize)]
struct DuplicateEntry {
    disk: String,
    disk_path: String,
    path: String,
    size_bytes: u64,
    size_human: String,
}

/// Data structure for duplicate group.
#[derive(Debug, Clone, serde::Serialize)]
struct DuplicateGroup {
    tmdb_id: u64,
    title: String,
    year: Option<u16>,
    media_type: String,
    confidence: String,
    entries: Vec<DuplicateEntry>,
    total_size_bytes: u64,
    total_size_human: String,
}

/// Format bytes to human-readable string.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Calculate title similarity using Levenshtein distance.
pub fn title_similarity(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    
    if a_lower == b_lower {
        return 1.0;
    }
    
    let max_len = std::cmp::max(a_lower.len(), b_lower.len()) as f64;
    if max_len == 0.0 {
        return 0.0;
    }
    
    // Simple Levenshtein distance approximation
    let distance = levenshtein_distance(&a_lower, &b_lower);
    1.0 - (distance as f64 / max_len)
}

/// Simple Levenshtein distance implementation.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    
    let mut dp = vec![vec![0; b_chars.len() + 1]; a_chars.len() + 1];
    
    for i in 0..=a_chars.len() {
        dp[i][0] = i;
    }
    for j in 0..=b_chars.len() {
        dp[0][j] = j;
    }
    
    for i in 1..=a_chars.len() {
        for j in 1..=b_chars.len() {
            let cost = if a_chars[i-1] == b_chars[j-1] { 0 } else { 1 };
            dp[i][j] = std::cmp::min(
                dp[i-1][j] + 1,
                std::cmp::min(dp[i][j-1] + 1, dp[i-1][j-1] + cost)
            );
        }
    }
    
    dp[a_chars.len()][b_chars.len()]
}

/// Find duplicates with enhanced matching logic.
async fn find_duplicates(media_type: &str, format: &str) -> Result<()> {
    let index = indexer::load_central_index()?;

    let show_movies = media_type == "all" || media_type == "movies";
    let show_tv_series = media_type == "all" || media_type == "tv_series";

    let mut duplicates: Vec<DuplicateGroup> = Vec::new();

    // Find duplicate movies using multiple criteria
    if show_movies {
        // First pass: Group by TMDB ID (most reliable)
        let mut tmdb_groups: std::collections::HashMap<u64, Vec<&MovieEntry>> = std::collections::HashMap::new();
        for movie in &index.movies {
            if let Some(tmdb_id) = movie.tmdb_id {
                tmdb_groups.entry(tmdb_id).or_default().push(movie);
            }
        }

        for (tmdb_id, movies) in tmdb_groups {
            if movies.len() > 1 {
                let total_size: u64 = movies.iter().map(|m| m.size_bytes).sum();
                duplicates.push(DuplicateGroup {
                    tmdb_id,
                    title: movies[0].title.clone(),
                    year: movies[0].year,
                    media_type: "movie".to_string(),
                    confidence: "high".to_string(),
                    entries: movies
                        .iter()
                        .map(|m| {
                            let disk_path = index.disks.get(&m.disk)
                                .and_then(|d| {
                                    d.paths.values().next().cloned()
                                        .or_else(|| Some(d.base_path.clone()))
                                })
                                .unwrap_or_default();
                            DuplicateEntry {
                                disk: m.disk.clone(),
                                disk_path,
                                path: m.relative_path.clone(),
                                size_bytes: m.size_bytes,
                                size_human: format_size(m.size_bytes),
                            }
                        })
                        .collect(),
                    total_size_bytes: total_size,
                    total_size_human: format_size(total_size),
                });
            }
        }

        // Second pass: Group by IMDB ID (for entries without TMDB ID)
        let movies_without_tmdb: Vec<_> = index.movies.iter().filter(|m| m.tmdb_id.is_none()).collect();
        let mut imdb_groups: std::collections::HashMap<String, Vec<&MovieEntry>> = std::collections::HashMap::new();
        for movie in &movies_without_tmdb {
            if let Some(ref imdb_id) = movie.imdb_id {
                imdb_groups.entry(imdb_id.clone()).or_default().push(movie);
            }
        }

        for (_, movies) in imdb_groups {
            if movies.len() > 1 {
                let total_size: u64 = movies.iter().map(|m| m.size_bytes).sum();
                duplicates.push(DuplicateGroup {
                    tmdb_id: 0,
                    title: movies[0].title.clone(),
                    year: movies[0].year,
                    media_type: "movie".to_string(),
                    confidence: "medium".to_string(),
                    entries: movies
                        .iter()
                        .map(|m| {
                            let disk_path = index.disks.get(&m.disk)
                                .and_then(|d| {
                                    d.paths.values().next().cloned()
                                        .or_else(|| Some(d.base_path.clone()))
                                })
                                .unwrap_or_default();
                            DuplicateEntry {
                                disk: m.disk.clone(),
                                disk_path,
                                path: m.relative_path.clone(),
                                size_bytes: m.size_bytes,
                                size_human: format_size(m.size_bytes),
                            }
                        })
                        .collect(),
                    total_size_bytes: total_size,
                    total_size_human: format_size(total_size),
                });
            }
        }

        // Third pass: Title-based matching with high similarity threshold
        // Only for movies without TMDB or IMDB ID
        let movies_without_ids: Vec<_> = movies_without_tmdb
            .into_iter()
            .filter(|m| m.imdb_id.is_none())
            .collect();
        
        for i in 0..movies_without_ids.len() {
            for j in (i + 1)..movies_without_ids.len() {
                let m1 = movies_without_ids[i];
                let m2 = movies_without_ids[j];
                
                // Check if they're on different disks (same disk duplicates are less interesting)
                if m1.disk == m2.disk {
                    continue;
                }
                
                // Check title similarity (must be very high)
                let similarity = title_similarity(&m1.title, &m2.title);
                if similarity < 0.9 {
                    continue;
                }
                
                // Check year match if both have years
                if m1.year.is_some() && m2.year.is_some() && m1.year != m2.year {
                    continue;
                }
                
                // Found a potential duplicate pair
                let exists = duplicates.iter_mut().any(|d| {
                    d.title == m1.title && d.media_type == "movie"
                });
                
                if !exists {
                    let total_size = m1.size_bytes + m2.size_bytes;
                    
                    let disk_path1 = index.disks.get(&m1.disk)
                        .and_then(|d| {
                            d.paths.values().next().cloned()
                                .or_else(|| Some(d.base_path.clone()))
                        })
                        .unwrap_or_default();
                    let disk_path2 = index.disks.get(&m2.disk)
                        .and_then(|d| {
                            d.paths.values().next().cloned()
                                .or_else(|| Some(d.base_path.clone()))
                        })
                        .unwrap_or_default();
                    
                    duplicates.push(DuplicateGroup {
                        tmdb_id: 0,
                        title: m1.title.clone(),
                        year: m1.year.or(m2.year),
                        media_type: "movie".to_string(),
                        confidence: "low".to_string(),
                        entries: vec![
                            DuplicateEntry {
                                disk: m1.disk.clone(),
                                disk_path: disk_path1,
                                path: m1.relative_path.clone(),
                                size_bytes: m1.size_bytes,
                                size_human: format_size(m1.size_bytes),
                            },
                            DuplicateEntry {
                                disk: m2.disk.clone(),
                                disk_path: disk_path2,
                                path: m2.relative_path.clone(),
                                size_bytes: m2.size_bytes,
                                size_human: format_size(m2.size_bytes),
                            },
                        ],
                        total_size_bytes: total_size,
                        total_size_human: format_size(total_size),
                    });
                }
            }
        }
    }

    // Find duplicate TV shows using similar logic
    if show_tv_series {
        // First pass: Group by TMDB ID
        let mut tmdb_groups: std::collections::HashMap<u64, Vec<&TvSeriesEntry>> = std::collections::HashMap::new();
        for tvshow in &index.tv_series {
            if let Some(tmdb_id) = tvshow.tmdb_id {
                tmdb_groups.entry(tmdb_id).or_default().push(tvshow);
            }
        }

        for (tmdb_id, tv_series) in tmdb_groups {
            if tv_series.len() > 1 {
                let total_size: u64 = tv_series.iter().map(|t| t.size_bytes).sum();
                duplicates.push(DuplicateGroup {
                    tmdb_id,
                    title: tv_series[0].title.clone(),
                    year: tv_series[0].year,
                    media_type: "tv_series".to_string(),
                    confidence: "high".to_string(),
                    entries: tv_series
                        .iter()
                        .map(|t| {
                            let disk_path = index.disks.get(&t.disk)
                                .and_then(|d| {
                                    d.paths.values().next().cloned()
                                        .or_else(|| Some(d.base_path.clone()))
                                })
                                .unwrap_or_default();
                            DuplicateEntry {
                                disk: t.disk.clone(),
                                disk_path,
                                path: t.relative_path.clone(),
                                size_bytes: t.size_bytes,
                                size_human: format_size(t.size_bytes),
                            }
                        })
                        .collect(),
                    total_size_bytes: total_size,
                    total_size_human: format_size(total_size),
                });
            }
        }

        // Second pass: Group by IMDB ID
        let tv_without_tmdb: Vec<_> = index.tv_series.iter().filter(|t| t.tmdb_id.is_none()).collect();
        let mut imdb_groups: std::collections::HashMap<String, Vec<&TvSeriesEntry>> = std::collections::HashMap::new();
        for tvshow in &tv_without_tmdb {
            if let Some(ref imdb_id) = tvshow.imdb_id {
                imdb_groups.entry(imdb_id.clone()).or_default().push(tvshow);
            }
        }

        for (_, tv_series) in imdb_groups {
            if tv_series.len() > 1 {
                let total_size: u64 = tv_series.iter().map(|t| t.size_bytes).sum();
                duplicates.push(DuplicateGroup {
                    tmdb_id: 0,
                    title: tv_series[0].title.clone(),
                    year: tv_series[0].year,
                    media_type: "tv_series".to_string(),
                    confidence: "medium".to_string(),
                    entries: tv_series
                        .iter()
                        .map(|t| {
                            let disk_path = index.disks.get(&t.disk)
                                .and_then(|d| {
                                    d.paths.values().next().cloned()
                                        .or_else(|| Some(d.base_path.clone()))
                                })
                                .unwrap_or_default();
                            DuplicateEntry {
                                disk: t.disk.clone(),
                                disk_path,
                                path: t.relative_path.clone(),
                                size_bytes: t.size_bytes,
                                size_human: format_size(t.size_bytes),
                            }
                        })
                        .collect(),
                    total_size_bytes: total_size,
                    total_size_human: format_size(total_size),
                });
            }
        }

        // Third pass: Title-based matching for TV shows without IDs
        let tv_without_ids: Vec<_> = tv_without_tmdb
            .into_iter()
            .filter(|t| t.imdb_id.is_none())
            .collect();
        
        for i in 0..tv_without_ids.len() {
            for j in (i + 1)..tv_without_ids.len() {
                let t1 = tv_without_ids[i];
                let t2 = tv_without_ids[j];
                
                if t1.disk == t2.disk {
                    continue;
                }
                
                let similarity = title_similarity(&t1.title, &t2.title);
                if similarity < 0.9 {
                    continue;
                }
                
                if t1.year.is_some() && t2.year.is_some() && t1.year != t2.year {
                    continue;
                }
                
                let exists = duplicates.iter_mut().any(|d| {
                    d.title == t1.title && d.media_type == "tv_series"
                });
                
                if !exists {
                    let total_size = t1.size_bytes + t2.size_bytes;
                    
                    let disk_path1 = index.disks.get(&t1.disk)
                        .and_then(|d| {
                            d.paths.values().next().cloned()
                                .or_else(|| Some(d.base_path.clone()))
                        })
                        .unwrap_or_default();
                    let disk_path2 = index.disks.get(&t2.disk)
                        .and_then(|d| {
                            d.paths.values().next().cloned()
                                .or_else(|| Some(d.base_path.clone()))
                        })
                        .unwrap_or_default();
                    
                    duplicates.push(DuplicateGroup {
                        tmdb_id: 0,
                        title: t1.title.clone(),
                        year: t1.year.or(t2.year),
                        media_type: "tv_series".to_string(),
                        confidence: "low".to_string(),
                        entries: vec![
                            DuplicateEntry {
                                disk: t1.disk.clone(),
                                disk_path: disk_path1,
                                path: t1.relative_path.clone(),
                                size_bytes: t1.size_bytes,
                                size_human: format_size(t1.size_bytes),
                            },
                            DuplicateEntry {
                                disk: t2.disk.clone(),
                                disk_path: disk_path2,
                                path: t2.relative_path.clone(),
                                size_bytes: t2.size_bytes,
                                size_human: format_size(t2.size_bytes),
                            },
                        ],
                        total_size_bytes: total_size,
                        total_size_human: format_size(total_size),
                    });
                }
            }
        }
    }

    // Sort by confidence (high first), then by total size (largest first)
    duplicates.sort_by(|a, b| {
        let confidence_order = match (a.confidence.as_str(), b.confidence.as_str()) {
            ("high", "high") => std::cmp::Ordering::Equal,
            ("high", _) => std::cmp::Ordering::Less,
            (_, "high") => std::cmp::Ordering::Greater,
            ("medium", "medium") => std::cmp::Ordering::Equal,
            ("medium", _) => std::cmp::Ordering::Less,
            (_, "medium") => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        };
        confidence_order.then_with(|| b.total_size_bytes.cmp(&a.total_size_bytes))
    });

    // Output
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&duplicates)?;
            println!("{}", json);
        }
        "simple" => {
            if duplicates.is_empty() {
                println!("No duplicates found.");
            } else {
                println!("Found {} duplicate groups:\n", duplicates.len());
                for group in &duplicates {
                    let year_str = group.year.map(|y| y.to_string()).unwrap_or_default();
                    let confidence_str = match group.confidence.as_str() {
                        "high" => "[HIGH]".green(),
                        "medium" => "[MED]".yellow(),
                        "low" => "[LOW]".cyan(),
                        _ => "[?]".white(),
                    };
                    println!(
                        "[{}] {} {} ({}) - tmdb{} - {} copies - {}",
                        group.media_type.to_uppercase(),
                        confidence_str,
                        group.title,
                        year_str,
                        group.tmdb_id,
                        group.entries.len(),
                        group.total_size_human
                    );
                    for entry in &group.entries {
                        println!(
                            "  - {} [{}]: {}",
                            entry.disk, entry.size_human, entry.path
                        );
                    }
                    println!();
                }
            }
        }
        _ => {
            // Table format (default)
            if duplicates.is_empty() {
                println!("{}", "No duplicates found.".green());
            } else {
                // Collect unique disk paths for display at the beginning
                let mut disk_paths: std::collections::HashMap<String, String> = std::collections::HashMap::new();
                for group in &duplicates {
                    for entry in &group.entries {
                        if !disk_paths.contains_key(&entry.disk) {
                            disk_paths.insert(entry.disk.clone(), entry.disk_path.clone());
                        }
                    }
                }
                
                println!(
                    "{}",
                    format!("Found {} duplicate groups:", duplicates.len())
                        .bold()
                        .yellow()
                );
                println!();
                
                // Show disk locations at the beginning
                if !disk_paths.is_empty() {
                    println!("{}", "Volume Groups:".bold());
                    let mut sorted_disks: Vec<(&String, &String)> = disk_paths.iter().collect();
                    sorted_disks.sort_by(|a, b| a.0.cmp(b.0));
                    for (disk, path) in sorted_disks {
                        println!("  {} -> {}", disk.bold(), path.dimmed());
                    }
                    println!();
                }

                for group in &duplicates {
                    let year_str = group.year.map(|y| format!("({})", y)).unwrap_or_default();
                    let type_badge = match group.media_type.as_str() {
                        "movie" => "[MOVIE]".cyan(),
                        "tv_series" => "[TV_SERIES]".magenta(),
                        _ => "[?]".white(),
                    };
                    let confidence_badge = match group.confidence.as_str() {
                        "high" => "[HIGH]".green(),
                        "medium" => "[MED]".yellow(),
                        "low" => "[LOW]".cyan(),
                        _ => "[?]".white(),
                    };

                    println!(
                        "{} {} {} {} - tmdb{} - {} copies",
                        type_badge,
                        confidence_badge,
                        group.title.bold(),
                        year_str,
                        group.tmdb_id,
                        group.entries.len()
                    );
                    println!("  Total size: {}", group.total_size_human.bold().red());
                    println!("  {}", "-".repeat(60));

                    for entry in &group.entries {
                        println!(
                            "  {:>12} | {:>10} | {}",
                            entry.disk.bold(),
                            entry.size_human,
                            entry.path
                        );
                    }
                    println!();
                }

                // Summary
                let total_wasted: u64 = duplicates
                    .iter()
                    .map(|g| {
                        // Wasted = total - smallest copy
                        let min_size = g.entries.iter().map(|e| e.size_bytes).min().unwrap_or(0);
                        g.total_size_bytes - min_size
                    })
                    .sum();

                println!("{}", "=".repeat(60));
                println!(
                    "Total duplicate groups: {}",
                    duplicates.len().to_string().bold()
                );
                println!(
                    "Potential space savings: {}",
                    format_size(total_wasted).bold().green()
                );
            }
        }
    }

    Ok(())
}

/// Update collection totals from TMDB API and write back to NFO files.
async fn update_collections(config: &Config) -> Result<()> {
    use crate::services::tmdb::{TmdbClient, TmdbConfig};
    use std::path::PathBuf;

    println!(
        "{}",
        "[UPDATE] Fetching collection details from TMDB..."
            .bold()
            .cyan()
    );

    let mut index = indexer::load_central_index()?;

    // Initialize TMDB client from config
    let tmdb_config = TmdbConfig::from_config(&config.tmdb)?;
    let tmdb_client = std::sync::Arc::new(TmdbClient::new(tmdb_config));

    // Step 1: Find movies without collection_id but with tmdb_id
    // and fetch their collection info from TMDB
    let movies_without_collection: Vec<(String, u64)> = index
        .movies
        .iter()
        .filter(|m| m.collection_id.is_none() && m.tmdb_id.is_some())
        .map(|m| (m.id.clone(), m.tmdb_id.unwrap()))
        .collect();

    if !movies_without_collection.is_empty() {
        println!(
            "  Found {} movies without collection info, fetching from TMDB...",
            movies_without_collection.len()
        );

        let pb = ProgressBar::new(movies_without_collection.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=> "),
        );

        // Use concurrent requests with controlled concurrency using JoinSet
        let concurrency_limit = 20; // TMDB allows ~40 req/sec, so 20 concurrent is safe
        let mut collection_updates: Vec<(String, u64, String)> = Vec::new();
        let mut tasks = tokio::task::JoinSet::new();

        for (movie_id, tmdb_id) in movies_without_collection {
            let client = tmdb_client.clone();
            let pb = pb.clone();
            
            tasks.spawn(async move {
                pb.set_message(format!("Movie {}", tmdb_id));
                let result = client.get_movie_details(tmdb_id).await;
                pb.inc(1);
                (movie_id, tmdb_id, result)
            });

            // Limit concurrency
            if tasks.len() >= concurrency_limit {
                if let Some(result) = tasks.join_next().await {
                    if let Ok((movie_id, _tmdb_id, result)) = result {
                        if let Ok(movie_details) = result {
                            if let Some(collection) = movie_details.belongs_to_collection {
                                collection_updates.push((movie_id, collection.id, collection.name));
                            }
                        }
                    }
                }
            }
        }

        // Wait for remaining tasks
        while let Some(result) = tasks.join_next().await {
            if let Ok((movie_id, _tmdb_id, result)) = result {
                if let Ok(movie_details) = result {
                    if let Some(collection) = movie_details.belongs_to_collection {
                        collection_updates.push((movie_id, collection.id, collection.name));
                    }
                }
            }
        }

        pb.finish_with_message("Done fetching movie collection info");
        
        // Apply updates to movies
        for (movie_id, collection_id, collection_name) in collection_updates {
            if let Some(m) = index.movies.iter_mut().find(|m| m.id == movie_id) {
                m.collection_id = Some(collection_id);
                m.collection_name = Some(collection_name.clone());
                
                tracing::debug!(
                    "[MOVIE] Added collection {} (tmdb{}) to {}",
                    collection_name,
                    collection_id,
                    m.title
                );
            }
        }
        
        // Rebuild collections after adding new collection info
        index.rebuild_indexes();
    }

    // Step 2: Find collections that need updating (total_in_collection == 0)
    let collections_to_update: Vec<u64> = index
        .collections
        .values()
        .filter(|c| c.total_in_collection == 0 && c.owned_count > 0)
        .map(|c| c.id)
        .collect();

    if collections_to_update.is_empty() {
        println!("  All collections already have complete info.");
        return Ok(());
    }

    println!(
        "  Found {} collections to update",
        collections_to_update.len()
    );

    // Track which NFO files need updating (disk -> relative_path -> total_movies)
    let mut nfo_updates: std::collections::HashMap<(String, String), usize> =
        std::collections::HashMap::new();

    let pb = ProgressBar::new(collections_to_update.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    // Use concurrent requests for collection details using JoinSet
    let concurrency_limit = 20;
    let mut tasks = tokio::task::JoinSet::new();
    let mut collection_results: Vec<(u64, Option<crate::services::tmdb::CollectionDetails>)> = Vec::new();

    for collection_id in collections_to_update {
        let client = tmdb_client.clone();
        let pb = pb.clone();
        
        tasks.spawn(async move {
            pb.set_message(format!("Collection {}", collection_id));
            let result = client.get_collection_details(collection_id).await.ok();
            pb.inc(1);
            (collection_id, result)
        });

        // Limit concurrency
        if tasks.len() >= concurrency_limit {
            if let Some(result) = tasks.join_next().await {
                if let Ok(result) = result {
                    collection_results.push(result);
                }
            }
        }
    }

    // Wait for remaining tasks
    while let Some(result) = tasks.join_next().await {
        if let Ok(result) = result {
            collection_results.push(result);
        }
    }

    // Process results
    for (collection_id, details_opt) in collection_results {
        if let Some(details) = details_opt {
            let total = details.parts.len();

            // Update the collection in index
            if let Some(collection) = index.collections.get_mut(&collection_id) {
                collection.total_in_collection = total;

                // Track NFO files that need updating
                for movie in &collection.movies {
                    if movie.owned {
                        if let Some(ref disk) = movie.disk {
                            // Find the movie entry to get the relative path
                            if let Some(movie_entry) = index
                                .movies
                                .iter()
                                .find(|m| m.tmdb_id == Some(movie.tmdb_id) && m.disk == *disk)
                            {
                                nfo_updates.insert(
                                    (disk.clone(), movie_entry.relative_path.clone()),
                                    total,
                                );
                            }
                        }
                    }
                }

                tracing::debug!(
                    "[COLLECTION] Updated {} (tmdb{}): {} movies",
                    details.name,
                    collection_id,
                    total
                );
            }
        } else {
            tracing::warn!("[COLLECTION] Failed to fetch {}", collection_id);
        }
    }

    pb.finish_with_message("Done fetching from TMDB");

    // Update NFO files
    if !nfo_updates.is_empty() {
        println!(
            "{}",
            format!("[UPDATE] Writing to {} NFO files...", nfo_updates.len()).cyan()
        );

        for ((disk_label, relative_path), total_movies) in &nfo_updates {
            // Find the disk base path
            if let Some(disk_info) = index.disks.get(disk_label) {
                let nfo_path = PathBuf::from(&disk_info.base_path)
                    .join(relative_path)
                    .join("movie.nfo");

                if nfo_path.exists() {
                    match update_nfo_with_totalmovies(&nfo_path, *total_movies) {
                        Ok(_) => {
                            tracing::debug!("[NFO] Updated: {}", nfo_path.display());
                        }
                        Err(e) => {
                            tracing::warn!("[NFO] Failed to update {}: {}", nfo_path.display(), e);
                        }
                    }
                } else {
                    tracing::debug!(
                        "[NFO] File not found (disk offline?): {}",
                        nfo_path.display()
                    );
                }
            }
        }
    }

    // Also update the movie entries with collection_total_movies
    for movie in &mut index.movies {
        if let Some(collection_id) = movie.collection_id {
            if let Some(collection) = index.collections.get(&collection_id) {
                if collection.total_in_collection > 0 {
                    movie.collection_total_movies = Some(collection.total_in_collection);
                }
            }
        }
    }

    // Save updated index
    index.update_statistics();
    indexer::save_central_index(&index)?;

    println!("{}", "[OK] Collection info updated".bold().green());

    Ok(())
}

/// Update an NFO file to include <totalmovies> tag within <set>.
fn update_nfo_with_totalmovies(nfo_path: &std::path::Path, total_movies: usize) -> Result<()> {
    use std::fs;

    let content = fs::read_to_string(nfo_path)?;

    // Check if <totalmovies> already exists
    if content.contains("<totalmovies>") {
        // Update existing value
        let re = regex::Regex::new(r"<totalmovies>\d+</totalmovies>")?;
        let updated = re.replace(
            &content,
            format!("<totalmovies>{}</totalmovies>", total_movies),
        );
        fs::write(nfo_path, updated.as_ref())?;
    } else if content.contains("</set>") {
        // Add <totalmovies> before </set>
        let updated = content.replace(
            "</set>",
            &format!("    <totalmovies>{}</totalmovies>\n  </set>", total_movies),
        );
        fs::write(nfo_path, updated)?;
    } else {
        // No <set> tag found, skip
        tracing::debug!("[NFO] No <set> tag found in {}", nfo_path.display());
    }

    Ok(())
}

/// Data structure for collection output.
#[derive(Debug, Clone, serde::Serialize)]
struct CollectionOutput {
    id: u64,
    name: String,
    owned_count: usize,
    total_in_collection: usize,
    status: String,
    movies: Vec<CollectionMovieOutput>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CollectionMovieOutput {
    title: String,
    year: Option<u16>,
    disk: String,
    path: Option<String>,
}

/// List movie collections.
async fn list_collections(filter: &str, format: &str, hide_paths: bool) -> Result<()> {
    let index = indexer::load_central_index()?;

    // Build collection output list
    let collections: Vec<CollectionOutput> = index
        .collections
        .values()
        .filter(|c| c.owned_count > 0)
        .map(|c| {
            let status = if c.total_in_collection > 0 && c.owned_count >= c.total_in_collection {
                "Complete".to_string()
            } else if c.total_in_collection > 0 {
                "Incomplete".to_string()
            } else {
                // total_in_collection == 0 means we don't know the total
                "Unknown".to_string()
            };

            // Get movie paths from central index
            let movies: Vec<CollectionMovieOutput> = c
                .movies
                .iter()
                .filter(|cm| cm.owned) // Only show owned movies
                .map(|cm| {
                    let disk = cm.disk.clone().unwrap_or_default();

                    // Find the movie in central index to get path
                    let movie_entry = index
                        .movies
                        .iter()
                        .find(|m| m.tmdb_id == Some(cm.tmdb_id) && m.disk == disk);

                    // Always get path (combine disk path + relative path)
                    let full_path = movie_entry.map(|m| {
                        let disk_info = index.disks.get(&m.disk);
                        let disk_base = disk_info.as_ref()
                            .and_then(|d| {
                                if !d.base_path.is_empty() {
                                    Some(d.base_path.clone())
                                } else {
                                    d.paths.values().next().cloned()
                                }
                            })
                            .unwrap_or_default();
                        if disk_base.is_empty() {
                            m.relative_path.clone()
                        } else {
                            format!("{}/{}", disk_base, m.relative_path)
                        }
                    });

                    // Hide paths if requested
                    let display_path = if hide_paths { None } else { full_path };

                    CollectionMovieOutput {
                        title: cm.title.clone(),
                        year: cm.year,
                        disk,
                        path: display_path,
                    }
                })
                .collect();

            CollectionOutput {
                id: c.id,
                name: c.name.clone(),
                owned_count: c.owned_count,
                total_in_collection: c.total_in_collection,
                status,
                movies,
            }
        })
        .collect();

    // Apply filter
    let collections: Vec<_> = match filter {
        "complete" => collections
            .into_iter()
            .filter(|c| c.status == "Complete")
            .collect(),
        "incomplete" => collections
            .into_iter()
            .filter(|c| c.status == "Incomplete")
            .collect(),
        _ => collections,
    };

    // Sort by name
    let mut collections = collections;
    collections.sort_by(|a, b| a.name.cmp(&b.name));

    // Output
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&collections)?;
            println!("{}", json);
        }
        "simple" => {
            if collections.is_empty() {
                println!("No collections found.");
            } else {
                println!("Found {} collections:\n", collections.len());
                for c in &collections {
                    let status_str = match c.status.as_str() {
                        "Complete" => "[COMPLETE]",
                        "Incomplete" => "[INCOMPLETE]",
                        _ => "[UNKNOWN]",
                    };
                    println!(
                        "{} {} ({}/{}) - tmdb{}",
                        status_str, c.name, c.owned_count, c.total_in_collection, c.id
                    );
                    for movie in &c.movies {
                        let year_str = movie.year.map(|y| format!("({})", y)).unwrap_or_default();
                        if let Some(ref path) = movie.path {
                            println!(
                                "  - {} {} [{}] {}",
                                movie.title, year_str, movie.disk, path
                            );
                        } else {
                            println!(
                                "  - {} {} [{}]",
                                movie.title, year_str, movie.disk
                            );
                        }
                    }
                    println!();
                }
            }
        }
        _ => {
            // Table format (default)
            if collections.is_empty() {
                println!("{}", "No collections found.".yellow());
            } else {
                let complete_count = collections
                    .iter()
                    .filter(|c| c.status == "Complete")
                    .count();
                let incomplete_count = collections
                    .iter()
                    .filter(|c| c.status == "Incomplete")
                    .count();
                let unknown_count = collections.iter().filter(|c| c.status == "Unknown").count();

                // Show Volume Groups with paths
                if !index.disks.is_empty() {
                    println!("{}", "Volume Groups:".bold());
                    let mut sorted_disks: Vec<(&String, &VolumeGroupInfo)> = index.disks.iter().collect();
                    sorted_disks.sort_by(|a, b| a.0.cmp(b.0));
                    for (label, disk) in &sorted_disks {
                        let disk_path = if !disk.base_path.is_empty() {
                            disk.base_path.clone()
                        } else if let Some((_, path)) = disk.paths.iter().next() {
                            path.clone()
                        } else {
                            String::new()
                        };
                        println!("  {} -> {}", label.bold(), disk_path.dimmed());
                    }
                    println!();
                }

                println!("{}", "Movie Collections".bold().cyan());
                println!("{}", "=".repeat(60));
                println!(
                    "Total: {} | Complete: {} | Incomplete: {} | Unknown: {}",
                    collections.len().to_string().bold(),
                    complete_count.to_string().green(),
                    incomplete_count.to_string().yellow(),
                    unknown_count.to_string().white()
                );
                println!();

                for c in &collections {
                    let status_badge = match c.status.as_str() {
                        "Complete" => "[COMPLETE]".green(),
                        "Incomplete" => "[INCOMPLETE]".yellow(),
                        _ => "[UNKNOWN]".white(),
                    };

                    let progress = if c.total_in_collection > 0 {
                        format!("{}/{}", c.owned_count, c.total_in_collection)
                    } else {
                        format!("{}", c.owned_count)
                    };

                    println!(
                        "{} {} {} (tmdb{})",
                        status_badge,
                        c.name.bold(),
                        progress,
                        c.id
                    );

                    for movie in &c.movies {
                        let year_str = movie.year.map(|y| format!("({})", y)).unwrap_or_default();

                        if let Some(ref path) = movie.path {
                            println!(
                                "    {} {} | {} | {}",
                                movie.title,
                                year_str,
                                movie.disk.bold(),
                                path
                            );
                        } else {
                            println!(
                                "    {} {} | {}",
                                movie.title,
                                year_str,
                                movie.disk.bold()
                            );
                        }
                    }
                    println!();
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
struct TvSeriesOutput {
    id: String,
    title: String,
    year: Option<u16>,
    tmdb_id: Option<u64>,
    status: String,
    owned_seasons: u16,
    total_seasons: u16,
    owned_episodes: u32,
    total_episodes: u32,
    disk: String,
    path: Option<String>,
}

/// Update TV show details from TMDB API.
async fn update_tv(config: &Config) -> Result<()> {
    use crate::services::tmdb::{TmdbClient, TmdbConfig};

    println!(
        "{}",
        "[UPDATE] Fetching TV show details from TMDB..."
            .bold()
            .cyan()
    );

    let mut index = indexer::load_central_index()?;

    // Initialize TMDB client from config
    let tmdb_config = TmdbConfig::from_config(&config.tmdb)?;
    let tmdb_client = TmdbClient::new(tmdb_config);

    // Find TV shows that need updating (seasons == 0 means no TMDB data)
    let tv_shows_to_update: Vec<_> = index
        .tv_series
        .iter()
        .filter(|t| t.tmdb_id.is_some() && t.seasons == 0)
        .map(|t| (t.id.clone(), t.tmdb_id.unwrap()))
        .collect();

    if tv_shows_to_update.is_empty() {
        println!("  All TV shows already have complete info.");
        return Ok(());
    }

    println!(
        "  Found {} TV shows to update",
        tv_shows_to_update.len()
    );

    let pb = ProgressBar::new(tv_shows_to_update.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    for (tv_id, tmdb_id) in &tv_shows_to_update {
        pb.set_message(format!("TV Show {}", tmdb_id));
        
        if let Ok(tv_details) = tmdb_client.get_tv_details(*tmdb_id).await {
            // Update the TV show entry
            if let Some(tv) = index.tv_series.iter_mut().find(|t| t.id == *tv_id) {
                tv.seasons = tv_details.number_of_seasons as u16;
                tv.episodes = tv_details.number_of_episodes as u32;
                
                tracing::debug!(
                    "[TV] Updated {} (tmdb{}): {} seasons, {} episodes",
                    tv.title,
                    tmdb_id,
                    tv.seasons,
                    tv.episodes
                );
            }
        }
        
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        pb.inc(1);
    }

    pb.finish_with_message("Done fetching from TMDB");

    // Update statistics and save
    index.update_statistics();
    indexer::save_central_index(&index)?;

    println!("{}", "[OK] TV show info updated".bold().green());

    Ok(())
}

/// List TV shows with season/episode statistics.
async fn list_tv(filter: &str, format: &str, hide_paths: bool) -> Result<()> {
    let index = indexer::load_central_index()?;

    // Build TV series output list
    let tv_series: Vec<TvSeriesOutput> = index
        .tv_series
        .iter()
        .filter(|t| t.seasons > 0 || t.owned_seasons > 0)
        .map(|t| {
            let status = if t.seasons > 0 && t.owned_seasons > 0 && t.owned_seasons >= t.seasons {
                "Complete".to_string()
            } else if t.seasons > 0 && t.owned_seasons > 0 && t.owned_seasons < t.seasons {
                "Incomplete".to_string()
            } else {
                "Unknown".to_string()
            };

            // Always get full path (combine disk path + relative path)
            let full_path = {
                let disk_info = index.disks.get(&t.disk);
                let disk_base = disk_info.as_ref()
                    .and_then(|d| {
                        if !d.base_path.is_empty() {
                            Some(d.base_path.clone())
                        } else {
                            d.paths.values().next().cloned()
                        }
                    })
                    .unwrap_or_default();
                if disk_base.is_empty() {
                    t.relative_path.clone()
                } else {
                    format!("{}/{}", disk_base, t.relative_path)
                }
            };

            // Hide paths if requested
            let display_path = if hide_paths { None } else { Some(full_path) };

            TvSeriesOutput {
                id: t.id.clone(),
                title: t.title.clone(),
                year: t.year,
                tmdb_id: t.tmdb_id,
                status,
                owned_seasons: t.owned_seasons,
                total_seasons: t.seasons,
                owned_episodes: t.owned_episodes,
                total_episodes: t.episodes,
                disk: t.disk.clone(),
                path: display_path,
            }
        })
        .collect();

    // Apply filter
    let tv_series: Vec<_> = match filter {
        "complete" => tv_series
            .into_iter()
            .filter(|t| t.status == "Complete")
            .collect(),
        "incomplete" => tv_series
            .into_iter()
            .filter(|t| t.status == "Incomplete")
            .collect(),
        _ => tv_series,
    };

    // Sort by name
    let mut tv_series = tv_series;
    tv_series.sort_by(|a, b| a.title.cmp(&b.title));

    // Output
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&tv_series)?;
            println!("{}", json);
        }
        "simple" => {
            if tv_series.is_empty() {
                println!("No TV shows found.");
            } else {
                println!("Found {} TV shows:\n", tv_series.len());
                for t in &tv_series {
                    let status_str = match t.status.as_str() {
                        "Complete" => "[COMPLETE]",
                        "Incomplete" => "[INCOMPLETE]",
                        _ => "[UNKNOWN]",
                    };
                    let year_str = t.year.map(|y| format!("({})", y)).unwrap_or_default();
                    println!(
                        "{} {} {} - {} seasons ({}/{}), {} episodes ({}/{})",
                        status_str,
                        t.title,
                        year_str,
                        t.total_seasons,
                        t.owned_seasons,
                        t.total_seasons,
                        t.total_episodes,
                        t.owned_episodes,
                        t.total_episodes
                    );
                    if let Some(ref path) = t.path {
                        println!("  {}: {}", t.disk, path);
                    }
                    println!();
                }
            }
        }
        _ => {
            // Table format (default)
            if tv_series.is_empty() {
                println!("{}", "No TV shows found.".yellow());
            } else {
                let complete_count = tv_series
                    .iter()
                    .filter(|t| t.status == "Complete")
                    .count();
                let incomplete_count = tv_series
                    .iter()
                    .filter(|t| t.status == "Incomplete")
                    .count();
                let unknown_count = tv_series.iter().filter(|t| t.status == "Unknown").count();

                // Show Volume Groups with paths
                if !index.disks.is_empty() {
                    println!("{}", "Volume Groups:".bold());
                    let mut sorted_disks: Vec<(&String, &VolumeGroupInfo)> = index.disks.iter().collect();
                    sorted_disks.sort_by(|a, b| a.0.cmp(b.0));
                    for (label, disk) in &sorted_disks {
                        let disk_path = if !disk.base_path.is_empty() {
                            disk.base_path.clone()
                        } else if let Some((_, path)) = disk.paths.iter().next() {
                            path.clone()
                        } else {
                            String::new()
                        };
                        println!("  {} -> {}", label.bold(), disk_path.dimmed());
                    }
                    println!();
                }

                println!("{}", "TV Shows".bold().cyan());
                println!("{}", "=".repeat(60));
                println!(
                    "Total: {} | Complete: {} | Incomplete: {} | Unknown: {}",
                    tv_series.len().to_string().bold(),
                    complete_count.to_string().green(),
                    incomplete_count.to_string().yellow(),
                    unknown_count.to_string().white()
                );
                println!();

                for t in &tv_series {
                    let status_badge = match t.status.as_str() {
                        "Complete" => "[COMPLETE]".green(),
                        "Incomplete" => "[INCOMPLETE]".yellow(),
                        _ => "[UNKNOWN]".white(),
                    };

                    let season_progress = if t.total_seasons > 0 {
                        format!("{}/{}", t.owned_seasons, t.total_seasons)
                    } else {
                        format!("{}", t.owned_seasons)
                    };

                    let episode_progress = if t.total_episodes > 0 {
                        format!("{}/{}", t.owned_episodes, t.total_episodes)
                    } else {
                        format!("{}", t.owned_episodes)
                    };

                    let year_str = t.year.map(|y| format!("({})", y)).unwrap_or_default();

                    println!(
                        "{} {} {} - {} seasons, {} episodes",
                        status_badge,
                        t.title.bold(),
                        year_str,
                        season_progress,
                        episode_progress
                    );

                    if let Some(ref path) = t.path {
                        println!(
                            "    {} | {}",
                            t.disk.bold(),
                            path
                        );
                    } else {
                        println!(
                            "    {}",
                            t.disk.bold()
                        );
                    }
                    println!();
                }
            }
        }
    }

    Ok(())
}
