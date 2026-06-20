//! Unit tests for the metadata utility functions.
//! 
//! Tests cover:
//! - Title normalization
//! - Title similarity comparison
//! - ID validation (IMDB, TMDB)

use media_organizer::utils::metadata::{
    compare_titles, is_valid_imdb_id, is_valid_tmdb_id, normalize_title, title_contains,
};

// ========== NORMALIZE TITLE TESTS ==========

#[test]
fn test_normalize_title_basic() {
    assert_eq!(normalize_title("Hello World"), "hello world");
    assert_eq!(normalize_title("HELLO WORLD"), "hello world");
    assert_eq!(normalize_title("Hello  World"), "hello world");
}

#[test]
fn test_normalize_title_with_special_chars() {
    assert_eq!(normalize_title("Love, Death & Robots"), "love death robots");
    assert_eq!(normalize_title("The Terminal List!"), "the terminal list");
    assert_eq!(normalize_title("Avengers: Endgame"), "avengers endgame");
}

#[test]
fn test_normalize_title_chinese() {
    assert_eq!(normalize_title("爱，死亡和机器人"), "爱死亡和机器人");
    assert_eq!(normalize_title("终极名单"), "终极名单");
    assert_eq!(normalize_title("复仇者联盟：终局之战"), "复仇者联盟终局之战");
}

#[test]
fn test_normalize_title_empty() {
    assert_eq!(normalize_title(""), "");
    assert_eq!(normalize_title("   "), "");
}

// ========== TITLE CONTAINS TESTS ==========

#[test]
fn test_title_contains_exact_match() {
    assert!(title_contains("hello world", "hello world"));
    assert!(title_contains("love death robots", "love death robots"));
}

#[test]
fn test_title_contains_subset() {
    assert!(title_contains("love death robots", "love"));
    assert!(title_contains("the terminal list", "terminal"));
    assert!(title_contains("复仇者联盟终局之战", "复仇者"));
}

#[test]
fn test_title_contains_partial_match() {
    assert!(title_contains("love death robots", "death robots"));
    assert!(title_contains("the terminal list", "terminal list"));
}

#[test]
fn test_title_contains_no_match() {
    assert!(!title_contains("hello world", "foo"));
    assert!(!title_contains("love death robots", "terminator"));
}

// ========== COMPARE TITLES TESTS ==========

#[test]
fn test_compare_titles_exact_match_with_year() {
    let result = compare_titles(
        "Love, Death & Robots",
        Some(2025),
        "Love, Death & Robots",
        Some("Love, Death & Robots"),
        Some(2025),
    );
    assert!(result.matched);
    assert_eq!(result.score, 1.0);
    assert_eq!(result.reason, "标题和年份匹配");
}

#[test]
fn test_compare_titles_chinese_exact_match() {
    let result = compare_titles(
        "爱，死亡和机器人",
        Some(2025),
        "爱，死亡和机器人",
        Some("Love, Death & Robots"),
        Some(2025),
    );
    assert!(result.matched);
    assert_eq!(result.score, 1.0);
}

#[test]
fn test_compare_titles_year_within_tolerance() {
    let result = compare_titles(
        "The Terminal List",
        Some(2022),
        "The Terminal List",
        Some("The Terminal List"),
        Some(2021),
    );
    assert!(result.matched);
    assert_eq!(result.score, 1.0);
}

#[test]
fn test_compare_titles_title_match_year_mismatch() {
    let result = compare_titles(
        "The Terminal List",
        Some(2022),
        "The Terminal List",
        Some("The Terminal List"),
        Some(2025),
    );
    assert!(result.matched);
    assert_eq!(result.score, 0.6);
    assert_eq!(result.reason, "标题匹配但年份不匹配");
}

#[test]
fn test_compare_titles_partial_title_match() {
    let result = compare_titles(
        "Love Death Robots",
        Some(2025),
        "Love, Death & Robots",
        Some("Love, Death & Robots"),
        Some(2025),
    );
    assert!(result.matched);
    assert_eq!(result.score, 1.0); // 规范化后完全匹配
}

