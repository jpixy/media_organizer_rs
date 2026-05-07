//! Integration tests for the indexer module.
//!
//! Tests cover:
//! - Composite storage (one disk label with multiple media types)
//! - Search functionality (by title, year, actor, director, genre, country)
//! - Collection indexing and completeness detection
//! - Cross-disk duplicate detection
//! - Edge cases (empty scans, repeated scans, path updates)

use media_organizer::core::indexer::{merge_disk_into_central, search};
use media_organizer::models::index::{CentralIndex, DiskIndex, DiskInfo, MovieEntry, TvSeriesEntry};
use std::collections::HashMap;

// ========== TEST FIXTURES ==========

/// Create a test DiskIndex with movies
fn create_movie_disk_index(label: &str, path: &str, movies: Vec<MovieEntry>) -> DiskIndex {
    let mut paths = HashMap::new();
    paths.insert("movies".to_string(), path.to_string());

    DiskIndex {
        version: "1.0".to_string(),
        disk: DiskInfo {
            label: label.to_string(),
            uuid: Some("test-uuid".to_string()),
            last_indexed: chrono::Utc::now().to_rfc3339(),
            movie_count: movies.len(),
            tv_series_count: 0,
            total_size_bytes: movies.iter().map(|m| m.size_bytes).sum(),
            base_path: path.to_string(),
            paths,
        },
        movies,
        tv_series: Vec::new(),
    }
}

/// Create a test DiskIndex with TV shows
fn create_tv_series_disk_index(label: &str, path: &str, tv_series: Vec<TvSeriesEntry>) -> DiskIndex {
    let mut paths = HashMap::new();
    paths.insert("tv_series".to_string(), path.to_string());

    DiskIndex {
        version: "1.0".to_string(),
        disk: DiskInfo {
            label: label.to_string(),
            uuid: Some("test-uuid".to_string()),
            last_indexed: chrono::Utc::now().to_rfc3339(),
            movie_count: 0,
            tv_series_count: tv_series.len(),
            total_size_bytes: tv_series.iter().map(|t| t.size_bytes).sum(),
            base_path: path.to_string(),
            paths,
        },
        movies: Vec::new(),
        tv_series,
    }
}

/// Create a test movie entry
fn create_test_movie(id: &str, title: &str, disk: &str, tmdb_id: u64) -> MovieEntry {
    MovieEntry {
        id: id.to_string(),
        disk: disk.to_string(),
        disk_uuid: Some("test-uuid".to_string()),
        relative_path: format!("{}/movie.nfo", title),
        title: title.to_string(),
        original_title: None,
        year: Some(2024),
        tmdb_id: Some(tmdb_id),
        imdb_id: None,
        collection_id: None,
        collection_name: None,
        collection_total_movies: None,
        country: Some("US".to_string()),
        genres: vec!["Action".to_string()],
        actors: vec!["Actor1".to_string()],
        directors: vec!["Director1".to_string()],
        runtime: Some(120),
        rating: Some(7.5),
        size_bytes: 1_000_000_000,
        resolution: Some("1080p".to_string()),
        indexed_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Create a test TV show entry
fn create_test_tv_series(id: &str, title: &str, disk: &str, tmdb_id: u64) -> TvSeriesEntry {
    TvSeriesEntry {
        id: id.to_string(),
        disk: disk.to_string(),
        disk_uuid: Some("test-uuid".to_string()),
        relative_path: format!("{}/tvshow.nfo", title),
        title: title.to_string(),
        original_title: None,
        year: Some(2024),
        tmdb_id: Some(tmdb_id),
        imdb_id: None,
        country: Some("US".to_string()),
        genres: vec!["Drama".to_string()],
        actors: vec!["Actor1".to_string()],
        seasons: 3,
        episodes: 24,
        size_bytes: 5_000_000_000,
        indexed_at: chrono::Utc::now().to_rfc3339(),
    }
}

// ========== COMPOSITE STORAGE TESTS ==========

#[test]
fn test_merge_disk_into_central_new_disk() {
    let mut central = CentralIndex::default();

    let movies = vec![
        create_test_movie("m1", "Movie 1", "TestDisk", 1001),
        create_test_movie("m2", "Movie 2", "TestDisk", 1002),
    ];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);

    merge_disk_into_central(&mut central, disk);

    assert_eq!(central.disks.len(), 1);
    assert_eq!(central.movies.len(), 2);
    assert_eq!(central.tv_series.len(), 0);

    let disk_info = central.disks.get("TestDisk").unwrap();
    assert_eq!(disk_info.movie_count, 2);
    assert_eq!(disk_info.tv_series_count, 0);
    assert!(disk_info.paths.contains_key("movies"));
}

#[test]
fn test_merge_disk_into_central_composite_storage() {
    let mut central = CentralIndex::default();

    // First: add movies
    let movies = vec![create_test_movie("m1", "Movie 1", "TestDisk", 1001)];
    let movie_disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, movie_disk);

    assert_eq!(central.movies.len(), 1);
    assert_eq!(central.tv_series.len(), 0);

    // Second: add tv_series (same disk label, different path)
    let tv_series = vec![create_test_tv_series("t1", "TV Show 1", "TestDisk", 2001)];
    let tv_series_disk = create_tv_series_disk_index("TestDisk", "/mnt/TestDisk/TV_Series", tv_series);
    merge_disk_into_central(&mut central, tv_series_disk);

    // Verify composite storage works
    assert_eq!(central.disks.len(), 1, "Should still be one disk");
    assert_eq!(central.movies.len(), 1, "Movies should be preserved");
    assert_eq!(central.tv_series.len(), 1, "TV shows should be added");

    let disk_info = central.disks.get("TestDisk").unwrap();
    assert_eq!(disk_info.movie_count, 1);
    assert_eq!(disk_info.tv_series_count, 1);
    assert!(
        disk_info.paths.contains_key("movies"),
        "Movies path should exist"
    );
    assert!(
        disk_info.paths.contains_key("tv_series"),
        "TV_Series path should exist"
    );
    assert_eq!(
        disk_info.paths.get("movies").unwrap(),
        "/mnt/TestDisk/Movies"
    );
    assert_eq!(
        disk_info.paths.get("tv_series").unwrap(),
        "/mnt/TestDisk/TV_Series"
    );
}

