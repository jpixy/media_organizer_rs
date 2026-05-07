//! Search command implementation.

use crate::core::indexer;
use anyhow::Result;
use colored::Colorize;

/// Execute search command.
#[allow(clippy::too_many_arguments)]
pub async fn execute_search(
    title: Option<String>,
    actor: Option<String>,
    director: Option<String>,
    collection: Option<String>,
    year: Option<String>,
    genre: Option<String>,
    country: Option<String>,
    show_status: bool,
    format: String,
) -> Result<()> {
    let index = indexer::load_central_index()?;

    // Parse year or year range
    let (year_single, year_range) = if let Some(ref y) = year {
        if y.contains('-') {
            let parts: Vec<&str> = y.split('-').collect();
            if parts.len() == 2 {
                let start: u16 = parts[0].parse().unwrap_or(0);
                let end: u16 = parts[1].parse().unwrap_or(9999);
                (None, Some((start, end)))
            } else {
                (None, None)
            }
        } else {
            (y.parse().ok(), None)
        }
    } else {
        (None, None)
    };

    let results = indexer::search(
        &index,
        title.as_deref(),
        actor.as_deref(),
        director.as_deref(),
        collection.as_deref(),
        year_single,
        year_range,
        genre.as_deref(),
        country.as_deref(),
    );

    match format.as_str() {
        "json" => print_json(&results),
        "simple" => print_simple(&results, show_status),
        _ => print_table(&results, show_status),
    }

    Ok(())
}

/// Print results as JSON.
fn print_json(results: &indexer::SearchResults) {
    #[derive(serde::Serialize)]
    struct JsonOutput {
        movies: Vec<MovieJson>,
        tv_series: Vec<TvSeriesJson>,
    }

    #[derive(serde::Serialize)]
    struct MovieJson {
        title: String,
        original_title: Option<String>,
        year: Option<u16>,
        disk: String,
        country: Option<String>,
        tmdb_id: Option<u64>,
        imdb_id: Option<String>,
    }

    #[derive(serde::Serialize)]
    struct TvSeriesJson {
        title: String,
        original_title: Option<String>,
        year: Option<u16>,
        disk: String,
        country: Option<String>,
        episodes: u32,
        tmdb_id: Option<u64>,
        imdb_id: Option<String>,
    }

    let output = JsonOutput {
        movies: results
            .movies
            .iter()
            .map(|m| MovieJson {
                title: m.title.clone(),
                original_title: m.original_title.clone(),
                year: m.year,
                disk: m.disk.clone(),
                country: m.country.clone(),
                tmdb_id: m.tmdb_id,
                imdb_id: m.imdb_id.clone(),
            })
            .collect(),
        tv_series: results
            .tv_series
            .iter()
            .map(|t| TvSeriesJson {
                title: t.title.clone(),
                original_title: t.original_title.clone(),
                year: t.year,
                disk: t.disk.clone(),
                country: t.country.clone(),
                episodes: t.episodes,
                tmdb_id: t.tmdb_id,
                imdb_id: t.imdb_id.clone(),
            })
            .collect(),
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

/// Print results in simple format.
fn print_simple(results: &indexer::SearchResults, show_status: bool) {
    if results.movies.is_empty() && results.tv_series.is_empty() {
        println!("No results found.");
        return;
    }

    for movie in &results.movies {
        let status = if show_status {
            if indexer::is_disk_online(&movie.disk) {
                " (Online)"
            } else {
                " (Offline)"
            }
        } else {
            ""
        };
        println!(
            "[{}] {} ({}) - {}{}",
            movie.disk,
            movie.title,
            movie.year.map(|y| y.to_string()).unwrap_or_default(),
            movie.country.as_deref().unwrap_or("??"),
            status
        );
    }

    for tvshow in &results.tv_series {
        let status = if show_status {
            if indexer::is_disk_online(&tvshow.disk) {
                " (Online)"
            } else {
                " (Offline)"
            }
        } else {
            ""
        };
        println!(
            "[{}] {} ({}) - {} episodes{}",
            tvshow.disk,
            tvshow.title,
            tvshow.year.map(|y| y.to_string()).unwrap_or_default(),
            tvshow.episodes,
            status
        );
    }
}

/// Print results as table.
fn print_table(results: &indexer::SearchResults, show_status: bool) {
    if results.movies.is_empty() && results.tv_series.is_empty() {
        println!("{}", "No results found.".yellow());
        return;
    }

    let total = results.movies.len() + results.tv_series.len();
    println!("{}", format!("Found {} results:", total).bold().cyan());
    println!();

    if !results.movies.is_empty() {
        println!("{}", format!("Movies ({}):", results.movies.len()).bold());
        println!(
            " {:>4} | {:>4} | {:<40} | {:<12} | {}",
            "#",
            "Year",
            "Title",
            "Disk",
            if show_status { "Status" } else { "Country" }
        );
        println!("{}", "-".repeat(80));

        for (i, movie) in results.movies.iter().enumerate() {
            let title = if movie.title.chars().count() > 38 {
                format!("{}...", movie.title.chars().take(35).collect::<String>())
            } else {
                movie.title.clone()
            };

            let last_col = if show_status {
                if indexer::is_disk_online(&movie.disk) {
                    "Online".green().to_string()
                } else {
                    "Offline".red().to_string()
                }
            } else {
                movie.country.clone().unwrap_or_else(|| "??".to_string())
            };

            println!(
                " {:>4} | {:>4} | {:<40} | {:<12} | {}",
                i + 1,
                movie.year.map(|y| y.to_string()).unwrap_or_default(),
                title,
                movie.disk,
                last_col
            );
        }
        println!();
    }

    if !results.tv_series.is_empty() {
        println!(
            "{}",
            format!("TV Shows ({}):", results.tv_series.len()).bold()
        );
        println!(
            " {:>4} | {:>4} | {:<40} | {:<12} | {}",
            "#",
            "Year",
            "Title",
            "Disk",
            if show_status { "Status" } else { "Episodes" }
        );
        println!("{}", "-".repeat(80));

        for (i, tvshow) in results.tv_series.iter().enumerate() {
            let title = if tvshow.title.chars().count() > 38 {
                format!("{}...", tvshow.title.chars().take(35).collect::<String>())
            } else {
                tvshow.title.clone()
            };

            let last_col = if show_status {
                if indexer::is_disk_online(&tvshow.disk) {
                    "Online".green().to_string()
                } else {
                    "Offline".red().to_string()
                }
            } else {
                tvshow.episodes.to_string()
            };

            println!(
                " {:>4} | {:>4} | {:<40} | {:<12} | {}",
                i + 1,
                tvshow.year.map(|y| y.to_string()).unwrap_or_default(),
                title,
                tvshow.disk,
                last_col
            );
        }
        println!();
    }

    // Print collections if any
    if !results.collections.is_empty() {
        println!("{}", "Collections:".bold());
        for collection in &results.collections {
            println!(
                "  {} - {}/{} owned",
                collection.name, collection.owned_count, collection.total_in_collection
            );
            for movie in &collection.movies {
                let status = if movie.owned {
                    format!("[{}]", movie.disk.as_deref().unwrap_or("?")).green()
                } else {
                    "[Not owned]".red().to_string().into()
                };
                println!(
                    "    {} ({}) - {}",
                    movie.title,
                    movie.year.map(|y| y.to_string()).unwrap_or_default(),
                    status
                );
            }
        }
    }
}
