//! Folder name generator tests
//!
//! Tests cover:
//! - Chinese movie folder generation
//! - Japanese movie folder generation (with Kanji characters)
//! - English movie folder generation
//! - TV series folder generation
//! - Black Widow scenario with category prefix

use media_organizer::generators::folder::{generate_movie_folder, generate_tv_series_folder};
use media_organizer::models::media::{MovieMetadata, TvSeriesMetadata};
use media_organizer::core::parser::parse_organized_movie_folder;

/// Test folder generation for Chinese movies
#[test]
fn test_chinese_movie_folder() {
    let metadata = MovieMetadata {
        tmdb_id: 483214,
        imdb_id: None,
        original_title: "一夜情深".to_string(),
        title: "一夜情深".to_string(),
        original_language: "zh".to_string(),
        year: 2013,
        ..Default::default()
    };
    
    let folder = generate_movie_folder(&metadata, None);
    assert!(folder.starts_with("[Y]"), "Expected Chinese movie to start with [Y], got: {}", folder);
    assert!(folder.contains("[一夜情深]"), "Expected folder to contain Chinese title");
}

/// Test folder generation for Japanese movies with Kanji characters
#[test]
fn test_japanese_movie_folder_with_kanji() {
    let test_cases = vec![
        ("横道世之介", "ja", "H"),
        ("蜜月", "ja", "M"),
        ("超伝合体", "ja", "C"),
        ("妖艶女忍者", "ja", "Y"),
        ("宇宙戦艦", "ja", "Y"),
        ("陰陽師", "ja", "Y"),
        ("忠犬ハチ公", "ja", "Z"),
        ("最終幻想", "ja", "Z"),
    ];
    
    for (title, lang, expected_prefix) in test_cases {
        let metadata = MovieMetadata {
            tmdb_id: 12345,
            imdb_id: None,
            original_title: title.to_string(),
            title: title.to_string(),
            original_language: lang.to_string(),
            year: 2020,
            ..Default::default()
        };
        
        let folder = generate_movie_folder(&metadata, None);
        let expected = format!("[{}]", expected_prefix);
        assert!(folder.starts_with(&expected), 
            "Expected Japanese movie '{}' to start with {}, got: {}", title, expected, folder);
    }
}

/// Test folder generation for English movies
#[test]
fn test_english_movie_folder() {
    // Test with "The" prefix removal
    let metadata1 = MovieMetadata {
        tmdb_id: 155,
        imdb_id: Some("tt0468569".to_string()),
        original_title: "The Dark Knight".to_string(),
        title: "黑暗骑士".to_string(),
        original_language: "en".to_string(),
        year: 2008,
        ..Default::default()
    };
    
    let folder1 = generate_movie_folder(&metadata1, None);
    // Chinese title takes priority since it contains Chinese characters
    assert!(folder1.starts_with("[H]"), "Expected Chinese localized title to use pinyin, got: {}", folder1);
    
    // Test English-only movie (no Chinese title)
    let metadata2 = MovieMetadata {
        tmdb_id: 27205,
        imdb_id: Some("tt1375666".to_string()),
        original_title: "Inception".to_string(),
        title: "Inception".to_string(),
        original_language: "en".to_string(),
        year: 2010,
        ..Default::default()
    };
    
    let folder2 = generate_movie_folder(&metadata2, None);
    assert!(folder2.starts_with("[I]"), "Expected English movie to start with [I], got: {}", folder2);
    
    // Test "The" removal for English-only
    let metadata3 = MovieMetadata {
        tmdb_id: 1,
        imdb_id: None,
        original_title: "The Godfather".to_string(),
        title: "The Godfather".to_string(),
        original_language: "en".to_string(),
        year: 1972,
        ..Default::default()
    };
    
    let folder3 = generate_movie_folder(&metadata3, None);
    assert!(folder3.starts_with("[G]"), "Expected 'The Godfather' to start with [G] (T removed), got: {}", folder3);
}

/// Test folder generation for movies without Chinese translation (both titles English)
/// When TMDB has no Chinese translation and no fallback is available
#[test]
fn test_movie_folder_no_chinese_no_fallback() {
    // Scenario: TMDB has no Chinese translation (title == original_title == English)
    let metadata = MovieMetadata {
        tmdb_id: 497582, // Black Widow
        imdb_id: Some("tt3480822".to_string()),
        original_title: "Black Widow".to_string(),
        title: "Black Widow".to_string(),
        original_language: "en".to_string(),
        year: 2021,
        ..Default::default()
    };
    
    let folder = generate_movie_folder(&metadata, None);
    // Should show both English titles since there's no Chinese translation
    assert!(folder.starts_with("[B]"), "Expected English movie to start with [B], got: {}", folder);
    assert!(folder.contains("[Black Widow]"), "Expected folder to contain title");
}

