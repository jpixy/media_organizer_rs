//! Filename parser module using Python guessit library.
//!
//! Uses Python's guessit library via subprocess to extract:
//! - Title (original and alternative)
//! - Release year
//! - Season/episode numbers
//! - Technical metadata (resolution, codec, etc.)

use crate::models::media::MediaType;
use crate::services::guessit_parser::GuessItParser;
use crate::services::ollama::OllamaClient;
use crate::utils::chinese::contains_chinese;
use crate::Result;
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use serde_json;

/// Parsed filename information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedFilename {
    /// Original title (usually English).
    pub original_title: Option<String>,
    /// Localized title (Chinese).
    pub title: Option<String>,
    /// Year of release.
    pub year: Option<u16>,
    /// Season number (for TV shows).
    pub season: Option<u16>,
    /// Episode number (for TV shows).
    pub episode: Option<u16>,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f32,
    /// Raw AI response for debugging.
    pub raw_response: Option<String>,
}

/// AI response structure for parsing.
/// Uses serde_json::Value for season/episode to handle both string ("S01") and number (1) formats.
#[derive(Debug, Deserialize)]
struct AiParseResponse {
    original_title: Option<String>,
    title: Option<String>,
    year: Option<u16>,
    season: Option<serde_json::Value>,
    episode: Option<serde_json::Value>,
    confidence: Option<f32>,
}

impl AiParseResponse {
    /// Parse season value from various formats: "S01", "1", 1, etc.
    fn parse_season(&self) -> Option<u16> {
        self.season.as_ref().and_then(Self::parse_number)
    }

    /// Parse episode value from various formats: "E05", "5", 5, etc.
    fn parse_episode(&self) -> Option<u16> {
        self.episode.as_ref().and_then(Self::parse_number)
    }

    /// Parse a number from various formats.
    fn parse_number(value: &serde_json::Value) -> Option<u16> {
        match value {
            serde_json::Value::Number(n) => n.as_u64().map(|n| n as u16),
            serde_json::Value::String(s) => {
                // Try direct parse first
                if let Ok(n) = s.parse::<u16>() {
                    return Some(n);
                }
                // Try extracting number from "S01", "E05", etc.
                let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
                digits.parse::<u16>().ok()
            }
            _ => None,
        }
    }
}

/// Parser configuration.
#[derive(Debug, Clone)]
pub struct ParserConfig {
    /// Maximum concurrent parsing requests.
    pub max_concurrent: usize,
    /// Minimum confidence threshold for valid results.
    pub min_confidence: f32,
    /// Whether AI (Ollama) parsing is enabled.
    /// When false, only local parsing (guessit) is used.
    pub ai_enabled: bool,
    /// Ollama model to use for AI parsing.
    pub model: String,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 3,
            min_confidence: 0.5,
            ai_enabled: false, // AI disabled by default per requirements
            model: "qwen3.5:4b".to_string(), // Default model
        }
    }
}

/// Filename parser using Ollama AI.
pub struct FilenameParser {
    client: OllamaClient,
    config: ParserConfig,
    guessit: GuessItParser,
}

impl FilenameParser {
    /// Create a new parser with default configuration.
    pub fn new() -> Self {
        Self {
            client: OllamaClient::new(),
            config: ParserConfig::default(),
            guessit: GuessItParser::new(),
        }
    }

    /// Create a new parser with custom configuration.
    pub fn with_config(config: ParserConfig) -> Self {
        Self {
            client: OllamaClient::new(),
            config,
            guessit: GuessItParser::new(),
        }
    }

    /// Create a new parser with custom Ollama client.
    pub fn with_client(client: OllamaClient) -> Self {
        Self {
            client,
            config: ParserConfig::default(),
            guessit: GuessItParser::new(),
        }
    }

    /// Get the model name.
    pub fn get_model(&self) -> &str {
        &self.config.model
    }

    /// Generate the prompt for parsing a filename.
    ///
    /// The prompt is in Chinese to better handle Chinese filenames and leverage
    /// the AI model's understanding of Chinese media naming conventions.
    fn generate_prompt(&self, filename: &str, media_type: MediaType) -> String {
        // Type hint: "This is a movie file" / "This is a TV show file"
        let type_hint = match media_type {
            MediaType::Movies => "这是一个电影文件",
            MediaType::TvSeries => "这是一个电视剧/剧集文件",
        };

        format!(
            r#"你是一个视频文件名解析专家。请分析以下视频文件名，提取关键信息。

文件名: {filename}
提示: {type_hint}

请提取以下信息并以JSON格式返回：
1. original_title: 原始标题（通常是英文）
2. title: 中文标题（如果有的话）
3. year: 发行年份（4位数字）
4. season: 季数（仅电视剧，如S01表示第1季）
5. episode: 集数（仅电视剧，如E05表示第5集）
6. confidence: 你对解析结果的置信度（0.0到1.0之间的小数）

注意事项：
- 忽略分辨率（如1080p、4K、2160p）、编码格式（如x265、HEVC）、音频格式（如DTS、AAC）等技术信息
- **重要**: 忽略字幕组/发布组名称！常见字幕组：霸王龙压制组、T-Rex、YYeTs、字幕侠、FIX字幕侠、人人影视、ZhuixinFan、rarbg、DEFLATE 等
- 字幕组名称通常在文件名末尾，或在方括号/横杠后面，不是真正的标题！
- 例如："流人.S01E01.HD1080P.中英双字.霸王龙压制组T-Rex.mp4" 的中文标题是"流人"，不是"霸王龙压制组"
- 如果文件名中包含中英文混合，请分别提取
- 如果无法确定某个字段，返回null
- **重要**: 年份必须是文件名中明确出现的4位数字(1900-2030)，不要猜测！如果文件名中没有年份，返回null
- 例如："动物农场.mp4"没有年份，应返回null；"雏菊 导演剪辑版 2006.mp4"年份是2006
- **重要**: 续集编号（如2、3、II、III）是标题的一部分！例如"刺杀小说家2"的标题是"刺杀小说家2"而不是"刺杀小说家"
- **重要**: 不要把紧跟在标题后面的数字当作分辨率。例如"刺杀小说家2.4k.mp4"中，"2"是续集编号，"4k"才是分辨率
- 常见续集模式：标题2、标题3、标题II、标题III、标题:副标题
- **重要**: 版本信息不是标题的一部分！如"导演剪辑版"、"加长版"、"未删减版"、"特效版"、"IMAX版"、"3D版"等都不应包含在标题中
- 例如："雏菊 导演剪辑版 2006.mp4" 的标题是"雏菊"，年份是2006

只返回JSON对象，不要包含其他文字：
{{"original_title": "...", "title": "...", "year": ..., "season": ..., "episode": ..., "confidence": ...}}"#
        )
    }

    /// Split a mixed Chinese-English title into separate Chinese and English parts.
    /// Returns (chinese_part, english_part)
    fn split_chinese_english(&self, title: &str) -> (Option<String>, Option<String>) {
        let has_chinese = title.chars().any(|c| c >= '\u{4e00}' && c <= '\u{9fff}');
        let has_ascii = title.chars().any(|c| c.is_ascii_alphabetic());
        
        if !has_chinese || !has_ascii {
            return (None, None);
        }
        
        // Split by common separators: space, dot, dash
        let separators = [' ', '.', '-', '_'];
        let parts: Vec<&str> = title.split(|c| separators.contains(&c))
            .filter(|s| !s.is_empty())
            .collect();
        
        let mut chinese_part = String::new();
        let mut english_part = String::new();
        
        for part in parts {
            if part.chars().any(|c| c >= '\u{4e00}' && c <= '\u{9fff}') {
                if !chinese_part.is_empty() {
                    chinese_part.push(' ');
                }
                chinese_part.push_str(part);
            } else if part.chars().any(|c| c.is_ascii_alphabetic()) {
                if !english_part.is_empty() {
                    english_part.push(' ');
                }
                english_part.push_str(part);
            }
        }
        
        (
            if chinese_part.is_empty() { None } else { Some(chinese_part) },
            if english_part.is_empty() { None } else { Some(english_part) }
        )
    }

    /// Parse a single filename using guessit library as primary parser.
    pub async fn parse(&self, filename: &str, media_type: MediaType) -> Result<ParsedFilename> {
        // Step 0: Strip website/source prefixes from filename
        // Common patterns: "阳光电影dygod.org.世界大战.2025..." -> "世界大战.2025..."
        let cleaned_filename = strip_website_prefix(filename);
        let parse_input = if cleaned_filename != filename {
            tracing::debug!("Stripped website prefix: '{}' -> '{}'", filename, cleaned_filename);
            &cleaned_filename
        } else {
            filename
        };

        // Step 1: Primary parser - guessit library via Python subprocess
        let type_hint = match media_type {
            MediaType::Movies => Some("movie"),
            MediaType::TvSeries => Some("episode"),
        };
        
        let guessit_result = self.guessit.parse_with_type(parse_input, type_hint)?;
        let mut parsed = ParsedFilename::default();
        
        // Map guessit result to ParsedFilename
        // primary_title() already handles fallback to alternative_title
        if let Some(title) = guessit_result.primary_title() {
            // Try to separate Chinese and English parts from mixed titles
            let (chinese_part, english_part) = self.split_chinese_english(&title);
            
            if let Some(chinese) = chinese_part {
                parsed.title = Some(chinese);
            }
            if let Some(english) = english_part {
                parsed.original_title = Some(english);
            }
            
            // Fallback: if no separation worked, use original logic
            if parsed.title.is_none() && parsed.original_title.is_none() {
                if title.contains('/') || title.chars().count() > 10 {
                    // Likely a Chinese/dual title
                    parsed.title = Some(title);
                } else if title.is_ascii() {
                    parsed.original_title = Some(title);
                } else {
                    parsed.title = Some(title);
                }
            }
        }
        
        // If primary_title() only provided a Chinese title (no English part),
        // try to extract the English title from guessit's alternative_title field.
        // This handles cases like "首都坠落.DC Down.(2023)" where guessit correctly
        // separates them into title="首都坠落" and alternative_title=["DC Down"].
        if parsed.original_title.is_none() {
            if let Some(ref alt_titles) = guessit_result.alternative_title {
                for alt in alt_titles {
                    let has_ascii = alt.chars().any(|c| c.is_ascii_alphabetic());
                    let has_chinese = alt.chars().any(|c| c >= '\u{4e00}' && c <= '\u{9fff}');
                    if has_ascii && !has_chinese {
                        let clean_alt = alt.trim().to_string();
                        if !clean_alt.is_empty() {
                            parsed.original_title = Some(clean_alt);
                            tracing::debug!(
                                "[PARSE] Extracted English title from alternative_title: {:?}",
                                parsed.original_title
                            );
                            break;
                        }
                    }
                }
            }
        }
        
        parsed.year = guessit_result.year;
        parsed.season = guessit_result.season;
        parsed.episode = guessit_result.episode;
        parsed.confidence = guessit_result.confidence;
        
        // Check if guessit returned valid result
        if parsed.title.is_some() || parsed.original_title.is_some() {
            // If AI is enabled, validate the result with AI
            if self.config.ai_enabled {
                println!("    [AI] Validating local parse result...");
                let validation_prompt = self.generate_validation_prompt(&parsed, filename, media_type);
                let validation = self.client.generate(&validation_prompt).await?;
                
                // If AI validation confirms the result, return guessit result
                if self.is_validation_confirmed(&validation.response) {
                    println!("    [OK] AI validation passed");
                    return Ok(parsed);
                }
                
                // AI validation failed, use AI to parse
                println!("    [AI] AI validation failed, using AI to parse...");
            } else {
                return Ok(parsed);
            }
        }
        
        // If AI is disabled, return the guessit result (even if no title found)
        // This ensures the tool works without AI when guessit fails
        if !self.config.ai_enabled {
            tracing::debug!("AI disabled, returning guessit-only result for: {}", filename);
            return Ok(parsed);
        }
        
        // Fallback to AI parser if guessit fails or AI validation failed
        let prompt = self.generate_prompt(filename, media_type);

        tracing::debug!("Parsing filename: {}", filename);
        println!("    [AI] Parsing: {}...", filename);

        let start = std::time::Instant::now();

        // Call Ollama API with JSON format
        let response = self
            .client
            .generate_with_format(&prompt, Some("json"))
            .await?;

        let elapsed = start.elapsed();
        println!("    [OK] Parsed in {:.1}s", elapsed.as_secs_f32());
        tracing::debug!("AI response: {}", response.response);

        // Parse the JSON response
        let parsed = self.parse_ai_response(&response.response, filename)?;

        // Validate the result
        let validated = self.validate_result(parsed)?;

        Ok(validated)
    }