#[test]
fn test_merge_disk_into_central_update_movies_only() {
    let mut central = CentralIndex::default();

    // Initial: add movies and tv_series
    let movies = vec![create_test_movie("m1", "Movie 1", "TestDisk", 1001)];
    let movie_disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, movie_disk);

    let tv_series = vec![create_test_tv_series("t1", "TV Show 1", "TestDisk", 2001)];
    let tv_series_disk = create_tv_series_disk_index("TestDisk", "/mnt/TestDisk/TV_Series", tv_series);
    merge_disk_into_central(&mut central, tv_series_disk);

    assert_eq!(central.movies.len(), 1);
    assert_eq!(central.tv_series.len(), 1);

    // Update: re-scan movies with new movie
    let new_movies = vec![
        create_test_movie("m1", "Movie 1", "TestDisk", 1001),
        create_test_movie("m2", "Movie 2", "TestDisk", 1002),
    ];
    let updated_movie_disk =
        create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", new_movies);
    merge_disk_into_central(&mut central, updated_movie_disk);

    // Verify: movies updated, tv_series preserved
    assert_eq!(central.movies.len(), 2, "Movies should be updated to 2");
    assert_eq!(central.tv_series.len(), 1, "TV shows should be preserved");

    let disk_info = central.disks.get("TestDisk").unwrap();
    assert_eq!(disk_info.movie_count, 2);
    assert_eq!(disk_info.tv_series_count, 1);
}

#[test]
fn test_merge_disk_into_central_separate_disks() {
    let mut central = CentralIndex::default();

    // Disk 1: movies
    let movies = vec![create_test_movie("m1", "Movie 1", "Disk1", 1001)];
    let disk1 = create_movie_disk_index("Disk1", "/mnt/Disk1/Movies", movies);
    merge_disk_into_central(&mut central, disk1);

    // Disk 2: different disk, movies
    let movies2 = vec![create_test_movie("m2", "Movie 2", "Disk2", 1002)];
    let disk2 = create_movie_disk_index("Disk2", "/mnt/Disk2/Movies", movies2);
    merge_disk_into_central(&mut central, disk2);

    assert_eq!(central.disks.len(), 2);
    assert_eq!(central.movies.len(), 2);

    // Verify each disk has its own entry
    assert!(central.disks.contains_key("Disk1"));
    assert!(central.disks.contains_key("Disk2"));
}

