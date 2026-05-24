//! Integration tests for the indexer module.
//!
//! Tests cover:
//! - Composite storage (one disk label with multiple media types)
//! - Search functionality (by title, year, actor, director, genre, country)
//! - Collection indexing and completeness detection
//! - Cross-disk duplicate detection
//! - Edge cases (empty scans, repeated scans, path updates)

use media_organizer::core::indexer::{merge_disk_into_central, search};
use media_organizer::models::index::{CentralIndex, DiskIndex, VolumeGroupInfo, MovieEntry, TvSeriesEntry};
use std::collections::HashMap;

// ========== TEST FIXTURES ==========

/// Create a test DiskIndex with movies
fn create_movie_disk_index(label: &str, path: &str, movies: Vec<MovieEntry>) -> DiskIndex {
    let mut paths = HashMap::new();
    paths.insert("movies".to_string(), path.to_string());

    DiskIndex {
        version: "1.0".to_string(),
        disk: VolumeGroupInfo {
            label: label.to_string(),
            uuid: Some("test-uuid".to_string()),
            last_indexed: chrono::Utc::now().to_rfc3339(),
            movie_count: movies.len(),
            tv_series_count: 0,
            total_size_bytes: movies.iter().map(|m| m.size_bytes).sum(),
            base_path: path.to_string(),
            paths,
            content_hash: String::new(),
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
        disk: VolumeGroupInfo {
            label: label.to_string(),
            uuid: Some("test-uuid".to_string()),
            last_indexed: chrono::Utc::now().to_rfc3339(),
            movie_count: 0,
            tv_series_count: tv_series.len(),
            total_size_bytes: tv_series.iter().map(|t| t.size_bytes).sum(),
            base_path: path.to_string(),
            paths,
            content_hash: String::new(),
        },
        movies: Vec::new(),
        tv_series,
    }
}

/// Create a test movie entry
fn create_test_movie(id: &str, title: &str, disk: &str, tmdb_id: u64) -> MovieEntry {
    use media_organizer::models::index::VideoFileInfo;
    
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
        video_files: vec![VideoFileInfo {
            file_name: format!("{}.mkv", title),
            file_path: format!("{}/{}.mkv", title, title),
            size_bytes: 1_000_000_000,
            resolution: Some("1080p".to_string()),
            format: Some("mkv".to_string()),
            codec: Some("h264".to_string()),
        }],
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
        owned_seasons: 3,
        owned_episodes: 24,
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

    let disk_info: VolumeGroupInfo = serde_json::from_str(disk_info_json).unwrap();

    // Verify backward compatibility - paths should be empty (default)
    assert_eq!(disk_info.label, "OldDisk");
    assert_eq!(disk_info.base_path, "/mnt/OldDisk/Movies");
    assert!(disk_info.paths.is_empty()); // Default empty HashMap
}

// ========== IDEMPOTENCY TESTS ==========

#[test]
fn test_content_hash_consistency() {
    // Test that the same directory produces the same hash
    use media_organizer::core::indexer::calculate_directory_hash;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let dir_path = dir.path();

    // Create some test NFO files
    let nfo1_path = dir_path.join("movie.nfo");
    let mut nfo1 = File::create(&nfo1_path).unwrap();
    writeln!(nfo1, "<movie><title>Test Movie</title></movie>").unwrap();

    let nfo2_path = dir_path.join("tvshow.nfo");
    let mut nfo2 = File::create(&nfo2_path).unwrap();
    writeln!(nfo2, "<tvshow><title>Test TV</title></tvshow>").unwrap();

    // Calculate hash twice
    let hash1 = calculate_directory_hash(dir_path).unwrap();
    let hash2 = calculate_directory_hash(dir_path).unwrap();

    // Should be the same
    assert_eq!(hash1, hash2);
}

#[test]
fn test_content_hash_detects_changes() {
    use media_organizer::core::indexer::calculate_directory_hash;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let dir_path = dir.path();

    // Create initial NFO file
    let nfo_path = dir_path.join("movie.nfo");
    let mut nfo = File::create(&nfo_path).unwrap();
    writeln!(nfo, "<movie><title>Original</title></movie>").unwrap();

    let hash1 = calculate_directory_hash(dir_path).unwrap();

    // Modify the file
    writeln!(nfo, "<movie><title>Modified</title></movie>").unwrap();

    let hash2 = calculate_directory_hash(dir_path).unwrap();

    // Should be different
    assert_ne!(hash1, hash2);
}

// ========== DUPLICATE DETECTION ACCURACY TESTS ==========

#[test]
fn test_duplicate_detection_tmdb_id_high_confidence() {
    // Test that same TMDB ID produces high confidence duplicates
    let movie1 = create_test_movie("1", "Movie A", "disk1", 12345);
    let movie2 = create_test_movie("2", "Movie A", "disk2", 12345);

    let disk1 = create_movie_disk_index("disk1", "/mnt/disk1", vec![movie1]);
    let disk2 = create_movie_disk_index("disk2", "/mnt/disk2", vec![movie2]);

    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk1);
    merge_disk_into_central(&mut central, disk2);

    // Should have 2 movies total
    assert_eq!(central.statistics.total_movies, 2);
    assert_eq!(central.movies.len(), 2);

    // Both should have the same TMDB ID
    assert_eq!(central.movies[0].tmdb_id, Some(12345));
    assert_eq!(central.movies[1].tmdb_id, Some(12345));
}

