
//! GuessIt parser module - calls Python's guessit via subprocess
//!
//! This module provides filename parsing by invoking the Python guessit library
//! through subprocess calls. GuessIt is superior to hunch for CJK filename parsing.
//!
//! # Architecture
//!
//! - Uses `std::process::Command` to invoke Python with guessit
//! - Each call spawns a single Python process for simplicity and isolation
//! - JSON is used for data exchange between Rust and Python
//!
//! # Performance Considerations
//!
//! - Process spawn overhead: ~50-100ms per parse
//! - For batch operations, consider using the batch method which minimizes overhead
//! - The parser is designed to be called concurrently from multiple async tasks

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde::de::{self, Visitor};
use std::fmt;
use std::result::Result as StdResult;

fn deserialize_optional_string_or_vec<'de, D>(deserializer: D) -> StdResult<Option<Vec<String>>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct OptionalStringOrVecVisitor;

    impl<'de> Visitor<'de> for OptionalStringOrVecVisitor {
        type Value = Option<Vec<String>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or an array of strings")
        }

        fn visit_none<E>(self) -> StdResult<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> StdResult<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_str<E>(self, value: &str) -> StdResult<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(vec![value.to_string()]))
        }

        fn visit_seq<A>(self, mut seq: A) -> StdResult<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut result = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                result.push(s);
            }
            Ok(Some(result))
        }
    }

    deserializer.deserialize_any(OptionalStringOrVecVisitor)
}
use std::process::Command;
use tracing::{debug, error, info, warn};

/// Clean known download site prefixes from filenames before parsing.
/// This prevents guessit from incorrectly including these prefixes in the title.
pub fn clean_filename(filename: &str) -> String {
    let prefixes = [
        "阳光电影dygod.org.",
        "阳光电影www.dygod.org.",
        "电影天堂dygod.org.",
        "www.dygod.org.",
        "dygod.org.",
        "阳光电影www.verycd.org.",
        "电影天堂www.verycd.org.",
    ];

    let mut cleaned = filename.to_string();
    for prefix in &prefixes {
        if cleaned.starts_with(prefix) {
            cleaned = cleaned[prefix.len()..].to_string();
            info!("[GuessIt] Cleaned prefix '{}': {}", prefix, cleaned);
            break;
        }
    }
    cleaned
}

/// Parsed filename information from guessit.
/// Maps guessit's output to our internal format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuessItResult {
    /// Title (may be in original language).
    pub title: Option<String>,
    /// Alternative/secondary title(s) (e.g., country edition).
    /// Can be a string or array of strings.
    #[serde(deserialize_with = "deserialize_optional_string_or_vec")]
    pub alternative_title: Option<Vec<String>>,
    /// Year of release.
    pub year: Option<u16>,
    /// Season number (TV shows).
    pub season: Option<u16>,
    /// Episode number (TV shows).
    pub episode: Option<u16>,
    /// Episode title (if available).
    pub episode_title: Option<String>,
    /// Media type: "movie" or "episode".
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    /// Screen size (e.g., "1080p").
    pub screen_size: Option<String>,
    /// Video source (e.g., "Blu-ray", "Web").
    pub source: Option<String>,
    /// Video codec (e.g., "H.264", "H.265").
    pub video_codec: Option<String>,
    /// Release group (e.g., "DEMAND", "YIFY").
    pub release_group: Option<String>,
    /// Container format (e.g., "mkv", "mp4").
    pub container: Option<String>,
    /// Audio codec (e.g., "DTS", "AAC"). Can be a single string or multiple codecs as array.
    #[serde(deserialize_with = "deserialize_optional_string_or_vec")]
    pub audio_codec: Option<Vec<String>>,
    /// Audio channels (e.g., "5.1").
    pub audio_channels: Option<String>,
    /// Edition (e.g., "Director's Cut", "Extended").
    pub edition: Option<String>,
    /// Absolute episode number (for anime).
    pub absolute_episode: Option<u16>,
    /// Disk number (for multi-disk media).
    pub disk_number: Option<u16>,
    /// Part number (for multi-part releases).
    pub part: Option<u16>,
    /// Whether this is a proper release.
    pub proper_count: Option<u16>,
    /// Whether this is a repack.
    pub repack_count: Option<u16>,
    /// Streaming service (e.g., "Netflix", "Hulu").
    pub streaming_service: Option<String>,
    /// Confidence score if available.
    #[serde(default)]
    pub confidence: f32,
}

