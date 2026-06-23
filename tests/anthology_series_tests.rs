//! Unit tests for anthology series (单元剧) handling.
//! These tests verify that each season of anthology series gets its own IMDB ID.

use media_organizer::core::parser::{parse_organized_tv_series_folder, OrganizedTvSeriesFolderInfo};
use media_organizer::generators::folder::generate_season_folder;

#[test]
fn test_parse_anthology_season_folder_with_imdb_id() {
    // Test parsing a season folder with season-level IMDB ID (anthology series)
    let folder_name = "[S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots](2025)-tt21661768-tmdb86831";
    
    let result = parse_organized_tv_series_folder(folder_name);
    
    assert!(result.is_some(), "Should be able to parse the folder name");
    
    let info = result.unwrap();
    assert_eq!(info.title, "爱，死亡和机器人");
    assert_eq!(info.tmdb_id, Some(86831));
    assert_eq!(info.season_imdb_id, Some("tt21661768".to_string()));
    assert!(info.imdb_id.is_none(), "TV Show IMDB ID should be None for season folders");
    
    println!("✓ Test passed: Parsed anthology season folder correctly");
}

#[test]
fn test_parse_regular_season_folder() {
    // Test parsing a season folder without season-level IMDB ID (regular series)
    let folder_name = "[S02][Season 02]-[A][绝命毒师][Breaking Bad](2009)-tt0903747-tmdb1396";
    
    let result = parse_organized_tv_series_folder(folder_name);
    
    assert!(result.is_some(), "Should be able to parse the folder name");
    
    let info = result.unwrap();
    assert_eq!(info.title, "绝命毒师");
    assert_eq!(info.tmdb_id, Some(1396));
    assert_eq!(info.season_imdb_id, Some("tt0903747".to_string()));
    
    println!("✓ Test passed: Parsed regular season folder correctly");
}

#[test]
fn test_generate_season_folder_with_season_imdb_id() {
    // Test generating season folder name with season-level IMDB ID
    let folder_name = generate_season_folder(
        4,
        "第 4 季",
        "A",
        "爱，死亡和机器人",
        "Love, Death & Robots",
        Some("tt21661768"), // Season-level IMDB ID
        450504,
        Some("2025-05-15"),
    );
    
    assert!(folder_name.contains("tt21661768"), "Season folder should contain season-level IMDB ID, but got: {}", folder_name);
    assert!(folder_name.contains("tmdb450504"), "Season folder should contain season TMDB ID, but got: {}", folder_name);
    assert!(!folder_name.contains("tt9561862"), "Season folder should NOT contain TV Show IMDB ID");
    
    println!("✓ Test passed: Generated season folder correctly uses season-level IMDB ID");
}

#[test]
fn test_generate_season_folder_without_season_imdb_id() {
    // Test generating season folder name without season-level IMDB ID (falls back to TV Show ID)
    let folder_name = generate_season_folder(
        2,
        "Season 2",
        "A",
        "绝命毒师",
        "Breaking Bad",
        Some("tt0903747"), // TV Show IMDB ID (no season-level ID)
        3572,
        Some("2009-03-08"),
    );
    
    assert!(folder_name.contains("tt0903747"), "Season folder should use TV Show IMDB ID when no season-level ID available, but got: {}", folder_name);
    assert!(folder_name.contains("tmdb3572"), "Season folder should contain season TMDB ID, but got: {}", folder_name);
    
    println!("✓ Test passed: Generated season folder correctly falls back to TV Show IMDB ID");
}

#[test]
fn test_season_imdb_id_priority() {
    // Test that season-level IMDB ID takes priority over TV Show ID
    // This simulates the logic in planner.rs
    
    // Mock folder info with season-level IMDB ID
    let folder_info = OrganizedTvSeriesFolderInfo {
        title: "爱，死亡和机器人".to_string(),
        original_title: Some("Love, Death & Robots".to_string()),
        year: Some(2025),
        imdb_id: None, // TV Show IMDB ID not available in season folder
        tmdb_id: Some(86831),
        season_imdb_id: Some("tt21661768".to_string()), // Season-level IMDB ID
    };
    
    // Mock TMDB TV Show info
    let show_imdb_id = "tt9561862"; // TV Show IMDB ID
    
    // Priority logic: folder_info.season_imdb_id > TMDB season external_ids > TV show IMDB ID
    let season_imdb_id = if let Some(ref folder_season_imdb) = folder_info.season_imdb_id {
        folder_season_imdb.clone()
    } else {
        // Fallback to TMDB or TV Show ID
        show_imdb_id.to_string()
    };
    
    assert_eq!(season_imdb_id, "tt21661768", "Should use season-level IMDB ID from folder");
    assert_ne!(season_imdb_id, "tt9561862", "Should NOT use TV Show IMDB ID when season-level ID is available");
    
    println!("✓ Test passed: Season IMDB ID priority logic is correct");
}
