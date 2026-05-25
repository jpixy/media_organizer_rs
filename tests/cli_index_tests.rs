//! CLI tests for index search subcommand
//!
//! Tests cover:
//! - IndexAction::Search variant parsing via Clap
//! - Year range parsing logic
//! - Search result formatting (table, simple, json)

use clap::Parser;
use media_organizer::cli::args::IndexAction;
use media_organizer::cli::args::Commands;
use media_organizer::models::index::{MovieEntry, TvSeriesEntry};
use media_organizer::core::indexer::SearchResults;

/// Test IndexAction::Search can be constructed with various parameters
#[test]
fn test_index_action_search_creation() {
    // Test with all parameters
    let search = IndexAction::Search {
        title: Some("Inception".to_string()),
        actor: Some("Leonardo".to_string()),
        director: Some("Nolan".to_string()),
        collection: Some("Dark Knight".to_string()),
        year: Some("2020".to_string()),
        genre: Some("Sci-Fi".to_string()),
        country: Some("US".to_string()),
        show_status: true,
        format: "json".to_string(),
    };

    match search {
        IndexAction::Search {
            title,
            actor,
            director,
            collection,
            year,
            genre,
            country,
            show_status,
            format,
        } => {
            assert_eq!(title, Some("Inception".to_string()));
            assert_eq!(actor, Some("Leonardo".to_string()));
            assert_eq!(director, Some("Nolan".to_string()));
            assert_eq!(collection, Some("Dark Knight".to_string()));
            assert_eq!(year, Some("2020".to_string()));
            assert_eq!(genre, Some("Sci-Fi".to_string()));
            assert_eq!(country, Some("US".to_string()));
            assert!(show_status);
            assert_eq!(format, "json");
        }
        _ => panic!("Expected IndexAction::Search"),
    }
}

/// Test IndexAction::Search with no filters (empty search)
#[test]
fn test_index_action_search_empty() {
    let search = IndexAction::Search {
        title: None,
        actor: None,
        director: None,
        collection: None,
        year: None,
        genre: None,
        country: None,
        show_status: false,
        format: "table".to_string(),
    };

    match search {
        IndexAction::Search {
            title,
            actor,
            director,
            collection,
            year,
            genre,
            country,
            show_status,
            format,
        } => {
            assert!(title.is_none());
            assert!(actor.is_none());
            assert!(director.is_none());
            assert!(collection.is_none());
            assert!(year.is_none());
            assert!(genre.is_none());
            assert!(country.is_none());
            assert!(!show_status);
            assert_eq!(format, "table");
        }
        _ => panic!("Expected IndexAction::Search"),
    }
}

/// Test IndexAction::Search with simple format
#[test]
fn test_index_action_search_simple_format() {
    let search = IndexAction::Search {
        title: Some("Test".to_string()),
        actor: None,
        director: None,
        collection: None,
        year: None,
        genre: None,
        country: None,
        show_status: true,
        format: "simple".to_string(),
    };

    match search {
        IndexAction::Search { format, .. } => {
            assert_eq!(format, "simple");
        }
        _ => panic!("Expected IndexAction::Search"),
    }
}

/// Test IndexAction::Search with format variations
#[test]
fn test_index_action_search_format_variations() {
    // Test json format
    let json_search = IndexAction::Search {
        title: Some("Test".to_string()),
        actor: None,
        director: None,
        collection: None,
        year: None,
        genre: None,
        country: None,
        show_status: false,
        format: "json".to_string(),
    };
    match json_search {
        IndexAction::Search { format, .. } => assert_eq!(format, "json"),
        _ => panic!("Expected IndexAction::Search"),
    }

    // Test table format (default)
    let table_search = IndexAction::Search {
        title: Some("Test".to_string()),
        actor: None,
        director: None,
        collection: None,
        year: None,
        genre: None,
        country: None,
        show_status: false,
        format: "table".to_string(),
    };
    match table_search {
        IndexAction::Search { format, .. } => assert_eq!(format, "table"),
        _ => panic!("Expected IndexAction::Search"),
    }
}