/// Test folder generation for Black Widow scenario with category prefix [B]
/// This is the ACTUAL scenario user reported: [B][Black Widow](2021)-tt3480822-tmdb497698
/// When TMDB details returns English but fallback search returns Chinese
#[test]
fn test_movie_folder_black_widow_category_prefix_b() {
    // This is the scenario user reported:
    // Folder name: [B][Black Widow](2021)-tt3480822-tmdb497698
    // - Parser extracts: original_title="Black Widow", title=None
    // - TMDB details API returns English (Black Widow) without Chinese
    // - But TMDB search with zh-CN language returns Chinese "黑寡妇"
    // - Final result should be: [H][黑寡妇][Black Widow](2021)-tt3480822-tmdb497698
    
    // When TMDB search returns Chinese translation as fallback
    let metadata_with_chinese_fallback = MovieMetadata {
        tmdb_id: 497698, // Black Widow TMDB ID
        imdb_id: Some("tt3480822".to_string()),
        original_title: "Black Widow".to_string(),
        title: "黑寡妇".to_string(), // Chinese translation from search
        original_language: "en".to_string(),
        year: 2021,
        ..Default::default()
    };
    
    let folder = generate_movie_folder(&metadata_with_chinese_fallback, None);
    
    // Should use Chinese title for sorting prefix
    assert!(folder.starts_with("[H]"), 
        "Expected Black Widow with Chinese fallback to start with [H], got: {}", folder);
    assert!(folder.contains("[黑寡妇]"), 
        "Expected folder to contain Chinese title '黑寡妇', got: {}", folder);
    assert!(folder.contains("[Black Widow]"), 
        "Expected folder to contain original title 'Black Widow', got: {}", folder);
    assert!(folder.contains("(2021)"), 
        "Expected folder to contain year '2021', got: {}", folder);
    assert!(folder.contains("-tt3480822-tmdb497698"), 
        "Expected folder to contain IDs, got: {}", folder);
    
    // Verify the complete format
    let expected = "[H][黑寡妇][Black Widow](2021)-tt3480822-tmdb497698";
    assert_eq!(folder, expected, 
        "Expected folder '{}', got: {}", expected, folder);
}

/// Test folder generation for Spider-Man: No Way Home scenario without Chinese
/// This tests when TMDB has no Chinese translation
#[test]
fn test_movie_folder_spider_man_no_chinese_tmdb() {
    // Spider-Man: No Way Home - scenario where TMDB might not have Chinese
    let metadata = MovieMetadata {
        tmdb_id: 634649,
        imdb_id: Some("tt10872600".to_string()),
        original_title: "Spider-Man: No Way Home".to_string(),
        title: "Spider-Man: No Way Home".to_string(), // No Chinese translation
        original_language: "en".to_string(),
        year: 2021,
        ..Default::default()
    };
    
    let folder = generate_movie_folder(&metadata, None);
    assert!(folder.starts_with("[S]"), "Expected to start with [S], got: {}", folder);
}

/// Test folder generation for Black Widow scenario
/// When both folder titles are English but TMDB might have Chinese translation
#[test]
fn test_movie_folder_black_widow_scenario() {
    // Scenario 1: TMDB returns Chinese translation
    let metadata_with_chinese = MovieMetadata {
        tmdb_id: 497582, // Black Widow
        imdb_id: Some("tt3480822".to_string()),
        original_title: "Black Widow".to_string(),
        title: "黑寡妇".to_string(), // TMDB has Chinese translation
        original_language: "en".to_string(),
        year: 2021,
        ..Default::default()
    };
    
    let folder_with_chinese = generate_movie_folder(&metadata_with_chinese, None);
    assert!(folder_with_chinese.starts_with("[H]"), 
        "Expected Black Widow with Chinese to start with [H], got: {}", folder_with_chinese);
    assert!(folder_with_chinese.contains("[黑寡妇]"), 
        "Expected folder to contain Chinese title, got: {}", folder_with_chinese);
    
    // Scenario 2: TMDB returns English only (no Chinese translation)
    let metadata_no_chinese = MovieMetadata {
        tmdb_id: 497582,
        imdb_id: Some("tt3480822".to_string()),
        original_title: "Black Widow".to_string(),
        title: "Black Widow".to_string(), // No Chinese translation
        original_language: "en".to_string(),
        year: 2021,
        ..Default::default()
    };
    
    let folder_no_chinese = generate_movie_folder(&metadata_no_chinese, None);
    assert!(folder_no_chinese.starts_with("[B]"), 
        "Expected Black Widow without Chinese to start with [B], got: {}", folder_no_chinese);
}

