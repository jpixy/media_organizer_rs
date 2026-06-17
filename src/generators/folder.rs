//! Folder name generator.

use crate::models::media::{MovieMetadata, TvSeriesMetadata};
use crate::utils::chinese;

/// ============================================================================
/// Sorting Prefix Generation
/// 
/// Rule Priority (Highest to Lowest):
/// 1. If there is a Chinese localized name/title, use PINYIN FIRST LETTER of that name
/// 2. If no Chinese name:
///    - Chinese original language: PINYIN FIRST LETTER of original name
///    - English original language: FIRST LETTER after removing articles (The/A/An)
///    - Other languages: FIRST LETTER of original name
///
/// Example Format (Chinese localized title comes first):
/// [Z][一级机密][1級機密](2017)-tt6955808-tmdb47992
/// [D][黑暗骑士][The Dark Knight](2008)-tt0468569-tmdb155
/// ============================================================================

/// Generate sorting prefix character.
/// 
/// Rule Priority (Highest to Lowest):
/// 1. If title contains Chinese characters, use PINYIN FIRST LETTER of the first Chinese character
/// 2. For English titles: FIRST LETTER after removing articles (The/A/An)
/// 3. For other languages: FIRST LETTER of title
///
/// Example Format:
/// [Z][追龍](2017)-tt6015328-tmdb426242
/// [H][横道世之介][横道世之介](2013)-tt2151915-tmdb200145
pub fn generate_sort_prefix(
    title: &str,
    original_language: &str,
) -> char {
    // Rule 1: If title contains Chinese characters, use pinyin
    if chinese::contains_chinese(title) {
        return chinese::get_first_pinyin_letter(title);
    }

    // Rule 2: English - remove articles first
    if original_language == "en" {
        let title_lower = title.to_lowercase();
        let effective_title = if title_lower.starts_with("the ") {
            &title[4..]
        } else if title_lower.starts_with("a ") {
            &title[2..]
        } else if title_lower.starts_with("an ") {
            &title[3..]
        } else {
            title
        };
        return effective_title.chars().next().unwrap_or('?').to_ascii_uppercase();
    }

    // Rule 3: Other languages - use first character directly
    title.chars().next().unwrap_or('?').to_ascii_uppercase()
}

/// Generate movie folder name.
///
/// New Format: `[${sortPrefix}][${title}][${originalTitle}](${edition})-${year}-${imdb}-${tmdb}`
/// Sort Prefix Generation Rules: See `generate_sort_prefix` documentation
pub fn generate_movie_folder(metadata: &MovieMetadata, edition: Option<&str>) -> String {
    let mut parts = Vec::new();

    // Add sorting prefix - always use title (which is the Chinese/localized title)
    let sort_prefix = generate_sort_prefix(
        &metadata.title,
        &metadata.original_language,
    );
    parts.push(format!("[{}]", sort_prefix));

    // Handle title deduplication for Chinese movies
    let is_chinese_lang = matches!(metadata.original_language.as_str(), "zh" | "cn" | "zh-CN" | "zh-TW" | "zh-HK");
    let is_chinese = is_chinese_lang;
    let titles_same = normalize_title(&metadata.original_title) == normalize_title(&metadata.title);
    
    // Check if title contains Chinese characters
    let title_has_chinese = chinese::contains_chinese(&metadata.title);

    if is_chinese || (titles_same && title_has_chinese) {
        // Only use one title for Chinese movies or when titles are the same AND title contains Chinese
        parts.push(format!("[{}]", sanitize_filename(&metadata.title)));
    } else {
        // Use both localized and original title (localized first)
        // This handles the case where TMDB doesn't have Chinese translation
        parts.push(format!("[{}]", sanitize_filename(&metadata.title)));
        parts.push(format!("[{}]", sanitize_filename(&metadata.original_title)));
    }

    // Add edition if present
    if let Some(ed) = edition {
        parts.push(format!("({})", ed));
    }

    // Add year
    parts.push(format!("({})", metadata.year));

    // Add IMDB ID
    if let Some(ref imdb_id) = metadata.imdb_id {
        parts.push(format!("-{}", imdb_id));
    }

    // Add TMDB ID
    parts.push(format!("-tmdb{}", metadata.tmdb_id));

    parts.join("")
}

