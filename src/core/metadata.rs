//! Metadata extraction module.
//!
//! This module provides a unified approach to extracting metadata from video files,
//! implementing the following processing phases:
//!
//! 1. **File type detection**: Check if file is already organized format
//! 2. **Information collection**: Extract info from filename and directory
//! 3. **AI augmentation**: Use AI parsing when needed
//! 4. **TMDB matching**: Match and validate against TMDB
//! 5. **Decision**: Validate match quality, skip if uncertain

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Source of metadata information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MetadataSource {
    /// Extracted from already-organized filename format
    OrganizedFilename,
    /// Extracted from already-organized folder format
    OrganizedFolder,
    /// Extracted from filename using regex
    FilenameRegex,
    /// Extracted from directory name
    DirectoryName,
    /// Obtained from AI parsing
    AiParsing,
    /// Merged from multiple sources
    Merged,
}

/// Candidate metadata extracted before TMDB validation.
///
/// This structure collects all available information from various sources
/// before querying TMDB for validation and enrichment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CandidateMetadata {
    /// Chinese title (if found)
    pub chinese_title: Option<String>,
    /// English/original title (if found)
    pub english_title: Option<String>,
    /// Release year
    pub year: Option<u16>,
    /// Season number (TV shows only)
    pub season: Option<u16>,
    /// Episode number (TV shows only)
    pub episode: Option<u16>,
    /// TMDB ID (if already known from organized format)
    pub tmdb_id: Option<u64>,
    /// IMDB ID (if already known from organized format)
    pub imdb_id: Option<String>,
    /// Source of metadata
    pub source: Option<MetadataSource>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Whether AI parsing is needed
    pub needs_ai_parsing: bool,
}

impl CandidateMetadata {
    /// Check if this metadata has enough information to search TMDB.
    pub fn has_searchable_info(&self) -> bool {
        self.chinese_title.is_some() || self.english_title.is_some()
    }

    /// Check if this metadata already has a TMDB ID (fast path).
    pub fn has_tmdb_id(&self) -> bool {
        self.tmdb_id.is_some()
    }

    /// Get the best title for display.
    pub fn display_title(&self) -> Option<String> {
        self.chinese_title
            .clone()
            .or_else(|| self.english_title.clone())
    }

    /// Check if AI parsing is needed.
    ///
    /// AI parsing is needed when:
    /// - No title found from filename/directory
    /// - Title looks like codec/technical info
    pub fn should_use_ai(&self) -> bool {
        if self.has_tmdb_id() {
            return false; // Already have ID, no need for AI
        }
        if self.needs_ai_parsing {
            return true;
        }
        !self.has_searchable_info()
    }

    /// Merge with AI parsing result.
    pub fn merge_ai_result(&mut self, ai_result: &super::parser::ParsedFilename) {
        // Only update if AI provided better info
        if self.chinese_title.is_none() && ai_result.title.is_some() {
            self.chinese_title = ai_result.title.clone();
        }
        if self.english_title.is_none() && ai_result.original_title.is_some() {
            self.english_title = ai_result.original_title.clone();
        }
        if self.year.is_none() && ai_result.year.is_some() {
            self.year = ai_result.year;
        }
        if self.season.is_none() && ai_result.season.is_some() {
            self.season = ai_result.season;
        }
        if self.episode.is_none() && ai_result.episode.is_some() {
            self.episode = ai_result.episode;
        }
        // Update confidence based on AI result
        self.confidence = self.confidence.max(ai_result.confidence);
        self.source = Some(MetadataSource::Merged);
    }
}

/// Directory type classification.
///
/// Used to understand the semantic meaning of directory names
/// in the video file path.
#[derive(Debug, Clone, PartialEq)]
pub enum DirectoryType {
    /// Title directory: contains work name (e.g., "Breaking Bad (2008)")
    TitleDirectory(TitleInfo),
    /// Season directory (e.g., "Season 01", "第一季")
    SeasonDirectory(u16),
    /// Quality/technical directory (e.g., "4K", "1080P", "BluRay")
    QualityDirectory,
    /// Category directory (e.g., "韩剧", "2024", "Marvel")
    CategoryDirectory(CategoryType),
    /// Already organized directory with TMDB ID
    OrganizedDirectory(OrganizedInfo),
    /// Unknown directory type
    Unknown,
}

/// Title information extracted from directory name.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleInfo {
    /// Chinese title
    pub chinese_title: Option<String>,
    /// English title
    pub english_title: Option<String>,
    /// Year
    pub year: Option<u16>,
}