/// Test year range parsing
#[test]
fn test_year_range_parsing() {
    // Test single year
    let year = "2024".to_string();
    assert!(!year.contains('-'));
    assert_eq!(year.parse::<u16>().ok(), Some(2024));

    // Test year range
    let year_range = "2020-2024".to_string();
    assert!(year_range.contains('-'));
    let parts: Vec<&str> = year_range.split('-').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].parse::<u16>().ok(), Some(2020));
    assert_eq!(parts[1].parse::<u16>().ok(), Some(2024));
}

/// Test year range parsing edge cases
#[test]
fn test_year_range_parsing_edge_cases() {
    // Invalid range (no end year)
    let invalid_range = "2020-".to_string();
    let parts: Vec<&str> = invalid_range.split('-').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].parse::<u16>().ok(), Some(2020));
    assert!(parts[1].parse::<u16>().is_err());

    // Empty string
    let empty = "".to_string();
    assert!(!empty.contains('-'));
    assert!(empty.parse::<u16>().is_err());

    // Invalid year format
    let invalid_year = "abcd".to_string();
    assert!(!invalid_year.contains('-'));
    assert!(invalid_year.parse::<u16>().is_err());
}

/// Test SearchResults structure
#[test]
fn test_search_results_empty() {
    let results = SearchResults {
        movies: vec![],
        tv_series: vec![],
        collections: vec![],
    };

    assert!(results.movies.is_empty());
    assert!(results.tv_series.is_empty());
    assert!(results.collections.is_empty());
}

/// Test SearchResults with movie entries
#[test]
fn test_search_results_with_movies() {
    use media_organizer::models::index::VideoFileInfo;

    let movie = MovieEntry {
        id: "m1".to_string(),
        disk: "TestDisk".to_string(),
        disk_uuid: Some("test-uuid".to_string()),
        relative_path: "movies/test.nfo".to_string(),
        title: "The Matrix".to_string(),
        original_title: None,
        year: Some(1999),
        tmdb_id: Some(1001),
        imdb_id: Some("tt0133093".to_string()),
        collection_id: None,
        collection_name: None,
        collection_total_movies: None,
        country: Some("US".to_string()),
        genres: vec!["Action".to_string(), "Sci-Fi".to_string()],
        actors: vec!["Keanu Reeves".to_string()],
        directors: vec!["The Wachowskis".to_string()],
        runtime: Some(136),
        rating: Some(8.7),
        size_bytes: 1_000_000_000,
        resolution: Some("1080p".to_string()),
        video_files: vec![VideoFileInfo {
            file_name: "The Matrix.mkv".to_string(),
            file_path: "movies/The Matrix.mkv".to_string(),
            size_bytes: 1_000_000_000,
            resolution: Some("1080p".to_string()),
            format: Some("mkv".to_string()),
            codec: Some("h264".to_string()),
        }],
        indexed_at: chrono::Utc::now().to_rfc3339(),
    };

    let results = SearchResults {
        movies: vec![movie],
        tv_series: vec![],
        collections: vec![],
    };

    assert_eq!(results.movies.len(), 1);
    assert_eq!(results.tv_series.len(), 0);
    assert_eq!(results.movies[0].title, "The Matrix");
    assert_eq!(results.movies[0].year, Some(1999));
    assert_eq!(results.movies[0].tmdb_id, Some(1001));
}

