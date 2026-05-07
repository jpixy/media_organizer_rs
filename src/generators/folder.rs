//! Folder name generator.

use crate::models::media::{MovieMetadata, TvSeriesMetadata};

/// Generate movie folder name.
///
/// Format: `[${originalTitle}]-[${title}](${edition})-${year}-${imdb}-${tmdb}`
pub fn generate_movie_folder(metadata: &MovieMetadata, edition: Option<&str>) -> String {
    let mut parts = Vec::new();

    // Handle title deduplication for Chinese movies
    let is_chinese = metadata.original_language == "zh";
    let titles_same = normalize_title(&metadata.original_title) == normalize_title(&metadata.title);

    if is_chinese || titles_same {
        // Only use one title for Chinese movies or when titles are the same
        parts.push(format!("[{}]", sanitize_filename(&metadata.title)));
    } else {
        // Use both original and localized title
        parts.push(format!("[{}]", sanitize_filename(&metadata.original_title)));
        parts.push(format!("[{}]", sanitize_filename(&metadata.title)));
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
/// Format: `[${showOriginalTitle}][${showTitle}](${year})-${showImdb}-${showTmdb}`
pub fn generate_tv_series_folder(metadata: &TvSeriesMetadata) -> String {
    let mut parts = Vec::new();

    // Handle title deduplication
    let is_chinese = metadata.original_language == "zh";
    let titles_same = normalize_title(&metadata.original_name) == normalize_title(&metadata.name);

    if is_chinese || titles_same {
        parts.push(format!("[{}]", sanitize_filename(&metadata.name)));
    } else {
        parts.push(format!("[{}]", sanitize_filename(&metadata.original_name)));
        parts.push(format!("[{}]", sanitize_filename(&metadata.name)));
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
/// Format: `S${seasonNr2}.${showYear}`
pub fn generate_season_folder(season_number: u16, year: u16) -> String {
    format!("S{:02}.{}", season_number, year)
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
        // Should only have one title
        assert_eq!(folder.matches("[霸王别姬]").count(), 1);
    }
}