    /// Parse AI response into ParsedFilename.
    fn parse_ai_response(&self, response: &str, filename: &str) -> Result<ParsedFilename> {
        // Try to parse as JSON
        match serde_json::from_str::<AiParseResponse>(response) {
            Ok(ai_response) => {
                // Normalize confidence to 0.0-1.0 range (AI sometimes returns 0-100)
                let raw_confidence = ai_response.confidence.unwrap_or(0.5);
                let confidence = if raw_confidence > 1.0 {
                    raw_confidence / 100.0
                } else {
                    raw_confidence
                };

                let season = ai_response.parse_season();
                let episode = ai_response.parse_episode();
                Ok(ParsedFilename {
                    original_title: ai_response.original_title,
                    title: ai_response.title,
                    year: ai_response.year,
                    season,
                    episode,
                    confidence,
                    raw_response: Some(response.to_string()),
                })
            }
            Err(e) => {
                tracing::warn!("Failed to parse AI response for '{}': {}", filename, e);
                // Return a low-confidence result with raw response
                Ok(ParsedFilename {
                    raw_response: Some(response.to_string()),
                    confidence: 0.0,
                    ..Default::default()
                })
            }
        }
    }

    /// Validate parsed result.
    fn validate_result(&self, mut parsed: ParsedFilename) -> Result<ParsedFilename> {
        // Filter out subtitle group names from titles
        parsed.title = parsed.title.and_then(|t| filter_subtitle_group(&t));
        parsed.original_title = parsed
            .original_title
            .and_then(|t| filter_subtitle_group(&t));

        // Validate year range (1900 - current year + 5)
        if let Some(year) = parsed.year {
            let current_year = chrono::Utc::now().year() as u16;
            if year < 1900 || year > current_year + 5 {
                tracing::warn!("Invalid year {}, ignoring", year);
                parsed.year = None;
                parsed.confidence *= 0.5;
            }
        }

        // Validate season/episode numbers
        if let Some(season) = parsed.season {
            if season == 0 || season > 100 {
                parsed.season = None;
            }
        }
        if let Some(episode) = parsed.episode {
            if episode == 0 || episode > 1000 {
                parsed.episode = None;
            }
        }

        // Validate titles are not empty
        if let Some(ref title) = parsed.original_title {
            if title.trim().is_empty() {
                parsed.original_title = None;
            }
        }
        if let Some(ref title) = parsed.title {
            if title.trim().is_empty() {
                parsed.title = None;
            }
        }

        // Adjust confidence if missing critical fields
        if parsed.original_title.is_none() && parsed.title.is_none() {
            parsed.confidence = 0.0;
        }

        Ok(parsed)
    }

    /// Parse multiple filenames in batch with concurrency control.
    pub async fn parse_batch(
        &self,
        filenames: &[String],
        media_type: MediaType,
    ) -> Vec<(String, Result<ParsedFilename>)> {
        use std::sync::Arc;
        use tokio::sync::Semaphore;

        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent));
        let mut handles = Vec::new();

        for filename in filenames {
            let filename = filename.clone();
            let prompt = self.generate_prompt(&filename, media_type);
            let client = self.client.clone();
            let semaphore = semaphore.clone();

            let handle = tokio::spawn(async move {
                // Acquire permit inside the spawned task to properly limit concurrency
                let _permit = semaphore.acquire_owned().await.unwrap();

                let result = async {
                    let response = client.generate_with_format(&prompt, Some("json")).await?;

                    // Parse response
                    let parsed: Result<ParsedFilename> =
                        match serde_json::from_str::<AiParseResponse>(&response.response) {
                            Ok(ai_response) => {
                                // Normalize confidence to 0.0-1.0 range
                                let raw_confidence = ai_response.confidence.unwrap_or(0.5);
                                let confidence = if raw_confidence > 1.0 {
                                    raw_confidence / 100.0
                                } else {
                                    raw_confidence
                                };

                                let season = ai_response.parse_season();
                                let episode = ai_response.parse_episode();
                                Ok(ParsedFilename {
                                    original_title: ai_response.original_title,
                                    title: ai_response.title,
                                    year: ai_response.year,
                                    season,
                                    episode,
                                    confidence,
                                    raw_response: Some(response.response),
                                })
                            }
                            Err(_) => Ok(ParsedFilename {
                                raw_response: Some(response.response),
                                confidence: 0.0,
                                ..Default::default()
                            }),
                        };
                    parsed
                }
                .await;

                // Apply validation to batch results (same as single-file parse)
                let validated = match result {
                    Ok(parsed) => {
                        // We need a FilenameParser to call validate_result, but we're in a spawned task.
                        // Instead, apply the same validation logic inline.
                        let mut p = parsed;
                        // Filter out subtitle group names from titles
                        p.title = p.title.and_then(|t| filter_subtitle_group(&t));
                        p.original_title = p
                            .original_title
                            .and_then(|t| filter_subtitle_group(&t));

                        // Validate year range (1900 - current year + 5)
                        if let Some(year) = p.year {
                            let current_year = chrono::Utc::now().year() as u16;
                            if year < 1900 || year > current_year + 5 {
                                tracing::warn!("Invalid year {}, ignoring", year);
                                p.year = None;
                                p.confidence *= 0.5;
                            }
                        }

                        // Validate season/episode numbers
                        if let Some(season) = p.season {
                            if season == 0 || season > 100 {
                                p.season = None;
                            }
                        }
                        if let Some(episode) = p.episode {
                            if episode == 0 || episode > 1000 {
                                p.episode = None;
                            }
                        }

                        // Validate titles are not empty
                        if let Some(ref title) = p.original_title {
                            if title.trim().is_empty() {
                                p.original_title = None;
                            }
                        }
                        if let Some(ref title) = p.title {
                            if title.trim().is_empty() {
                                p.title = None;
                            }
                        }

                        // Adjust confidence if missing critical fields
                        if p.original_title.is_none() && p.title.is_none() {
                            p.confidence = 0.0;
                        }

                        Ok(p)
                    }
                    Err(e) => Err(e),
                };

                (filename, validated)
            });

            handles.push(handle);
        }

        // Collect results
        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::error!("Task failed: {}", e);
                }
            }
        }

        results
    }

    /// Check if a parsed result meets the minimum confidence threshold.
    pub fn is_valid(&self, parsed: &ParsedFilename) -> bool {
        parsed.confidence >= self.config.min_confidence
            && (parsed.original_title.is_some() || parsed.title.is_some())
    }

    /// Generate a validation prompt to verify hunch parsing results with AI.
    fn generate_validation_prompt(&self, parsed: &ParsedFilename, filename: &str, media_type: MediaType) -> String {
        // Type hint: "This is a movie file" / "This is a TV show file"
        let type_hint = match media_type {
            MediaType::Movies => "这是一个电影文件",
            MediaType::TvSeries => "这是一个电视剧/剧集文件",
        };

        // Build the parsed result summary
        let parsed_summary = format!(
            "原始解析结果:\n- original_title: {:?}\n- title: {:?}\n- year: {:?}\n- season: {:?}\n- episode: {:?}",
            parsed.original_title,
            parsed.title,
            parsed.year,
            parsed.season,
            parsed.episode
        );

        format!(
            r#"你是一个视频文件名验证专家。请验证以下AI解析结果是否正确。

文件名: {filename}
提示: {type_hint}

{parsed_summary}

请判断这个解析结果是否正确：
1. original_title 和 title 是否正确提取？
2. year 是否正确？年份必须是文件名中明确出现的4位数字(1900-2030)
3. season/episode 是否正确？仅电视剧有这些字段

注意事项：
- 忽略分辨率（如1080p、4K、2160p）、编码格式（如x265、HEVC）、音频格式（如DTS、AAC）等技术信息
- **重要**: 忽略字幕组/发布组名称！常见字幕组：霸王龙压制组、T-Rex、YYeTs、字幕侠、FIX字幕侠、人人影视、ZhuixinFan、rarbg、DEFLATE 等
- 字幕组名称通常在文件名末尾，或在方括号/横杠后面，不是真正的标题！
- 如果文件名中包含中英文混合，请分别提取
- 如果无法确定某个字段，返回null
- **重要**: 年份必须是文件名中明确出现的4位数字(1900-2030)，不要猜测！如果文件名中没有年份，返回null
- **重要**: 续集编号（如2、3、II、III）是标题的一部分！例如"刺杀小说家2"的标题是"刺杀小说家2"而不是"刺杀小说家"
- **重要**: 版本信息不是标题的一部分！如"导演剪辑版"、"加长版"、"未删减版"、"特效版"、"IMAX版"、"3D版"等都不应包含在标题中

请以JSON格式返回验证结果：
{{
    "valid": true/false,
    "corrected_title": "如果需要修正，返回正确的中文标题；否则返回null",
    "corrected_original_title": "如果需要修正，返回正确的英文标题；否则返回null",
    "corrected_year": "如果需要修正，返回正确的年份；否则返回null",
    "confidence": 0.0-1.0
}}

只返回JSON对象，不要包含其他文字。"#
        )
    }

    /// Check if AI validation confirms the parsing result.
    fn is_validation_confirmed(&self, response: &str) -> bool {
        // Try to parse the response as JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(response) {
            // Check if "valid" field is true
            if let Some(valid) = json.get("valid") {
                if valid.as_bool() == Some(true) {
                    return true;
                }
            }
        }

        // Check if response contains "valid": true (fallback)
        response.contains("\"valid\":true") || response.contains("\"valid\": true")
    }
}

impl Default for FilenameParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a video filename using AI (convenience function).
pub async fn parse_filename(filename: &str, media_type: MediaType) -> Result<ParsedFilename> {
    let parser = FilenameParser::new();
    parser.parse(filename, media_type).await
}