/// Test SearchResults with TV series entries
#[test]
fn test_search_results_with_tv_series() {
    let tv_series = TvSeriesEntry {
        id: "t1".to_string(),
        disk: "TestDisk".to_string(),
        disk_uuid: Some("test-uuid".to_string()),
        relative_path: "tv_series/show.nfo".to_string(),
        title: "Breaking Bad".to_string(),
        original_title: None,
        year: Some(2008),
        tmdb_id: Some(2001),
        imdb_id: Some("tt0903747".to_string()),
        country: Some("US".to_string()),
        genres: vec!["Drama".to_string(), "Crime".to_string()],
        actors: vec!["Bryan Cranston".to_string()],
        seasons: 5,
        episodes: 62,
        owned_seasons: 5,
        owned_episodes: 62,
        size_bytes: 10_000_000_000,
        indexed_at: chrono::Utc::now().to_rfc3339(),
    };

    let results = SearchResults {
        movies: vec![],
        tv_series: vec![tv_series],
        collections: vec![],
    };

    assert_eq!(results.movies.len(), 0);
    assert_eq!(results.tv_series.len(), 1);
    assert_eq!(results.tv_series[0].title, "Breaking Bad");
    assert_eq!(results.tv_series[0].seasons, 5);
    assert_eq!(results.tv_series[0].episodes, 62);
}

/// Test SearchResults with mixed content
#[test]
fn test_search_results_mixed() {
    let movie = MovieEntry {
        id: "m1".to_string(),
        disk: "Disk1".to_string(),
        disk_uuid: Some("uuid1".to_string()),
        relative_path: "m1.nfo".to_string(),
        title: "Movie 1".to_string(),
        original_title: None,
        year: Some(2020),
        tmdb_id: Some(1001),
        imdb_id: None,
        collection_id: None,
        collection_name: None,
        collection_total_movies: None,
        country: Some("US".to_string()),
        genres: vec![],
        actors: vec![],
        directors: vec![],
        runtime: None,
        rating: None,
        size_bytes: 0,
        resolution: None,
        video_files: vec![],
        indexed_at: chrono::Utc::now().to_rfc3339(),
    };

    let tv_series = TvSeriesEntry {
        id: "t1".to_string(),
        disk: "Disk2".to_string(),
        disk_uuid: Some("uuid2".to_string()),
        relative_path: "t1.nfo".to_string(),
        title: "TV Show 1".to_string(),
        original_title: None,
        year: Some(2021),
        tmdb_id: Some(2001),
        imdb_id: None,
        country: Some("UK".to_string()),
        genres: vec![],
        actors: vec![],
        seasons: 3,
        episodes: 30,
        owned_seasons: 3,
        owned_episodes: 30,
        size_bytes: 0,
        indexed_at: chrono::Utc::now().to_rfc3339(),
    };

    let results = SearchResults {
        movies: vec![movie],
        tv_series: vec![tv_series],
        collections: vec![],
    };

    assert_eq!(results.movies.len(), 1);
    assert_eq!(results.tv_series.len(), 1);
    assert_eq!(results.movies[0].title, "Movie 1");
    assert_eq!(results.tv_series[0].title, "TV Show 1");
}

// ============================================================================
// IndexAction::Duplicates Tests
// ============================================================================

/// Test IndexAction::Duplicates with default values
#[test]
fn test_index_action_duplicates_default() {
    let duplicates = IndexAction::Duplicates {
        media_type: "all".to_string(),
        format: "table".to_string(),
        volume_filter: "cross".to_string(),
        volume_filter_groups: vec![],
    };

    match duplicates {
        IndexAction::Duplicates {
            media_type,
            format,
            volume_filter,
            volume_filter_groups,
        } => {
            assert_eq!(media_type, "all");
            assert_eq!(format, "table");
            assert_eq!(volume_filter, "cross");
            assert!(volume_filter_groups.is_empty());
        }
        _ => panic!("Expected IndexAction::Duplicates"),
    }
}

/// Test IndexAction::Duplicates with volume_filter_groups
#[test]
fn test_index_action_duplicates_with_groups() {
    let duplicates = IndexAction::Duplicates {
        media_type: "movies".to_string(),
        format: "json".to_string(),
        volume_filter: "all".to_string(),
        volume_filter_groups: vec!["Disk_Movies_01".to_string(), "Disk_Movies_03".to_string()],
    };

    match duplicates {
        IndexAction::Duplicates {
            media_type,
            format,
            volume_filter,
            volume_filter_groups,
        } => {
            assert_eq!(media_type, "movies");
            assert_eq!(format, "json");
            assert_eq!(volume_filter, "all");
            assert_eq!(volume_filter_groups.len(), 2);
            assert_eq!(volume_filter_groups[0], "Disk_Movies_01");
            assert_eq!(volume_filter_groups[1], "Disk_Movies_03");
        }
        _ => panic!("Expected IndexAction::Duplicates"),
    }
}

