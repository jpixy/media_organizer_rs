//! Filename generator.

use crate::models::media::{EpisodeMetadata, MovieMetadata, TvSeriesMetadata, VideoMetadata};

/// ============================================================================
/// Sorting Prefix Generation
/// ============================================================================
/// 
/// Rule Priority (Highest to Lowest):
/// 1. If there is a Chinese localized name/title, use PINYIN FIRST LETTER of that name
/// 2. If no Chinese name:
///    - Chinese original language: PINYIN FIRST LETTER of original name
///    - English original language: FIRST LETTER after removing articles (The/A/An)
///    - Other languages: FIRST LETTER of original name
///
/// Example Format (Chinese localized title comes first):
/// [Z][一级机密][1급기밀](2017)-tt6955808-tmdb464927.mkv
/// [D][黑暗骑士][The Dark Knight](2008)-tt0468569-tmdb155.mkv
/// [Y][一级机密][1급기밀](2017)-tt6955808-tmdb464927.mkv
/// [J][极恶非道3][アウトレイジ 最終章](2017)-tt6293042-tmdb452323.mkv
/// ============================================================================

/// Generate sorting prefix character.
/// 
/// Rule Priority (Highest to Lowest):
/// 1. If there is a Chinese localized name/title, use PINYIN FIRST LETTER of that name
/// 2. If no Chinese name:
///    - Chinese original language: PINYIN FIRST LETTER of original name
///    - English original language: FIRST LETTER after removing articles (The/A/An)
///    - Other languages: FIRST LETTER of original name
///
/// Example Format:
/// [Z][追龍](2017)-tt6015328-tmdb426242.mkv
/// [D][The Dark Knight][黑暗骑士](2008)-tt0468569-tmdb155.mkv
/// [Y][1급기밀][一级机密](2017)-tt6955808-tmdb464927.mkv
/// [J][アウトレイジ 最終章][极恶非道3](2017)-tt6293042-tmdb452323.mkv
fn generate_sort_prefix(
    has_chinese_name: bool,
    chinese_name: &str,
    original_language: &str,
    original_name: &str,
) -> char {
    // Rule 1: Highest priority - use Chinese name pinyin if available
    if has_chinese_name {
        if let Some(first_char) = chinese_name.chars().next() {
            // Check if it's a CJK character
            if ('\u{4E00}'..='\u{9FFF}').contains(&first_char) {
                // It's a Chinese character, try to get pinyin
                use pinyin::ToPinyin;
                if let Some(pinyin) = first_char.to_pinyin() {
                    let pinyin_str = pinyin.plain();
                    if let Some(first_pinyin_char) = pinyin_str.chars().next() {
                        return first_pinyin_char.to_ascii_uppercase();
                    }
                }
            }
        }
        // Fallback to first character if pinyin fails
        return chinese_name.chars().next().unwrap_or('?').to_ascii_uppercase();
    }

    // Rule 2: No Chinese name, decide by original language
    match original_language {
        // Chinese original language: use pinyin of original name
        "zh" => {
            if let Some(first_char) = original_name.chars().next() {
                // Check if it's a CJK character
                if ('\u{4E00}'..='\u{9FFF}').contains(&first_char) {
                    // It's a Chinese character, try to get pinyin
                    use pinyin::ToPinyin;
                    if let Some(pinyin) = first_char.to_pinyin() {
                        let pinyin_str = pinyin.plain();
                        if let Some(first_pinyin_char) = pinyin_str.chars().next() {
                            return first_pinyin_char.to_ascii_uppercase();
                        }
                    }
                }
            }
            original_name.chars().next().unwrap_or('?').to_ascii_uppercase()
        }
        // English: remove articles first
        "en" => {
            let title_lower = original_name.to_lowercase();
            let effective_title = if title_lower.starts_with("the ") {
                &original_name[4..]
            } else if title_lower.starts_with("a ") {
                &original_name[2..]
            } else if title_lower.starts_with("an ") {
                &original_name[3..]
            } else {
                original_name
            };
            effective_title.chars().next().unwrap_or('?').to_ascii_uppercase()
        }
        // Other languages: use first character directly
        _ => original_name.chars().next().unwrap_or('?').to_ascii_uppercase(),
    }
}