#[test]
fn test_duplicate_detection_imdb_id_matching() {
    // Test IMDB ID based duplicate detection
    let mut movie1 = create_test_movie("1", "Movie A", "disk1", 0);
    movie1.imdb_id = Some("tt1234567".to_string());
    
    let mut movie2 = create_test_movie("2", "Movie A", "disk2", 0);
    movie2.imdb_id = Some("tt1234567".to_string());

    let disk1 = create_movie_disk_index("disk1", "/mnt/disk1", vec![movie1]);
    let disk2 = create_movie_disk_index("disk2", "/mnt/disk2", vec![movie2]);

    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk1);
    merge_disk_into_central(&mut central, disk2);

    // Should have 2 movies with same IMDB ID
    assert_eq!(central.statistics.total_movies, 2);
    assert_eq!(central.movies[0].imdb_id, central.movies[1].imdb_id);
}

#[test]
fn test_duplicate_detection_different_tmdb_ids() {
    // Test that different TMDB IDs are not considered duplicates
    let movie1 = create_test_movie("1", "Movie A", "disk1", 12345);
    let movie2 = create_test_movie("2", "Movie A", "disk2", 67890);

    let disk1 = create_movie_disk_index("disk1", "/mnt/disk1", vec![movie1]);
    let disk2 = create_movie_disk_index("disk2", "/mnt/disk2", vec![movie2]);

    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk1);
    merge_disk_into_central(&mut central, disk2);

    // Should have 2 movies with different TMDB IDs
    assert_eq!(central.statistics.total_movies, 2);
    assert_ne!(central.movies[0].tmdb_id, central.movies[1].tmdb_id);
}

// ========== TITLE SIMILARITY TESTS ==========

#[test]
fn test_title_similarity_exact_match() {
    use media_organizer::cli::commands::index::title_similarity;
    
    let sim = title_similarity("Hello World", "Hello World");
    assert_eq!(sim, 1.0);
}

#[test]
fn test_title_similarity_case_insensitive() {
    use media_organizer::cli::commands::index::title_similarity;
    
    let sim = title_similarity("Hello World", "hello world");
    assert_eq!(sim, 1.0);
}

#[test]
fn test_title_similarity_high_similarity() {
    use media_organizer::cli::commands::index::title_similarity;
    
    // 90% similarity threshold - "Hello World" vs "Hello World!"
    let sim = title_similarity("Hello World", "Hello World!");
    assert!(sim >= 0.9, "Similarity should be >= 0.9, got {}", sim);
}

#[test]
fn test_title_similarity_low_similarity() {
    use media_organizer::cli::commands::index::title_similarity;
    
    let sim = title_similarity("Inception", "Interstellar");
    assert!(sim < 0.9, "Similarity should be < 0.9, got {}", sim);
}