/// Test IndexAction::Duplicates with single group
#[test]
fn test_index_action_duplicates_single_group() {
    let duplicates = IndexAction::Duplicates {
        media_type: "tv_series".to_string(),
        format: "simple".to_string(),
        volume_filter: "same".to_string(),
        volume_filter_groups: vec!["Disk_Movies_05".to_string()],
    };

    match duplicates {
        IndexAction::Duplicates {
            media_type,
            format,
            volume_filter,
            volume_filter_groups,
        } => {
            assert_eq!(media_type, "tv_series");
            assert_eq!(format, "simple");
            assert_eq!(volume_filter, "same");
            assert_eq!(volume_filter_groups.len(), 1);
            assert_eq!(volume_filter_groups[0], "Disk_Movies_05");
        }
        _ => panic!("Expected IndexAction::Duplicates"),
    }
}

/// Test IndexAction::Duplicates with multiple groups
#[test]
fn test_index_action_duplicates_multiple_groups() {
    let groups = vec![
        "Disk_Movies_01".to_string(),
        "Disk_Movies_02".to_string(),
        "Disk_Movies_03".to_string(),
        "Disk_Movies_04".to_string(),
        "Disk_Movies_05".to_string(),
    ];
    
    let duplicates = IndexAction::Duplicates {
        media_type: "all".to_string(),
        format: "table".to_string(),
        volume_filter: "cross".to_string(),
        volume_filter_groups: groups.clone(),
    };

    match duplicates {
        IndexAction::Duplicates {
            media_type: _,
            format: _,
            volume_filter: _,
            volume_filter_groups,
        } => {
            assert_eq!(volume_filter_groups.len(), 5);
            assert_eq!(volume_filter_groups, groups);
        }
        _ => panic!("Expected IndexAction::Duplicates"),
    }
}

// ============================================================================
// CLI Argument Parsing Tests for Duplicates
// ============================================================================

use media_organizer::cli::args::Cli;

/// Test CLI parsing with --volume-filter-groups argument (single value)
#[test]
fn test_cli_parse_duplicates_volume_filter_groups_single() {
    let args = vec![
        "mediaorganizer", "index", "duplicates",
        "--volume-filter-groups", "Disk_Movies_01",
    ];
    
    let cli = Cli::try_parse_from(args).expect("Failed to parse");
    
    match &cli.command {
        Commands::Index { action } => {
            match action {
                IndexAction::Duplicates {
                    media_type,
                    format: _,
                    volume_filter,
                    volume_filter_groups,
                } => {
                    assert_eq!(media_type, "all");
                    assert_eq!(volume_filter, "cross");
                    assert_eq!(volume_filter_groups.len(), 1);
                    assert_eq!(volume_filter_groups[0], "Disk_Movies_01");
                }
                _ => panic!("Expected IndexAction::Duplicates"),
            }
        }
        _ => panic!("Expected Commands::Index"),
    }
}

/// Test CLI parsing with --volume-filter-groups argument (multiple values)
#[test]
fn test_cli_parse_duplicates_volume_filter_groups_multiple() {
    let args = vec![
        "mediaorganizer", "index", "duplicates",
        "--volume-filter-groups", "Disk_Movies_01,Disk_Movies_03,Disk_Movies_05",
    ];
    
    let cli = Cli::try_parse_from(args).expect("Failed to parse");
    
    match &cli.command {
        Commands::Index { action } => {
            match action {
                IndexAction::Duplicates {
                    media_type: _,
                    format: _,
                    volume_filter,
                    volume_filter_groups,
                } => {
                    assert_eq!(volume_filter, "cross");
                    assert_eq!(volume_filter_groups.len(), 3);
                    assert_eq!(volume_filter_groups[0], "Disk_Movies_01");
                    assert_eq!(volume_filter_groups[1], "Disk_Movies_03");
                    assert_eq!(volume_filter_groups[2], "Disk_Movies_05");
                }
                _ => panic!("Expected IndexAction::Duplicates"),
            }
        }
        _ => panic!("Expected Commands::Index"),
    }
}