/// Add a part to the filename only if it's not "Unknown" or empty.
/// Returns the part with the separator if valid, or empty string if skipped.
fn add_part_if_valid(parts: &mut Vec<String>, value: &str, separator: &str) {
    if !value.is_empty() && value != "Unknown" {
        parts.push(format!("{}{}", separator, value));
    }
}

/// Format resolution string with actual dimensions.
///
/// If width and height are available, returns format like "1920x1080(1080p)".
/// Otherwise, returns just the resolution category like "1080p".
fn format_resolution(video: &VideoMetadata) -> String {
    if video.width > 0 && video.height > 0 {
        format!("{}x{}({})", video.width, video.height, video.resolution)
    } else {
        video.resolution.clone()
    }
}

/// Extract disc/part identifier from filename.
///
/// Detects patterns like: cd1, cd2, disc1, disc2, part1, part2, dvd1, dvd2, etc.
/// Returns the identifier in lowercase format (e.g., "cd1", "part2").
pub fn extract_disc_identifier(filename: &str) -> Option<String> {
    let filename_lower = filename.to_lowercase();

    // Patterns to match: cd1, cd2, disc1, disc2, part1, part2, dvd1, dvd2
    let patterns = [
        r"[_\s\-\.](cd)(\d+)",
        r"[_\s\-\.](disc)(\d+)",
        r"[_\s\-\.](part)(\d+)",
        r"[_\s\-\.](dvd)(\d+)",
        r"[_\s\-\.](disk)(\d+)",
    ];

    for pattern in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(&filename_lower) {
                if let (Some(prefix), Some(num)) = (caps.get(1), caps.get(2)) {
                    return Some(format!("{}{}", prefix.as_str(), num.as_str()));
                }
            }
        }
    }

    // Also try without separator at the end of filename (before extension)
    let patterns_end = [
        r"(cd)(\d+)\.[a-z0-9]+$",
        r"(disc)(\d+)\.[a-z0-9]+$",
        r"(part)(\d+)\.[a-z0-9]+$",
        r"(dvd)(\d+)\.[a-z0-9]+$",
        r"(disk)(\d+)\.[a-z0-9]+$",
    ];

    for pattern in &patterns_end {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(&filename_lower) {
                if let (Some(prefix), Some(num)) = (caps.get(1), caps.get(2)) {
                    return Some(format!("{}{}", prefix.as_str(), num.as_str()));
                }
            }
        }
    }

    None
}

/// Generate movie filename.
///
/// New Format: `[${sortPrefix}][${title}][${originalTitle}](${edition})-${year}-${resolution}-${format}-${codec}-${bitDepth}bit-${audioCodec}-${audioChannels}(-${discId})`
/// Sort Prefix Generation Rules: See `generate_sort_prefix` documentation
pub fn generate_movie_filename(
    movie: &MovieMetadata,
    video: &VideoMetadata,
    edition: Option<&str>,
    extension: &str,
) -> String {
    generate_movie_filename_with_disc(movie, video, edition, None, extension)
}