/// Category type for category directories.
#[derive(Debug, Clone, PartialEq)]
pub enum CategoryType {
    /// Region category (e.g., "韩剧", "美剧", "日剧")
    Region(String),
    /// Year category (e.g., "2024", "2024年")
    Year(u16),
    /// Genre category (e.g., "电影", "综艺", "动漫")
    Genre(String),
    /// Series/Collection category (e.g., "漫威", "DC", "哈利波特")
    Series(String),
    /// Actor/Director category (e.g., "周星驰")
    Person(String),
}

/// Information from organized directory.
#[derive(Debug, Clone, PartialEq)]
pub struct OrganizedInfo {
    /// Title
    pub title: String,
    /// Year
    pub year: Option<u16>,
    /// TMDB ID
    pub tmdb_id: u64,
    /// IMDB ID (optional)
    pub imdb_id: Option<String>,
}

/// Classify a directory name to determine its type.
///
/// This function analyzes directory names to understand their semantic meaning,
/// which helps in extracting metadata from the file path.
pub fn classify_directory(dirname: &str) -> DirectoryType {
    let name = dirname.trim();

    // Check if it's an organized directory first
    if let Some(info) = parse_organized_directory(name) {
        return DirectoryType::OrganizedDirectory(info);
    }

    // Check if it's a season directory
    if let Some(season) = extract_season_number(name) {
        return DirectoryType::SeasonDirectory(season);
    }

    // Check if it's a quality/technical directory
    if is_quality_directory(name) {
        return DirectoryType::QualityDirectory;
    }

    // Check if it's a category directory
    if let Some(category) = classify_as_category(name) {
        return DirectoryType::CategoryDirectory(category);
    }

    // Check if it's a title directory
    if let Some(title_info) = extract_title_from_dirname(name) {
        return DirectoryType::TitleDirectory(title_info);
    }

    DirectoryType::Unknown
}

/// Parse an organized directory format.
///
/// Formats:
/// - `[Title](Year)-ttIMDB-tmdbID`
/// - `[Title](Year)-tmdbID`
fn parse_organized_directory(name: &str) -> Option<OrganizedInfo> {
    // Pattern with IMDB: [Title](Year)-ttIMDB-tmdbID
    let re_with_imdb = regex::Regex::new(r"^\[([^\]]+)\]\((\d{4})\)-tt(\d+)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_with_imdb.captures(name) {
        return Some(OrganizedInfo {
            title: caps.get(1)?.as_str().to_string(),
            year: caps.get(2)?.as_str().parse().ok(),
            imdb_id: Some(format!("tt{}", caps.get(3)?.as_str())),
            tmdb_id: caps.get(4)?.as_str().parse().ok()?,
        });
    }

    // Pattern without IMDB: [Title](Year)-tmdbID
    let re_no_imdb = regex::Regex::new(r"^\[([^\]]+)\]\((\d{4})\)-tmdb(\d+)$").ok()?;

    if let Some(caps) = re_no_imdb.captures(name) {
        return Some(OrganizedInfo {
            title: caps.get(1)?.as_str().to_string(),
            year: caps.get(2)?.as_str().parse().ok(),
            imdb_id: None,
            tmdb_id: caps.get(3)?.as_str().parse().ok()?,
        });
    }

    None
}

/// Extract season number from directory name.
fn extract_season_number(name: &str) -> Option<u16> {
    let name_lower = name.to_lowercase();

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

    // Pattern: "第X季" or "第X部" with Chinese numerals
    for (cn, num) in &chinese_nums {
        if name.contains(&format!("第{}季", cn)) || name.contains(&format!("第{}部", cn)) {
            return Some(*num);
        }
    }

    // Pattern: "第N季" with Arabic numerals
    if let Ok(re) = regex::Regex::new(r"第(\d{1,2})季") {
        if let Some(caps) = re.captures(name) {
            if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                return Some(num);
            }
        }
    }

    // Pattern: "Season N", "Season 0N"
    if let Ok(re) = regex::Regex::new(r"(?i)^season\s*(\d{1,2})$") {
        if let Some(caps) = re.captures(&name_lower) {
            if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                return Some(num);
            }
        }
    }

    // Pattern: "S01", "S1" (standalone)
    if let Ok(re) = regex::Regex::new(r"(?i)^s(\d{1,2})$") {
        if let Some(caps) = re.captures(&name_lower) {
            if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                return Some(num);
            }
        }
    }

    None
}

