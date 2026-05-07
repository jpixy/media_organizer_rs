//! Index command implementation.

use crate::cli::args::IndexAction;
use crate::core::indexer;
use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;

/// Execute index subcommand.
pub async fn execute_index(action: IndexAction) -> Result<()> {
    match action {
        IndexAction::Scan {
            path,
            media_type,
            disk_label,
            force,
        } => scan_directory(&path, &media_type, disk_label, force).await,
        IndexAction::Stats => show_stats().await,
        IndexAction::List {
            disk_label,
            media_type,
        } => list_disk(&disk_label, &media_type).await,
        IndexAction::Verify { path } => verify_index(&path).await,
        IndexAction::Remove {
            disk_label,
            confirm,
        } => remove_disk(&disk_label, confirm).await,
        IndexAction::Duplicates { media_type, format } => {
            find_duplicates(&media_type, &format).await
        }
        IndexAction::Collections {
            filter,
            format,
            paths,
            update,
        } => {
            if update {
                update_collections().await?;
            }
            list_collections(&filter, &format, paths).await
        }
    }
}

/// Scan and index a directory.
async fn scan_directory(
    path: &Path,
    media_type: &str,
    disk_label: Option<String>,
    force: bool,
) -> Result<()> {
    println!("{}", "[INDEX] Scanning directory...".bold().cyan());
    println!("  Path: {}", path.display());
    println!("  Media type: {}", media_type);

    // Detect or use provided disk label
    let label = disk_label.unwrap_or_else(|| {
        indexer::detect_disk_label(path).unwrap_or_else(|| "unknown".to_string())
    });
    println!("  Disk label: {}", label);

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
                    "[WARN] Disk '{}' already indexed ({} movies, {} TV shows)",
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

    let disk_index = indexer::scan_directory(path, &label, uuid, media_type)?;

    pb.finish_with_message("Scan complete");

    // Save disk index
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

    // Disks
    println!("{}", "Disks:".bold());
    for (label, disk) in &index.disks {
        let status = if indexer::is_disk_online(label) {
            "Online".green()
        } else {
            "Offline".red()
        };
        println!(
            "  {} | {} movies | {} TV shows | {:.1} GB | {}",
            label.bold(),
            disk.movie_count,
            disk.tv_series_count,
            disk.total_size_bytes as f64 / 1_073_741_824.0,
            status
        );
        // Show paths if multiple media types are stored
        if disk.paths.len() > 1 {
            for (media_type, path) in &disk.paths {
                println!("      {} -> {}", media_type, path.dimmed());
            }
        } else if !disk.paths.is_empty() {
            // Single path: show inline
            if let Some((media_type, path)) = disk.paths.iter().next() {
                println!("      {} -> {}", media_type, path.dimmed());
            }
        } else if !disk.base_path.is_empty() {
            // Legacy fallback
            println!("      path: {}", disk.base_path.dimmed());
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

    // By country
    if !index.statistics.by_country.is_empty() {
        println!("{}", "By Country:".bold());
        let mut countries: Vec<_> = index.statistics.by_country.iter().collect();
        countries.sort_by(|a, b| b.1.cmp(a.1));
        let total = index.statistics.total_movies + index.statistics.total_tv_series;
        for (country, count) in countries.iter().take(10) {
            let pct = **count as f64 / total as f64 * 100.0;
            let bar_len = (pct / 2.0) as usize;
            let bar = "█".repeat(bar_len);
            println!("  {} {:>15} {} ({:.0}%)", country, bar, count, pct);
        }
        println!();
    }

    // By decade
    if !index.statistics.by_decade.is_empty() {
        println!("{}", "By Decade:".bold());
        let mut decades: Vec<_> = index.statistics.by_decade.iter().collect();
        decades.sort_by(|a, b| b.0.cmp(a.0));
        let total = index.statistics.total_movies;
        for (decade, count) in decades.iter().take(5) {
            let pct = **count as f64 / total as f64 * 100.0;
            let bar_len = (pct / 2.0) as usize;
            let bar = "█".repeat(bar_len);
            println!("  {} {:>15} {} ({:.0}%)", decade, bar, count, pct);
        }
        println!();
    }

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

    Ok(())
}

/// List contents of a specific disk.
async fn list_disk(disk_label: &str, media_type: &str) -> Result<()> {
    let index = indexer::load_central_index()?;

    let show_movies = media_type == "all" || media_type == "movies";
    let show_tv_series = media_type == "all" || media_type == "tv_series";

    if show_movies {
        let movies: Vec<_> = index
            .movies
            .iter()
            .filter(|m| m.disk == disk_label)
            .collect();

        if !movies.is_empty() {
            println!(
                "{}",
                format!("Movies on {} ({}):", disk_label, movies.len()).bold()
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
            .filter(|t| t.disk == disk_label)
            .collect();

        if !tv_series.is_empty() {
            println!(
                "{}",
                format!("TV Shows on {} ({}):", disk_label, tv_series.len()).bold()
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

/// Remove a disk from the index.
async fn remove_disk(disk_label: &str, confirm: bool) -> Result<()> {
    if !confirm {
        println!(
            "{}",
            format!(
                "[WARN] This will remove all entries for disk '{}' from the index.",
                disk_label
            )
            .yellow()
        );
        println!("  Use --confirm to proceed");
        return Ok(());
    }

    let mut index = indexer::load_central_index()?;

    let movies_before = index.movies.len();
    let tv_series_before = index.tv_series.len();

    index.movies.retain(|m| m.disk != disk_label);
    index.tv_series.retain(|t| t.disk != disk_label);
    index.disks.remove(disk_label);

    let movies_removed = movies_before - index.movies.len();
    let tv_series_removed = tv_series_before - index.tv_series.len();

    index.rebuild_indexes();
    index.update_statistics();
    indexer::save_central_index(&index)?;

    // Remove disk index file
    let disk_index_path = indexer::disk_indexes_dir()?.join(format!("{}.json", disk_label));
    if disk_index_path.exists() {
        std::fs::remove_file(&disk_index_path)?;
    }

    println!(
        "{}",
        format!(
            "[OK] Removed disk '{}': {} movies, {} TV shows",
            disk_label, movies_removed, tv_series_removed
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
    path: String,
    size_bytes: u64,
    size_human: String,
    online: bool,
}

/// Data structure for duplicate group.
#[derive(Debug, Clone, serde::Serialize)]
struct DuplicateGroup {
    tmdb_id: u64,
    title: String,
    year: Option<u16>,
    media_type: String,
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

/// Find duplicates by TMDB ID across disks.
async fn find_duplicates(media_type: &str, format: &str) -> Result<()> {
    let index = indexer::load_central_index()?;

    let show_movies = media_type == "all" || media_type == "movies";
    let show_tv_series = media_type == "all" || media_type == "tv_series";

    let mut duplicates: Vec<DuplicateGroup> = Vec::new();

    // Find duplicate movies
    if show_movies {
        let mut tmdb_to_movies: std::collections::HashMap<u64, Vec<_>> =
            std::collections::HashMap::new();

        for movie in &index.movies {
            if let Some(tmdb_id) = movie.tmdb_id {
                tmdb_to_movies.entry(tmdb_id).or_default().push(movie);
            }
        }

        for (tmdb_id, movies) in tmdb_to_movies {
            if movies.len() > 1 {
                let total_size: u64 = movies.iter().map(|m| m.size_bytes).sum();
                duplicates.push(DuplicateGroup {
                    tmdb_id,
                    title: movies[0].title.clone(),
                    year: movies[0].year,
                    media_type: "movie".to_string(),
                    entries: movies
                        .iter()
                        .map(|m| DuplicateEntry {
                            disk: m.disk.clone(),
                            path: m.relative_path.clone(),
                            size_bytes: m.size_bytes,
                            size_human: format_size(m.size_bytes),
                            online: indexer::is_disk_online(&m.disk),
                        })
                        .collect(),
                    total_size_bytes: total_size,
                    total_size_human: format_size(total_size),
                });
            }
        }
    }

    // Find duplicate TV shows
    if show_tv_series {
        let mut tmdb_to_tv_series: std::collections::HashMap<u64, Vec<_>> =
            std::collections::HashMap::new();

        for tvshow in &index.tv_series {
            if let Some(tmdb_id) = tvshow.tmdb_id {
                tmdb_to_tv_series.entry(tmdb_id).or_default().push(tvshow);
            }
        }

        for (tmdb_id, tv_series) in tmdb_to_tv_series {
            if tv_series.len() > 1 {
                let total_size: u64 = tv_series.iter().map(|t| t.size_bytes).sum();
                duplicates.push(DuplicateGroup {
                    tmdb_id,
                    title: tv_series[0].title.clone(),
                    year: tv_series[0].year,
                    media_type: "tv_series".to_string(),
                    entries: tv_series
                        .iter()
                        .map(|t| DuplicateEntry {
                            disk: t.disk.clone(),
                            path: t.relative_path.clone(),
                            size_bytes: t.size_bytes,
                            size_human: format_size(t.size_bytes),
                            online: indexer::is_disk_online(&t.disk),
                        })
                        .collect(),
                    total_size_bytes: total_size,
                    total_size_human: format_size(total_size),
                });
            }
        }
    }

    // Sort by total size (largest first)
    duplicates.sort_by(|a, b| b.total_size_bytes.cmp(&a.total_size_bytes));

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
                    println!(
                        "[{}] {} ({}) - tmdb{} - {} copies - {}",
                        group.media_type.to_uppercase(),
                        group.title,
                        year_str,
                        group.tmdb_id,
                        group.entries.len(),
                        group.total_size_human
                    );
                    for entry in &group.entries {
                        let status = if entry.online { "online" } else { "offline" };
                        println!(
                            "  - {} ({}) [{}]: {}",
                            entry.disk, status, entry.size_human, entry.path
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
                println!(
                    "{}",
                    format!("Found {} duplicate groups:", duplicates.len())
                        .bold()
                        .yellow()
                );
                println!();

                for group in &duplicates {
                    let year_str = group.year.map(|y| format!("({})", y)).unwrap_or_default();
                    let type_badge = match group.media_type.as_str() {
                        "movie" => "[MOVIE]".cyan(),
                        "tv_series" => "[TV_SERIES]".magenta(),
                        _ => "[?]".white(),
                    };

                    println!(
                        "{} {} {} - tmdb{} - {} copies",
                        type_badge,
                        group.title.bold(),
                        year_str,
                        group.tmdb_id,
                        group.entries.len()
                    );
                    println!("  Total size: {}", group.total_size_human.bold().red());
                    println!("  {}", "-".repeat(60));

                    for entry in &group.entries {
                        let status = if entry.online {
                            "Online".green()
                        } else {
                            "Offline".red()
                        };
                        println!(
                            "  {:>12} | {:>10} | {} | {}",
                            entry.disk.bold(),
                            entry.size_human,
                            status,
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
async fn update_collections() -> Result<()> {
    use crate::services::tmdb::{TmdbClient, TmdbConfig};
    use std::path::PathBuf;

    println!(
        "{}",
        "[UPDATE] Fetching collection details from TMDB..."
            .bold()
            .cyan()
    );

    let mut index = indexer::load_central_index()?;

    // Find collections that need updating (total_in_collection == 0)
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

    // Initialize TMDB client
    let tmdb_config = TmdbConfig::from_env()?;
    let tmdb_client = TmdbClient::new(tmdb_config);

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

    for collection_id in &collections_to_update {
        pb.set_message(format!("Collection {}", collection_id));

        match tmdb_client.get_collection_details(*collection_id).await {
            Ok(details) => {
                let total = details.parts.len();

                // Update the collection in index
                if let Some(collection) = index.collections.get_mut(collection_id) {
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
                }

                tracing::debug!(
                    "[COLLECTION] Updated {} (tmdb{}): {} movies",
                    details.name,
                    collection_id,
                    total
                );
            }
            Err(e) => {
                tracing::warn!("[COLLECTION] Failed to fetch {}: {}", collection_id, e);
            }
        }

        pb.inc(1);

        // Rate limiting: small delay between API calls
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
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
    online: bool,
}

/// List movie collections.
async fn list_collections(filter: &str, format: &str, show_paths: bool) -> Result<()> {
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

                    let path = if show_paths {
                        movie_entry.map(|m| m.relative_path.clone())
                    } else {
                        None
                    };

                    let online = if !disk.is_empty() {
                        indexer::is_disk_online(&disk)
                    } else {
                        false
                    };

                    CollectionMovieOutput {
                        title: cm.title.clone(),
                        year: cm.year,
                        disk,
                        path,
                        online,
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
                        let disk_status = if movie.online { "online" } else { "offline" };
                        if let Some(ref path) = movie.path {
                            println!(
                                "  - {} {} [{}:{}] {}",
                                movie.title, year_str, movie.disk, disk_status, path
                            );
                        } else {
                            println!(
                                "  - {} {} [{}:{}]",
                                movie.title, year_str, movie.disk, disk_status
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
                        let disk_status = if movie.online {
                            "Online".green()
                        } else {
                            "Offline".red()
                        };

                        if let Some(ref path) = movie.path {
                            println!(
                                "    {} {} | {} {} | {}",
                                movie.title,
                                year_str,
                                movie.disk.bold(),
                                disk_status,
                                path
                            );
                        } else {
                            println!(
                                "    {} {} | {} {}",
                                movie.title,
                                year_str,
                                movie.disk.bold(),
                                disk_status
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