/// Generate movie filename with optional disc identifier.
///
/// New Format: `[${sortPrefix}][${title}][${originalTitle}](${edition})-${year}-${resolution}-${format}-${codec}-${bitDepth}bit-${audioCodec}-${audioChannels}(-${discId})`
/// Sort Prefix Generation Rules: See `generate_sort_prefix` documentation
pub fn generate_movie_filename_with_disc(
    movie: &MovieMetadata,
    video: &VideoMetadata,
    edition: Option<&str>,
    disc_id: Option<&str>,
    extension: &str,
) -> String {
    let mut parts = Vec::new();

    // Add sorting prefix
    let has_chinese = movie.original_language == "zh" 
        || normalize_title(&movie.title) != normalize_title(&movie.original_title);
    let sort_prefix = generate_sort_prefix(
        has_chinese,
        &movie.title,
        &movie.original_language,
        &movie.original_title,
    );
    parts.push(format!("[{}]", sort_prefix));

    // Handle title deduplication for Chinese movies
    let is_chinese = movie.original_language == "zh";
    let titles_same = normalize_title(&movie.original_title) == normalize_title(&movie.title);

    if is_chinese || titles_same {
        parts.push(format!("[{}]", sanitize_filename(&movie.title)));
    } else {
        // Use both localized and original title (localized first)
        parts.push(format!("[{}]", sanitize_filename(&movie.title)));
        parts.push(format!("[{}]", sanitize_filename(&movie.original_title)));
    }

    // Add edition if present, then year
    // When edition exists: [Title](edition)-(year)
    // When no edition: [Title](year)
    if let Some(ed) = edition {
        parts.push(format!("({})", ed));
        parts.push(format!("-({})", movie.year));
    } else {
        parts.push(format!("({})", movie.year));
    }

    // Add video info with actual resolution (skip if Unknown)
    parts.push(format!("-{}", format_resolution(video)));
    add_part_if_valid(&mut parts, &video.format, "-");
    add_part_if_valid(&mut parts, &video.video_codec, "-");
    if video.bit_depth > 0 {
        parts.push(format!("-{}bit", video.bit_depth));
    }
    add_part_if_valid(&mut parts, &video.audio_codec, "-");
    add_part_if_valid(&mut parts, &video.audio_channels, "-");

    // Add disc identifier if present (for multi-disc movies)
    if let Some(disc) = disc_id {
        parts.push(format!("-{}", disc));
    }

    format!("{}.{}", parts.join(""), extension)
}