/// Check if directory name is a quality/technical directory.
fn is_quality_directory(name: &str) -> bool {
    let name_lower = name.to_lowercase();

    // Exact match patterns - directory name must exactly match one of these
    let exact_patterns = [
        "4k",
        "2160p",
        "1080p",
        "720p",
        "480p",
        "uhd",
        "bluray",
        "blu-ray",
        "bdrip",
        "brrip",
        "dvdrip",
        "hdtv",
        "web-dl",
        "webdl",
        "webrip",
        "hdrip",
        "内封字幕",
        "外挂字幕",
        "中字",
        "内嵌字幕",
        "hevc",
        "x265",
        "h265",
        "x264",
        "h264",
        "remux",
        "dts",
        "truehd",
        "atmos",
    ];

    // Use exact matching to avoid false positives
    // e.g., "dts" should not match "results" or "outskirts"
    for pattern in &exact_patterns {
        if name_lower == *pattern {
            return true;
        }
    }

    // For compound names (e.g., "1080p.WEB-DL", "4K BluRay"), check if the name
    // is primarily composed of quality descriptors (name must be short)
    if name_lower.len() < 15 {
        // Check if name contains a quality pattern as a word
        for pattern in &exact_patterns {
            if name_lower.contains(pattern) {
                return true;
            }
        }
    }

    false
}

/// Classify directory as a category type.
fn classify_as_category(name: &str) -> Option<CategoryType> {
    // Region patterns (Chinese names for drama regions)
    let region_patterns = [
        ("韩剧", "KR"),
        ("韩国", "KR"),
        ("韩影", "KR"),
        ("美剧", "US"),
        ("美国", "US"),
        ("美影", "US"),
        ("日剧", "JP"),
        ("日本", "JP"),
        ("日影", "JP"),
        ("日番", "JP"),
        ("英剧", "GB"),
        ("英国", "GB"),
        ("国产剧", "CN"),
        ("国产", "CN"),
        ("大陆", "CN"),
        ("内地", "CN"),
        ("港剧", "HK"),
        ("香港", "HK"),
        ("台剧", "TW"),
        ("台湾", "TW"),
        ("泰剧", "TH"),
        ("泰国", "TH"),
    ];

    for (pattern, code) in &region_patterns {
        if name.contains(pattern) || name == *pattern {
            return Some(CategoryType::Region(code.to_string()));
        }
    }

    // Year patterns
    if let Ok(re) = regex::Regex::new(r"^(\d{4})(?:年)?(?:新番)?$") {
        if let Some(caps) = re.captures(name) {
            if let Some(year) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                if (1900..=2100).contains(&year) {
                    return Some(CategoryType::Year(year));
                }
            }
        }
    }

    // Genre patterns
    let genre_patterns = [
        "电影",
        "电视剧",
        "剧集",
        "综艺",
        "动漫",
        "动画",
        "纪录片",
        "movies",
        "tv_series",
        "tv shows",
        "anime",
        "documentary",
    ];

    for pattern in &genre_patterns {
        if name.to_lowercase().contains(pattern) {
            return Some(CategoryType::Genre(pattern.to_string()));
        }
    }

    // Series/Collection patterns
    let series_patterns = [
        "漫威",
        "marvel",
        "dc",
        "星球大战",
        "star wars",
        "哈利波特",
        "harry potter",
        "指环王",
        "lord of the rings",
        "变形金刚",
        "transformers",
    ];

    for pattern in &series_patterns {
        if name.to_lowercase().contains(pattern) {
            return Some(CategoryType::Series(pattern.to_string()));
        }
    }

    None
}