/// Parse multiple filenames in batch (convenience function).
pub async fn parse_filenames(
    filenames: &[String],
    media_type: MediaType,
) -> Vec<(String, Result<ParsedFilename>)> {
    let parser = FilenameParser::new();
    parser.parse_batch(filenames, media_type).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_filename_default() {
        let parsed = ParsedFilename::default();
        assert!(parsed.original_title.is_none());
        assert!(parsed.title.is_none());
        assert!(parsed.year.is_none());
        assert_eq!(parsed.confidence, 0.0);
    }

    #[test]
    fn test_parser_config_default() {
        let config = ParserConfig::default();
        assert_eq!(config.max_concurrent, 3);
        assert_eq!(config.min_confidence, 0.5);
        assert!(!config.ai_enabled); // AI disabled by default
    }

    #[test]
    fn test_parser_config_ai_disabled_by_default() {
        let config = ParserConfig::default();
        assert!(!config.ai_enabled);
    }

    #[test]
    fn test_generate_prompt_movie() {
        let parser = FilenameParser::new();
        let prompt = parser.generate_prompt("Avatar.2009.1080p.BluRay.mkv", MediaType::Movies);

        assert!(prompt.contains("Avatar.2009.1080p.BluRay.mkv"));
        assert!(prompt.contains("电影"));
    }

    #[test]
    fn test_generate_prompt_tv_series() {
        let parser = FilenameParser::new();
        let prompt = parser.generate_prompt("Breaking.Bad.S01E01.720p.mkv", MediaType::TvSeries);

        assert!(prompt.contains("Breaking.Bad.S01E01.720p.mkv"));
        assert!(prompt.contains("电视剧"));
    }

    #[test]
    fn test_validate_year_range() {
        let parser = FilenameParser::new();

        // Valid year
        let parsed = ParsedFilename {
            year: Some(2020),
            confidence: 1.0,
            original_title: Some("Test".to_string()),
            ..Default::default()
        };
        let result = parser.validate_result(parsed).unwrap();
        assert_eq!(result.year, Some(2020));

        // Invalid year (too old)
        let parsed = ParsedFilename {
            year: Some(1800),
            confidence: 1.0,
            original_title: Some("Test".to_string()),
            ..Default::default()
        };
        let result = parser.validate_result(parsed).unwrap();
        assert!(result.year.is_none());
    }

    #[test]
    fn test_is_valid() {
        let parser = FilenameParser::new();

        // Valid result
        let parsed = ParsedFilename {
            original_title: Some("Avatar".to_string()),
            confidence: 0.8,
            ..Default::default()
        };
        assert!(parser.is_valid(&parsed));

        // Low confidence
        let parsed = ParsedFilename {
            original_title: Some("Avatar".to_string()),
            confidence: 0.3,
            ..Default::default()
        };
        assert!(!parser.is_valid(&parsed));

        // No title
        let parsed = ParsedFilename {
            confidence: 0.8,
            ..Default::default()
        };
        assert!(!parser.is_valid(&parsed));
    }

    // ========================================================================
    // extract_episode_from_filename 测试
    // ========================================================================

    #[test]
    fn test_extract_episode_sxxexx() {
        // Standard S01E01 format
        assert_eq!(extract_episode_from_filename("S01E01.mkv"), (Some(1), Some(1)));
        assert_eq!(extract_episode_from_filename("s01e05.mkv"), (Some(1), Some(5)));
        assert_eq!(extract_episode_from_filename("S02E10.mkv"), (Some(2), Some(10)));
        assert_eq!(extract_episode_from_filename("S12E100.mkv"), (Some(12), Some(100)));
    }

    #[test]
    fn test_extract_episode_season_episode_text() {
        // "Season 01 Episode 05" format
        assert_eq!(extract_episode_from_filename("Season 01 Episode 05.mkv"), (Some(1), Some(5)));
        assert_eq!(extract_episode_from_filename("season2 episode10.mkv"), (Some(2), Some(10)));
    }

    #[test]
    fn test_extract_episode_exx_only() {
        // E01, EP01 format (default season 1)
        assert_eq!(extract_episode_from_filename("E01.mkv"), (Some(1), Some(1)));
        assert_eq!(extract_episode_from_filename("EP05.mkv"), (Some(1), Some(5)));
        assert_eq!(extract_episode_from_filename("Show.E03.mkv"), (Some(1), Some(3)));
    }

    #[test]
    fn test_extract_episode_chinese() {
        // 第01集 format
        assert_eq!(extract_episode_from_filename("第01集.mp4"), (Some(1), Some(1)));
        assert_eq!(extract_episode_from_filename("第5集.mkv"), (Some(1), Some(5)));
        assert_eq!(extract_episode_from_filename("流人第12集.mp4"), (Some(1), Some(12)));
    }

    #[test]
    fn test_extract_episode_leading_number() {
        // Just a number at the start
        assert_eq!(extract_episode_from_filename("01.mp4"), (Some(1), Some(1)));
        assert_eq!(extract_episode_from_filename("05.mkv"), (Some(1), Some(5)));
        assert_eq!(extract_episode_from_filename("01 4K.mp4"), (Some(1), Some(1)));
    }

    #[test]
    fn test_extract_episode_leading_number_not_year() {
        // Year-like numbers should NOT be treated as episodes
        assert_eq!(extract_episode_from_filename("2001.mkv"), (None, None));
        assert_eq!(extract_episode_from_filename("2024.mp4"), (None, None));
    }

    #[test]
    fn test_extract_episode_trailing() {
        // Title-02, Title_02 format
        assert_eq!(extract_episode_from_filename("不伦食堂-02.mp4"), (Some(1), Some(2)));
        assert_eq!(extract_episode_from_filename("今夜我用身体恋爱_03.mp4"), (Some(1), Some(3)));
        assert_eq!(extract_episode_from_filename("标题.05.mp4"), (Some(1), Some(5)));
    }

    #[test]
    fn test_extract_episode_trailing_with_end() {
        // "Title-04 end" format
        assert_eq!(extract_episode_from_filename("不伦食堂-04 end.mp4"), (Some(1), Some(4)));
    }

    #[test]
    fn test_extract_episode_cjk_direct_number() {
        // CJK title directly followed by episode number
        assert_eq!(extract_episode_from_filename("孤芳不自赏02.1024高清.mp4"), (Some(1), Some(2)));
        assert_eq!(extract_episode_from_filename("那年青春我们正好02.1280高清.mp4"), (Some(1), Some(2)));
    }

    #[test]
    fn test_extract_episode_special() {
        // Special episodes -> Season 0
        assert_eq!(extract_episode_from_filename("S02.Special.White.Christmas.mkv"), (Some(0), Some(1)));
        assert_eq!(extract_episode_from_filename("Show.SP.1080p.mkv"), (Some(0), Some(1)));
        assert_eq!(extract_episode_from_filename("[sp]episode.mkv"), (Some(0), Some(1)));
    }

    #[test]
    fn test_extract_episode_no_match() {
        // No episode info
        assert_eq!(extract_episode_from_filename("movie.mkv"), (None, None));
        assert_eq!(extract_episode_from_filename("Avatar.2009.1080p.mkv"), (None, None));
    }

    // ========================================================================
    // extract_season_from_dirname 测试
    // ========================================================================

    #[test]
    fn test_extract_season_chinese_numeral() {
        assert_eq!(extract_season_from_dirname("第一季"), Some(1));
        assert_eq!(extract_season_from_dirname("第二季"), Some(2));
        assert_eq!(extract_season_from_dirname("第十季"), Some(10));
        assert_eq!(extract_season_from_dirname("第一部"), Some(1));
    }

    #[test]
    fn test_extract_season_chinese_arabic() {
        assert_eq!(extract_season_from_dirname("第1季"), Some(1));
        assert_eq!(extract_season_from_dirname("第2季"), Some(2));
        assert_eq!(extract_season_from_dirname("第10季"), Some(10));
    }

    #[test]
    fn test_extract_season_english() {
        assert_eq!(extract_season_from_dirname("Season 01"), Some(1));
        assert_eq!(extract_season_from_dirname("Season 2"), Some(2));
        assert_eq!(extract_season_from_dirname("S01"), Some(1));
        assert_eq!(extract_season_from_dirname("S1"), Some(1));
        assert_eq!(extract_season_from_dirname("S04"), Some(4));
        assert_eq!(extract_season_from_dirname("S10"), Some(10));
    }

    #[test]
    fn test_extract_season_no_match() {
        assert_eq!(extract_season_from_dirname("4K"), None);
        assert_eq!(extract_season_from_dirname("1080p"), None);
        assert_eq!(extract_season_from_dirname("Movie Name"), None);
    }

    /// Test the bug fix: when filename only has episode number (e.g., "01.mp4")
    /// but directory contains season info (e.g., "S04"), the season should be
    /// extracted from directory name, not default to 1.
    #[test]
    fn test_season_extraction_fallback_scenario() {
        // Simulate the bug scenario: filename "01.mp4" extracts to S1 by default
        // but parent directory "S04" should override to S4
        let (season_from_file, episode) = extract_episode_from_filename("01.mp4");
        assert_eq!(season_from_file, Some(1)); // Default behavior
        assert_eq!(episode, Some(1));

        // Directory "S04" should extract to season 4
        let season_from_dir = extract_season_from_dirname("S04");
        assert_eq!(season_from_dir, Some(4));

        // The bug: if season == Some(1), we should check directory
        // After fix: use directory season when file season is None or Some(1)
        let mut season = season_from_file;
        if season.is_none() || season == Some(1) {
            if let Some(dir_season) = season_from_dir {
                season = Some(dir_season);
            }
        }
        assert_eq!(season, Some(4)); // Should be 4, not 1!
    }

    /// Test another bug scenario: filename "02.mp4" in directory "第二季"
    #[test]
    fn test_season_extraction_chinese_directory() {
        let (season_from_file, episode) = extract_episode_from_filename("02.mp4");
        assert_eq!(season_from_file, Some(1)); // Default
        assert_eq!(episode, Some(2));

        let season_from_dir = extract_season_from_dirname("第二季");
        assert_eq!(season_from_dir, Some(2));

        let mut season = season_from_file;
        if season.is_none() || season == Some(1) {
            if let Some(dir_season) = season_from_dir {
                season = Some(dir_season);
            }
        }
        assert_eq!(season, Some(2));
    }

    // ========================================================================
    // is_organized_filename 测试
    // ========================================================================

    #[test]
    fn test_is_organized_tv_series() {
        assert!(is_organized_filename("[Breaking Bad]-S01E01-[Pilot]-1080p.mkv"));
        assert!(is_organized_filename("[流人]-S01E01-[Episode 1]-720p.mkv"));
    }

    #[test]
    fn test_is_organized_movie_with_id() {
        assert!(is_organized_filename("[Avatar][阿凡达](2009)-tt0499549-tmdb19995-1080p.mkv"));
        assert!(is_organized_filename("[焚城](2024)-tt29495090-tmdb1305642-2160p.mkv"));
    }

    #[test]
    fn test_is_organized_movie_with_tech() {
        assert!(is_organized_filename("[Upgrade][升级](2018)-1080p-WEB-DL-h264-8bit-aac-2.0.mp4"));
        assert!(is_organized_filename("[焚城](2024)-2160p-WEB-DL-hevc-8bit-aac-2.0.mp4"));
    }

    #[test]
    fn test_is_not_organized() {
        assert!(!is_organized_filename("Avatar.2009.1080p.BluRay.mkv"));
        assert!(!is_organized_filename("movie.mp4"));
        assert!(!is_organized_filename("S01E01.mkv"));
    }

    // ========================================================================
    // parse_organized_tv_series_filename 测试
    // ========================================================================

    #[test]
    fn test_parse_organized_tv_series() {
        let info = parse_organized_tv_series_filename("[Breaking Bad]-S01E01-[Pilot]-1080p.mkv").unwrap();
        assert_eq!(info.title, "Breaking Bad");
        assert_eq!(info.season, 1);
        assert_eq!(info.episode, 1);
        assert_eq!(info.episode_name, "Pilot");
    }

    #[test]
    fn test_parse_organized_tv_series_chinese() {
        let info = parse_organized_tv_series_filename("[流人]-S02E05-[Episode Name]-720p.mkv").unwrap();
        assert_eq!(info.title, "流人");
        assert_eq!(info.season, 2);
        assert_eq!(info.episode, 5);
    }

    #[test]
    fn test_parse_organized_tv_series_no_match() {
        assert!(parse_organized_tv_series_filename("Avatar.2009.1080p.mkv").is_none());
    }

    // ========================================================================
    // parse_organized_movie_filename 测试
    // ========================================================================

    #[test]
    fn test_parse_organized_movie_dual_title_with_id() {
        let info = parse_organized_movie_filename("[Avatar][阿凡达](2009)-tt0499549-tmdb19995-1080p.mkv").unwrap();
        assert_eq!(info.original_title, Some("Avatar".to_string()));
        assert_eq!(info.title, Some("阿凡达".to_string()));
        assert_eq!(info.year, 2009);
        assert_eq!(info.imdb_id, Some("tt0499549".to_string()));
        assert_eq!(info.tmdb_id, Some(19995));
    }

    #[test]
    fn test_parse_organized_movie_single_title_with_id() {
        let info = parse_organized_movie_filename("[焚城](2024)-tt29495090-tmdb1305642-2160p.mkv").unwrap();
        assert_eq!(info.original_title, Some("焚城".to_string()));
        assert_eq!(info.title, None);
        assert_eq!(info.year, 2024);
        assert_eq!(info.imdb_id, Some("tt29495090".to_string()));
        assert_eq!(info.tmdb_id, Some(1305642));
    }

    #[test]
    fn test_parse_organized_movie_dual_title_tech_only() {
        let info = parse_organized_movie_filename("[Upgrade][升级](2018)-1080p-WEB-DL-h264-8bit-aac-2.0.mp4").unwrap();
        assert_eq!(info.original_title, Some("Upgrade".to_string()));
        assert_eq!(info.title, Some("升级".to_string()));
        assert_eq!(info.year, 2018);
        assert_eq!(info.imdb_id, None);
        assert_eq!(info.tmdb_id, None);
    }

    #[test]
    fn test_parse_organized_movie_single_title_tech_only() {
        let info = parse_organized_movie_filename("[焚城](2024)-2160p-WEB-DL-hevc-8bit-aac-2.0.mp4").unwrap();
        assert_eq!(info.original_title, Some("焚城".to_string()));
        assert_eq!(info.title, None);
        assert_eq!(info.year, 2024);
    }

    #[test]
    fn test_parse_organized_movie_no_match() {
        assert!(parse_organized_movie_filename("Avatar.2009.1080p.mkv").is_none());
    }

    // ========================================================================
    // parse_organized_movie_folder 测试
    // ========================================================================

    #[test]
    fn test_parse_organized_movie_folder_category_prefix_dual_title() {
        // Black Widow folder with category prefix [B] and dual English titles
        let info = parse_organized_movie_folder("[B][Black Widow][Black Widow](2021)-tt3480822-tmdb497698").unwrap();
        assert_eq!(info.original_title, Some("Black Widow".to_string()));
        assert_eq!(info.title, Some("Black Widow".to_string())); // Both are English
        assert_eq!(info.year, 2021);
        assert_eq!(info.imdb_id, Some("tt3480822".to_string()));
        assert_eq!(info.tmdb_id, 497698);
    }

    #[test]
    fn test_parse_organized_movie_folder_with_category_prefix() {
        // Single title with category prefix
        let info = parse_organized_movie_folder("[B][Black Widow](2021)-tt3480822-tmdb497698").unwrap();
        assert_eq!(info.original_title, Some("Black Widow".to_string()));
        assert_eq!(info.title, None); // No second title
        assert_eq!(info.year, 2021);
        assert_eq!(info.imdb_id, Some("tt3480822".to_string()));
        assert_eq!(info.tmdb_id, 497698);
    }

    #[test]
    fn test_parse_organized_movie_folder_category_prefix_dual_title_chinese() {
        // Category prefix + dual title with Chinese second title
        let info = parse_organized_movie_folder("[B][Spider-Man][蜘蛛侠：英雄无归](2021)-tt10872600-tmdb634649").unwrap();
        assert_eq!(info.original_title, Some("Spider-Man".to_string()));
        assert_eq!(info.title, Some("蜘蛛侠：英雄无归".to_string()));
        assert_eq!(info.year, 2021);
        assert_eq!(info.tmdb_id, 634649);
    }

    #[test]
    fn test_parse_organized_movie_folder_category_prefix_dual_title_both_english() {
        // Category prefix + dual title with both English (the bug scenario)
        let info = parse_organized_movie_folder("[B][Black Widow][Black Widow](2021)-tt3480822-tmdb497698").unwrap();
        assert_eq!(info.original_title, Some("Black Widow".to_string()));
        assert_eq!(info.title, Some("Black Widow".to_string())); // Both English
        assert_eq!(info.year, 2021);
        assert_eq!(info.tmdb_id, 497698);
    }

    #[test]
    fn test_parse_organized_movie_folder_different_category_codes() {
        // Test different single-character category codes
        let categories = vec!["B", "H", "S", "M", "D", "A", "C"];
        
        for cat in categories {
            let folder = format!("[{}][Test Movie](2020)-tt1234567-tmdb123456", cat);
            let info = parse_organized_movie_folder(&folder).unwrap();
            assert_eq!(info.original_title, Some("Test Movie".to_string()));
            assert_eq!(info.title, None);
            assert_eq!(info.year, 2020);
        }
    }

    #[test]
    fn test_parse_organized_movie_folder_uppercase_category() {
        // Uppercase category code
        let info = parse_organized_movie_folder("[B][Black Widow](2021)-tt3480822-tmdb497698").unwrap();
        assert_eq!(info.original_title, Some("Black Widow".to_string()));
    }

    #[test]
    fn test_parse_organized_movie_folder_lowercase_category() {
        // Lowercase category code should also work
        let info = parse_organized_movie_folder("[b][Black Widow](2021)-tt3480822-tmdb497698").unwrap();
        assert_eq!(info.original_title, Some("Black Widow".to_string()));
    }

    #[test]
    fn test_parse_organized_movie_folder_dual() {
        let info = parse_organized_movie_folder("[Upgrade][升级](2018)-tt6499752-tmdb500664").unwrap();
        assert_eq!(info.original_title, Some("Upgrade".to_string()));
        assert_eq!(info.title, Some("升级".to_string()));
        assert_eq!(info.year, 2018);
        assert_eq!(info.imdb_id, Some("tt6499752".to_string()));
        assert_eq!(info.tmdb_id, 500664);
    }

    #[test]
    fn test_parse_organized_movie_folder_single() {
        let info = parse_organized_movie_folder("[焚城](2024)-tt29495090-tmdb1305642").unwrap();
        assert_eq!(info.original_title, Some("焚城".to_string()));
        assert_eq!(info.year, 2024);
        assert_eq!(info.tmdb_id, 1305642);
    }

    #[test]
    fn test_parse_organized_movie_folder_no_imdb() {
        let info = parse_organized_movie_folder("[焚城](2024)-tmdb1305642").unwrap();
        assert_eq!(info.imdb_id, None);
        assert_eq!(info.tmdb_id, 1305642);
    }

    #[test]
    fn test_parse_organized_movie_folder_smart_extraction() {
        // Non-standard format: year before title
        let info = parse_organized_movie_folder("2024-[Title]-[标题]-tt12345678-12345").unwrap();
        assert_eq!(info.tmdb_id, 12345);
        assert_eq!(info.year, 2024);
    }

    // ========================================================================
    // parse_organized_tv_series_folder 测试
    // ========================================================================

    #[test]
    fn test_parse_organized_tv_series_folder_dual_with_year() {
        let info = parse_organized_tv_series_folder("[러브 미][爱我](2025)-tt35451747-tmdb275989").unwrap();
        assert_eq!(info.title, "爱我");
        assert_eq!(info.year, Some(2025));
        assert_eq!(info.imdb_id, Some("tt35451747".to_string()));
        assert_eq!(info.tmdb_id, 275989);
    }

    #[test]
    fn test_parse_organized_tv_series_folder_dual_no_year() {
        let info = parse_organized_tv_series_folder("[러브 미][爱我]-tt35451747-tmdb275989").unwrap();
        assert_eq!(info.title, "爱我");
        assert_eq!(info.year, None);
        assert_eq!(info.tmdb_id, 275989);
    }

    #[test]
    fn test_parse_organized_tv_series_folder_single() {
        let info = parse_organized_tv_series_folder("[罚罪2](2025)-tt36771056-tmdb296146").unwrap();
        assert_eq!(info.title, "罚罪2");
        assert_eq!(info.year, Some(2025));
        assert_eq!(info.tmdb_id, 296146);
    }

    #[test]
    fn test_parse_organized_tv_series_folder_no_imdb() {
        let info = parse_organized_tv_series_folder("[罚罪2](2025)-tmdb296146").unwrap();
        assert_eq!(info.imdb_id, None);
        assert_eq!(info.tmdb_id, 296146);
    }

    #[test]
    fn test_parse_organized_tv_series_folder_empty_chinese() {
        let info = parse_organized_tv_series_folder("[러브 미][ ]-tt35451747-tmdb275989").unwrap();
        // Empty Chinese title falls back to original title
        assert_eq!(info.title, "러브 미");
    }

    // ========================================================================
    // filter_subtitle_group 测试
    // ========================================================================

    #[test]
    fn test_filter_subtitle_group_exact() {
        assert_eq!(filter_subtitle_group("霸王龙压制组"), None);
        assert_eq!(filter_subtitle_group("T-Rex"), None);
        assert_eq!(filter_subtitle_group("YYeTs"), None);
        assert_eq!(filter_subtitle_group("rarbg"), None);
        assert_eq!(filter_subtitle_group("DEFLATE"), None);
        assert_eq!(filter_subtitle_group("中英双字"), None);
    }

    #[test]
    fn test_filter_subtitle_group_contains() {
        // Short title containing subtitle group pattern
        assert_eq!(filter_subtitle_group("FIX字幕侠压制"), None);
        assert_eq!(filter_subtitle_group("人人影视发布"), None);
    }

    #[test]
    fn test_filter_subtitle_group_normal_title() {
        // Normal titles should pass through
        assert_eq!(filter_subtitle_group("Avatar"), Some("Avatar".to_string()));
        assert_eq!(filter_subtitle_group("流人"), Some("流人".to_string()));
        assert_eq!(filter_subtitle_group("Breaking Bad"), Some("Breaking Bad".to_string()));
    }

    #[test]
    fn test_filter_subtitle_group_long_title_with_pattern() {
        // Long title containing subtitle group pattern should pass
        let long_title = "这是一个很长的电影标题包含了字幕侠的信息但标题本身足够长";
        assert_eq!(filter_subtitle_group(long_title), Some(long_title.to_string()));
    }

    #[test]
    fn test_filter_subtitle_group_empty() {
        assert_eq!(filter_subtitle_group(""), None);
        assert_eq!(filter_subtitle_group("  "), None);
    }

    // ========================================================================
    // extract_smart_metadata 测试
    // ========================================================================

    #[test]
    fn test_extract_smart_metadata_standard() {
        let meta = extract_smart_metadata("[Title](2024)-tt12345678-tmdb67890");
        assert_eq!(meta.imdb_id, Some("tt12345678".to_string()));
        assert_eq!(meta.tmdb_id, Some(67890));
        assert_eq!(meta.year, Some(2024));
        assert_eq!(meta.titles, vec!["Title"]);
    }

    #[test]
    fn test_extract_smart_metadata_dual_title() {
        let meta = extract_smart_metadata("[Original][中文](2024)-tt12345678-tmdb67890");
        assert_eq!(meta.titles, vec!["Original", "中文"]);
        assert_eq!(meta.primary_title(), Some("中文".to_string()));
        assert_eq!(meta.original_title(), Some("Original".to_string()));
    }

    #[test]
    fn test_extract_smart_metadata_non_standard_order() {
        // Year before title, tmdb before imdb
        let meta = extract_smart_metadata("2024-[Title]-tmdb67890-tt12345678");
        assert_eq!(meta.tmdb_id, Some(67890));
        assert_eq!(meta.imdb_id, Some("tt12345678".to_string()));
        assert_eq!(meta.year, Some(2024));
    }

    #[test]
    fn test_extract_smart_metadata_tmdb_after_imdb() {
        // Legacy format: -ttIMDB-TMDBID (TMDB ID must be 5-8 digits for smart extraction)
        let meta = extract_smart_metadata("[Title]-tt0372183-500664");
        assert_eq!(meta.imdb_id, Some("tt0372183".to_string()));
        assert_eq!(meta.tmdb_id, Some(500664));
    }

    #[test]
    fn test_extract_smart_metadata_year_in_parentheses() {
        let meta = extract_smart_metadata("[Title](2024)-tmdb12345");
        assert_eq!(meta.year, Some(2024));
    }

    #[test]
    fn test_extract_smart_metadata_no_ids() {
        let meta = extract_smart_metadata("Just a title 2024");
        assert_eq!(meta.tmdb_id, None);
        assert_eq!(meta.imdb_id, None);
        assert_eq!(meta.year, Some(2024));
    }

    // ========================================================================
    // regex_match_trailing_episode 边界测试
    // ========================================================================

    #[test]
    fn test_trailing_episode_quality_suffix() {
        // "幽灵 01_超清" -> episode 1
        assert_eq!(extract_episode_from_filename("幽灵 01_超清.mp4"), (Some(1), Some(1)));
    }

    #[test]
    fn test_trailing_episode_cjk_end() {
        // CJK char + 2-digit number at end
        assert_eq!(extract_episode_from_filename("孤芳不自赏27.mp4"), (Some(1), Some(27)));
    }

    #[test]
    fn test_trailing_episode_not_year() {
        // Should not match year-like numbers
        assert_eq!(extract_episode_from_filename("Movie 2024.mp4"), (None, None));
    }

    // ========================================================================
    // validate_result 综合测试
    // ========================================================================

    #[test]
    fn test_validate_result_filters_subtitle_groups() {
        let parser = FilenameParser::new();
        let parsed = ParsedFilename {
            title: Some("霸王龙压制组".to_string()),
            original_title: Some("T-Rex".to_string()),
            confidence: 0.9,
            ..Default::default()
        };
        let result = parser.validate_result(parsed).unwrap();
        assert!(result.title.is_none());
        assert!(result.original_title.is_none());
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_validate_result_future_year() {
        let parser = FilenameParser::new();
        let current_year = chrono::Utc::now().year() as u16;
        let parsed = ParsedFilename {
            year: Some(current_year + 10),
            confidence: 1.0,
            original_title: Some("Test".to_string()),
            ..Default::default()
        };
        let result = parser.validate_result(parsed).unwrap();
        assert!(result.year.is_none());
        assert!(result.confidence < 1.0);
    }

    #[test]
    fn test_validate_result_empty_titles() {
        let parser = FilenameParser::new();
        let parsed = ParsedFilename {
            title: Some("  ".to_string()),
            original_title: Some("".to_string()),
            confidence: 0.9,
            ..Default::default()
        };
        let result = parser.validate_result(parsed).unwrap();
        assert!(result.title.is_none());
        assert!(result.original_title.is_none());
    }

    #[test]
    fn test_validate_result_season_episode_bounds() {
        let parser = FilenameParser::new();
        let parsed = ParsedFilename {
            season: Some(0),
            episode: Some(1001),
            confidence: 0.9,
            original_title: Some("Test".to_string()),
            ..Default::default()
        };
        let result = parser.validate_result(parsed).unwrap();
        assert!(result.season.is_none());
        assert!(result.episode.is_none());
    }

    // ========================================================================
    // strip_website_prefix 测试
    // ========================================================================

    #[test]
    fn test_strip_website_prefix_yangguang() {
        assert_eq!(
            strip_website_prefix("阳光电影dygod.org.世界大战.2025.BD.1080P.中英双字.mkv"),
            "世界大战.2025.BD.1080P.中英双字.mkv"
        );
        assert_eq!(
            strip_website_prefix("阳光电影dygod.org.伊甸.2024.BD.1080P.中英双字.mkv"),
            "伊甸.2024.BD.1080P.中英双字.mkv"
        );
        assert_eq!(
            strip_website_prefix("阳光电影dygod.org.制暴：无限杀机.2025.BD.1080P.中英双字.mkv"),
            "制暴：无限杀机.2025.BD.1080P.中英双字.mkv"
        );
    }

    #[test]
    fn test_strip_website_prefix_no_match() {
        // Normal filenames should not be modified
        assert_eq!(
            strip_website_prefix("Avatar.2009.1080p.BluRay.mkv"),
            "Avatar.2009.1080p.BluRay.mkv"
        );
        assert_eq!(
            strip_website_prefix("[七个会议] .mp4"),
            "[七个会议] .mp4"
        );
    }

    #[test]
    fn test_strip_website_prefix_generic_www() {
        assert_eq!(
            strip_website_prefix("www.example.com.世界大战.2025.mkv"),
            "世界大战.2025.mkv"
        );
    }
}

/// Extract season and episode numbers from filename using regex.
/// This avoids calling AI for each episode file.
///
/// Supports patterns like:
/// - "01.mp4", "02.mp4" (just episode number)
/// - "S01E01.mp4", "s01e05.mkv"
/// - "E01.mp4", "E05.mkv"
/// - "第01集.mp4", "第5集.mkv"
/// - "01 4K.mp4" (episode with quality suffix)
/// - "S02.Special.White.Christmas.mkv" (special episode -> Season 0)
pub fn extract_episode_from_filename(filename: &str) -> (Option<u16>, Option<u16>) {
    // Remove extension
    let name = filename
        .rsplit('.')
        .skip(1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(".");

    let name_lower = name.to_lowercase();

    // Pattern 0: S02.Special or SxxSpecial - treat as Season 0 (specials)
    // Must check BEFORE regular SxxExx pattern
    if let Some(caps) = regex_match_special(&name_lower) {
        return caps;
    }

    // Pattern 1: S01E01, s01e05
    if let Some(caps) = regex_match_sxxexx(&name_lower) {
        return caps;
    }

    // Pattern 2: Season 01 Episode 05
    if let Some(caps) = regex_match_season_episode(&name_lower) {
        return caps;
    }

    // Pattern 3: E01, E05 (episode only)
    if let Some(ep) = regex_match_exx(&name_lower) {
        return (Some(1), Some(ep)); // Default to season 1
    }

    // Pattern 4: 第01集, 第5集
    if let Some(ep) = regex_match_chinese_episode(&name) {
        return (Some(1), Some(ep)); // Default to season 1
    }

    // Pattern 5: Just a number at the start (01, 02, 1, 2)
    // Handle "01 4K.mp4", "02.mp4", etc.
    if let Some(ep) = regex_match_leading_number(&name) {
        return (Some(1), Some(ep)); // Default to season 1
    }

    // Pattern 6: Title-02, Title_02, Title.02, Title 02 (episode at end with separator)
    // Handle "不伦食堂-02.mp4", "今夜我用身体恋爱_02.mp4"
    if let Some(ep) = regex_match_trailing_episode(&name) {
        return (Some(1), Some(ep)); // Default to season 1
    }

    (None, None)
}

/// Match S02.Special, S02Special, etc. - returns Season 0
fn regex_match_special(s: &str) -> Option<(Option<u16>, Option<u16>)> {
    // Match patterns like "s02.special", "s02special", "s2.special"
    // Specials are placed in Season 0
    let re = regex::Regex::new(r"s(\d{1,2})[\.\s_-]?special").ok()?;
    if re.is_match(s) {
        // Found a special - return Season 0, Episode 1 (or we could try to extract a number)
        // Try to find a specific special number if present (e.g., "special.01")
        let ep_re = regex::Regex::new(r"special[\.\s_-]?(\d{1,2})").ok();
        let episode = ep_re
            .and_then(|re| re.captures(s))
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(1);

        return Some((Some(0), Some(episode))); // Season 0 for specials
    }

    // Also match standalone "special" or "special.01"
    // Must be a standalone word (preceded/followed by separator or start/end)
    // to avoid matching titles like "Special Delivery" or "The Special Agent"
    if !s.contains("e0") && !s.contains("e1") {
        let re = regex::Regex::new(r"(?:^|[\.\s_-])special(?:[\.\s_-]|$)").ok()?;
        if !re.is_match(s) {
            // Also check for "special" followed by a number
            let re_num = regex::Regex::new(r"(?:^|[\.\s_-])special[\.\s_-]?\d{0,2}(?:[\.\s_-]|$)").ok()?;
            if !re_num.is_match(s) {
                // Skip - not a standalone "special" pattern
            } else {
                let ep_re = regex::Regex::new(r"special[\.\s_-]?(\d{1,2})").ok();
                let episode = ep_re
                    .and_then(|re| re.captures(s))
                    .and_then(|c| c.get(1))
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(1);
                return Some((Some(0), Some(episode)));
            }
        } else {
            // Check if "special" is part of a longer word (e.g., "specialist", "specialized")
            // by looking at what follows after the separator
            let after_special = regex::Regex::new(r"(?:^|[\.\s_-])special([a-z])").ok()?;
            if after_special.is_match(s) {
                // "special" followed by a letter = part of a word, not a special marker
                // e.g., "specialist", "specialized"
            } else {
                let ep_re = regex::Regex::new(r"special[\.\s_-]?(\d{1,2})").ok();
                let episode = ep_re
                    .and_then(|re| re.captures(s))
                    .and_then(|c| c.get(1))
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(1);
                return Some((Some(0), Some(episode)));
            }
        }
    }

    // Match [sp] or [SP] pattern - common in fan-sub releases
    // e.g., "[EX8][2007](Galileo Season1)[sp][BDRIP]..."
    if regex::Regex::new(r"(?i)\[sp\]")
        .map(|re| re.is_match(s))
        .unwrap_or(false)
    {
        tracing::debug!("Detected [sp] special pattern in: {}", s);
        return Some((Some(0), Some(1))); // Season 0, Episode 1 for specials
    }

    // Match .sp. or _sp_ or -sp- pattern
    // e.g., "Show.SP.1080p" or "Show_sp_WEB"
    if regex::Regex::new(r"(?i)[\.\s_-]sp[\.\s_-]")
        .map(|re| re.is_match(s))
        .unwrap_or(false)
    {
        tracing::debug!("Detected .sp. special pattern in: {}", s);
        return Some((Some(0), Some(1)));
    }

    None
}

fn regex_match_sxxexx(s: &str) -> Option<(Option<u16>, Option<u16>)> {
    // Match S01E01, s1e5, etc.
    let re = regex::Regex::new(r"s(\d{1,2})e(\d{1,3})").ok()?;
    let caps = re.captures(s)?;
    let season: u16 = caps.get(1)?.as_str().parse().ok()?;
    let episode: u16 = caps.get(2)?.as_str().parse().ok()?;
    Some((Some(season), Some(episode)))
}

fn regex_match_season_episode(s: &str) -> Option<(Option<u16>, Option<u16>)> {
    // Match "season 01 episode 05", "season1 episode5"
    let re = regex::Regex::new(r"season\s*(\d{1,2}).*episode\s*(\d{1,3})").ok()?;
    let caps = re.captures(s)?;
    let season: u16 = caps.get(1)?.as_str().parse().ok()?;
    let episode: u16 = caps.get(2)?.as_str().parse().ok()?;
    Some((Some(season), Some(episode)))
}

fn regex_match_exx(s: &str) -> Option<u16> {
    // Match E01, e5, EP01, ep05
    let re = regex::Regex::new(r"(?:^|[^a-z])e[p]?(\d{1,3})(?:[^0-9]|$)").ok()?;
    let caps = re.captures(s)?;
    caps.get(1)?.as_str().parse().ok()
}

fn regex_match_chinese_episode(s: &str) -> Option<u16> {
    // Match 第01集, 第5集
    let re = regex::Regex::new(r"第(\d{1,3})集").ok()?;
    let caps = re.captures(s)?;
    caps.get(1)?.as_str().parse().ok()
}

fn regex_match_leading_number(s: &str) -> Option<u16> {
    // Match numbers at the start: "01", "02", "1", "2"
    // Also handles "01 4K", "02 1080p"
    let trimmed = s.trim();
    let re = regex::Regex::new(r"^(\d{1,3})(?:\s|$|[^0-9])").ok()?;
    let caps = re.captures(trimmed)?;
    let num: u16 = caps.get(1)?.as_str().parse().ok()?;
    // Sanity check: episode numbers are usually 1-999
    if (1..=999).contains(&num) {
        // Additional guard: if the number looks like a year (1900-2099), skip it
        // This prevents movies like "2001.A.Space.Odyssey.mkv" from being treated as episodes
        if (1900..=2099).contains(&(num as u32)) {
            return None;
        }
        // Additional guard: if the number is > 100, it's unlikely to be an episode number
        // unless it's part of a clear SxxExx pattern (already checked above)
        if num > 100 {
            return None;
        }
        Some(num)
    } else {
        None
    }
}

/// Match trailing episode number with separator.
///
/// Handles formats like:
/// - "不伦食堂-02" → episode 2
/// - "今夜我用身体恋爱_03" → episode 3
/// - "标题.05" → episode 5
/// - "Show Name 10" → episode 10
/// - "不伦食堂-04 end" → episode 4 (ignores "end" suffix)
/// - "幽灵 01_超清" → episode 1 (handles quality suffix after number)
/// - "那年青春我们正好02.1280高清" → episode 2 (CJK title + number + resolution)
/// - "孤芳不自赏02.1024高清" → episode 2 (CJK title + number + resolution)
fn regex_match_trailing_episode(s: &str) -> Option<u16> {
    let trimmed = s.trim();

    // Pattern 1: Episode number at end, optionally followed by "end/final"
    // Examples: "Show-04", "Title.05", "Show Name 10", "不伦食堂-04 end"
    let re1 = regex::Regex::new(r"[-_.\s](\d{1,3})(?:\s+(?:end|final|END|FINAL))?$").ok()?;
    if let Some(caps) = re1.captures(trimmed) {
        if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse::<u16>().ok()) {
            if (1..=999).contains(&num) && !(1900..=2099).contains(&(num as u32)) {
                return Some(num);
            }
        }
    }

    // Pattern 2: Episode number followed by separator and quality/description suffix
    // Examples: "幽灵 01_超清", "Show 02_HD", "Title 03 高清"
    // This handles CJK content where quality info comes after episode number
    let re2 = regex::Regex::new(r"[\s](\d{1,3})[_\s]").ok()?;
    if let Some(caps) = re2.captures(trimmed) {
        if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse::<u16>().ok()) {
            if (1..=999).contains(&num) && !(1900..=2099).contains(&(num as u32)) {
                return Some(num);
            }
        }
    }

    // Pattern 3: CJK title directly followed by episode number then dot and resolution
    // Examples: "那年青春我们正好02.1280高清未删减版", "孤芳不自赏02.1024高清"
    // Pattern: CJK char + 2-3 digit number + dot + 3-4 digit resolution
    let re3 =
        regex::Regex::new(r"[\u4e00-\u9fa5](\d{2,3})\.(?:1080|720|1024|1280|480|576|2160|4K|4k)")
            .ok()?;
    if let Some(caps) = re3.captures(trimmed) {
        if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse::<u16>().ok()) {
            if (1..=999).contains(&num) && !(1900..=2099).contains(&(num as u32)) {
                return Some(num);
            }
        }
    }

    // Pattern 4: CJK title directly followed by episode number at end
    // Examples: "[迅雷下载]那年青春我们正好02", "孤芳不自赏27"
    // Must end with CJK char + 2 digit number (to avoid matching years)
    let re4 = regex::Regex::new(r"[\u4e00-\u9fa5](\d{2})$").ok()?;
    if let Some(caps) = re4.captures(trimmed) {
        if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse::<u16>().ok()) {
            if (1..=99).contains(&num) {
                return Some(num);
            }
        }
    }

    None
}