/// Test: Composite storage order independence
/// Verifies that the order of adding movies vs tv_series doesn't affect the result
#[test]
fn test_composite_storage_order_independence() {
    // Order 1: movies first, then tv_series
    let mut central1 = CentralIndex::default();
    let movies = vec![create_test_movie("m1", "Movie 1", "Disk", 1001)];
    let movie_disk = create_movie_disk_index("Disk", "/mnt/Disk/Movies", movies);
    merge_disk_into_central(&mut central1, movie_disk);

    let tv_series = vec![create_test_tv_series("t1", "Show 1", "Disk", 2001)];
    let tv_series_disk = create_tv_series_disk_index("Disk", "/mnt/Disk/TV_Series", tv_series);
    merge_disk_into_central(&mut central1, tv_series_disk);

    // Order 2: tv_series first, then movies
    let mut central2 = CentralIndex::default();
    let tv_series2 = vec![create_test_tv_series("t1", "Show 1", "Disk", 2001)];
    let tv_series_disk2 = create_tv_series_disk_index("Disk", "/mnt/Disk/TV_Series", tv_series2);
    merge_disk_into_central(&mut central2, tv_series_disk2);

    let movies2 = vec![create_test_movie("m1", "Movie 1", "Disk", 1001)];
    let movie_disk2 = create_movie_disk_index("Disk", "/mnt/Disk/Movies", movies2);
    merge_disk_into_central(&mut central2, movie_disk2);

    // Both should have identical results
    assert_eq!(central1.disks.len(), central2.disks.len());
    assert_eq!(central1.movies.len(), central2.movies.len());
    assert_eq!(central1.tv_series.len(), central2.tv_series.len());

    // Both should have both paths
    let disk1 = central1.disks.get("Disk").unwrap();
    let disk2 = central2.disks.get("Disk").unwrap();
    assert_eq!(disk1.paths.len(), disk2.paths.len());
    assert!(disk1.paths.contains_key("movies"));
    assert!(disk1.paths.contains_key("tv_series"));
}

// ========== EDGE CASE TESTS ==========

/// Test: Re-scanning updates path correctly
/// Verifies that re-scanning with a different path updates the stored path
#[test]
fn test_rescan_updates_path() {
    let mut central = CentralIndex::default();

    // Initial scan at path A
    let movies = vec![create_test_movie("m1", "Movie 1", "Disk", 1001)];
    let disk = create_movie_disk_index("Disk", "/mnt/old_path/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    assert_eq!(
        central.disks.get("Disk").unwrap().paths.get("movies"),
        Some(&"/mnt/old_path/Movies".to_string())
    );

    // Re-scan at path B (disk moved to new location)
    let movies2 = vec![
        create_test_movie("m1", "Movie 1", "Disk", 1001),
        create_test_movie("m2", "Movie 2", "Disk", 1002),
    ];
    let disk2 = create_movie_disk_index("Disk", "/mnt/new_path/Movies", movies2);
    merge_disk_into_central(&mut central, disk2);

    // Path should be updated
    assert_eq!(
        central.disks.get("Disk").unwrap().paths.get("movies"),
        Some(&"/mnt/new_path/Movies".to_string())
    );
    // Movies should be updated
    assert_eq!(central.movies.len(), 2);
}

/// Test: Empty scan doesn't corrupt existing data
/// Edge case: scanning an empty directory shouldn't delete existing entries
#[test]
fn test_empty_scan_preserves_other_media_type() {
    let mut central = CentralIndex::default();

    // Add movies
    let movies = vec![create_test_movie("m1", "Movie 1", "Disk", 1001)];
    let movie_disk = create_movie_disk_index("Disk", "/mnt/Disk/Movies", movies);
    merge_disk_into_central(&mut central, movie_disk);

    assert_eq!(central.movies.len(), 1);

    // Scan tv_series with empty result (no tv_series found)
    let empty_tv_series_disk = create_tv_series_disk_index("Disk", "/mnt/Disk/TV_Series", vec![]);
    merge_disk_into_central(&mut central, empty_tv_series_disk);

    // Movies should still exist
    assert_eq!(central.movies.len(), 1, "Movies should be preserved");
    assert_eq!(central.tv_series.len(), 0, "TV_Series should be empty");

    // Both paths should exist
    let disk = central.disks.get("Disk").unwrap();
    assert!(disk.paths.contains_key("movies"));
    assert!(disk.paths.contains_key("tv_series"));
}