/// Test folder generation for Black Widow scenario with Chinese title from search
/// This tests when TMDB details API returns English but search returns Chinese
#[test]
fn test_movie_folder_black_widow_with_chinese_from_search() {
    // Simulate the scenario where search finds Chinese title
    let metadata_with_chinese_from_search = MovieMetadata {
        tmdb_id: 497582, // Black Widow
        imdb_id: Some("tt3480822".to_string()),
        original_title: "Black Widow".to_string(),
        title: "黑寡妇".to_string(), // From search result
        original_language: "en".to_string(),
        year: 2021,
        ..Default::default()
    };
    
    let folder = generate_movie_folder(&metadata_with_chinese_from_search, None);
    assert!(folder.starts_with("[H]"), 
        "Expected Black Widow with Chinese to start with [H], got: {}", folder);
    assert!(folder.contains("[黑寡妇]"), 
        "Expected folder to contain Chinese title, got: {}", folder);
}

/// Test folder generation for Black Widow scenario with category prefix
/// This tests when folder has format [B][Black Widow][Black Widow](2021)...
#[test]
fn test_movie_folder_black_widow_category_prefix() {
    // Simulate the scenario where folder has category prefix and dual English titles
    // but TMDB search returns Chinese translation
    let metadata = MovieMetadata {
        tmdb_id: 497582, // Black Widow
        imdb_id: Some("tt3480822".to_string()),
        original_title: "Black Widow".to_string(),
        title: "黑寡妇".to_string(), // From TMDB search
        original_language: "en".to_string(),
        year: 2021,
        ..Default::default()
    };
    
    let folder = generate_movie_folder(&metadata, None);
    assert!(folder.starts_with("[H]"), 
        "Expected Black Widow with Chinese to start with [H], got: {}", folder);
    assert!(folder.contains("[黑寡妇]"), 
        "Expected folder to contain Chinese title, got: {}", folder);
}

/// Test folder generation for movies WITH Chinese translation
#[test]
fn test_movie_folder_with_chinese_translation() {
    // Aladdin 2019 - has Chinese translation
    let metadata = MovieMetadata {
        tmdb_id: 278,
        imdb_id: Some("tt0468569".to_string()),
        original_title: "The Shawshank Redemption".to_string(),
        title: "肖申克的救赎".to_string(), // Has Chinese translation
        original_language: "en".to_string(),
        year: 1994,
        ..Default::default()
    };
    
    let folder = generate_movie_folder(&metadata, None);
    // Should use Chinese title for sorting
    assert!(folder.starts_with("[X]"), "Expected Chinese title to start with [X], got: {}", folder);
    assert!(folder.contains("[肖申克的救赎]"), "Expected folder to contain Chinese title");
}

/// Test folder generation for TV series with Chinese titles
#[test]
fn test_chinese_tv_series_folder() {
    let metadata = TvSeriesMetadata {
        tmdb_id: 123,
        imdb_id: Some("tt1234567".to_string()),
        name: "三国演义".to_string(),
        original_name: "三国演义".to_string(),
        original_language: "zh".to_string(),
        first_air_date: Some("2020-01-01".to_string()),
        ..Default::default()
    };
    
    let folder = generate_tv_series_folder(&metadata);
    assert!(folder.starts_with("[S]"), "Expected Chinese TV series to start with [S], got: {}", folder);
}

/// Test folder generation for Japanese TV series with Kanji
#[test]
fn test_japanese_tv_series_folder_with_kanji() {
    let test_cases = vec![
        ("半沢直樹", "ja", "B"),
        ("東大特訓班", "ja", "D"),
        ("龍馬伝", "ja", "L"),
    ];
    
    for (name, lang, expected_prefix) in test_cases {
        let metadata = TvSeriesMetadata {
            tmdb_id: 12345,
            imdb_id: None,
            name: name.to_string(),
            original_name: name.to_string(),
            original_language: lang.to_string(),
            first_air_date: Some("2020-01-01".to_string()),
            ..Default::default()
        };
        
        let folder = generate_tv_series_folder(&metadata);
        let expected = format!("[{}]", expected_prefix);
        assert!(folder.starts_with(&expected), 
            "Expected Japanese TV series '{}' to start with {}, got: {}", name, expected, folder);
    }
}