/// Check if a filename matches the organized output format.
///
/// Organized formats:
/// - TV: `[Title]-S01E01-[Episode Name]-1080p-...`
/// - Movie: `[EnglishTitle][ChineseTitle](Year)-tt12345-tmdb67890-1080p-...`
pub fn is_organized_filename(filename: &str) -> bool {
    // TV show pattern: [Title]-S01E01-[...]
    let tv_pattern = regex::Regex::new(r"^\[.+\]-S\d{2}E\d{2,3}-\[.+\]-").ok();
    if let Some(re) = tv_pattern {
        if re.is_match(filename) {
            return true;
        }
    }

    // Movie pattern with TMDB ID: [Title](Year)-tt...-tmdb...-
    // or [Title][Title](Year)-tt...-tmdb...-
    let movie_pattern_with_id =
        regex::Regex::new(r"^\[.+\](?:\[.+\])?\(\d{4}\)-(?:tt\d+)?-?tmdb\d+-").ok();
    if let Some(re) = movie_pattern_with_id {
        if re.is_match(filename) {
            return true;
        }
    }

    // Movie pattern with technical info (no TMDB ID in filename):
    // [Title](Year)-Resolution-Format-Codec-BitDepth-Audio-Channels.ext
    // [Title][Title](Year)-Resolution-Format-Codec-BitDepth-Audio-Channels.ext
    // Examples:
    //   [Upgrade][升级](2018)-1080p-WEB-DL-h264-8bit-aac-2.0.mp4
    //   [焚城](2024)-2160p-WEB-DL-hevc-8bit-aac-2.0.mp4
    let movie_pattern_with_tech = regex::Regex::new(r"^\[.+\](?:\[.+\])?\(\d{4}\)-\d{3,4}p-").ok();
    if let Some(re) = movie_pattern_with_tech {
        if re.is_match(filename) {
            return true;
        }
    }

    false
}