#[test]
fn test_compare_titles_no_match() {
    let result = compare_titles(
        "The Avengers",
        Some(2012),
        "The Terminal List",
        Some("The Terminal List"),
        Some(2022),
    );
    assert!(!result.matched);
    assert_eq!(result.score, 0.0);
    assert_eq!(result.reason, "标题不匹配");
}

#[test]
fn test_compare_titles_with_original_title() {
    let result = compare_titles(
        "Love, Death & Robots",
        Some(2025),
        "爱，死亡和机器人",
        Some("Love, Death & Robots"),
        Some(2025),
    );
    assert!(result.matched);
    assert_eq!(result.score, 0.75); // 通过 original_title 匹配，但 api_title(中文) 和 parsed_title(英文) 不直接匹配
}

#[test]
fn test_compare_titles_empty_parsed_title() {
    let result = compare_titles(
        "",
        Some(2025),
        "Love, Death & Robots",
        Some("Love, Death & Robots"),
        Some(2025),
    );
    assert!(!result.matched);
    assert_eq!(result.score, 0.0);
}

#[test]
fn test_compare_titles_no_year() {
    let result = compare_titles(
        "Love, Death & Robots",
        None,
        "Love, Death & Robots",
        Some("Love, Death & Robots"),
        Some(2025),
    );
    assert!(result.matched);
    assert_eq!(result.score, 1.0);
}

// ========== ID VALIDATION TESTS ==========

#[test]
fn test_is_valid_imdb_id_valid() {
    assert!(is_valid_imdb_id("tt1234567"));
    assert!(is_valid_imdb_id("tt12345678"));
    assert!(is_valid_imdb_id("tt123456789"));
    assert!(is_valid_imdb_id("tt1234567890"));
    assert!(is_valid_imdb_id("tt11743610")); // 终极名单
    assert!(is_valid_imdb_id("tt21661768")); // 爱死亡机器人
}

#[test]
fn test_is_valid_imdb_id_invalid() {
    assert!(!is_valid_imdb_id("1234567")); // missing tt prefix
    assert!(!is_valid_imdb_id("tt12345")); // too short
    assert!(!is_valid_imdb_id("tt1234567890123")); // too long
    assert!(!is_valid_imdb_id("ttabcdefg")); // non-numeric
    assert!(!is_valid_imdb_id("imdb1234567")); // wrong prefix
}

#[test]
fn test_is_valid_tmdb_id_valid() {
    assert!(is_valid_tmdb_id("12345"));
    assert!(is_valid_tmdb_id("450504")); // 爱死亡机器人
    assert!(is_valid_tmdb_id("186250")); // 终极名单
    assert!(is_valid_tmdb_id("1"));
}

#[test]
fn test_is_valid_tmdb_id_invalid() {
    assert!(!is_valid_tmdb_id(""));
    assert!(!is_valid_tmdb_id("abc"));
    assert!(!is_valid_tmdb_id("123abc"));
    assert!(!is_valid_tmdb_id("-1"));
}

// ========== INTEGRATION TESTS ==========

#[test]
fn test_real_world_examples() {
    // 爱，死亡和机器人 - 中文标题匹配
    let result = compare_titles(
        "爱，死亡和机器人",
        Some(2025),
        "爱，死亡和机器人",
        Some("Love, Death & Robots"),
        Some(2025),
    );
    assert!(result.matched);
    assert_eq!(result.score, 1.0);

    // 终极名单 - 通过英文原名匹配
    let result = compare_titles(
        "The Terminal List",
        Some(2022),
        "终极名单",
        Some("The Terminal List"),
        Some(2022),
    );
    assert!(result.matched);
    assert_eq!(result.score, 0.75); // 通过 original_title 匹配，但中英文不直接匹配

    // 标题不完全匹配但足够相似
    let result = compare_titles(
        "Love Death Robots Season 4",
        Some(2025),
        "爱，死亡和机器人",
        Some("Love, Death & Robots"),
        Some(2025),
    );
    assert!(result.matched);
    assert!(result.score >= 0.75);
}