/// Test folder generation for movies with special characters in title
#[test]
fn test_movie_folder_with_special_characters() {
    let test_cases = vec![
        ("\"吃吃\"的爱", "zh", "C"),
        ("【英雄】", "zh", "Y"),
        ("《泰坦尼克号》", "en", "T"),
        ("  卧虎藏龙", "zh", "W"),
        ("-黑客帝国", "en", "H"),
    ];
    
    for (title, lang, expected_prefix) in test_cases {
        let metadata = MovieMetadata {
            tmdb_id: 12345,
            imdb_id: None,
            original_title: title.to_string(),
            title: title.to_string(),
            original_language: lang.to_string(),
            year: 2020,
            ..Default::default()
        };
        
        let folder = generate_movie_folder(&metadata, None);
        let expected = format!("[{}]", expected_prefix);
        assert!(folder.starts_with(&expected), 
            "Expected movie '{}' to start with {}, got: {}", title, expected, folder);
    }
}

/// End-to-end test for Black Widow scenario with category prefix
/// Tests the complete flow: parse -> search TMDB -> generate folder
/// This is the exact scenario reported by user: [B][Black Widow](2021)-tt3480822-tmdb497698
#[test]
fn test_black_widow_end_to_end_scenario() {
    // Step 1: Parse the folder name
    let folder_name = "[B][Black Widow](2021)-tt3480822-tmdb497698";
    let parsed_info = parse_organized_movie_folder(folder_name);
    
    assert!(parsed_info.is_some(), 
        "Failed to parse folder name: {}", folder_name);
    
    let info = parsed_info.unwrap();
    assert_eq!(info.original_title, Some("Black Widow".to_string()),
        "Expected original_title to be 'Black Widow'");
    assert_eq!(info.title, None,
        "Expected title to be None (no second title in folder name)");
    assert_eq!(info.year, 2021,
        "Expected year to be 2021");
    assert_eq!(info.tmdb_id, 497698,
        "Expected tmdb_id to be 497698");
    
    // Step 2: Simulate TMDB search returning Chinese translation
    // In real scenario, this would come from tmdb.search_movie_with_language("Black Widow", 2021, "zh-CN")
    let chinese_title_from_tmdb = "黑寡妇";
    
    // Step 3: Build metadata with Chinese title
    let metadata = MovieMetadata {
        tmdb_id: info.tmdb_id,
        imdb_id: info.imdb_id,
        original_title: info.original_title.unwrap(),
        title: chinese_title_from_tmdb.to_string(), // Chinese title from TMDB search
        original_language: "en".to_string(),
        year: info.year,
        ..Default::default()
    };
    
    // Step 4: Generate folder name
    let generated_folder = generate_movie_folder(&metadata, None);
    
    // Verify the result
    assert!(generated_folder.starts_with("[H]"),
        "Expected folder to start with [H] (from '黑寡妇'), got: {}", generated_folder);
    assert!(generated_folder.contains("[黑寡妇]"),
        "Expected folder to contain Chinese title '黑寡妇', got: {}", generated_folder);
    assert!(generated_folder.contains("[Black Widow]"),
        "Expected folder to contain original title 'Black Widow', got: {}", generated_folder);
    assert!(generated_folder.contains("(2021)"),
        "Expected folder to contain year '(2021)', got: {}", generated_folder);
    assert!(generated_folder.contains("-tt3480822-tmdb497698"),
        "Expected folder to contain IDs, got: {}", generated_folder);
    
    // Final verification: Full expected format
    let expected_folder = "[H][黑寡妇][Black Widow](2021)-tt3480822-tmdb497698";
    assert_eq!(generated_folder, expected_folder,
        "Expected folder '{}', got: {}", expected_folder, generated_folder);
}

/// Test folder generation for movies where TMDB returns English title only
/// This simulates the Spider-Man: No Way Home scenario where TMDB doesn't have Chinese translation
#[test]
fn test_movie_folder_without_chinese_translation() {
    // Spider-Man: No Way Home scenario - TMDB returns English for both title and original_title
    // But we should get Chinese title from fallback
    let metadata = MovieMetadata {
        tmdb_id: 634649,
        imdb_id: Some("tt10872600".to_string()),
        original_title: "Spider-Man: No Way Home".to_string(),
        title: "蜘蛛侠：英雄无归".to_string(),  // Now using fallback Chinese title
        original_language: "en".to_string(),
        year: 2021,
        ..Default::default()
    };
    
    let folder = generate_movie_folder(&metadata, None);
    
    // Since title contains Chinese, should show both titles
    assert!(folder.starts_with("[Z]"), "Expected Spider-Man with Chinese to start with [Z], got: {}", folder);
    assert!(folder.contains("[蜘蛛侠"), "Expected folder to contain Chinese title, got: {}", folder);
    assert!(folder.contains("[Spider-Man"), "Expected folder to contain English title, got: {}", folder);
}