/// Generate TV episode filename.
///
/// New Format: `S${seasonNr2}E${episodeNr2}-[${title}]-[${sortPrefix}][${showTitle}][${showOriginalTitle}]-${format}-${codec}-${bitDepth}bit-${audioCodec}-${audioChannels}`
/// Sort Prefix Generation Rules: See `generate_sort_prefix` documentation
pub fn generate_episode_filename(
    show: &TvSeriesMetadata,
    episode: &EpisodeMetadata,
    video: &VideoMetadata,
    extension: &str,
) -> String {
    let mut parts = Vec::new();

    // Season and episode number FIRST: S04E02
    parts.push(format!(
        "S{:02}E{:02}",
        episode.season_number, episode.episode_number
    ));

    // Episode title: -[与微型物的近距离接触]
    parts.push(format!("-[{}]", sanitize_filename(&episode.name)));

    // Show title part: -[A][爱死亡与机器人][Love, Death & Robots]
    let has_chinese = show.original_language == "zh" 
        || normalize_title(&show.name) != normalize_title(&show.original_name);
    let sort_prefix = generate_sort_prefix(
        has_chinese,
        &show.name,
        &show.original_language,
        &show.original_name,
    );
    
    let mut title_part = format!("[{}]", sort_prefix);
    
    let is_chinese = show.original_language == "zh";
    let titles_same = normalize_title(&show.original_name) == normalize_title(&show.name);

    if is_chinese || titles_same {
        title_part.push_str(&format!("[{}]", sanitize_filename(&show.name)));
    } else {
        // Use both localized and original title (localized first)
        title_part.push_str(&format!("[{}][{}]", sanitize_filename(&show.name), sanitize_filename(&show.original_name)));
    }
    
    parts.push(format!("-{}", title_part));

    // Video info with actual resolution (skip if Unknown)
    parts.push(format!("-{}", format_resolution(video)));
    add_part_if_valid(&mut parts, &video.format, "-");
    add_part_if_valid(&mut parts, &video.video_codec, "-");
    if video.bit_depth > 0 {
        parts.push(format!("-{}bit", video.bit_depth));
    }
    add_part_if_valid(&mut parts, &video.audio_codec, "-");
    add_part_if_valid(&mut parts, &video.audio_channels, "-");

    format!("{}.{}", parts.join(""), extension)
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

/// Normalize title for comparison.
fn normalize_title(s: &str) -> String {
    s.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_movie_filename() {
        let movie = MovieMetadata {
            original_title: "Avatar".to_string(),
            title: "阿凡达".to_string(),
            original_language: "en".to_string(),
            year: 2009,
            ..Default::default()
        };

        let video = VideoMetadata {
            width: 3840,
            height: 2160,
            resolution: "2160p".to_string(),
            format: "BluRay".to_string(),
            video_codec: "x265".to_string(),
            bit_depth: 10,
            audio_codec: "TrueHD".to_string(),
            audio_channels: "7.1".to_string(),
        };

        let filename = generate_movie_filename(&movie, &video, None, "mkv");
        assert!(filename.contains("[A]")); // Sort prefix
        assert!(filename.contains("[阿凡达]")); // Chinese title first
        assert!(filename.contains("[Avatar]")); // Original title second
        assert!(filename.contains("3840x2160(2160p)"));
        assert!(filename.contains("10bit"));
        assert!(filename.ends_with(".mkv"));
    }

    #[test]
    fn test_extract_disc_identifier() {
        // Various disc identifier patterns
        assert_eq!(
            extract_disc_identifier("movie-cd1.avi"),
            Some("cd1".to_string())
        );
        assert_eq!(
            extract_disc_identifier("movie-cd2.avi"),
            Some("cd2".to_string())
        );
        assert_eq!(
            extract_disc_identifier("movie_part1.mkv"),
            Some("part1".to_string())
        );
        assert_eq!(
            extract_disc_identifier("movie part2.mkv"),
            Some("part2".to_string())
        );
        assert_eq!(
            extract_disc_identifier("movie.disc1.avi"),
            Some("disc1".to_string())
        );
        assert_eq!(
            extract_disc_identifier("movie-dvd1.mkv"),
            Some("dvd1".to_string())
        );
        assert_eq!(
            extract_disc_identifier("2007-[太阳照常升起]-672x288-480p-XVID-8bit-AC3-6ch cd1.avi"),
            Some("cd1".to_string())
        );
        assert_eq!(
            extract_disc_identifier("2007-[太阳照常升起]-672x288-480p-XVID-8bit-AC3-6ch cd2.avi"),
            Some("cd2".to_string())
        );

        // No disc identifier
        assert_eq!(extract_disc_identifier("movie.mkv"), None);
        assert_eq!(extract_disc_identifier("movie-2024.avi"), None);
    }

    #[test]
    fn test_generate_movie_filename_with_disc() {
        let movie = MovieMetadata {
            original_title: "太阳照常升起".to_string(),
            title: "太阳照常升起".to_string(),
            original_language: "zh".to_string(),
            year: 2007,
            ..Default::default()
        };

        let video = VideoMetadata {
            width: 672,
            height: 288,
            resolution: "288p".to_string(),
            format: "DVDRip".to_string(),
            video_codec: "xvid".to_string(),
            bit_depth: 8,
            audio_codec: "ac3".to_string(),
            audio_channels: "5.1".to_string(),
        };

        // Without disc id
        let filename1 = generate_movie_filename(&movie, &video, None, "avi");
        assert!(filename1.contains("[T]")); // Sort prefix
        assert!(!filename1.contains("-cd"));

        // With disc id
        let filename2 =
            generate_movie_filename_with_disc(&movie, &video, None, Some("cd1"), "avi");
        assert!(filename2.contains("-cd1.avi"));

        let filename3 =
            generate_movie_filename_with_disc(&movie, &video, None, Some("cd2"), "avi");
        assert!(filename3.contains("-cd2.avi"));

        // Ensure different filenames for different discs
        assert_ne!(filename2, filename3);
    }

    #[test]
    fn test_generate_episode_filename_new_format() {
        // Test new episode filename format: S04E02-[与微型物的近距离接触]-[A][爱死亡与机器人][Love, Death & Robots]-1920x1080(1080p)-h264-8bit-eac3-5.1.mkv
        let show = TvSeriesMetadata {
            name: "爱死亡与机器人".to_string(),
            original_name: "Love, Death & Robots".to_string(),
            original_language: "en".to_string(),
            year: 2019,
            imdb_id: Some("tt9561862".to_string()),
            tmdb_id: 86831,
            ..Default::default()
        };

        let episode = EpisodeMetadata {
            season_number: 4,
            episode_number: 2,
            name: "与微型物的近距离接触".to_string(),
            original_name: Some("Jibaro".to_string()),
            ..Default::default()
        };

        let video = VideoMetadata {
            width: 1920,
            height: 1080,
            resolution: "1080p".to_string(),
            format: "BluRay".to_string(),
            video_codec: "h264".to_string(),
            bit_depth: 8,
            audio_codec: "eac3".to_string(),
            audio_channels: "5.1".to_string(),
        };

        let filename = generate_episode_filename(&show, &episode, &video, "mkv");
        
        // Verify format: S04E02-[title]-[A][爱死亡与机器人][Love, Death & Robots]-resolution-format-codec-bitdepth-audio.mkv
        assert!(filename.starts_with("S04E02-"), "Should start with S04E02-, got: {}", filename);
        assert!(filename.contains("-[与微型物的近距离接触]-"), "Should contain episode title, got: {}", filename);
        assert!(filename.contains("[A]"), "Should contain sort prefix, got: {}", filename);
        assert!(filename.contains("[爱死亡与机器人]"), "Should contain Chinese name, got: {}", filename);
        assert!(filename.contains("[Love, Death & Robots]"), "Should contain original name, got: {}", filename);
        assert!(filename.contains("1920x1080(1080p)"), "Should contain resolution, got: {}", filename);
        assert!(filename.contains("-h264-"), "Should contain codec, got: {}", filename);
        assert!(filename.ends_with(".mkv"), "Should end with .mkv, got: {}", filename);
    }

    #[test]
    fn test_generate_episode_filename_english_only() {
        // Test episode with English only title (same show and episode name)
        let show = TvSeriesMetadata {
            name: "The Terminal List".to_string(),
            original_name: "The Terminal List".to_string(),
            original_language: "en".to_string(),
            year: 2022,
            imdb_id: Some("tt11743610".to_string()),
            tmdb_id: 120911,
            ..Default::default()
        };

        let episode = EpisodeMetadata {
            season_number: 1,
            episode_number: 1,
            name: "Order".to_string(),
            original_name: Some("Order".to_string()),
            ..Default::default()
        };

        let video = VideoMetadata {
            width: 1920,
            height: 1080,
            resolution: "1080p".to_string(),
            format: "WEB-DL".to_string(),
            video_codec: "h265".to_string(),
            bit_depth: 10,
            audio_codec: "aac".to_string(),
            audio_channels: "2.0".to_string(),
        };

        let filename = generate_episode_filename(&show, &episode, &video, "mp4");
        
        // For English only, should NOT duplicate the same name
        assert!(filename.starts_with("S01E01-"), "Should start with S01E01-, got: {}", filename);
        assert!(filename.contains("-[Order]-"), "Should contain episode title, got: {}", filename);
        assert!(filename.contains("[T]"), "Should contain sort prefix T, got: {}", filename);
        assert!(filename.contains("[The Terminal List]"), "Should contain show name, got: {}", filename);
        assert!(filename.ends_with(".mp4"), "Should end with .mp4, got: {}", filename);
    }
}