/// Test: No duplicate entries after repeated scans
#[test]
fn test_rescan_no_duplicates() {
    let mut central = CentralIndex::default();

    let movies = vec![
        create_test_movie("m1", "Movie 1", "Disk", 1001),
        create_test_movie("m2", "Movie 2", "Disk", 1002),
    ];

    // Scan 3 times
    for _ in 0..3 {
        let disk = create_movie_disk_index("Disk", "/mnt/Disk/Movies", movies.clone());
        merge_disk_into_central(&mut central, disk);
    }

    // Should still only have 2 movies (not 6)
    assert_eq!(
        central.movies.len(),
        2,
        "No duplicates after repeated scans"
    );
    assert_eq!(central.disks.len(), 1, "Only one disk entry");
}

// ========== SEARCH TESTS ==========

#[test]
fn test_search_by_title() {
    let mut central = CentralIndex::default();

    let movies = vec![
        create_test_movie("m1", "The Matrix", "TestDisk", 1001),
        create_test_movie("m2", "Inception", "TestDisk", 1002),
    ];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    let results = search(
        &central,
        Some("matrix"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results.movies.len(), 1);
    assert_eq!(results.movies[0].title, "The Matrix");
}

#[test]
fn test_search_by_year() {
    let mut central = CentralIndex::default();

    let mut movie1 = create_test_movie("m1", "Movie 2020", "TestDisk", 1001);
    movie1.year = Some(2020);
    let mut movie2 = create_test_movie("m2", "Movie 2024", "TestDisk", 1002);
    movie2.year = Some(2024);

    let movies = vec![movie1, movie2];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    let results = search(
        &central,
        None,
        None,
        None,
        None,
        Some(2024),
        None,
        None,
        None,
    );

    assert_eq!(results.movies.len(), 1);
    assert_eq!(results.movies[0].title, "Movie 2024");
}

/// Test: Search returns empty for non-existent content
#[test]
fn test_search_nonexistent_returns_empty() {
    let mut central = CentralIndex::default();

    let movies = vec![create_test_movie("m1", "The Matrix", "TestDisk", 1001)];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    // Search for non-existent title
    let results = search(
        &central,
        Some("NonExistentMovie12345"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results.movies.len(), 0);
    assert_eq!(results.tv_series.len(), 0);
}

/// Test: Search is case-insensitive
#[test]
fn test_search_case_insensitive() {
    let mut central = CentralIndex::default();

    let movies = vec![create_test_movie("m1", "The Matrix", "TestDisk", 1001)];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    // Uppercase search
    let results1 = search(
        &central,
        Some("THE MATRIX"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    // Lowercase search
    let results2 = search(
        &central,
        Some("the matrix"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    // Mixed case search
    let results3 = search(
        &central,
        Some("ThE MaTrIx"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results1.movies.len(), 1);
    assert_eq!(results2.movies.len(), 1);
    assert_eq!(results3.movies.len(), 1);
}

/// Test: Partial title match works
#[test]
fn test_search_partial_match() {
    let mut central = CentralIndex::default();

    let movies = vec![
        create_test_movie("m1", "The Matrix Reloaded", "TestDisk", 1001),
        create_test_movie("m2", "The Matrix Revolutions", "TestDisk", 1002),
        create_test_movie("m3", "Inception", "TestDisk", 1003),
    ];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    // Partial match should find both Matrix movies
    let results = search(
        &central,
        Some("Matrix"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results.movies.len(), 2);
    assert!(results.movies.iter().all(|m| m.title.contains("Matrix")));
}

#[test]
fn test_search_year_range() {
    let mut central = CentralIndex::default();

    let mut movie1 = create_test_movie("m1", "Movie 2018", "TestDisk", 1001);
    movie1.year = Some(2018);
    let mut movie2 = create_test_movie("m2", "Movie 2020", "TestDisk", 1002);
    movie2.year = Some(2020);
    let mut movie3 = create_test_movie("m3", "Movie 2024", "TestDisk", 1003);
    movie3.year = Some(2024);

    let movies = vec![movie1, movie2, movie3];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    let results = search(
        &central,
        None,
        None,
        None,
        None,
        None,
        Some((2019, 2022)), // Year range
        None,
        None,
    );

    assert_eq!(results.movies.len(), 1);
    assert_eq!(results.movies[0].title, "Movie 2020");
}

// ========== COLLECTION TESTS ==========

#[test]
fn test_collection_indexing() {
    let mut central = CentralIndex::default();

    let mut movie1 = create_test_movie("m1", "Pirates 1", "TestDisk", 1001);
    movie1.collection_id = Some(100);
    movie1.collection_name = Some("Pirates of the Caribbean Collection".to_string());
    movie1.collection_total_movies = Some(5);

    let mut movie2 = create_test_movie("m2", "Pirates 2", "TestDisk", 1002);
    movie2.collection_id = Some(100);
    movie2.collection_name = Some("Pirates of the Caribbean Collection".to_string());
    movie2.collection_total_movies = Some(5);

    let movies = vec![movie1, movie2];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    // Verify collection was created
    assert_eq!(central.collections.len(), 1);
    let collection = central.collections.get(&100).unwrap();
    assert_eq!(collection.name, "Pirates of the Caribbean Collection");
    assert_eq!(collection.owned_count, 2);
    assert_eq!(collection.total_in_collection, 5);
    assert_eq!(collection.movies.len(), 2);
}

/// Test: Collection without total_in_collection (fallback heuristic)
#[test]
fn test_collection_without_total() {
    let mut central = CentralIndex::default();

    // Collection without total_in_collection set
    let mut movie1 = create_test_movie("m1", "Pirates 1", "TestDisk", 1001);
    movie1.collection_id = Some(100);
    movie1.collection_name = Some("Pirates Collection".to_string());
    movie1.collection_total_movies = None; // Unknown total

    let movies = vec![movie1];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    let collection = central.collections.get(&100).unwrap();
    assert_eq!(collection.owned_count, 1);
    assert_eq!(collection.total_in_collection, 0); // Unknown

    // With fallback heuristic: 1 movie = likely incomplete
    assert_eq!(central.statistics.incomplete_collections, 1);
}

#[test]
fn test_collection_complete_incomplete() {
    let mut central = CentralIndex::default();

    // Collection A: 2 of 2 movies owned (complete)
    let mut movie1 = create_test_movie("m1", "Trilogy A-1", "TestDisk", 1001);
    movie1.collection_id = Some(100);
    movie1.collection_name = Some("Complete Trilogy".to_string());
    movie1.collection_total_movies = Some(2);

    let mut movie2 = create_test_movie("m2", "Trilogy A-2", "TestDisk", 1002);
    movie2.collection_id = Some(100);
    movie2.collection_name = Some("Complete Trilogy".to_string());
    movie2.collection_total_movies = Some(2);

    // Collection B: 1 of 3 movies owned (incomplete)
    let mut movie3 = create_test_movie("m3", "Series B-1", "TestDisk", 1003);
    movie3.collection_id = Some(200);
    movie3.collection_name = Some("Incomplete Series".to_string());
    movie3.collection_total_movies = Some(3);

    let movies = vec![movie1, movie2, movie3];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    // Verify collection statistics
    assert_eq!(central.statistics.complete_collections, 1);
    assert_eq!(central.statistics.incomplete_collections, 1);

    // Verify collection details
    let collection_a = central.collections.get(&100).unwrap();
    assert_eq!(collection_a.owned_count, 2);
    assert_eq!(collection_a.total_in_collection, 2);

    let collection_b = central.collections.get(&200).unwrap();
    assert_eq!(collection_b.owned_count, 1);
    assert_eq!(collection_b.total_in_collection, 3);
}

// ========== DUPLICATE DETECTION TESTS ==========

#[test]
fn test_find_duplicates_same_tmdb_id() {
    let mut central = CentralIndex::default();

    // Same movie on two different disks (same TMDB ID)
    let movie1 = create_test_movie("m1", "The Matrix", "Disk1", 1001);
    let disk1 = create_movie_disk_index("Disk1", "/mnt/Disk1/Movies", vec![movie1]);
    merge_disk_into_central(&mut central, disk1);

    let movie2 = create_test_movie("m2", "The Matrix HD", "Disk2", 1001); // Same TMDB ID!
    let disk2 = create_movie_disk_index("Disk2", "/mnt/Disk2/Movies", vec![movie2]);
    merge_disk_into_central(&mut central, disk2);

    // Find duplicates by TMDB ID
    let mut tmdb_count: HashMap<u64, Vec<&MovieEntry>> = HashMap::new();
    for movie in &central.movies {
        if let Some(tmdb_id) = movie.tmdb_id {
            tmdb_count.entry(tmdb_id).or_default().push(movie);
        }
    }

    let duplicates: Vec<_> = tmdb_count
        .iter()
        .filter(|(_, movies)| movies.len() > 1)
        .collect();

    assert_eq!(duplicates.len(), 1);
    let (tmdb_id, movies) = duplicates[0];
    assert_eq!(*tmdb_id, 1001);
    assert_eq!(movies.len(), 2);

    // Verify they are on different disks
    let disks: std::collections::HashSet<_> = movies.iter().map(|m| &m.disk).collect();
    assert_eq!(disks.len(), 2);
}

/// Test: Multiple disks with same movie (cross-disk duplicate detection)
#[test]
fn test_cross_disk_duplicates() {
    let mut central = CentralIndex::default();

    // Same movie on 3 different disks (same TMDB ID, different quality)
    let mut movie1 = create_test_movie("m1", "Inception 720p", "Disk1", 27205);
    movie1.resolution = Some("720p".to_string());
    movie1.size_bytes = 2_000_000_000;

    let mut movie2 = create_test_movie("m2", "Inception 1080p", "Disk2", 27205);
    movie2.resolution = Some("1080p".to_string());
    movie2.size_bytes = 5_000_000_000;

    let mut movie3 = create_test_movie("m3", "Inception 4K", "Disk3", 27205);
    movie3.resolution = Some("4K".to_string());
    movie3.size_bytes = 15_000_000_000;

    merge_disk_into_central(
        &mut central,
        create_movie_disk_index("Disk1", "/mnt/Disk1", vec![movie1]),
    );
    merge_disk_into_central(
        &mut central,
        create_movie_disk_index("Disk2", "/mnt/Disk2", vec![movie2]),
    );
    merge_disk_into_central(
        &mut central,
        create_movie_disk_index("Disk3", "/mnt/Disk3", vec![movie3]),
    );

    // All 3 should be stored (different disks)
    assert_eq!(central.movies.len(), 3);

    // Find duplicates by TMDB ID
    let mut tmdb_map: HashMap<u64, Vec<&MovieEntry>> = HashMap::new();
    for movie in &central.movies {
        if let Some(id) = movie.tmdb_id {
            tmdb_map.entry(id).or_default().push(movie);
        }
    }

    let duplicates = tmdb_map.get(&27205).unwrap();
    assert_eq!(duplicates.len(), 3);

    // Calculate total storage used by duplicates
    let total_dup_size: u64 = duplicates.iter().map(|m| m.size_bytes).sum();
    assert_eq!(total_dup_size, 22_000_000_000); // 2GB + 5GB + 15GB

    // Verify they are on different disks
    let unique_disks: std::collections::HashSet<_> = duplicates.iter().map(|m| &m.disk).collect();
    assert_eq!(unique_disks.len(), 3);
}

// ========== DISK REMOVAL TESTS ==========

#[test]
fn test_remove_disk_from_central() {
    let mut central = CentralIndex::default();

    // Add movies to disk
    let movies = vec![create_test_movie("m1", "Movie 1", "TestDisk", 1001)];
    let movie_disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, movie_disk);

    // Add tv_series to same disk
    let tv_series = vec![create_test_tv_series("t1", "TV Show 1", "TestDisk", 2001)];
    let tv_series_disk = create_tv_series_disk_index("TestDisk", "/mnt/TestDisk/TV_Series", tv_series);
    merge_disk_into_central(&mut central, tv_series_disk);

    assert_eq!(central.disks.len(), 1);
    assert_eq!(central.movies.len(), 1);
    assert_eq!(central.tv_series.len(), 1);

    // Simulate remove disk
    let disk_label = "TestDisk";
    central.movies.retain(|m| m.disk != disk_label);
    central.tv_series.retain(|t| t.disk != disk_label);
    central.disks.remove(disk_label);
    central.rebuild_indexes();
    central.update_statistics();

    // Verify disk is removed
    assert_eq!(central.disks.len(), 0);
    assert_eq!(central.movies.len(), 0);
    assert_eq!(central.tv_series.len(), 0);
    assert_eq!(central.statistics.total_movies, 0);
    assert_eq!(central.statistics.total_tv_series, 0);
}

/// Test: Remove one disk doesn't affect other disks
#[test]
fn test_remove_disk_isolation() {
    let mut central = CentralIndex::default();

    // Add 3 disks
    merge_disk_into_central(
        &mut central,
        create_movie_disk_index(
            "Disk1",
            "/mnt/Disk1",
            vec![create_test_movie("m1", "Movie A", "Disk1", 1001)],
        ),
    );
    merge_disk_into_central(
        &mut central,
        create_movie_disk_index(
            "Disk2",
            "/mnt/Disk2",
            vec![create_test_movie("m2", "Movie B", "Disk2", 1002)],
        ),
    );
    merge_disk_into_central(
        &mut central,
        create_movie_disk_index(
            "Disk3",
            "/mnt/Disk3",
            vec![create_test_movie("m3", "Movie C", "Disk3", 1003)],
        ),
    );

    assert_eq!(central.movies.len(), 3);
    assert_eq!(central.disks.len(), 3);

    // Remove Disk2
    central.movies.retain(|m| m.disk != "Disk2");
    central.disks.remove("Disk2");
    central.rebuild_indexes();
    central.update_statistics();

    // Disk1 and Disk3 should remain
    assert_eq!(central.movies.len(), 2);
    assert_eq!(central.disks.len(), 2);
    assert!(central.disks.contains_key("Disk1"));
    assert!(central.disks.contains_key("Disk3"));
    assert!(!central.disks.contains_key("Disk2"));

    // Verify remaining movies
    let titles: Vec<_> = central.movies.iter().map(|m| &m.title).collect();
    assert!(titles.contains(&&"Movie A".to_string()));
    assert!(titles.contains(&&"Movie C".to_string()));
    assert!(!titles.contains(&&"Movie B".to_string()));
}

// ========== STATISTICS TESTS ==========

#[test]
fn test_statistics_update() {
    let mut central = CentralIndex::default();

    let mut movie1 = create_test_movie("m1", "US Movie 2020", "TestDisk", 1001);
    movie1.country = Some("US".to_string());
    movie1.year = Some(2020);
    movie1.size_bytes = 1_000_000_000;

    let mut movie2 = create_test_movie("m2", "CN Movie 2024", "TestDisk", 1002);
    movie2.country = Some("CN".to_string());
    movie2.year = Some(2024);
    movie2.size_bytes = 2_000_000_000;

    let movies = vec![movie1, movie2];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    // Verify statistics
    assert_eq!(central.statistics.total_movies, 2);
    assert_eq!(central.statistics.total_disks, 1);
    assert_eq!(central.statistics.total_size_bytes, 3_000_000_000);
    assert_eq!(central.statistics.by_country.get("US"), Some(&1));
    assert_eq!(central.statistics.by_country.get("CN"), Some(&1));
    assert_eq!(central.statistics.by_decade.get("2020s"), Some(&2));
}

// ========== BACKWARD COMPATIBILITY TESTS ==========

#[test]
fn test_backward_compatibility_base_path() {
    // Simulate loading old index with only base_path (no paths HashMap)
    let disk_info_json = r#"{
        "label": "OldDisk",
        "uuid": "old-uuid",
        "last_indexed": "2024-01-01T00:00:00Z",
        "movie_count": 10,
        "tv_series_count": 0,
        "total_size_bytes": 10000000,
        "base_path": "/mnt/OldDisk/Movies"
    }"#;

    let disk_info: DiskInfo = serde_json::from_str(disk_info_json).unwrap();

    // Verify backward compatibility - paths should be empty (default)
    assert_eq!(disk_info.label, "OldDisk");
    assert_eq!(disk_info.base_path, "/mnt/OldDisk/Movies");
    assert!(disk_info.paths.is_empty()); // Default empty HashMap
}
