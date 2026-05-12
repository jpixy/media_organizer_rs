//! Filename generator.

use crate::models::media::{EpisodeMetadata, MovieMetadata, TvSeriesMetadata, VideoMetadata};

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
/// Format: `[${originalTitle}]-[${title}](${edition})-${year}-${resolution}-${format}-${codec}-${bitDepth}bit-${audioCodec}-${audioChannels}(-${discId})`
///
/// The optional `disc_id` parameter is used for multi-disc movies (cd1, cd2, part1, part2, etc.)
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
/// Format: `[${originalTitle}]-[${title}](${edition})-${year}-${resolution}-${format}-${codec}-${bitDepth}bit-${audioCodec}-${audioChannels}(-${discId})`
pub fn generate_movie_filename_with_disc(
    movie: &MovieMetadata,
    video: &VideoMetadata,
    edition: Option<&str>,
    disc_id: Option<&str>,
    extension: &str,
) -> String {
    let mut parts = Vec::new();

    // Handle title deduplication for Chinese movies
    let is_chinese = movie.original_language == "zh";
    let titles_same = normalize_title(&movie.original_title) == normalize_title(&movie.title);

    if is_chinese || titles_same {
        parts.push(format!("[{}]", sanitize_filename(&movie.title)));
    } else {
        parts.push(format!("[{}]", sanitize_filename(&movie.original_title)));
        parts.push(format!("[{}]", sanitize_filename(&movie.title)));
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
/// Format: `[${showOriginalTitle}]-S${seasonNr2}E${episodeNr2}-[${originalTitle}]-[${title}]-${format}-${codec}-${bitDepth}bit-${audioCodec}-${audioChannels}`
pub fn generate_episode_filename(
    show: &TvSeriesMetadata,
    episode: &EpisodeMetadata,
    video: &VideoMetadata,
    extension: &str,
) -> String {
    let mut parts = Vec::new();

    // Show title
    let is_chinese = show.original_language == "zh";
    let titles_same = normalize_title(&show.original_name) == normalize_title(&show.name);

    if is_chinese || titles_same {
        parts.push(format!("[{}]", sanitize_filename(&show.name)));
    } else {
        parts.push(format!("[{}]", sanitize_filename(&show.original_name)));
    }

    // Season and episode number
    parts.push(format!(
        "-S{:02}E{:02}",
        episode.season_number, episode.episode_number
    ));

    // Episode title
    if let Some(ref orig_name) = episode.original_name {
        if orig_name != &episode.name {
            parts.push(format!("-[{}]", sanitize_filename(orig_name)));
        }
    }
    parts.push(format!("-[{}]", sanitize_filename(&episode.name)));

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
        assert!(filename.contains("[Avatar]"));
        assert!(filename.contains("[阿凡达]"));
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
}