/// Parse an organized TV show filename to extract metadata.
///
/// Format: `[Title]-S01E01-[Episode Name]-1080p-WEB-DL-...`
/// Returns: (title, season, episode, episode_name)
pub fn parse_organized_tv_series_filename(filename: &str) -> Option<OrganizedTvSeriesInfo> {
    let re = regex::Regex::new(r"^\[([^\]]+)\]-S(\d{2})E(\d{2,3})-\[([^\]]+)\]-").ok()?;

    let caps = re.captures(filename)?;

    Some(OrganizedTvSeriesInfo {
        title: caps.get(1)?.as_str().to_string(),
        season: caps.get(2)?.as_str().parse().ok()?,
        episode: caps.get(3)?.as_str().parse().ok()?,
        episode_name: caps.get(4)?.as_str().to_string(),
    })
}

/// Parse an organized movie filename to extract metadata.
///
/// Format: `[EnglishTitle][ChineseTitle](Year)-tt12345-tmdb67890-1080p-...`
/// or: `[Title](Year)-tt12345-tmdb67890-1080p-...`
/// Returns: (original_title, title, year, imdb_id, tmdb_id)
pub fn parse_organized_movie_filename(filename: &str) -> Option<OrganizedMovieInfo> {
    // Try two-title format with TMDB ID: [English][Chinese](Year)-ttIMDB-tmdbID-
    let re_two_with_id =
        regex::Regex::new(r"^\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-(?:tt(\d+))?-?tmdb(\d+)-").ok()?;

    if let Some(caps) = re_two_with_id.captures(filename) {
        return Some(OrganizedMovieInfo {
            original_title: Some(caps.get(1)?.as_str().to_string()),
            title: Some(caps.get(2)?.as_str().to_string()),
            year: caps.get(3)?.as_str().parse().ok()?,
            imdb_id: caps.get(4).map(|m| format!("tt{}", m.as_str())),
            tmdb_id: Some(caps.get(5)?.as_str().parse().ok()?),
        });
    }

    // Single-title format with TMDB ID: [Title](Year)-ttIMDB-tmdbID-
    let re_one_with_id =
        regex::Regex::new(r"^\[([^\]]+)\]\((\d{4})\)-(?:tt(\d+))?-?tmdb(\d+)-").ok()?;

    if let Some(caps) = re_one_with_id.captures(filename) {
        return Some(OrganizedMovieInfo {
            original_title: Some(caps.get(1)?.as_str().to_string()),
            title: None,
            year: caps.get(2)?.as_str().parse().ok()?,
            imdb_id: caps.get(3).map(|m| format!("tt{}", m.as_str())),
            tmdb_id: Some(caps.get(4)?.as_str().parse().ok()?),
        });
    }

    // Two-title format with technical info (no TMDB ID): [English][Chinese](Year)-Resolution-...
    // Example: [Upgrade][升级](2018)-1080p-WEB-DL-h264-8bit-aac-2.0.mp4
    let re_two_tech = regex::Regex::new(r"^\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-\d{3,4}p-").ok()?;

    if let Some(caps) = re_two_tech.captures(filename) {
        return Some(OrganizedMovieInfo {
            original_title: Some(caps.get(1)?.as_str().to_string()),
            title: Some(caps.get(2)?.as_str().to_string()),
            year: caps.get(3)?.as_str().parse().ok()?,
            imdb_id: None,
            tmdb_id: None, // Will be filled from parent folder
        });
    }

    // Single-title format with technical info: [Title](Year)-Resolution-...
    // Example: [焚城](2024)-2160p-WEB-DL-hevc-8bit-aac-2.0.mp4
    let re_one_tech = regex::Regex::new(r"^\[([^\]]+)\]\((\d{4})\)-\d{3,4}p-").ok()?;

    if let Some(caps) = re_one_tech.captures(filename) {
        return Some(OrganizedMovieInfo {
            original_title: Some(caps.get(1)?.as_str().to_string()),
            title: None,
            year: caps.get(2)?.as_str().parse().ok()?,
            imdb_id: None,
            tmdb_id: None, // Will be filled from parent folder
        });
    }

    None
}