/// Extract title information from directory name.
///
/// Title directory formats:
/// - `中文标题 English Title (2024)`
/// - `中文标题 (2024)`
/// - `English Title (2024)`
/// - `中文标题.English.Title.2024`
fn extract_title_from_dirname(name: &str) -> Option<TitleInfo> {
    // Skip if too short
    if name.len() < 2 {
        return None;
    }

    // Pattern: Title (Year)
    if let Ok(re) = regex::Regex::new(r"^(.+?)\s*[\(\[（](\d{4})[\)\]）]$") {
        if let Some(caps) = re.captures(name) {
            let title_part = caps.get(1)?.as_str().trim();
            let year: u16 = caps.get(2)?.as_str().parse().ok()?;

            let (chinese, english) = split_chinese_english_title(title_part);

            return Some(TitleInfo {
                chinese_title: chinese,
                english_title: english,
                year: Some(year),
            });
        }
    }

    // Pattern: Title.Year (dots as separators)
    if let Ok(re) = regex::Regex::new(r"^(.+?)\.(\d{4})(?:\.|$)") {
        if let Some(caps) = re.captures(name) {
            let title_part = caps.get(1)?.as_str().replace('.', " ").trim().to_string();
            let year: u16 = caps.get(2)?.as_str().parse().ok()?;

            // Check if title looks like a real title (not just technical info)
            if !is_quality_directory(&title_part) {
                let (chinese, english) = split_chinese_english_title(&title_part);

                return Some(TitleInfo {
                    chinese_title: chinese,
                    english_title: english,
                    year: Some(year),
                });
            }
        }
    }

    // Pattern: Just a title without year (if it has Chinese characters)
    let has_chinese = name.chars().any(|c| {
        let u = c as u32;
        (0x4E00..=0x9FFF).contains(&u) || // CJK Unified Ideographs
        (0x3400..=0x4DBF).contains(&u) // CJK Extension A
    });

    if has_chinese && name.len() >= 2 {
        let (chinese, english) = split_chinese_english_title(name);
        if chinese.is_some() || english.is_some() {
            return Some(TitleInfo {
                chinese_title: chinese,
                english_title: english,
                year: None,
            });
        }
    }

    None
}

/// Split a mixed Chinese-English title into separate parts.
fn split_chinese_english_title(title: &str) -> (Option<String>, Option<String>) {
    let mut chinese_chars = String::new();
    let mut english_chars = String::new();

    for c in title.chars() {
        let u = c as u32;
        if (0x4E00..=0x9FFF).contains(&u) || // CJK Unified Ideographs
           (0x3400..=0x4DBF).contains(&u) || // CJK Extension A
           c == '：' || c == '·'
        {
            chinese_chars.push(c);
        } else if c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '\'' || c == ':' {
            english_chars.push(c);
        }
    }

    let chinese = chinese_chars.trim().to_string();
    let english = english_chars.trim().to_string();

    (
        if chinese.is_empty() {
            None
        } else {
            Some(chinese)
        },
        if english.is_empty() {
            None
        } else {
            Some(english)
        },
    )
}

/// Find the first TitleDirectory or OrganizedDirectory by traversing up from a path.
///
/// This is useful for files in nested directory structures like:
/// `Movies/漫威/复仇者联盟 (2012)/1080p/movie.mkv`
pub fn find_title_directory(path: &Path) -> Option<(DirectoryType, std::path::PathBuf)> {
    let mut current = path.to_path_buf();

    while let Some(parent) = current.parent() {
        if let Some(name) = current.file_name().and_then(|n| n.to_str()) {
            let dir_type = classify_directory(name);
            match dir_type {
                DirectoryType::TitleDirectory(_) | DirectoryType::OrganizedDirectory(_) => {
                    return Some((dir_type, current.clone()));
                }
                _ => {}
            }
        }
        current = parent.to_path_buf();
    }

    None
}