// ========== VOLUME GROUP STATISTICS TESTS ==========

#[test]
fn test_volume_group_combined_stats() {
    // Test that volume groups correctly track both movies and TV shows
    let movie = create_test_movie("1", "Test Movie", "local", 12345);
    let tv_show = create_test_tv_series("1", "Test Show", "local", 67890);

    let movie_disk = create_movie_disk_index("local", "/mnt/local", vec![movie]);
    let tv_disk = create_tv_series_disk_index("local", "/mnt/local", vec![tv_show]);

    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, movie_disk);
    merge_disk_into_central(&mut central, tv_disk);

    // Should have combined stats
    assert_eq!(central.statistics.total_movies, 1);
    assert_eq!(central.statistics.total_tv_series, 1);
}

#[test]
fn test_empty_disk_index() {
    // Test handling of empty disk index
    let disk = create_movie_disk_index("empty", "/mnt/empty", Vec::new());
    
    assert_eq!(disk.movies.len(), 0);
    assert_eq!(disk.tv_series.len(), 0);
    assert_eq!(disk.disk.movie_count, 0);
    assert_eq!(disk.disk.tv_series_count, 0);
    assert_eq!(disk.disk.total_size_bytes, 0);
}

#[test]
fn test_disk_info_content_hash_persistence() {
    // Test that content_hash is preserved in DiskInfo
    let movie = create_test_movie("1", "Test", "disk1", 12345);
    let mut disk = create_movie_disk_index("disk1", "/mnt/disk1", vec![movie]);
    
    disk.disk.content_hash = "test-hash-123".to_string();
    
    assert_eq!(disk.disk.content_hash, "test-hash-123");
}

// ========== TV SERIES COMPLETENESS TESTS ==========

#[test]
fn test_tv_series_complete_statistics() {
    // Test complete TV series (all seasons owned)
    let mut tv_complete = create_test_tv_series("tv1", "Complete Show", "local", 11111);
    tv_complete.seasons = 5;
    tv_complete.episodes = 60;
    tv_complete.owned_seasons = 5;
    tv_complete.owned_episodes = 60;

    let mut tv_incomplete = create_test_tv_series("tv2", "Incomplete Show", "local", 22222);
    tv_incomplete.seasons = 5;
    tv_incomplete.episodes = 60;
    tv_incomplete.owned_seasons = 3;
    tv_incomplete.owned_episodes = 36;

    let disk = create_tv_series_disk_index("local", "/mnt/local", vec![tv_complete, tv_incomplete]);
    
    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk);
    central.update_statistics();

    assert_eq!(central.statistics.complete_tv_series, 1);
    assert_eq!(central.statistics.incomplete_tv_series, 1);
}

#[test]
fn test_tv_series_incomplete_detection() {
    // Test incomplete TV series detection
    let mut tv = create_test_tv_series("tv1", "Test Show", "local", 11111);
    tv.seasons = 5;
    tv.episodes = 60;
    tv.owned_seasons = 2;  // Only 2 seasons owned out of 5
    tv.owned_episodes = 24;

    let disk = create_tv_series_disk_index("local", "/mnt/local", vec![tv]);
    
    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk);
    central.update_statistics();

    assert_eq!(central.statistics.complete_tv_series, 0);
    assert_eq!(central.statistics.incomplete_tv_series, 1);
}

#[test]
fn test_tv_series_unknown_status() {
    // Test TV series with unknown completeness (no total seasons info)
    let mut tv_unknown = create_test_tv_series("tv1", "Unknown Show", "local", 11111);
    tv_unknown.seasons = 0;  // No total seasons info
    tv_unknown.episodes = 0;
    tv_unknown.owned_seasons = 2;
    tv_unknown.owned_episodes = 24;

    let disk = create_tv_series_disk_index("local", "/mnt/local", vec![tv_unknown]);
    
    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk);
    central.update_statistics();

    assert_eq!(central.statistics.complete_tv_series, 0);
    assert_eq!(central.statistics.incomplete_tv_series, 0);
}