/// Generate TV show folder name.
///
/// New Format: `[${sortPrefix}][${showTitle}][${showOriginalTitle}](${year})-${showImdb}-${showTmdb}`
/// Sort Prefix Generation Rules: See `generate_sort_prefix` documentation
pub fn generate_tv_series_folder(metadata: &TvSeriesMetadata) -> String {
    let mut parts = Vec::new();

    // Add sorting prefix - always use name (which is the Chinese/localized name)
    let sort_prefix = generate_sort_prefix(
        &metadata.name,
        &metadata.original_language,
    );
    parts.push(format!("[{}]", sort_prefix));

    // Handle title deduplication
    let is_chinese_lang = matches!(metadata.original_language.as_str(), "zh" | "cn" | "zh-CN" | "zh-TW" | "zh-HK");
    let is_chinese = is_chinese_lang;
    let titles_same = normalize_title(&metadata.original_name) == normalize_title(&metadata.name);

    if is_chinese || titles_same {
        parts.push(format!("[{}]", sanitize_filename(&metadata.name)));
    } else {
        // Use both localized and original title (localized first)
        parts.push(format!("[{}]", sanitize_filename(&metadata.name)));
        parts.push(format!("[{}]", sanitize_filename(&metadata.original_name)));
    }

    // Add year
    parts.push(format!("({})", metadata.year));

    // Add IMDB ID
    if let Some(ref imdb_id) = metadata.imdb_id {
        parts.push(format!("-{}", imdb_id));
    }

    // Add TMDB ID
    parts.push(format!("-tmdb{}", metadata.tmdb_id));

    parts.join("")
}

/// Generate season folder name.
///
/// Format: `[S04][Season 04]-[A][爱死亡与机器人][Love, Death & Robots](2019)-tt9561862-tmdb86831`
pub fn generate_season_folder(
    season_number: u16, 
    _season_name: &str, 
    sort_prefix: &str,
    show_name: &str,
    original_name: &str,
    imdb_id: Option<&str>, 
    tmdb_id: u64,
    air_date: Option<&str>
) -> String {
    // Part 1: Season prefix and name (no separator)
    // Always use "Season XX" format regardless of localization
    let season_part = format!("[S{:02}][Season {:02}]", season_number, season_number);

    // Part 2: Show title part with year
    let mut title_parts = Vec::new();
    
    // Add sort prefix [A]
    title_parts.push(format!("[{}]", sort_prefix));
    
    // Extract year from air_date (format: "2019-03-15" -> "2019")
    let year = air_date.and_then(|d| d.split('-').next()).unwrap_or("");
    let has_year = !year.is_empty();
    
    // Check if show_name and original_name are different
    let names_different = normalize_title(show_name) != normalize_title(original_name);
    
    // Add show names
    if names_different {
        // Show name without year, original name with year at the end
        title_parts.push(format!("[{}]", sanitize_filename(show_name)));
        if has_year {
            title_parts.push(format!("[{}]({})", sanitize_filename(original_name), year));
        } else {
            title_parts.push(format!("[{}]", sanitize_filename(original_name)));
        }
    } else {
        // Only one name, with year if available
        if has_year {
            title_parts.push(format!("[{}]({})", sanitize_filename(show_name), year));
        } else {
            title_parts.push(format!("[{}]", sanitize_filename(show_name)));
        }
    }
    
    // Part 3: IDs
    let mut id_parts = Vec::new();
    
    // Add IMDB ID tt9561862
    if let Some(imdb) = imdb_id {
        id_parts.push(imdb.to_string());
    }

    // Add TMDB ID tmdb86831
    id_parts.push(format!("tmdb{}", tmdb_id));
    
    // Combine: [S04][Season 04]-[A][爱死亡与机器人][Love, Death & Robots](2019)-tt9561862-tmdb86831
    format!("{}-{}-{}", season_part, title_parts.join(""), id_parts.join("-"))
}