impl GuessItResult {
    /// Check if this is a movie.
    pub fn is_movie(&self) -> bool {
        self.media_type.as_deref() == Some("movie")
    }

    /// Check if this is an episode (TV show).
    pub fn is_episode(&self) -> bool {
        self.media_type.as_deref() == Some("episode")
    }

    /// Get primary title, preferring the main title over alternative.
    /// Returns the first alternative title if no primary title exists.
    pub fn primary_title(&self) -> Option<String> {
        if let Some(ref title) = self.title {
            return Some(title.clone());
        }
        if let Some(ref alt_titles) = self.alternative_title {
            if let Some(first) = alt_titles.first() {
                return Some(first.clone());
            }
        }
        None
    }

    /// Extract English title from mixed Chinese-English title.
    /// 
    /// GuessIt may extract only the Chinese part when parsing filenames like:
    /// "首都坠落.DC Down.(2023)"
    /// 
    /// This method attempts to extract the English portion using regex.
    /// Returns None if no English words are found or if the title is purely Chinese.
    pub fn extract_english_title(&self) -> Option<String> {
        if let Some(ref title) = self.title {
            // Use regex to extract English words
            use regex::Regex;
            let re = Regex::new(r"[A-Za-z][A-Za-z0-9\s]*").ok()?;
            let matches: Vec<&str> = re.find_iter(title)
                .map(|m| m.as_str())
                .filter(|s| !s.is_empty())
                .collect();
            
            if !matches.is_empty() {
                let english_title = matches.join(" ");
                // Only return if it looks like a meaningful English title (more than 1 char)
                if english_title.len() > 1 {
                    return Some(english_title);
                }
            }
        }
        None
    }

    /// Try to extract English title from filename when guessit fails to do so.
    /// This is useful for non-standard naming formats like "中文名.英文名.(年份)"
    pub fn extract_english_from_filename(filename: &str) -> Option<String> {
        use regex::Regex;
        let re = Regex::new(r"[A-Za-z][A-Za-z0-9\s]*").ok()?;
        
        let matches: Vec<&str> = re.find_iter(filename)
            .map(|m| m.as_str())
            .filter(|s| !s.is_empty())
            .collect();
        
        if !matches.is_empty() {
            let english = matches.join(" ");
            if english.len() > 1 {
                return Some(english);
            }
        }
        None
    }

    /// Get a confidence indicator based on which fields are populated.
    pub fn completeness_score(&self) -> f32 {
        let mut score = 0.0;
        let mut count = 0.0;

        if self.title.is_some() {
            score += 0.4;
        }
        count += 0.4;

        if self.year.is_some() {
            score += 0.1;
        }
        count += 0.1;

        if self.season.is_some() {
            score += 0.15;
        }
        count += 0.15;

        if self.episode.is_some() {
            score += 0.15;
        }
        count += 0.15;

        if self.media_type.is_some() {
            score += 0.2;
        }
        count += 0.2;

        if count > 0.0 {
            score / count
        } else {
            0.0
        }
    }

    /// Validate and normalize the result.
    /// Returns None if the result is invalid (e.g., no title).
    pub fn validate(&mut self) {
        // Validate year range (1900 - current year + 5)
        if let Some(year) = self.year {
            let current_year = 2026;
            if year < 1900 || year > current_year + 5 {
                warn!("[GuessIt] Invalid year {}, ignoring", year);
                self.year = None;
            }
        }

        // Validate season/episode numbers
        if let Some(season) = self.season {
            if season == 0 || season > 100 {
                warn!("[GuessIt] Invalid season {}, ignoring", season);
                self.season = None;
            }
        }

        if let Some(episode) = self.episode {
            if episode == 0 || episode > 10000 {
                warn!("[GuessIt] Invalid episode {}, ignoring", episode);
                self.episode = None;
            }
        }

        // Validate titles are not empty
        if let Some(ref title) = self.title {
            if title.trim().is_empty() {
                self.title = None;
            }
        }

        // Calculate confidence based on result completeness
        if self.confidence == 0.0 {
            self.confidence = self.completeness_score();
        }

        // If still no title, set very low confidence
        if self.title.is_none() {
            self.confidence = 0.0;
        }
    }
}