#[test]
fn test_tv_series_no_owned_content() {
    // Test TV series with no owned seasons
    let mut tv = create_test_tv_series("tv1", "No Owned", "local", 11111);
    tv.seasons = 5;
    tv.episodes = 60;
    tv.owned_seasons = 0;
    tv.owned_episodes = 0;

    let disk = create_tv_series_disk_index("local", "/mnt/local", vec![tv]);
    
    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk);
    central.update_statistics();

    assert_eq!(central.statistics.complete_tv_series, 0);
    assert_eq!(central.statistics.incomplete_tv_series, 0);
}

// ========== COLLECTION UPDATE IDEMPOTENCY TESTS ==========

#[test]
fn test_collection_update_idempotency() {
    // Test that updating collections multiple times produces the same result
    let mut movie1 = create_test_movie("1", "Movie 1", "disk1", 12345);
    movie1.collection_id = Some(100);
    movie1.collection_name = Some("Test Collection".to_string());
    
    let mut movie2 = create_test_movie("2", "Movie 2", "disk1", 12346);
    movie2.collection_id = Some(100);
    movie2.collection_name = Some("Test Collection".to_string());

    let disk = create_movie_disk_index("disk1", "/mnt/disk1", vec![movie1, movie2]);
    
    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk);
    
    // First update
    central.update_statistics();
    let stats1 = central.statistics.clone();
    
    // Second update (should be idempotent)
    central.update_statistics();
    let stats2 = central.statistics.clone();
    
    // Third update
    central.update_statistics();
    let stats3 = central.statistics;
    
    // All should be equal
    assert_eq!(stats1.complete_collections, stats2.complete_collections);
    assert_eq!(stats2.complete_collections, stats3.complete_collections);
    assert_eq!(stats1.incomplete_collections, stats2.incomplete_collections);
    assert_eq!(stats2.incomplete_collections, stats3.incomplete_collections);
}

#[test]
fn test_tv_update_idempotency() {
    // Test that updating TV series statistics multiple times produces the same result
    let mut tv = create_test_tv_series("tv1", "Test Series", "local", 11111);
    tv.seasons = 5;
    tv.episodes = 60;
    tv.owned_seasons = 5;
    tv.owned_episodes = 60;

    let disk = create_tv_series_disk_index("local", "/mnt/local", vec![tv]);
    
    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk);
    
    // First update
    central.update_statistics();
    let stats1 = central.statistics.clone();
    
    // Second update (should be idempotent)
    central.update_statistics();
    let stats2 = central.statistics.clone();
    
    // Third update
    central.update_statistics();
    let stats3 = central.statistics;
    
    // All should be equal
    assert_eq!(stats1.complete_tv_series, stats2.complete_tv_series);
    assert_eq!(stats2.complete_tv_series, stats3.complete_tv_series);
    assert_eq!(stats1.incomplete_tv_series, stats2.incomplete_tv_series);
    assert_eq!(stats2.incomplete_tv_series, stats3.incomplete_tv_series);
}

// ========== INDEX REBUILD IDEMPOTENCY TESTS ==========