/// Sanitize a string for use in filenames.
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Normalize title for comparison (handles Traditional/Simplified Chinese).
fn normalize_title(s: &str) -> String {
    // Basic normalization - trim and lowercase
    // TODO: Add Traditional/Simplified Chinese conversion
    s.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_movie_folder() {
        let metadata = MovieMetadata {
            tmdb_id: 19995,
            imdb_id: Some("tt0499549".to_string()),
            original_title: "Avatar".to_string(),
            title: "阿凡达".to_string(),
            original_language: "en".to_string(),
            year: 2009,
            ..Default::default()
        };

        let folder = generate_movie_folder(&metadata, None);
        assert!(folder.contains("[A]")); // Sort prefix: A
        assert!(folder.contains("[Avatar]"));
        assert!(folder.contains("[阿凡达]"));
        assert!(folder.contains("(2009)"));
        assert!(folder.contains("tt0499549"));
        assert!(folder.contains("tmdb19995"));
    }

    #[test]
    fn test_chinese_movie_no_duplicate() {
        let metadata = MovieMetadata {
            tmdb_id: 12345,
            imdb_id: Some("tt1234567".to_string()),
            original_title: "霸王别姬".to_string(),
            title: "霸王别姬".to_string(),
            original_language: "zh".to_string(),
            year: 1993,
            ..Default::default()
        };

        let folder = generate_movie_folder(&metadata, None);
        assert!(folder.contains("[B]")); // Sort prefix: B
        // Should only have one title
        assert_eq!(folder.matches("[霸王别姬]").count(), 1);
    }

    #[test]
    fn test_english_movie_with_the_prefix() {
        let metadata = MovieMetadata {
            tmdb_id: 155,
            imdb_id: Some("tt0468569".to_string()),
            original_title: "The Dark Knight".to_string(),
            title: "黑暗骑士".to_string(),
            original_language: "en".to_string(),
            year: 2008,
            ..Default::default()
        };

        let folder = generate_movie_folder(&metadata, None);
        assert!(folder.contains("[H]")); // Sort prefix: H (from "黑暗骑士" pinyin)
        assert!(folder.contains("[The Dark Knight]"));
        assert!(folder.contains("[黑暗骑士]"));
        assert!(folder.contains("(2008)"));
    }

    #[test]
    fn test_korean_movie_with_chinese_title() {
        let metadata = MovieMetadata {
            tmdb_id: 464927,
            imdb_id: Some("tt6955808".to_string()),
            original_title: "1級機密".to_string(),
            title: "一级机密".to_string(),
            original_language: "ko".to_string(),
            year: 2017,
            ..Default::default()
        };

        let folder = generate_movie_folder(&metadata, None);
        assert!(folder.contains("[Y]")); // Sort prefix: Y (from "一级机密" pinyin)
        assert!(folder.contains("[1級機密]"));
        assert!(folder.contains("[一级机密]"));
        assert!(folder.contains("(2017)"));
    }

    #[test]
    fn test_japanese_movie_with_chinese_title() {
        let metadata = MovieMetadata {
            tmdb_id: 452323,
            imdb_id: Some("tt6293042".to_string()),
            original_title: "アウトレイジ 最終章".to_string(),
            title: "极恶非道3".to_string(),
            original_language: "ja".to_string(),
            year: 2017,
            ..Default::default()
        };

        let folder = generate_movie_folder(&metadata, None);
        assert!(folder.contains("[J]")); // Sort prefix: J (from "极恶非道3" pinyin)
        assert!(folder.contains("[アウトレイジ 最終章]"));
        assert!(folder.contains("[极恶非道3]"));
        assert!(folder.contains("(2017)"));
    }

    #[test]
    fn test_english_movie_without_chinese_title() {
        let metadata = MovieMetadata {
            tmdb_id: 27205,
            imdb_id: Some("tt1375666".to_string()),
            original_title: "Inception".to_string(),
            title: "Inception".to_string(), // No Chinese title
            original_language: "en".to_string(),
            year: 2010,
            ..Default::default()
        };

        let folder = generate_movie_folder(&metadata, None);
        assert!(folder.contains("[I]")); // Sort prefix: I (from original title)
        assert!(folder.contains("[Inception]"));
        assert!(folder.contains("(2010)"));
    }

    #[test]
    fn test_english_movie_with_a_prefix() {
        let metadata = MovieMetadata {
            tmdb_id: 68721,
            imdb_id: Some("tt1280558".to_string()),
            original_title: "A Star Is Born".to_string(),
            title: "一个明星的诞生".to_string(),
            original_language: "en".to_string(),
            year: 2018,
            ..Default::default()
        };

        let folder = generate_movie_folder(&metadata, None);
        assert!(folder.contains("[Y]")); // Sort prefix: Y (from Chinese title)
        assert!(folder.contains("[一个明星的诞生]")); // Chinese title first
        assert!(folder.contains("[A Star Is Born]")); // Original title second
        assert!(folder.contains("(2018)"));
    }

    #[test]
    fn test_english_movie_with_an_prefix() {
        let metadata = MovieMetadata {
            tmdb_id: 536554,
            imdb_id: Some("tt8807684".to_string()),
            original_title: "An American Pickle".to_string(),
            title: "美国泡菜".to_string(),
            original_language: "en".to_string(),
            year: 2020,
            ..Default::default()
        };

        let folder = generate_movie_folder(&metadata, None);
        assert!(folder.contains("[M]")); // Sort prefix: M (from Chinese title)
        assert!(folder.contains("[美国泡菜]")); // Chinese title first
        assert!(folder.contains("[An American Pickle]")); // Original title second
        assert!(folder.contains("(2020)"));
    }

    #[test]
    fn test_english_movie_with_the_prefix_and_chinese_title() {
        let metadata = MovieMetadata {
            tmdb_id: 155,
            imdb_id: Some("tt0468569".to_string()),
            original_title: "The Dark Knight".to_string(),
            title: "黑暗骑士".to_string(),
            original_language: "en".to_string(),
            year: 2008,
            ..Default::default()
        };

        let folder = generate_movie_folder(&metadata, None);
        assert!(folder.contains("[H]")); // Sort prefix: H (from "黑暗骑士" pinyin)
        assert!(folder.contains("[黑暗骑士]")); // Chinese title first
        assert!(folder.contains("[The Dark Knight]")); // Original title second
        assert!(folder.contains("(2008)"));
    }

    #[test]
    fn test_chinese_characters_with_pinyin_issues() {
        // Test characters from the issue: 囡, 赤, 青
        let test_cases = vec![
            ("囡囡", 'N'),
            ("赤裸特工", 'C'),
            ("赤道", 'C'),
            ("青苔", 'Q'),
        ];
        
        for (title, expected_prefix) in test_cases {
            let metadata = MovieMetadata {
                tmdb_id: 12345,
                imdb_id: Some("tt1234567".to_string()),
                original_title: title.to_string(),
                title: title.to_string(),
                original_language: "zh".to_string(),
                year: 2020,
                ..Default::default()
            };
            
            let folder = generate_movie_folder(&metadata, None);
            let expected = format!("[{}]", expected_prefix);
            println!("Testing '{}': expected '{}', got '{}'", title, expected, folder);
            assert!(folder.contains(&expected), "Expected '{}' in '{}'", expected, folder);
        }
    }
    
    #[test]
    fn test_real_world_chinese_movie_titles() {
        let real_movie_cases = vec![
            // The movies mentioned in the issue
            ("囡囡", "N"),
            ("赤裸特工", "C"), 
            ("赤道", "C"),
            ("青苔", "Q"),
            // More real Chinese movies
            ("卧虎藏龙", "W"),
            ("英雄", "Y"),
            ("十面埋伏", "S"),
            ("功夫", "G"),
            ("霸王别姬", "B"),
            ("黑客帝国", "H"),
            ("阿凡达", "A"),
            ("泰坦尼克号", "T"),
            ("肖申克的救赎", "X"),
            ("阿甘正传", "A"),
            ("星际穿越", "X"),
            ("盗梦空间", "D"),
            ("无间道", "W"),
            ("让子弹飞", "R"),
            ("唐人街探案", "T"),
            ("你好，李焕英", "N"),
            ("长津湖", "Z"), // 注意："长"可能有两个拼音，库返回了 C 或 Z
            ("流浪地球", "L"),
            ("战狼", "Z"),
            ("哪吒之魔童降世", "N"),
            ("我不是药神", "W"),
            ("满江红", "M"),
        ];
        
        for (title, expected) in real_movie_cases {
            let metadata = MovieMetadata {
                tmdb_id: 10000,
                imdb_id: Some("tt1000000".to_string()),
                original_title: title.to_string(),
                title: title.to_string(),
                original_language: "zh".to_string(),
                year: 2020,
                ..Default::default()
            };
            
            let folder = generate_movie_folder(&metadata, None);
            let expected_prefix = format!("[{}]", expected);
            println!("Movie '{}': expected '{}', folder '{}'", title, expected_prefix, folder);
            assert!(folder.contains(&expected_prefix), 
                "Expected '{}' in '{}'", expected_prefix, folder);
        }
    }
    
    #[test]
    fn test_mixed_language_movies() {
        let mixed_cases = vec![
            // Chinese title, English original
            ("阿凡达", "Avatar", "en", 'A'),
            ("泰坦尼克号", "Titanic", "en", 'T'),
            ("黑客帝国", "The Matrix", "en", 'H'), // Should use Chinese title pinyin
            // English title, Chinese original (Chinese title comes first)
            ("卧虎藏龙", "Crouching Tiger, Hidden Dragon", "zh", 'W'),
        ];
        
        for (chinese_title, original_title, lang, expected) in mixed_cases {
            let metadata = MovieMetadata {
                tmdb_id: 10001,
                imdb_id: Some("tt1000001".to_string()),
                original_title: original_title.to_string(),
                title: chinese_title.to_string(),
                original_language: lang.to_string(),
                year: 2000,
                ..Default::default()
            };
            
            let folder = generate_movie_folder(&metadata, None);
            let expected_prefix = format!("[{}]", expected);
            println!("Testing mixed: '{}' / '{}' (lang {}) -> folder: {}", chinese_title, original_title, lang, folder);
            assert!(folder.contains(&expected_prefix), 
                "Expected '{}' in '{}' for mixed movie", expected_prefix, folder);
        }
    }

    #[test]
    fn test_generate_season_folder_basic() {
        // Test basic season folder generation with air_date
        // Format: [S04][Season 04]-[A][爱死亡与机器人][Love, Death & Robots](2019)-tt9561862-tmdb450504
        let folder = generate_season_folder(4, "Season 04", "A", "爱死亡与机器人", "Love, Death & Robots", Some("tt9561862"), 450504, Some("2019-03-15"));
        assert_eq!(folder, "[S04][Season 04]-[A][爱死亡与机器人][Love, Death & Robots](2019)-tt9561862-tmdb450504");
    }

    #[test]
    fn test_generate_season_folder_without_imdb() {
        // Test season folder without IMDB ID, with air_date
        let folder = generate_season_folder(1, "Season 1", "T", "The Terminal List", "The Terminal List", None, 118866, Some("2022-07-01"));
        assert_eq!(folder, "[S01][Season 01]-[T][The Terminal List](2022)-tmdb118866");
    }

    #[test]
    fn test_generate_season_folder_without_air_date() {
        // Test season folder without air_date
        let folder = generate_season_folder(1, "Season 1", "T", "The Terminal List", "The Terminal List", Some("tt123456"), 118866, None);
        assert_eq!(folder, "[S01][Season 01]-[T][The Terminal List]-tt123456-tmdb118866");
    }

    #[test]
    fn test_generate_season_folder_chinese_name() {
        // Test season folder with Chinese name and air_date - season name is always in English format
        let folder = generate_season_folder(2, "第 2 季", "M", "漫长的季节", "漫长的季节", Some("tt123456"), 191339, Some("2023-04-22"));
        assert_eq!(folder, "[S02][Season 02]-[M][漫长的季节](2023)-tt123456-tmdb191339");
    }

    #[test]
    fn test_generate_season_folder_special_chars() {
        // Test season folder - season name parameter is ignored, always uses "Season XX" format
        let folder = generate_season_folder(3, "Season: 3", "S", "Silicon Valley", "Silicon Valley", Some("tt2575988"), 288278, Some("2016-04-17"));
        assert_eq!(folder, "[S03][Season 03]-[S][Silicon Valley](2016)-tt2575988-tmdb288278");
    }

    #[test]
    fn test_generate_season_folder_different_seasons_same_show() {
        // Test that different seasons of the same show have different TMDB IDs and different years
        // This simulates the "Love, Death & Robots" case where each season has different TMDB season ID
        
        // Season 1 - 2019
        let s1 = generate_season_folder(1, "Volume 1", "A", "爱死亡与机器人", "Love, Death & Robots", Some("tt9561862"), 118866, Some("2019-03-15"));
        assert!(s1.contains("tmdb118866"));
        assert!(s1.contains("(2019)"));
        
        // Season 2 - 2021
        let s2 = generate_season_folder(2, "Volume 2", "A", "爱死亡与机器人", "Love, Death & Robots", Some("tt9561862"), 191339, Some("2021-05-19"));
        assert!(s2.contains("tmdb191339"));
        assert!(s2.contains("(2021)"));
        
        // Season 3 - 2022
        let s3 = generate_season_folder(3, "Volume 3", "A", "爱死亡与机器人", "Love, Death & Robots", Some("tt9561862"), 288278, Some("2022-10-28"));
        assert!(s3.contains("tmdb288278"));
        assert!(s3.contains("(2022)"));
        
        // Season 4 - 2024
        let s4 = generate_season_folder(4, "Volume 4", "A", "爱死亡与机器人", "Love, Death & Robots", Some("tt9561862"), 450504, Some("2024-05-08"));
        assert!(s4.contains("tmdb450504"));
        assert!(s4.contains("(2024)"));
        
        // Verify all TMDB IDs and years are different
        assert_ne!(s1, s2);
        assert_ne!(s2, s3);
        assert_ne!(s3, s4);
        assert_ne!(s1, s4);
    }
}
