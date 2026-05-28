//! Folder name generator tests
//!
//! Tests cover:
//! - Chinese movie folder generation
//! - Japanese movie folder generation (with Kanji characters)
//! - English movie folder generation
//! - TV series folder generation

use media_organizer::generators::folder::{generate_movie_folder, generate_tv_series_folder};
use media_organizer::models::media::{MovieMetadata, TvSeriesMetadata};

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