#[test]
fn test_rebuild_index_idempotency() {
    // Test that rebuilding index multiple times produces consistent results
    let movie1 = create_test_movie("1", "Movie 1", "disk1", 12345);
    let movie2 = create_test_movie("2", "Movie 2", "disk2", 12346);
    
    let mut tv = create_test_tv_series("tv1", "Test Series", "disk1", 11111);
    tv.seasons = 3;
    tv.episodes = 36;
    tv.owned_seasons = 2;
    tv.owned_episodes = 24;

    let disk1 = create_movie_disk_index("disk1", "/mnt/disk1", vec![movie1]);
    let disk2 = create_movie_disk_index("disk2", "/mnt/disk2", vec![movie2]);
    let tv_disk = create_tv_series_disk_index("disk1", "/mnt/disk1/TV", vec![tv]);
    
    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk1);
    merge_disk_into_central(&mut central, disk2);
    merge_disk_into_central(&mut central, tv_disk);
    
    // First rebuild
    central.update_statistics();
    let movies_count1 = central.movies.len();
    let tv_count1 = central.tv_series.len();
    let stats1 = central.statistics.clone();
    
    // Second rebuild (should be idempotent)
    central.update_statistics();
    let movies_count2 = central.movies.len();
    let tv_count2 = central.tv_series.len();
    let stats2 = central.statistics.clone();
    
    // Third rebuild
    central.update_statistics();
    let movies_count3 = central.movies.len();
    let tv_count3 = central.tv_series.len();
    let stats3 = central.statistics;
    
    // All counts should be equal
    assert_eq!(movies_count1, movies_count2);
    assert_eq!(movies_count2, movies_count3);
    assert_eq!(tv_count1, tv_count2);
    assert_eq!(tv_count2, tv_count3);
    
    // All statistics should be equal
    assert_eq!(stats1.total_movies, stats2.total_movies);
    assert_eq!(stats2.total_movies, stats3.total_movies);
    assert_eq!(stats1.total_tv_series, stats2.total_tv_series);
    assert_eq!(stats2.total_tv_series, stats3.total_tv_series);
}

// ========== STATISTICS UPDATE ACCURACY TESTS ==========

#[test]
fn test_statistics_update_with_mixed_media() {
    // Test statistics update with both movies and TV shows across multiple disks
    let movie1 = create_test_movie("1", "Movie 1", "disk1", 12345);
    let movie2 = create_test_movie("2", "Movie 2", "disk1", 12346);
    let movie3 = create_test_movie("3", "Movie 3", "disk2", 12347);
    
    let mut tv1 = create_test_tv_series("tv1", "Series 1", "disk1", 11111);
    tv1.seasons = 5;
    tv1.episodes = 60;
    tv1.owned_seasons = 5;
    tv1.owned_episodes = 60;  // Complete
    
    let mut tv2 = create_test_tv_series("tv2", "Series 2", "disk2", 22222);
    tv2.seasons = 3;
    tv2.episodes = 36;
    tv2.owned_seasons = 2;
    tv2.owned_episodes = 24;  // Incomplete

    let disk1_movies = create_movie_disk_index("disk1", "/mnt/disk1/Movies", vec![movie1, movie2]);
    let disk2_movies = create_movie_disk_index("disk2", "/mnt/disk2/Movies", vec![movie3]);
    let disk1_tv = create_tv_series_disk_index("disk1", "/mnt/disk1/TV", vec![tv1]);
    let disk2_tv = create_tv_series_disk_index("disk2", "/mnt/disk2/TV", vec![tv2]);
    
    let mut central = CentralIndex::default();
    merge_disk_into_central(&mut central, disk1_movies);
    merge_disk_into_central(&mut central, disk2_movies);
    merge_disk_into_central(&mut central, disk1_tv);
    merge_disk_into_central(&mut central, disk2_tv);
    
    central.update_statistics();
    
    // Verify total counts
    assert_eq!(central.statistics.total_movies, 3);
    assert_eq!(central.statistics.total_tv_series, 2);
    
    // Verify disk-specific counts
    assert_eq!(central.disks.get("disk1").unwrap().movie_count, 2);
    assert_eq!(central.disks.get("disk1").unwrap().tv_series_count, 1);
    assert_eq!(central.disks.get("disk2").unwrap().movie_count, 1);
    assert_eq!(central.disks.get("disk2").unwrap().tv_series_count, 1);
    
    // Verify completeness statistics
    assert_eq!(central.statistics.complete_tv_series, 1);  // Series 1 is complete
    assert_eq!(central.statistics.incomplete_tv_series, 1);  // Series 2 is incomplete
}

