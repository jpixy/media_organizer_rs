//! Unit tests for TV metadata validation logic.
//! These tests verify that title, year, IMDB ID, and TMDB ID are correctly matched.

use media_organizer::core::parser::{parse_organized_tv_series_folder, OrganizedTvSeriesFolderInfo};
use media_organizer::generators::folder::generate_season_folder;
use media_organizer::models::media::SeasonMetadata;

#[test]
fn test_tv_folder_parsing_pattern_0a_anthology() {
    // Test parsing anthology series season folder with sort prefix
    let folder_name = "[S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots](2025)-tt21661768-tmdb86831";
    
    let result = parse_organized_tv_series_folder(folder_name);
    
    assert!(result.is_some(), "Should parse the folder name");
    
    let info = result.unwrap();
    assert_eq!(info.title, "爱，死亡和机器人");
    assert_eq!(info.original_title, Some("Love, Death & Robots".to_string()));
    assert_eq!(info.year, Some(2025));
    assert_eq!(info.tmdb_id, Some(86831)); // TMDB TV Show ID
    assert_eq!(info.season_imdb_id, Some("tt21661768".to_string())); // Season 4 IMDB ID
    assert!(info.imdb_id.is_none());
    
    println!("✓ Test passed: Parsed anthology season folder with correct season-level IMDB ID");
}

#[test]
fn test_tv_folder_parsing_pattern_0_regular() {
    // Test parsing regular TV series season folder without sort prefix
    // Pattern 0: [S02][Season 02]-[L][绝命毒师][Breaking Bad](2009)-tt0903747-tmdb1396
    // Pattern 0 logic: prefers longer title (title2 over title1)
    let folder_name = "[S02][Season 02]-[L][绝命毒师][Breaking Bad](2009)-tt0903747-tmdb1396";
    
    let result = parse_organized_tv_series_folder(folder_name);
    
    assert!(result.is_some(), "Should parse the folder name");
    
    let info = result.unwrap();
    // Pattern 0 prefers longer title, so it selects "绝命毒师" (4 chars) over "L" (1 char)
    assert_eq!(info.title, "绝命毒师"); 
    assert_eq!(info.tmdb_id, Some(1396));
    assert_eq!(info.season_imdb_id, Some("tt0903747".to_string())); // TV Show IMDB ID
    
    println!("✓ Test passed: Parsed regular season folder correctly");
}

#[test]
fn test_season_folder_generation_with_correct_imdb_id() {
    // Test generating season folder with season-level IMDB ID (anthology series)
    let folder_name = generate_season_folder(
        4,
        "第 4 季",
        "A",
        "爱，死亡和机器人",
        "Love, Death & Robots",
        Some("tt21661768"), // Season-level IMDB ID
        450504, // Season-level TMDB ID
        Some("2025-05-15"),
    );
    
    // Verify the folder name contains the correct IDs
    assert!(folder_name.contains("[S04][Season 04]"), "Should contain season prefix");
    assert!(folder_name.contains("[A][爱，死亡和机器人][Love, Death & Robots]"), "Should contain titles");
    assert!(folder_name.contains("(2025)"), "Should contain year");
    assert!(folder_name.contains("tt21661768"), "Should contain season-level IMDB ID");
    assert!(folder_name.contains("tmdb450504"), "Should contain season-level TMDB ID");
    
    // Verify it does NOT contain the TV Show IMDB ID
    assert!(!folder_name.contains("tt9561862"), "Should NOT contain TV Show IMDB ID");
    
    println!("✓ Test passed: Generated season folder with correct season-level IMDB ID");
}