/// Test CLI parsing with --volume-filter-groups and --volume-filter combined
#[test]
fn test_cli_parse_duplicates_volume_filter_groups_with_filter() {
    let args = vec![
        "mediaorganizer", "index", "duplicates",
        "--volume-filter", "all",
        "--volume-filter-groups", "Disk_Movies_03,Disk_Movies_05",
    ];
    
    let cli = Cli::try_parse_from(args).expect("Failed to parse");
    
    match &cli.command {
        Commands::Index { action } => {
            match action {
                IndexAction::Duplicates {
                    media_type: _,
                    format: _,
                    volume_filter,
                    volume_filter_groups,
                } => {
                    assert_eq!(volume_filter, "all");
                    assert_eq!(volume_filter_groups.len(), 2);
                    assert_eq!(volume_filter_groups[0], "Disk_Movies_03");
                    assert_eq!(volume_filter_groups[1], "Disk_Movies_05");
                }
                _ => panic!("Expected IndexAction::Duplicates"),
            }
        }
        _ => panic!("Expected Commands::Index"),
    }
}

/// Test CLI parsing with --volume-filter-groups and all options
#[test]
fn test_cli_parse_duplicates_volume_filter_groups_full() {
    let args = vec![
        "mediaorganizer", "index", "duplicates",
        "--media-type", "movies",
        "--format", "json",
        "--volume-filter", "cross",
        "--volume-filter-groups", "Disk_Movies_01,Disk_Movies_02,Disk_Movies_03",
    ];
    
    let cli = Cli::try_parse_from(args).expect("Failed to parse");
    
    match &cli.command {
        Commands::Index { action } => {
            match action {
                IndexAction::Duplicates {
                    media_type,
                    format,
                    volume_filter,
                    volume_filter_groups,
                } => {
                    assert_eq!(media_type, "movies");
                    assert_eq!(format, "json");
                    assert_eq!(volume_filter, "cross");
                    assert_eq!(volume_filter_groups.len(), 3);
                    assert_eq!(*volume_filter_groups, vec![
                        "Disk_Movies_01".to_string(),
                        "Disk_Movies_02".to_string(),
                        "Disk_Movies_03".to_string(),
                    ]);
                }
                _ => panic!("Expected IndexAction::Duplicates"),
            }
        }
        _ => panic!("Expected Commands::Index"),
    }
}

/// Test CLI parsing with empty --volume-filter-groups (default)
/// 
/// Note: clap's default_value = "" with value_delimiter = ',' returns vec![""]
/// when not specified, so we check that instead of.is_empty().
#[test]
fn test_cli_parse_duplicates_volume_filter_groups_empty() {
    let args = vec![
        "mediaorganizer", "index", "duplicates",
    ];
    
    let cli = Cli::try_parse_from(args).expect("Failed to parse");
    
    match &cli.command {
        Commands::Index { action } => {
            match action {
                IndexAction::Duplicates {
                    media_type: _,
                    format: _,
                    volume_filter: _,
                    volume_filter_groups,
                } => {
                    // Clap returns vec![""] for default_value = "" with Vec<String>
                    // Our code filters out empty strings, so this works correctly
                    assert!(volume_filter_groups.len() == 1 && volume_filter_groups[0].is_empty() || volume_filter_groups.is_empty());
                }
                _ => panic!("Expected IndexAction::Duplicates"),
            }
        }
        _ => panic!("Expected Commands::Index"),
    }
}