/// GuessIt parser service.
/// Calls Python's guessit library via subprocess.
pub struct GuessItParser {
    python_path: String,
    timeout_secs: u64,
}

impl GuessItParser {
    /// Create a new GuessItParser with default settings.
    pub fn new() -> Self {
        Self {
            python_path: "python3".to_string(),
            timeout_secs: 30,
        }
    }

    /// Create a new parser with custom Python path.
    pub fn with_python(python_path: impl Into<String>) -> Self {
        Self {
            python_path: python_path.into(),
            timeout_secs: 30,
        }
    }

    /// Create a new parser with custom timeout.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Check if guessit is available and working.
    pub fn health_check(&self) -> Result<bool> {
        info!("[GuessIt] Running health check...");

        let test_code = r#"
import sys
import guessit
print('OK')
sys.exit(0)
"#;

        let output = Command::new(&self.python_path)
            .args(["-c", test_code])
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim() == "OK" {
                info!("[GuessIt] Health check passed");
                return Ok(true);
            }
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("[GuessIt] Health check failed: {}", stderr);
        Err(Error::Parser(format!("Health check failed: {}", stderr)))
    }

    /// Parse a single filename.
    pub fn parse(&self, filename: &str) -> Result<GuessItResult> {
        self.parse_with_type(filename, None)
    }