/// Information extracted from an organized TV show filename.
#[derive(Debug, Clone)]
pub struct OrganizedTvSeriesInfo {
    pub title: String,
    pub season: u16,
    pub episode: u16,
    pub episode_name: String,
}

/// Information extracted from an organized movie filename.
#[derive(Debug, Clone)]
pub struct OrganizedMovieInfo {
    pub original_title: Option<String>,
    pub title: Option<String>,
    pub year: u16,
    pub imdb_id: Option<String>,
    /// TMDB ID. None if not present in filename (needs to be extracted from parent folder).
    pub tmdb_id: Option<u64>,
}

/// Information extracted from an organized TV show folder name.
#[derive(Debug, Clone)]
pub struct OrganizedTvSeriesFolderInfo {
    pub title: String,
    pub year: Option<u16>,
    pub imdb_id: Option<String>,
    pub tmdb_id: u64,
}

/// Information extracted from an organized movie folder name.
#[derive(Debug, Clone)]
pub struct OrganizedMovieFolderInfo {
    pub original_title: Option<String>,
    pub title: Option<String>,
    pub year: u16,
    pub imdb_id: Option<String>,
    pub tmdb_id: u64,
}

// ============================================================================
// Smart Metadata Extraction (Order-Independent)
// ============================================================================
//
// This module provides intelligent extraction of metadata from folder/file names
// without relying on strict format ordering. It identifies elements by their
// unique characteristics:
//
// - IMDB ID: `tt\d{7,9}` (highly unique pattern)
// - TMDB ID: `tmdb\d+` or number following IMDB ID
// - Year: standalone 4-digit number in range 1900-2099
// - Titles: content within square brackets `[...]`
// ============================================================================