#[test]
fn test_movie_entry_backward_compatibility() {
    // Test backward compatibility: old index data without video_files field
    let old_format_json = r#"{
        "id": "test-movie-id",
        "disk": "test-disk",
        "disk_uuid": "test-uuid",
        "relative_path": "Movies/Test Movie",
        "title": "Test Movie",
        "original_title": null,
        "year": 2024,
        "tmdb_id": 123456,
        "imdb_id": "tt1234567",
        "collection_id": null,
        "collection_name": null,
        "collection_total_movies": null,
        "country": "US",
        "genres": ["Action", "Adventure"],
        "actors": ["Actor One", "Actor Two"],
        "directors": ["Director One"],
        "runtime": 120,
        "rating": 7.5,
        "size_bytes": 1073741824,
        "resolution": "1080p",
        "indexed_at": "2024-01-01T00:00:00Z"
    }"#;
    
    // Should successfully deserialize old format, video_files defaults to empty vec
    let movie: MovieEntry = serde_json::from_str(old_format_json)
        .expect("Should deserialize old format without video_files field");
    
    assert_eq!(movie.title, "Test Movie");
    assert_eq!(movie.tmdb_id, Some(123456));
    assert!(movie.video_files.is_empty());
}

#[test]
fn test_movie_entry_new_format_with_video_files() {
    // Test new format with video_files field
    let new_format_json = r#"{
        "id": "test-movie-id",
        "disk": "test-disk",
        "disk_uuid": "test-uuid",
        "relative_path": "Movies/Test Movie",
        "title": "Test Movie",
        "original_title": null,
        "year": 2024,
        "tmdb_id": 123456,
        "imdb_id": "tt1234567",
        "collection_id": null,
        "collection_name": null,
        "collection_total_movies": null,
        "country": "US",
        "genres": ["Action"],
        "actors": [],
        "directors": [],
        "runtime": 120,
        "rating": 7.5,
        "size_bytes": 2147483648,
        "resolution": "1080p",
        "video_files": [
            {"file_name": "movie_1080p.mkv", "file_path": "Movies/Test Movie/movie_1080p.mkv", "size_bytes": 1073741824, "resolution": "1080p", "format": "mkv", "codec": "h264"},
            {"file_name": "movie_4k.mkv", "file_path": "Movies/Test Movie/movie_4k.mkv", "size_bytes": 1073741824, "resolution": "4K", "format": "mkv", "codec": "hevc"}
        ],
        "indexed_at": "2024-01-01T00:00:00Z"
    }"#;
    
    let movie: MovieEntry = serde_json::from_str(new_format_json)
        .expect("Should deserialize new format with video_files field");
    
    assert_eq!(movie.title, "Test Movie");
    assert_eq!(movie.video_files.len(), 2);
    assert_eq!(movie.video_files[0].file_name, "movie_1080p.mkv");
    assert_eq!(movie.video_files[1].resolution, Some("4K".to_string()));
}