    /// Parse a filename with an expected type hint.
    /// This helps GuessIt disambiguate between movie and episode patterns.
    ///
    /// `expected_type` can be "movie", "episode", "tv", or None for auto-detection.
    pub fn parse_with_type(
        &self,
        filename: &str,
        expected_type: Option<&str>,
    ) -> Result<GuessItResult> {
        let cleaned_filename = clean_filename(filename);
        debug!("[GuessIt] Parsing: {} (expected_type={:?})", cleaned_filename, expected_type);

        let start = std::time::Instant::now();

        // Escape single quotes in filename for shell safety
        let escaped_filename = cleaned_filename.replace("'", "'\"'\"'");

        let type_hint = match expected_type {
            Some(t) => format!(r#", options={{"type": "{}"}}"#, t),
            None => String::new(),
        };

        let python_code = format!(
            r#"
import sys
import json
import guessit

try:
    result = guessit.guessit('{}'{})
    # Convert to JSON with proper type handling
    output = {{
        'title': result.get('title'),
        'alternative_title': result.get('alternative_title'),
        'year': result.get('year'),
        'season': result.get('season'),
        'episode': result.get('episode'),
        'episode_title': result.get('episode_title'),
        'type': result.get('type'),
        'screen_size': result.get('screen_size'),
        'source': result.get('source'),
        'video_codec': result.get('video_codec'),
        'release_group': result.get('release_group'),
        'container': result.get('container'),
        'audio_codec': result.get('audio_codec'),
        'audio_channels': result.get('audio_channels'),
        'edition': result.get('edition'),
        'absolute_episode': result.get('absolute_episode'),
        'disk_number': result.get('disk_number'),
        'part': result.get('part'),
        'proper_count': result.get('proper_count'),
        'repack_count': result.get('repack_count'),
        'streaming_service': result.get('streaming_service'),
        'confidence': 0.0
    }}
    print(json.dumps(output, ensure_ascii=False))
    sys.exit(0)
except Exception as e:
    print(json.dumps({{'error': str(e)}}), file=sys.stderr)
    sys.exit(1)
"#,
            escaped_filename, type_hint
        );

        let output = Command::new(&self.python_path)
            .args(["-c", &python_code])
            .output()
            .map_err(|e| Error::Parser(format!("Failed to execute Python: {}", e)))?;

        let elapsed = start.elapsed();

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!(
                "[GuessIt] Parse failed for '{}' in {:.2}s: {}",
                filename, elapsed.as_secs_f32(), stderr
            );
            return Err(Error::Parser(format!(
                "GuessIt parse failed: {} (took {:.2}s)",
                stderr,
                elapsed.as_secs_f32()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Check for error in JSON output
        if stdout.contains("\"error\"") {
            error!(
                "[GuessIt] Parse error for '{}': {}",
                filename, stdout
            );
            return Err(Error::Parser(format!("GuessIt error: {}", stdout)));
        }

        let mut result: GuessItResult = serde_json::from_str(&stdout).map_err(|e| {
            error!(
                "[GuessIt] Failed to parse JSON for '{}': {}",
                filename, e
            );
            Error::Parser(format!(
                "Failed to parse guessit output: {} (output: {})",
                e, stdout
            ))
        })?;

        result.validate();

        debug!(
            "[GuessIt] Parsed '{}' in {:.2}s: title={:?}, year={:?}, season={:?}, episode={:?}, type={:?}",
            filename,
            elapsed.as_secs_f32(),
            result.title,
            result.year,
            result.season,
            result.episode,
            result.media_type
        );

        Ok(result)
    }

    /// Parse multiple filenames in a single Python process.
    /// This is more efficient than calling parse() multiple times.
    pub fn parse_batch(&self, filenames: &[&str]) -> Result<Vec<GuessItResult>> {
        if filenames.is_empty() {
            return Ok(Vec::new());
        }

        info!("[GuessIt] Batch parsing {} filenames...", filenames.len());

        let start = std::time::Instant::now();

        // Build Python code to parse multiple files
        let cleaned_filenames: Vec<String> = filenames
            .iter()
            .map(|f| clean_filename(f))
            .collect();
        
        let escaped_filenames: Vec<String> = cleaned_filenames
            .iter()
            .map(|f| f.replace("'", "'\"'\"'"))
            .collect();

        let filenames_json = serde_json::to_string(&escaped_filenames)
            .map_err(|e| Error::Parser(format!("Failed to serialize filenames: {}", e)))?;

        let python_code = format!(
            r#"
import sys
import json
import guessit

filenames = json.loads('{}')

results = []
for filename in filenames:
    try:
        result = guessit.guessit(filename)
        parsed = {{
            'title': result.get('title'),
            'alternative_title': result.get('alternative_title'),
            'year': result.get('year'),
            'season': result.get('season'),
            'episode': result.get('episode'),
            'episode_title': result.get('episode_title'),
            'type': result.get('type'),
            'screen_size': result.get('screen_size'),
            'source': result.get('source'),
            'video_codec': result.get('video_codec'),
            'release_group': result.get('release_group'),
            'container': result.get('container'),
            'audio_codec': result.get('audio_codec'),
            'audio_channels': result.get('audio_channels'),
            'edition': result.get('edition'),
            'absolute_episode': result.get('absolute_episode'),
            'disk_number': result.get('disk_number'),
            'part': result.get('part'),
            'proper_count': result.get('proper_count'),
            'repack_count': result.get('repack_count'),
            'streaming_service': result.get('streaming_service'),
            'confidence': 0.0,
            '_filename': filename
        }}
        results.append(parsed)
    except Exception as e:
        results.append({{'error': str(e), '_filename': filename}})

print(json.dumps(results, ensure_ascii=False))
"#,
            filenames_json
        );

        let output = Command::new(&self.python_path)
            .args(["-c", &python_code])
            .output()
            .map_err(|e| Error::Parser(format!("Failed to execute Python: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("[GuessIt] Batch parse failed: {}", stderr);
            return Err(Error::Parser(format!("Batch parse failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        let raw_results: Vec<serde_json::Value> = serde_json::from_str(&stdout)
            .map_err(|e| Error::Parser(format!("Failed to parse batch results: {}", e)))?;

        let mut results = Vec::new();
        for (idx, raw) in raw_results.iter().enumerate() {
            if raw.get("error").is_some() {
                let filename = filenames.get(idx).unwrap_or(&"<unknown>");
                warn!(
                    "[GuessIt] Failed to parse '{}': {:?}",
                    filename,
                    raw.get("error")
                );
                // Add a minimal result for failed parses
                results.push(GuessItResult {
                    confidence: 0.0,
                    ..Default::default()
                });
                continue;
            }

            let mut result: GuessItResult = serde_json::from_value(raw.clone())
                .unwrap_or_default();
            result.validate();
            results.push(result);
        }

        let elapsed = start.elapsed();
        let success_count = results.iter().filter(|r| r.confidence > 0.0).count();

        info!(
            "[GuessIt] Batch parsed {} files in {:.2}s: {}/{} successful",
            filenames.len(),
            elapsed.as_secs_f32(),
            success_count,
            filenames.len()
        );

        Ok(results)
    }
}

impl Default for GuessItParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_english_episode() {
        let parser = GuessItParser::new();
        let result = parser.parse("Breaking.Bad.S01E01.720p.mkv").unwrap();
        assert_eq!(result.title, Some("Breaking Bad".to_string()));
        assert_eq!(result.season, Some(1));
        assert_eq!(result.episode, Some(1));
        assert_eq!(result.media_type, Some("episode".to_string()));
    }

    #[test]
    fn test_parse_english_movie() {
        let parser = GuessItParser::new();
        let result = parser.parse("Avatar.2009.1080p.BluRay.x264-DEMAND.mkv").unwrap();
        assert_eq!(result.title, Some("Avatar".to_string()));
        assert_eq!(result.year, Some(2009));
        assert_eq!(result.media_type, Some("movie".to_string()));
    }

    #[test]
    fn test_parse_chinese_episode() {
        let parser = GuessItParser::new();
        let result = parser.parse("纸牌屋 S01E01.mkv").unwrap();
        assert_eq!(result.title, Some("纸牌屋".to_string()));
        assert_eq!(result.season, Some(1));
        assert_eq!(result.episode, Some(1));
    }

    #[test]
    fn test_parse_chinese_movie() {
        let parser = GuessItParser::new();
        let result = parser
            .parse("七王国的骑士 A Knight of the Seven Kingdoms 2026.mkv")
            .unwrap();
        assert!(result.title.is_some());
    }

    #[test]
    fn test_validate_year() {
        let parser = GuessItParser::new();
        let mut result = parser.parse("Test.1800.mkv").unwrap();
        result.validate();
        assert_eq!(result.year, None);

        let mut result = parser.parse("Test.2099.mkv").unwrap();
        result.validate();
        assert_eq!(result.year, None);
    }

    #[test]
    fn test_batch_parse() {
        let parser = GuessItParser::new();
        let filenames = vec![
            "Breaking.Bad.S01E01.720p.mkv",
            "Avatar.2009.1080p.BluRay.mkv",
            "纸牌屋 S01E01.mkv",
        ];
        let results = parser.parse_batch(&filenames).unwrap();
        assert_eq!(results.len(), 3);
        assert!(results[0].is_episode());
        assert!(results[1].is_movie());
    }

    #[test]
    fn test_completeness_score() {
        let result = GuessItResult {
            title: Some("Test".to_string()),
            year: Some(2020),
            season: Some(1),
            episode: Some(1),
            media_type: Some("episode".to_string()),
            confidence: 0.0,
            ..Default::default()
        };
        assert!(result.completeness_score() > 0.8);
    }

    #[test]
    fn test_clean_filename_prefix() {
        let dirty1 = "阳光电影dygod.org.在失落之地.2025.BD.1080P.中英双字.mkv";
        let cleaned1 = clean_filename(dirty1);
        assert_eq!(cleaned1, "在失落之地.2025.BD.1080P.中英双字.mkv");

        let dirty2 = "www.dygod.org.世界大战.2025.mkv";
        let cleaned2 = clean_filename(dirty2);
        assert_eq!(cleaned2, "世界大战.2025.mkv");
    }
}