/// Metadata extracted using smart pattern recognition (order-independent).
#[derive(Debug, Clone, Default)]
pub struct SmartExtractedMetadata {
    /// All titles found in square brackets, in order of appearance
    pub titles: Vec<String>,
    /// Year (4-digit number in valid range, not part of IDs)
    pub year: Option<u16>,
    /// IMDB ID (tt followed by 7-9 digits)
    pub imdb_id: Option<String>,
    /// TMDB ID (with or without tmdb prefix)
    pub tmdb_id: Option<u64>,
}

impl SmartExtractedMetadata {
    /// Check if we have minimum required data for movies (at least TMDB ID)
    pub fn has_movie_essentials(&self) -> bool {
        self.tmdb_id.is_some()
    }

    /// Check if we have minimum required data for TV shows (at least TMDB ID)
    pub fn has_tv_series_essentials(&self) -> bool {
        self.tmdb_id.is_some()
    }

    /// Get the primary title (first non-empty title, preferring Chinese)
    pub fn primary_title(&self) -> Option<String> {
        // First, try to find a Chinese title
        if let Some(chinese_title) = self.titles.iter().find(|t| contains_chinese(t)) {
            return Some(chinese_title.trim().to_string());
        }
        // Fall back to first title
        self.titles
            .first()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Get the original title
    pub fn original_title(&self) -> Option<String> {
        // Find the first non-Chinese title
        if let Some(original_title) = self.titles.iter().find(|t| !contains_chinese(t)) {
            return Some(original_title.trim().to_string());
        }
        // If all are Chinese, just take the first one
        self.titles
            .first()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

/// Extract metadata from a string using smart pattern recognition.
///
/// This function identifies elements by their unique characteristics,
/// independent of their order in the input string.
///
/// # Examples
///
/// ```
/// // All these formats will be correctly parsed:
/// // "2024-[不讨好的勇气]-[不讨好的勇气]-tt29510241-270853"
/// // "[Title](2024)-tt12345-tmdb67890"
/// // "[Title][中文](2024)-tt12345-67890"
/// // "tmdb12345-[Title]-2024"
/// ```
pub fn extract_smart_metadata(input: &str) -> SmartExtractedMetadata {
    let mut result = SmartExtractedMetadata::default();

    // 1. Extract IMDB ID (most unique pattern: tt followed by 7-9 digits)
    if let Ok(re) = regex::Regex::new(r"tt(\d{7,9})") {
        if let Some(caps) = re.captures(input) {
            if let Some(id) = caps.get(1) {
                result.imdb_id = Some(format!("tt{}", id.as_str()));
            }
        }
    }

    // 2. Extract TMDB ID
    // Priority 1: Explicit tmdb prefix
    if let Ok(re) = regex::Regex::new(r"tmdb(\d+)") {
        if let Some(caps) = re.captures(input) {
            if let Some(id) = caps.get(1) {
                result.tmdb_id = id.as_str().parse().ok();
            }
        }
    }

    // Priority 2: If no tmdb prefix, look for number after IMDB ID
    if result.tmdb_id.is_none() && result.imdb_id.is_some() {
        // Pattern: -tt12345-67890 or -tt12345678-123456
        if let Ok(re) = regex::Regex::new(r"tt\d{7,9}[^0-9]*(\d{5,8})") {
            if let Some(caps) = re.captures(input) {
                if let Some(id) = caps.get(1) {
                    let num: u64 = id.as_str().parse().unwrap_or(0);
                    // TMDB IDs are typically 5-8 digits
                    if (10000..=99999999).contains(&num) {
                        result.tmdb_id = Some(num);
                    }
                }
            }
        }
    }

    // 3. Extract all titles from square brackets
    if let Ok(re) = regex::Regex::new(r"\[([^\]]+)\]") {
        for caps in re.captures_iter(input) {
            if let Some(title) = caps.get(1) {
                let t = title.as_str().trim().to_string();
                if !t.is_empty() {
                    result.titles.push(t);
                }
            }
        }
    }

    // 4. Extract year (4-digit number in valid range, not part of IDs)
    // First, create a version of input with IDs masked
    let mut masked = input.to_string();

    // Mask IMDB ID
    if let Some(ref imdb) = result.imdb_id {
        masked = masked.replace(imdb, "XXXXXXXX");
    }

    // Mask TMDB ID (both with and without prefix)
    if let Some(tmdb) = result.tmdb_id {
        masked = masked.replace(&format!("tmdb{}", tmdb), "XXXXXXXX");
        masked = masked.replace(&tmdb.to_string(), "XXXXXXXX");
    }

    // Now find standalone year
    if let Ok(re) = regex::Regex::new(r"(?:^|[^0-9])(\d{4})(?:[^0-9]|$)") {
        for caps in re.captures_iter(&masked) {
            if let Some(year_match) = caps.get(1) {
                if let Ok(year) = year_match.as_str().parse::<u16>() {
                    if (1900..=2099).contains(&year) {
                        result.year = Some(year);
                        break; // Take the first valid year
                    }
                }
            }
        }
    }

    // Also try year in parentheses (common format)
    if result.year.is_none() {
        if let Ok(re) = regex::Regex::new(r"\((\d{4})\)") {
            if let Some(caps) = re.captures(input) {
                if let Some(year_match) = caps.get(1) {
                    if let Ok(year) = year_match.as_str().parse::<u16>() {
                        if (1900..=2099).contains(&year) {
                            result.year = Some(year);
                        }
                    }
                }
            }
        }
    }

    result
}

/// Parse an organized movie folder name to extract metadata.
///
/// Supported formats:
/// - `[Title](Year)-ttIMDB-tmdbID` - single title
/// - `[OriginalTitle][ChineseTitle](Year)-ttIMDB-tmdbID` - dual title
///
/// Examples:
/// - `[Upgrade][升级](2018)-tt6499752-tmdb500664`
/// - `[焚城](2024)-tt29495090-tmdb1305642`
pub fn parse_organized_movie_folder(dirname: &str) -> Option<OrganizedMovieFolderInfo> {
    // Pattern 0: Category prefix + Dual title: [C][Original][Chinese](Year)-ttIMDB-tmdbID
    // Where C is a single character category code (e.g., [B], [H], [S])
    let re_category_dual =
        regex::Regex::new(r"^\[([A-Za-z])\]\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_category_dual.captures(dirname) {
        let _category = caps.get(1)?.as_str();
        let original_title = caps.get(2)?.as_str();
        let title = caps.get(3)?.as_str();
        return Some(OrganizedMovieFolderInfo {
            original_title: Some(original_title.to_string()),
            title: Some(title.to_string()),
            year: caps.get(4)?.as_str().parse().ok()?,
            imdb_id: Some(format!("tt{}", caps.get(5)?.as_str())),
            tmdb_id: caps.get(6)?.as_str().parse().ok()?,
        });
    }

    // Pattern 1: Category prefix + Single title: [C][Title](Year)-ttIMDB-tmdbID
    // Where C is a single character category code (e.g., [B], [H], [S])
    let re_category_single =
        regex::Regex::new(r"^\[([A-Za-z])\]\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_category_single.captures(dirname) {
        let _category = caps.get(1)?.as_str();
        let title = caps.get(2)?.as_str();
        // Category prefix is single char, so treat this as single title format
        return Some(OrganizedMovieFolderInfo {
            original_title: Some(title.to_string()),
            title: None,
            year: caps.get(3)?.as_str().parse().ok()?,
            imdb_id: Some(format!("tt{}", caps.get(4)?.as_str())),
            tmdb_id: caps.get(5)?.as_str().parse().ok()?,
        });
    }

    // Pattern 2: Dual title: [Original][Chinese](Year)-ttIMDB-tmdbID
    let re_dual =
        regex::Regex::new(r"^\[([^\]]+)\]\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_dual.captures(dirname) {
        return Some(OrganizedMovieFolderInfo {
            original_title: Some(caps.get(1)?.as_str().to_string()),
            title: Some(caps.get(2)?.as_str().to_string()),
            year: caps.get(3)?.as_str().parse().ok()?,
            imdb_id: Some(format!("tt{}", caps.get(4)?.as_str())),
            tmdb_id: caps.get(5)?.as_str().parse().ok()?,
        });
    }

    // Pattern 2: Single title: [Title](Year)-ttIMDB-tmdbID
    let re_single = regex::Regex::new(r"^\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_single.captures(dirname) {
        return Some(OrganizedMovieFolderInfo {
            original_title: Some(caps.get(1)?.as_str().to_string()),
            title: None,
            year: caps.get(2)?.as_str().parse().ok()?,
            imdb_id: Some(format!("tt{}", caps.get(3)?.as_str())),
            tmdb_id: caps.get(4)?.as_str().parse().ok()?,
        });
    }

    // Pattern 3: Single title without IMDB: [Title](Year)-tmdbID
    let re_single_no_imdb = regex::Regex::new(r"^\[([^\]]+)\]\((\d{4})\)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_single_no_imdb.captures(dirname) {
        return Some(OrganizedMovieFolderInfo {
            original_title: Some(caps.get(1)?.as_str().to_string()),
            title: None,
            year: caps.get(2)?.as_str().parse().ok()?,
            imdb_id: None,
            tmdb_id: caps.get(3)?.as_str().parse().ok()?,
        });
    }

    // ========================================================================
    // FALLBACK: Smart extraction (order-independent)
    // ========================================================================
    // If strict patterns fail, try intelligent extraction based on unique
    // characteristics of each element (IMDB ID, TMDB ID, year, titles).
    // This handles non-standard formats like:
    // - "2024-[Title]-tt12345-67890"
    // - "[Title]-2024-tt12345-tmdb67890"
    // ========================================================================

    let smart = extract_smart_metadata(dirname);

    // Must have at least TMDB ID and year for movies
    if let (Some(tmdb_id), Some(year)) = (smart.tmdb_id, smart.year) {
        tracing::debug!(
            "[SMART] Movie folder extracted: tmdb={}, year={}, imdb={:?}, titles={:?}",
            tmdb_id,
            year,
            smart.imdb_id,
            smart.titles
        );

        // Get primary (usually Chinese) title
        let primary_title = smart.primary_title();
        
        // Get original title - make sure it's not the same as primary title
        let original_title = smart.original_title();
        
        let title = if primary_title != original_title {
            primary_title.clone()
        } else {
            // If they are the same, just set to None
            None
        };
        
        return Some(OrganizedMovieFolderInfo {
            original_title,
            title,
            year,
            imdb_id: smart.imdb_id,
            tmdb_id,
        });
    }

    None
}

/// Check if a folder name matches the organized TV show folder format.
///
/// This function uses both strict pattern matching and smart extraction
/// to detect organized folders in various formats.
///
/// Supported formats (strict):
/// - `[Title](Year)-ttIMDB-tmdbID`
/// - `[Title](Year)-tmdbID`
///
/// Also detected via smart extraction:
/// - `Year-[Title]-tt...-tmdbID` or similar variations
pub fn is_organized_tv_series_folder(dirname: &str) -> bool {
    // Fast path: strict pattern matching
    let re = regex::Regex::new(r"^\[.+\]\(\d{4}\)-(?:tt\d+)?-?tmdb\d+$").ok();
    if let Some(re) = re {
        if re.is_match(dirname) {
            return true;
        }
    }

    // Slow path: smart extraction can identify it
    let smart = extract_smart_metadata(dirname);
    smart.has_tv_series_essentials() && !smart.titles.is_empty()
}

/// Parse an organized TV show folder name to extract metadata.
///
/// Supported formats:
/// - `[Title](Year)-ttIMDB-tmdbID` - single title with IMDB
/// - `[Title](Year)-tmdbID` - single title without IMDB
/// - `[OriginalTitle][ChineseTitle](Year)-ttIMDB-tmdbID` - dual title with IMDB
/// - `[OriginalTitle][ChineseTitle]-ttIMDB-tmdbID` - dual title without year
///
/// Examples:
/// - `[罚罪2](2025)-tt36771056-tmdb296146`
/// - `[러브 미][爱我](2025)-tt35451747-tmdb275989`
/// - `[러브 미][ ]-tt35451747-tmdb275989` (empty Chinese title)
pub fn parse_organized_tv_series_folder(dirname: &str) -> Option<OrganizedTvSeriesFolderInfo> {
    // Pattern 1: Dual title with year and IMDB: [Original][Chinese](Year)-ttIMDB-tmdbID
    let re_dual_with_year_imdb =
        regex::Regex::new(r"^\[([^\]]+)\]\[([^\]]*)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_dual_with_year_imdb.captures(dirname) {
        let original = caps.get(1)?.as_str().to_string();
        let chinese = caps.get(2)?.as_str().trim().to_string();
        let title = if chinese.is_empty() {
            original.clone()
        } else {
            chinese
        };
        return Some(OrganizedTvSeriesFolderInfo {
            title,
            year: caps.get(3)?.as_str().parse().ok(),
            imdb_id: Some(format!("tt{}", caps.get(4)?.as_str())),
            tmdb_id: caps.get(5)?.as_str().parse().ok()?,
        });
    }

    // Pattern 2: Dual title without year: [Original][Chinese]-ttIMDB-tmdbID
    let re_dual_no_year =
        regex::Regex::new(r"^\[([^\]]+)\]\[([^\]]*)\]-tt(\d+)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_dual_no_year.captures(dirname) {
        let original = caps.get(1)?.as_str().to_string();
        let chinese = caps.get(2)?.as_str().trim().to_string();
        let title = if chinese.is_empty() {
            original.clone()
        } else {
            chinese
        };
        return Some(OrganizedTvSeriesFolderInfo {
            title,
            year: None,
            imdb_id: Some(format!("tt{}", caps.get(3)?.as_str())),
            tmdb_id: caps.get(4)?.as_str().parse().ok()?,
        });
    }

    // Pattern 3: Single title with year and IMDB: [Title](Year)-ttIMDB-tmdbID
    let re_with_imdb = regex::Regex::new(r"^\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_with_imdb.captures(dirname) {
        return Some(OrganizedTvSeriesFolderInfo {
            title: caps.get(1)?.as_str().to_string(),
            year: caps.get(2)?.as_str().parse().ok(),
            imdb_id: Some(format!("tt{}", caps.get(3)?.as_str())),
            tmdb_id: caps.get(4)?.as_str().parse().ok()?,
        });
    }

    // Pattern 4: Single title without IMDB: [Title](Year)-tmdbID
    let re_no_imdb = regex::Regex::new(r"^\[([^\]]+)\]\((\d{4})\)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_no_imdb.captures(dirname) {
        return Some(OrganizedTvSeriesFolderInfo {
            title: caps.get(1)?.as_str().to_string(),
            year: caps.get(2)?.as_str().parse().ok(),
            imdb_id: None,
            tmdb_id: caps.get(3)?.as_str().parse().ok()?,
        });
    }

    // ========================================================================
    // FALLBACK: Smart extraction (order-independent)
    // ========================================================================
    // If strict patterns fail, try intelligent extraction based on unique
    // characteristics of each element (IMDB ID, TMDB ID, year, titles).
    // This handles non-standard formats like:
    // - "2024-[Title]-[Title]-tt12345-67890"
    // - "[Title]-2024-tt12345-tmdb67890"
    // ========================================================================

    let smart = extract_smart_metadata(dirname);

    // Must have at least TMDB ID for TV shows
    if let Some(tmdb_id) = smart.tmdb_id {
        // Get primary title (prefer second title if available, as it's usually Chinese)
        let title = smart
            .primary_title()
            .unwrap_or_else(|| "Unknown".to_string());

        tracing::debug!(
            "[SMART] TV folder extracted: tmdb={}, year={:?}, imdb={:?}, title={}",
            tmdb_id,
            smart.year,
            smart.imdb_id,
            title
        );

        return Some(OrganizedTvSeriesFolderInfo {
            title,
            year: smart.year,
            imdb_id: smart.imdb_id,
            tmdb_id,
        });
    }

    None
}

/// Convert OrganizedTvSeriesInfo to ParsedFilename for consistent processing.
impl From<OrganizedTvSeriesInfo> for ParsedFilename {
    fn from(info: OrganizedTvSeriesInfo) -> Self {
        ParsedFilename {
            original_title: None,
            title: Some(info.title),
            year: None,
            season: Some(info.season),
            episode: Some(info.episode),
            confidence: 1.0, // High confidence since we parsed our own format
            raw_response: None,
        }
    }
}

/// Convert OrganizedMovieInfo to ParsedFilename for consistent processing.
impl From<OrganizedMovieInfo> for ParsedFilename {
    fn from(info: OrganizedMovieInfo) -> Self {
        ParsedFilename {
            original_title: info.original_title,
            title: info.title,
            year: Some(info.year),
            season: None,
            episode: None,
            confidence: 1.0, // High confidence since we parsed our own format
            raw_response: None,
        }
    }
}

/// Extract season number from directory name.
/// Supports Chinese season patterns like:
/// - "第一季", "第二季", "第1季", "第2季"
/// - "Season 01", "Season 1", "S01", "S1"
/// - "第一部", "第二部" (treated as seasons)
pub fn extract_season_from_dirname(dirname: &str) -> Option<u16> {
    let name = dirname.trim();

    // Chinese numeral to number mapping
    let chinese_nums = [
        ("一", 1),
        ("二", 2),
        ("三", 3),
        ("四", 4),
        ("五", 5),
        ("六", 6),
        ("七", 7),
        ("八", 8),
        ("九", 9),
        ("十", 10),
        ("十一", 11),
        ("十二", 12),
        ("十三", 13),
        ("十四", 14),
        ("十五", 15),
    ];

    // Pattern 1: "第X季" or "第X部" with Chinese numerals
    for (cn, num) in &chinese_nums {
        if name.contains(&format!("第{}季", cn)) || name.contains(&format!("第{}部", cn)) {
            return Some(*num);
        }
    }

    // Pattern 2: "第N季" with Arabic numerals
    if let Ok(re) = regex::Regex::new(r"第(\d{1,2})季") {
        if let Some(caps) = re.captures(name) {
            if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                return Some(num);
            }
        }
    }

    // Pattern 3: "Season N", "Season 0N"
    if let Ok(re) = regex::Regex::new(r"(?i)season\s*(\d{1,2})") {
        if let Some(caps) = re.captures(name) {
            if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                return Some(num);
            }
        }
    }

    // Pattern 4: "S01", "S1" at end or with space
    if let Ok(re) = regex::Regex::new(r"(?i)(?:^|[\s\-_])s(\d{1,2})(?:$|[\s\-_])") {
        if let Some(caps) = re.captures(name) {
            if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                return Some(num);
            }
        }
    }