/// Extract metadata from filename using regex (no AI).
///
/// This function extracts information from common filename patterns:
/// - Episode numbers (S01E01, E01, 01, 第1集)
/// - Year (2024)
/// - Resolution (1080p, 4K)
/// - Titles (when clearly separated)
pub fn extract_from_filename(filename: &str) -> CandidateMetadata {
    let mut metadata = CandidateMetadata {
        source: Some(MetadataSource::FilenameRegex),
        confidence: 0.5,
        ..Default::default()
    };

    // Remove extension
    let name = filename
        .rsplit('.')
        .skip(1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(".");

    // Extract episode info
    let (season, episode) = super::parser::extract_episode_from_filename(filename);
    metadata.season = season;
    metadata.episode = episode;

    // Extract year from filename
    if let Ok(re) = regex::Regex::new(r"[\.\s\-_\(\[](\d{4})[\.\s\-_\)\]]") {
        if let Some(caps) = re.captures(&name) {
            if let Some(year) = caps.get(1).and_then(|m| m.as_str().parse::<u16>().ok()) {
                if (1900..=2100).contains(&year) {
                    metadata.year = Some(year);
                }
            }
        }
    }

    // Try to extract title (before year, episode or technical info)
    // Pattern: "Title.Year" or "Title (Year)" or "Title 1080p" or "Title.E01" or "Title.S01E01"
    if let Ok(re) = regex::Regex::new(r"^([^\.]+?)[\.\s]*(?:(?:S\d{1,2})?E\d{1,3}|\d{4}|1080p|720p|4k|2160p)") {
        if let Some(caps) = re.captures(&name) {
            if let Some(title_match) = caps.get(1) {
                let title_part = title_match.as_str().trim();
                let (chinese, english) = split_chinese_english_title(title_part);
                metadata.chinese_title = chinese;
                metadata.english_title = english;
            }
        }
    }

    // Extract IMDB ID from filename (format: tt1234567 or tt12345678)
    if let Ok(re) = regex::Regex::new(r"(tt\d{7,8})") {
        if let Some(caps) = re.captures(&name) {
            if let Some(imdb_match) = caps.get(1) {
                metadata.imdb_id = Some(imdb_match.as_str().to_string());
            }
        }
    }

    // If we found episode but no title, we need AI
    if metadata.episode.is_some() && !metadata.has_searchable_info() {
        metadata.needs_ai_parsing = true;
    }

    // Increase confidence if we extracted useful info
    if metadata.has_searchable_info() {
        metadata.confidence = 0.7;
    }
    if metadata.year.is_some() {
        metadata.confidence += 0.1;
    }
    // High confidence if we have IMDB ID
    if metadata.imdb_id.is_some() {
        metadata.confidence = 0.95;
    }

    metadata
}

/// Extract IMDB/TMDB IDs from a file path (checks filename and all parent directories).
///
/// This is the highest priority check - if we find an ID anywhere in the path,
/// we should use it directly without AI parsing.
///
/// Returns (tmdb_id, imdb_id)
pub fn extract_ids_from_path(path: &std::path::Path) -> (Option<u64>, Option<String>) {
    extract_ids_from_path_starting_at(path, path)
}

/// Extract IMDB/TMDB IDs starting from a specific directory level.
///
/// This allows skipping certain directories (e.g., when a season directory has an invalid ID
/// and we want to check the parent show directory instead).
///
/// - `file_path`: The original file path (for filename checking)
/// - `start_dir`: The directory to start the parent walk from
///
/// Returns (tmdb_id, imdb_id)
pub fn extract_ids_from_path_starting_at(
    file_path: &std::path::Path,
    start_dir: &std::path::Path,
) -> (Option<u64>, Option<String>) {
    let mut tmdb_id: Option<u64> = None;
    let mut imdb_id: Option<String> = None;

    // Regex patterns
    let imdb_re = regex::Regex::new(r"(tt\d{7,8})").ok();
    let tmdb_re = regex::Regex::new(r"tmdb(\d+)").ok();
    // Also match plain numeric IDs at the end of organized folders: -IMDB-TMDB format
    // e.g., "2004-[Title]-[Title]-tt0372183-2502" -> TMDB is 2502
    let legacy_format_re = regex::Regex::new(r"-tt(\d+)-(\d+)$").ok();

    // Check filename first (only if starting from the file's own path)
    if file_path == start_dir {
        if let Some(filename) = file_path.file_name().and_then(|n| n.to_str()) {
            if let Some(ref re) = imdb_re {
                if let Some(caps) = re.captures(filename) {
                    imdb_id = caps.get(1).map(|m| m.as_str().to_string());
                }
            }
            if let Some(ref re) = tmdb_re {
                if let Some(caps) = re.captures(filename) {
                    tmdb_id = caps.get(1).and_then(|m| m.as_str().parse().ok());
                }
            }
        }
    }

    // Check parent directories (walk up the path from start_dir)
    let mut found_tmdb_from_season = false;
    let mut current = start_dir.parent();
    while let Some(dir) = current {
        if let Some(dirname) = dir.file_name().and_then(|n| n.to_str()) {
            tracing::debug!(
                "[EXTRACT-ID] Checking directory: {} for tmdb/imdb IDs",
                dirname
            );

            // Check for IMDB ID
            if imdb_id.is_none() {
                if let Some(ref re) = imdb_re {
                    if let Some(caps) = re.captures(dirname) {
                        imdb_id = caps.get(1).map(|m| m.as_str().to_string());
                        tracing::debug!("[EXTRACT-ID] Found IMDB ID: {:?} in {}", imdb_id, dirname);
                    }
                }
            }

            // Check for TMDB ID (format: tmdb12345)
            if tmdb_id.is_none() {
                if let Some(ref re) = tmdb_re {
                    if let Some(caps) = re.captures(dirname) {
                        tmdb_id = caps.get(1).and_then(|m| m.as_str().parse().ok());
                        tracing::debug!("[EXTRACT-ID] Found TMDB ID: {:?} in {}", tmdb_id, dirname);
                        
                        // Track if this looks like a Season folder (has S## prefix)
                        if dirname.starts_with("[S") || dirname.starts_with("[Season") {
                            found_tmdb_from_season = true;
                            tracing::debug!("[EXTRACT-ID] TMDB ID found in Season folder, may need correction");
                        }
                    }
                }
            }

            // Check for legacy format: -tt0372183-2502 (IMDB-TMDB at the end)
            if tmdb_id.is_none() {
                if let Some(ref re) = legacy_format_re {
                    if let Some(caps) = re.captures(dirname) {
                        if imdb_id.is_none() {
                            imdb_id = caps.get(1).map(|m| format!("tt{}", m.as_str()));
                        }
                        tmdb_id = caps.get(2).and_then(|m| m.as_str().parse().ok());
                    }
                }
            }
        }

        // CRITICAL: For organized TV show folders, prefer IDs from TV Show level (outermost)
        // instead of Season level. Season folders may have incorrect TMDB IDs.
        // Track if we found TMDB ID from a Season folder (to fix later)
        
        // Stop if we found both IDs from TV Show level (outermost)
        if tmdb_id.is_some() && imdb_id.is_some() && !found_tmdb_from_season {
            break;
        }

        // Continue searching if TMDB ID came from season folder
        // (to potentially find correct ID from TV Show folder)
        if found_tmdb_from_season {
            // Keep searching, but don't break early
        }

        current = dir.parent();
    }

    // If TMDB ID came from Season folder, search TV Show level again
    // This handles the case where Season folder has wrong TMDB ID
    if found_tmdb_from_season && tmdb_id.is_some() {
        tracing::debug!("[EXTRACT-ID] TMDB ID came from season folder, searching TV Show level...");
        
        // Find the TV Show folder (look for pattern with S## prefix)
        let mut current = start_dir.parent();
        while let Some(dir) = current {
            if let Some(dirname) = dir.file_name().and_then(|n| n.to_str()) {
                // Check if this is TV Show folder (no S## prefix)
                if !dirname.starts_with("[S") && !dirname.starts_with("[Season") {
                    // This should be the TV Show folder, extract IDs from here
                    if let Some(ref re) = imdb_re {
                        if imdb_id.is_none() {
                            if let Some(caps) = re.captures(dirname) {
                                imdb_id = caps.get(1).map(|m| m.as_str().to_string());
                            }
                        }
                    }
                    if let Some(ref re) = tmdb_re {
                        // Found TV Show level, use this TMDB ID instead of season's
                        if let Some(caps) = re.captures(dirname) {
                            if let Some(new_tmdb) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                                tracing::debug!("[EXTRACT-ID] Found TV Show level TMDB ID: {} (replacing season's {})", new_tmdb, tmdb_id.unwrap());
                                tmdb_id = Some(new_tmdb);
                            }
                        }
                    }
                    break; // Stop after first TV Show level folder
                }
            }
            current = dir.parent();
        }
    }

    if tmdb_id.is_none() && imdb_id.is_none() {
        tracing::debug!(
            "[EXTRACT-ID] No IDs found in path: {} (starting from {})",
            file_path.display(),
            start_dir.display()
        );
    }

    (tmdb_id, imdb_id)
}

/// Merge metadata from filename and directory, with filename taking priority.
pub fn merge_info(
    filename_info: CandidateMetadata,
    dir_info: CandidateMetadata,
) -> CandidateMetadata {
    CandidateMetadata {
        chinese_title: filename_info.chinese_title.or(dir_info.chinese_title),
        english_title: filename_info.english_title.or(dir_info.english_title),
        year: filename_info.year.or(dir_info.year),
        season: filename_info.season.or(dir_info.season),
        episode: filename_info.episode.or(dir_info.episode),
        tmdb_id: filename_info.tmdb_id.or(dir_info.tmdb_id),
        imdb_id: filename_info.imdb_id.or(dir_info.imdb_id),
        source: Some(MetadataSource::Merged),
        confidence: filename_info.confidence.max(dir_info.confidence),
        needs_ai_parsing: filename_info.needs_ai_parsing && dir_info.needs_ai_parsing,
    }
}

// Integration tests moved to tests/metadata_tests.rs