/// Helper function to create a test movie with specific actors and directors
fn create_test_movie_with_actors(
    id: &str,
    title: &str,
    disk: &str,
    tmdb_id: u64,
    actors: Vec<&str>,
    directors: Vec<&str>,
) -> MovieEntry {
    use media_organizer::models::index::VideoFileInfo;

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
        actors: actors.into_iter().map(|s| s.to_string()).collect(),
        directors: directors.into_iter().map(|s| s.to_string()).collect(),
        runtime: Some(120),
        rating: Some(7.5),
        size_bytes: 1_000_000_000,
        resolution: Some("1080p".to_string()),
        video_files: vec![VideoFileInfo {
            file_name: format!("{}.mkv", title),
            file_path: format!("{}/{}.mkv", title, title),
            size_bytes: 1_000_000_000,
            resolution: Some("1080p".to_string()),
            format: Some("mkv".to_string()),
            codec: Some("h264".to_string()),
        }],
        indexed_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Test: Search by actor name
#[test]
fn test_search_by_actor() {
    let mut central = CentralIndex::default();

    let movies = vec![
        create_test_movie_with_actors(
            "m1",
            "The Godfather",
            "TestDisk",
            238,
            vec!["Al Pacino", "Marlon Brando"],
            vec!["Francis Ford Coppola"],
        ),
        create_test_movie_with_actors(
            "m2",
            "The Matrix",
            "TestDisk",
            603,
            vec!["Keanu Reeves", "Laurence Fishburne"],
            vec!["The Wachowskis"],
        ),
    ];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    // Search by actor name (partial match)
    let results = search(
        &central,
        None,
        Some("Al Pacino"),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results.movies.len(), 1);
    assert_eq!(results.movies[0].title, "The Godfather");

    // Search by another actor
    let results2 = search(
        &central,
        None,
        Some("Keanu"),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results2.movies.len(), 1);
    assert_eq!(results2.movies[0].title, "The Matrix");

    // Search with partial name
    let results3 = search(
        &central,
        None,
        Some("Fishburne"),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results3.movies.len(), 1);
    assert_eq!(results3.movies[0].title, "The Matrix");
}

/// Test: Search by director name
#[test]
fn test_search_by_director() {
    let mut central = CentralIndex::default();

    let movies = vec![
        create_test_movie_with_actors(
            "m1",
            "The Godfather",
            "TestDisk",
            238,
            vec!["Al Pacino"],
            vec!["Francis Ford Coppola"],
        ),
        create_test_movie_with_actors(
            "m2",
            "Inception",
            "TestDisk",
            27205,
            vec!["Leonardo DiCaprio"],
            vec!["Christopher Nolan"],
        ),
        create_test_movie_with_actors(
            "m3",
            "The Dark Knight",
            "TestDisk",
            155,
            vec!["Christian Bale"],
            vec!["Christopher Nolan"],
        ),
    ];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    // Search by director name (exact match)
    let results = search(
        &central,
        None,
        None,
        Some("Christopher Nolan"),
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results.movies.len(), 2);
    assert!(results.movies.iter().any(|m| m.title == "Inception"));
    assert!(results.movies.iter().any(|m| m.title == "The Dark Knight"));

    // Search by partial director name
    let results2 = search(
        &central,
        None,
        None,
        Some("Nolan"),
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results2.movies.len(), 2);

    // Search by first name only
    let results3 = search(
        &central,
        None,
        None,
        Some("Christopher"),
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results3.movies.len(), 2);
}

/// Test: Combined search by actor and title
#[test]
fn test_search_by_actor_and_title() {
    let mut central = CentralIndex::default();

    let movies = vec![
        create_test_movie_with_actors(
            "m1",
            "The Godfather",
            "TestDisk",
            238,
            vec!["Al Pacino"],
            vec!["Francis Ford Coppola"],
        ),
        create_test_movie_with_actors(
            "m2",
            "The Godfather Part II",
            "TestDisk",
            240,
            vec!["Al Pacino"],
            vec!["Francis Ford Coppola"],
        ),
        create_test_movie_with_actors(
            "m3",
            "Scent of a Woman",
            "TestDisk",
            9542,
            vec!["Al Pacino"],
            vec!["Martin Brest"],
        ),
    ];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    // Search by actor alone
    let results = search(
        &central,
        None,
        Some("Al Pacino"),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results.movies.len(), 3);

    // Search by actor AND title (combined filter)
    let results2 = search(
        &central,
        Some("Godfather"),
        Some("Al Pacino"),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results2.movies.len(), 2);
    assert!(results2.movies.iter().all(|m| m.title.contains("Godfather")));
}

/// Test: Search is case-insensitive for actor/director
#[test]
fn test_search_actor_director_case_insensitive() {
    let mut central = CentralIndex::default();

    let movies = vec![create_test_movie_with_actors(
        "m1",
        "The Matrix",
        "TestDisk",
        603,
        vec!["KEANU REEVES"],
        vec!["THE WACHOWSKIS"],
    )];
    let disk = create_movie_disk_index("TestDisk", "/mnt/TestDisk/Movies", movies);
    merge_disk_into_central(&mut central, disk);

    // Search with lowercase
    let results = search(
        &central,
        None,
        Some("keanu"),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results.movies.len(), 1);
    assert_eq!(results.movies[0].title, "The Matrix");

    // Search director with lowercase
    let results2 = search(
        &central,
        None,
        None,
        Some("wachowski"),
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(results2.movies.len(), 1);
}