    None
}

/// Strip website/source prefixes from filenames.
///
/// Common patterns from Chinese download sites:
/// - "阳光电影dygod.org.世界大战.2025..." -> "世界大战.2025..."
/// - "阳光电影dygod.org.伊甸.2024..." -> "伊甸.2024..."
/// - "电影天堂www.dy2018.世界大战.2025..." -> "世界大战.2025..."
/// - "人人影视.世界大战.2025..." -> "世界大战.2025..."
fn strip_website_prefix(filename: &str) -> String {
    // Common website prefix patterns (case-insensitive)
    // These are download site names that appear at the start of filenames
    let prefix_patterns = [
        // Pattern: site name followed by domain or separator
        r"(?i)^阳光电影(?:dygod\.org)?[\.\s_-]+",
        r"(?i)^电影天堂(?:www\.dy2018\.com)?[\.\s_-]+",
        r"(?i)^人人影视[\.\s_-]+",
        r"(?i)^电影天堂[\.\s_-]+",
        r"(?i)^BT天堂[\.\s_-]+",
        r"(?i)^6v电影[\.\s_-]+",
        r"(?i)^6v\.com[\.\s_-]+",
        r"(?i)^影视帝国[\.\s_-]+",
        r"(?i)^破晓电影[\.\s_-]+",
        r"(?i)^rarbg[\.\s_-]+",
        r"(?i)^www\.[a-zA-Z0-9]+\.(?:com|org|net|cn)[\.\s_-]+",
    ];

    for pattern in &prefix_patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            let stripped = re.replace(filename, "").to_string();
            if stripped != filename && !stripped.is_empty() {
                tracing::debug!(
                    "Stripped website prefix: '{}' -> '{}'",
                    filename,
                    stripped
                );
                return stripped;
            }
        }
    }

    filename.to_string()
}

/// Filter out subtitle group names from title.
///
/// Returns None if the entire title is a subtitle group name,
/// otherwise returns the cleaned title (if different) or the original.
///
/// Common subtitle groups:
/// - Chinese: 霸王龙压制组, 字幕侠, FIX字幕侠, 人人影视, 追新番, 擦枪字幕组
/// - English: T-Rex, YYeTs, rarbg, DEFLATE, ZeroTV, NF, AMZN
fn filter_subtitle_group(title: &str) -> Option<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Common subtitle group patterns (case-insensitive for English)
    let subtitle_groups_exact = [
        // Chinese subtitle groups (exact match)
        "霸王龙压制组",
        "霸王龙压制组T-Rex",
        "霸王龙",
        "T-Rex",
        "字幕侠",
        "FIX字幕侠",
        "人人影视",
        "YYeTs",
        "追新番",
        "ZhuixinFan",
        "擦枪字幕组",
        "CMCT",
        "官方中字",
        "中英双字",
        "中字",
        // Common release groups (exact match)
        "rarbg",
        "DEFLATE",
        "ZeroTV",
        "NF",
        "AMZN",
        "HMAX",
        "DnO",
        "Coo7",
        "EX8",
        "huanyuezmz",
        "TheTaoSong",
    ];

    let lower = trimmed.to_lowercase();

    // Check for exact match with subtitle group
    for group in &subtitle_groups_exact {
        if lower == group.to_lowercase() {
            tracing::debug!("Filtering out subtitle group as title: '{}'", trimmed);
            return None;
        }
    }

    // Check if title contains mostly subtitle group patterns
    let contains_patterns = [
        "压制组",
        "字幕组",
        "字幕侠",
        "人人影视",
        "rarbg",
        "deflate",
        "zerotv",
    ];

    for pattern in &contains_patterns {
        if lower.contains(&pattern.to_lowercase()) {
            // If the title is primarily a subtitle group reference
            if trimmed.len() < 20 {
                tracing::debug!("Filtering out subtitle group pattern: '{}'", trimmed);
                return None;
            }
        }
    }

    Some(trimmed.to_string())
}