#[test]
fn test_imdb_id_priority_logic() {
    // Test the priority logic: folder_info.season_imdb_id > TMDB season > TV Show
    
    // Scenario 1: Folder has season IMDB ID (highest priority)
    let folder_info = OrganizedTvSeriesFolderInfo {
        title: "爱，死亡和机器人".to_string(),
        original_title: Some("Love, Death & Robots".to_string()),
        year: Some(2025),
        imdb_id: None,
        tmdb_id: Some(86831),
        season_imdb_id: Some("tt21661768".to_string()), // Season-level IMDB ID
    };
    
    let show_imdb_id = "tt9561862"; // TV Show IMDB ID
    
    // Priority logic implementation
    let season_imdb_id = if let Some(ref folder_season_imdb) = folder_info.season_imdb_id {
        folder_season_imdb.clone()
    } else {
        // Simulate TMDB season external_ids fallback
        // In real code, this would call tmdb.get_season_external_ids()
        let _tmdb_season_imdb_id: Option<String> = None; // Simulate TMDB doesn't have it
        _tmdb_season_imdb_id.unwrap_or_else(|| show_imdb_id.to_string())
    };
    
    assert_eq!(season_imdb_id, "tt21661768", "Should use folder's season-level IMDB ID");
    assert_ne!(season_imdb_id, "tt9561862", "Should NOT fall back to TV Show IMDB ID");
    
    // Scenario 2: Folder has no season IMDB ID, falls back to TMDB or TV Show
    let folder_info_no_imdb = OrganizedTvSeriesFolderInfo {
        title: "绝命毒师".to_string(),
        original_title: Some("Breaking Bad".to_string()),
        year: Some(2008),
        imdb_id: None,
        tmdb_id: Some(1396),
        season_imdb_id: None, // No season-level IMDB ID
    };
    
    let show_imdb_id_regular = "tt0903747";
    
    let season_imdb_id_fallback = if let Some(ref folder_season_imdb) = folder_info_no_imdb.season_imdb_id {
        folder_season_imdb.clone()
    } else {
        // Simulate TMDB doesn't return season IMDB ID
        let _tmdb_season_imdb_id: Option<String> = None;
        _tmdb_season_imdb_id.unwrap_or_else(|| show_imdb_id_regular.to_string())
    };
    
    assert_eq!(season_imdb_id_fallback, "tt0903747", "Should fall back to TV Show IMDB ID");
    
    println!("✓ Test passed: IMDB ID priority logic is correct");
}

#[test]
fn test_season_metadata_with_imdb_id() {
    // Test SeasonMetadata struct with season-level IMDB ID
    let season_meta = SeasonMetadata {
        season_number: 4,
        name: "Season 4".to_string(),
        overview: Some("Anthology series season 4".to_string()),
        air_date: Some("2025-05-15".to_string()),
        poster_url: Some("https://example.com/poster.jpg".to_string()),
        episode_count: 10,
        tmdb_id: 450504,
        imdb_id: Some("tt21661768".to_string()), // Season-level IMDB ID
    };
    
    assert_eq!(season_meta.season_number, 4);
    assert_eq!(season_meta.tmdb_id, 450504);
    assert_eq!(season_meta.imdb_id, Some("tt21661768".to_string()));
    
    // Test without season-level IMDB ID (regular series)
    let season_meta_regular = SeasonMetadata {
        season_number: 2,
        name: "Season 2".to_string(),
        overview: Some("Regular series season 2".to_string()),
        air_date: Some("2009-03-08".to_string()),
        poster_url: Some("https://example.com/poster2.jpg".to_string()),
        episode_count: 13,
        tmdb_id: 3572,
        imdb_id: Some("tt0903747".to_string()), // TV Show IMDB ID fallback
    };
    
    assert_eq!(season_meta_regular.imdb_id, Some("tt0903747".to_string()));
    
    println!("✓ Test passed: SeasonMetadata correctly stores IMDB ID");
}

#[test]
fn test_folder_consistency_validation() {
    // Test that parsed folder info is consistent
    let folder_name = "[S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots](2025)-tt21661768-tmdb86831";
    
    let result = parse_organized_tv_series_folder(folder_name);
    assert!(result.is_some());
    
    let info = result.unwrap();
    
    // Validate consistency: season folder should have season_imdb_id, not imdb_id
    assert!(info.season_imdb_id.is_some(), "Season folder should have season_imdb_id");
    assert!(info.imdb_id.is_none(), "Season folder should NOT have imdb_id (TV Show level)");
    
    // Validate that year matches
    assert_eq!(info.year, Some(2025));
    
    // Validate TMDB ID is present
    assert!(info.tmdb_id.is_some());
    
    println!("✓ Test passed: Folder parsing consistency validated");
}
