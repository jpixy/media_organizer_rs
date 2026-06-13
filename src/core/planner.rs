//! Plan generation module.
//!
//! Coordinates the entire planning process:
//! 1. Scan directory for video files
//! 2. Parse filenames with AI
//! 3. Query TMDB for metadata
//! 4. Extract video metadata with ffprobe
//! 5. Generate target paths and operations
//! 6. Output plan.json

use crate::core::metadata::{self, CandidateMetadata, DirectoryType};
use crate::core::parser::{self, FilenameParser, ParsedFilename};
use crate::core::scanner::scan_directory;
use crate::generators::{filename as gen_filename, folder as gen_folder};
use crate::services::guessit_parser::GuessItParser;
use crate::models::media::{
    EpisodeMetadata, MediaType, MovieMetadata, SeasonMetadata, TvSeriesMetadata, VideoFile, VideoMetadata,
};
use crate::models::plan::{
    Operation, OperationType, ParsedInfo, Plan, PlanItem, PlanItemStatus, PosterDownloadStatus,
    PosterStats, SampleItem, TargetInfo, UnknownItem,
};
use crate::utils::chinese;
use crate::services::ffprobe;
use crate::services::tmdb::{Credits, MovieDetails, TmdbClient};
use crate::Result;
use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use futures::stream::{self, StreamExt};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Cache for season episodes: (tmdb_id, season_number) -> Vec<EpisodeInfo>
type SeasonEpisodesCache =
    Arc<RwLock<HashMap<(u64, u16), Vec<crate::services::tmdb::EpisodeInfo>>>>;

/// Convert ISO 3166-1 country code to country name.
/// Convert ISO 3166-1 country code to human-readable name.
/// Used for metadata (countries field in NFO), NOT for folder classification.
fn country_code_to_name(code: &str) -> String {
    match code.to_uppercase().as_str() {
        "US" => "United States".to_string(),
        "GB" => "United Kingdom".to_string(),
        "CA" => "Canada".to_string(),
        "CN" => "China".to_string(),
        "JP" => "Japan".to_string(),
        "KR" => "South Korea".to_string(),
        "TW" => "Taiwan".to_string(),
        "HK" => "Hong Kong".to_string(),
        "FR" => "France".to_string(),
        "DE" => "Germany".to_string(),
        "ES" => "Spain".to_string(),
        "IT" => "Italy".to_string(),
        "AU" => "Australia".to_string(),
        "NZ" => "New Zealand".to_string(),
        "IN" => "India".to_string(),
        "TH" => "Thailand".to_string(),
        "ID" => "Indonesia".to_string(),
        "BR" => "Brazil".to_string(),
        "MX" => "Mexico".to_string(),
        "RU" => "Russia".to_string(),
        "NL" => "Netherlands".to_string(),
        "SE" => "Sweden".to_string(),
        "NO" => "Norway".to_string(),
        "DK" => "Denmark".to_string(),
        _ => code.to_uppercase(),
    }
}

/// Convert ISO 639-1 language code to human-readable name.
/// Used for folder naming: e.g., "zh" -> "Chinese" -> "ZH_Chinese"
fn language_code_to_name(code: &str) -> String {
    match code.to_lowercase().as_str() {
        // Major languages
        "en" => "English".to_string(),
        "zh" => "Chinese".to_string(),
        "ja" => "Japanese".to_string(),
        "ko" => "Korean".to_string(),
        "fr" => "French".to_string(),
        "de" => "German".to_string(),
        "es" => "Spanish".to_string(),
        "it" => "Italian".to_string(),
        "pt" => "Portuguese".to_string(),
        "ru" => "Russian".to_string(),
        // Asian languages
        "th" => "Thai".to_string(),
        "vi" => "Vietnamese".to_string(),
        "id" => "Indonesian".to_string(),
        "ms" => "Malay".to_string(),
        "tl" => "Filipino".to_string(),
        "hi" => "Hindi".to_string(),
        "ta" => "Tamil".to_string(),
        "te" => "Telugu".to_string(),
        "bn" => "Bengali".to_string(),
        // European languages
        "nl" => "Dutch".to_string(),
        "pl" => "Polish".to_string(),
        "sv" => "Swedish".to_string(),
        "no" => "Norwegian".to_string(),
        "da" => "Danish".to_string(),
        "fi" => "Finnish".to_string(),
        "cs" => "Czech".to_string(),
        "hu" => "Hungarian".to_string(),
        "el" => "Greek".to_string(),
        "tr" => "Turkish".to_string(),
        "uk" => "Ukrainian".to_string(),
        "ro" => "Romanian".to_string(),
        // Middle Eastern
        "ar" => "Arabic".to_string(),
        "he" => "Hebrew".to_string(),
        "fa" => "Persian".to_string(),
        // Chinese variants (TMDB sometimes uses these)
        "cn" => "Chinese".to_string(),
        "yue" => "Cantonese".to_string(),
        // Fallback
        _ => code.to_uppercase(),
    }
}

/// Normalize language code to standard ISO 639-1.
/// Handles TMDB quirks like "cn" -> "zh".
fn normalize_language_code(code: &str) -> &str {
    match code.to_lowercase().as_str() {
        "cn" => "zh",  // TMDB sometimes uses "cn" for Chinese
        _ => code,
    }
}

/// Format language folder name from original_language.
/// Returns format like "ZH_Chinese", "EN_English", etc.
fn format_language_folder(original_language: &str) -> String {
    let normalized = normalize_language_code(original_language);
    let name = language_code_to_name(normalized);
    format!("{}_{}", normalized.to_uppercase(), name)
}

/// Planner configuration.
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    /// Minimum confidence threshold for parsed filenames.
    pub min_confidence: f32,
    /// Whether to download posters.
    pub download_posters: bool,
    /// Poster size for TMDB.
    pub poster_size: String,
    /// Whether to generate NFO files.
    pub generate_nfo: bool,
    /// Whether to generate movie NFO files.
    pub generate_movie_nfo: bool,
    /// Whether to generate TV episode NFO files.
    pub generate_tv_episode_nfo: bool,
    /// Whether to generate TV season NFO files.
    pub generate_tv_season_nfo: bool,
    /// Whether AI parsing is enabled.
    pub ai_enabled: bool,
    /// Whether to move subtitle files and folders.
    pub move_subtitles: bool,
    /// Whether to move sample videos and folders.
    pub move_samples: bool,
    /// Whether to move extras videos and folders.
    pub move_extras: bool,
    /// Whether to move poster images.
    pub move_posters: bool,
    /// Whether to move OST (original soundtrack) folders.
    pub move_ost: bool,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.7, // Higher threshold: prefer skipping over wrong matches
            download_posters: true,
            poster_size: "w500".to_string(),
            generate_nfo: true,
            generate_movie_nfo: true,
            generate_tv_episode_nfo: true,
            generate_tv_season_nfo: true,
            ai_enabled: false, // AI disabled by default per requirements
            move_subtitles: true,
            move_samples: true,
            move_extras: true,
            move_posters: true,
            move_ost: true,
        }
    }
}

/// Plan generator.
pub struct Planner {
    config: PlannerConfig,
    parser: FilenameParser,
    tmdb_client: Option<TmdbClient>,
}

impl Planner {
    /// Create a new planner with default configuration.
    pub fn new() -> Result<Self> {
        let tmdb_client = TmdbClient::from_env().ok();
        Ok(Self {
            config: PlannerConfig::default(),
            parser: FilenameParser::new(),
            tmdb_client,
        })
    }

    /// Create a new planner with custom configuration.
    pub fn with_config(config: PlannerConfig) -> Result<Self> {
        let tmdb_client = TmdbClient::from_env().ok();
        Ok(Self {
            config,
            parser: FilenameParser::new(),
            tmdb_client,
        })
    }

    /// Create a new planner with application configuration.
    pub fn with_application_config(config: &crate::models::config::Config) -> Result<Self> {
        // 转换应用配置到服务层配置
        let tmdb_client = if let Some(api_key) = &config.tmdb.api_key {
            let tmdb_config = crate::services::tmdb::TmdbConfig {
                api_key: api_key.clone(),
                language: config.tmdb.language.clone(),
                use_bearer: api_key.starts_with("eyJ"),
                proxy_enabled: config.network.proxy_enabled,
                proxy: config.network.proxy.clone(),
            };
            Some(TmdbClient::new(tmdb_config))
        } else {
            None
        };

        // Get ai_enabled from OllamaConfig
        let ai_enabled = config.ollama.enabled;

        Ok(Self {
            config: PlannerConfig {
                min_confidence: 0.7,
                ai_enabled,
                download_posters: config.organize.download_posters,
                poster_size: config.organize.poster_size.clone(),
                generate_nfo: config.organize.generate_nfo,
                generate_movie_nfo: config.organize.generate_movie_nfo,
                generate_tv_episode_nfo: config.organize.generate_tv_episode_nfo,
                generate_tv_season_nfo: config.organize.generate_tv_season_nfo,
                move_subtitles: config.organize.move_subtitles,
                move_samples: config.organize.move_samples,
                move_extras: config.organize.move_extras,
                move_posters: config.organize.move_posters,
                move_ost: config.organize.move_ost,
            },
            parser: FilenameParser::new(),
            tmdb_client,
        })
    }

    /// Generate a plan for organizing videos.
    pub async fn generate(
        &self,
        source: &Path,
        target: &Path,
        media_type: MediaType,
    ) -> Result<Plan> {
        let total_start = Instant::now();
        tracing::info!("Generating plan for {:?}", source);
        tracing::info!("Target directory: {:?}", target);
        tracing::info!("Media type: {}", media_type);

        // Step 1: Scan directory
        let scan_start = Instant::now();
        println!("[INFO] Scanning directory...");
        let scan_result = scan_directory(source)?;
        let scan_time = scan_start.elapsed();
        println!(
            "   Found {} videos, {} samples (took {:.2}s)",
            scan_result.videos.len(),
            scan_result.samples.len(),
            scan_time.as_secs_f64()
        );

        if scan_result.videos.is_empty() {
            tracing::warn!("No video files found in {:?}", source);
        }

        // Step 2: Process videos (pass source for correct cache key calculation)
        let process_start = Instant::now();
        let (mut items, unknown) = self
            .process_videos(&scan_result.videos, source, target, media_type)
            .await?;
        let _process_time = process_start.elapsed();

        // Step 2.5: Process organized TV folders without videos (generate season NFOs)
        if media_type == MediaType::TvSeries {
            let tv_folder_items = self
                .process_organized_tv_folders(&scan_result.organized_tv_folders, target)
                .await?;
            items.extend(tv_folder_items);
        }

        // Step 3: Process samples
        let samples = self.process_samples(&scan_result.samples, &items, target);

        // Step 3.5: Deduplicate operations across all items
        // This handles cases where multiple videos share the same subtitles
        self.deduplicate_operations(&mut items);

        // Step 3.6: SAFETY CHECK - Detect duplicate target paths
        // This prevents data loss from files overwriting each other
        self.validate_no_duplicate_targets(&mut items)?;

        // Step 3.7: Calculate poster statistics
        let (poster_download_count, poster_skipped_count) = items.iter()
            .filter(|item| item.status == PlanItemStatus::Pending)
            .fold((0, 0), |(downloaded, skipped), item| {
                match item.poster_download {
                    Some(PosterDownloadStatus::Download) => (downloaded + 1, skipped),
                    Some(PosterDownloadStatus::SkippedLocalExists) => (downloaded, skipped + 1),
                    _ => (downloaded, skipped),
                }
            });

        // Step 4: Create plan
        let plan = Plan {
            version: "1.0".to_string(),
            created_at: Utc::now().to_rfc3339(),
            media_type: Some(media_type),
            source_path: source.to_path_buf(),
            target_path: target.to_path_buf(),
            items,
            samples,
            unknown,
            poster_stats: Some(PosterStats {
                download_count: poster_download_count,
                skipped_count: poster_skipped_count,
            }),
        };

        let total_time = total_start.elapsed();
        println!();
        println!("{}", "[Timing]");
        println!("  Total:         {:>8.2}s", total_time.as_secs_f64());

        Ok(plan)
    }

    /// Process video files: parse, query TMDB, extract metadata.
    ///
    /// OPTIMIZED DESIGN:
    /// 1. Group videos by their parent directory
    /// 2. For each group, call AI + TMDB only ONCE for show info
    /// 3. Fetch entire season info once (not per episode)
    /// 4. Use regex to extract episode numbers for individual files
    /// 5. Run ffprobe in parallel (up to 8 concurrent)
    async fn process_videos(
        &self,
        videos: &[VideoFile],
        source: &Path,
        target: &Path,
        media_type: MediaType,
    ) -> Result<(Vec<PlanItem>, Vec<UnknownItem>)> {
        let mut items = Vec::new();
        let mut unknown = Vec::new();

        if videos.is_empty() {
            return Ok((items, unknown));
        }

        // Step 1: Group videos by parent directory
        let groups = self.group_by_top_level_dir(videos, source);
        tracing::info!(
            "Grouped {} videos into {} directories",
            videos.len(),
            groups.len()
        );

        // Caches (wrapped for concurrent access across parallel groups)
        let tv_series_cache: Arc<RwLock<HashMap<PathBuf, TvSeriesMetadata>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let season_episodes_cache: SeasonEpisodesCache = Arc::new(RwLock::new(HashMap::new()));

        // Step 2: Run ffprobe in parallel for all videos (up to 8 concurrent)
        tracing::info!("Extracting video metadata with ffprobe (parallel)...");
        let ffprobe_results = self.parallel_ffprobe(videos).await;
        let ffprobe_map: HashMap<PathBuf, VideoMetadata> = ffprobe_results
            .into_iter()
            .filter_map(|(path, result)| result.ok().map(|meta| (path, meta)))
            .collect();
        tracing::info!("FFprobe completed for {} files", ffprobe_map.len());

        // Create progress bar with filename display
        let pb = ProgressBar::new(videos.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} [{elapsed}/{eta}] {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_message("Starting...");

        // Step 3: Process videos in parallel
        // - Movies: all videos are independent, process fully in parallel
        // - TV Series: groups processed in parallel; within each group,
        //   representative video first (AI + TMDB), then remaining in parallel
        const PARALLEL_LIMIT: usize = 10;

        if media_type == MediaType::Movies {
            let mut tasks = stream::iter(videos.iter())
                .map(|video| {
                    let ffprobe_meta = ffprobe_map.get(&video.path);
                    let season_episodes_cache = Arc::clone(&season_episodes_cache);
                    let pb = pb.clone();
                    async move {
                        pb.set_message(format!("Processing: {}", video.filename));
                        let result = self
                            .process_single_video_optimized(
                                video,
                                target,
                                media_type,
                                None,
                                &season_episodes_cache,
                                ffprobe_meta,
                            )
                            .await;
                        pb.inc(1);
                        (video, result)
                    }
                })
                .buffer_unordered(PARALLEL_LIMIT);

            while let Some((video, result)) = tasks.next().await {
                match result {
                    Ok(Some((item, _))) => items.push(item),
                    Ok(None) => {
                        unknown.push(UnknownItem {
                            source: video.clone(),
                            reason: "Failed to find TMDB match".to_string(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to process {}: {}", video.filename, e);
                        unknown.push(UnknownItem {
                            source: video.clone(),
                            reason: e.to_string(),
                        });
                    }
                }
            }
        } else {
            // TV Series: process all videos in parallel
            // Each video checks the shared cache for existing show metadata.
            // If found, it reuses the cached data (fast path, no TMDB query).
            // If not found, it queries TMDB and caches the result for other
            // videos in the same directory group.
            let mut tasks = stream::iter(videos.iter())
                .map(|video| {
                    let top_dir = video.parent_dir.clone();
                    let ffprobe_meta = ffprobe_map.get(&video.path);
                    let tv_series_cache = Arc::clone(&tv_series_cache);
                    let season_episodes_cache = Arc::clone(&season_episodes_cache);
                    let pb = pb.clone();
                    async move {
                        pb.set_message(format!("Processing: {}", video.filename));
                        // Check cache for existing show metadata from this directory
                        let cached_show =
                            tv_series_cache.read().await.get(&top_dir).cloned();

                        let result = self
                            .process_single_video_optimized(
                                video,
                                target,
                                media_type,
                                cached_show.as_ref(),
                                &season_episodes_cache,
                                ffprobe_meta,
                            )
                            .await;

                        // If we obtained show metadata, cache it for other videos
                        // in the same directory group
                        if let Ok(Some((_, Some(ref show_meta)))) = result {
                            tv_series_cache
                                .write()
                                .await
                                .insert(top_dir, show_meta.clone());
                        }

                        pb.inc(1);
                        (video, result)
                    }
                })
                .buffer_unordered(PARALLEL_LIMIT);

            while let Some((video, result)) = tasks.next().await {
                match result {
                    Ok(Some((item, _))) => items.push(item),
                    Ok(None) => {
                        unknown.push(UnknownItem {
                            source: video.clone(),
                            reason: "Failed to find TMDB match".to_string(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to process {}: {}", video.filename, e);
                        unknown.push(UnknownItem {
                            source: video.clone(),
                            reason: e.to_string(),
                        });
                    }
                }
            }
        }

        pb.finish_with_message("Done!");
        Ok((items, unknown))
    }

    /// Run ffprobe in parallel for multiple videos (up to 8 concurrent).
    async fn parallel_ffprobe(
        &self,
        videos: &[VideoFile],
    ) -> Vec<(PathBuf, Result<VideoMetadata>)> {
        const CONCURRENT_LIMIT: usize = 8;

        let results: Vec<_> = stream::iter(videos)
            .map(|video| {
                let path = video.path.clone();
                let filename = video.filename.clone();
                async move {
                    let ffprobe_result = ffprobe::extract_metadata(&path);
                    let filename_meta = ffprobe::parse_metadata_from_filename(&filename);

                    let result = match ffprobe_result {
                        Ok(meta) => Ok(ffprobe::merge_metadata(meta, filename_meta)),
                        Err(_) => Ok(filename_meta), // Fallback to filename-only metadata
                    };
                    (path, result)
                }
            })
            .buffer_unordered(CONCURRENT_LIMIT)
            .collect()
            .await;

        results
    }

    /// Optimized video processing with season-level caching.
    ///
    /// Key optimizations:
    /// 1. Uses pre-computed ffprobe results
    /// 2. Caches entire season info (one TMDB call per season, not per episode)
    /// 3. Detects and re-parses already-organized files
    async fn process_single_video_optimized(
        &self,
        video: &VideoFile,
        target: &Path,
        media_type: MediaType,
        cached_show: Option<&TvSeriesMetadata>,
        season_cache: &SeasonEpisodesCache,
        precomputed_ffprobe: Option<&VideoMetadata>,
    ) -> Result<Option<(PlanItem, Option<TvSeriesMetadata>)>> {
        // ============================================================
        // HIGHEST PRIORITY: Check for TMDB/IMDB ID in filename OR parent directories
        // If found, use direct lookup - this bypasses all other parsing logic
        // ============================================================
        let (path_tmdb_id, path_imdb_id) = metadata::extract_ids_from_path(&video.path);

        tracing::info!(
            "[PATH-ID] Extract result for {}: tmdb={:?}, imdb={:?}",
            video.filename,
            path_tmdb_id,
            path_imdb_id
        );

        if path_tmdb_id.is_some() || path_imdb_id.is_some() {
            tracing::debug!(
                "[PATH-ID] Found IDs in path: tmdb={:?}, imdb={:?} for {}",
                path_tmdb_id,
                path_imdb_id,
                video.filename
            );

            // Create metadata with extracted IDs
            let path_meta = metadata::CandidateMetadata {
                tmdb_id: path_tmdb_id,
                imdb_id: path_imdb_id.clone(),
                ..Default::default()
            };

            if media_type == MediaType::Movies {
                if let Some(movie_metadata) = self.try_direct_id_lookup(&path_meta).await? {
                    tracing::info!(
                        "[PATH-ID] Found movie via path ID: {} ({})",
                        movie_metadata.title,
                        video.filename
                    );

                    // Build parsed info
                    let parsed = ParsedFilename {
                        title: Some(movie_metadata.title.clone()),
                        original_title: Some(movie_metadata.original_title.clone()),
                        year: Some(movie_metadata.year),
                        confidence: 1.0,
                        raw_response: Some("path_id_lookup".to_string()),
                        ..Default::default()
                    };

                    // Get video metadata
                    let video_metadata = match precomputed_ffprobe {
                        Some(meta) => meta.clone(),
                        None => {
                            let ffprobe_meta =
                                ffprobe::extract_metadata(&video.path).unwrap_or_default();
                            let filename_parsed =
                                ffprobe::parse_metadata_from_filename(&video.filename);
                            ffprobe::merge_metadata(ffprobe_meta, filename_parsed)
                        }
                    };

                    // Generate target info
                    let (target_info, operations, poster_download) = match self.generate_target_info(
                        video,
                        &Some(movie_metadata.clone()),
                        &None,
                        &parsed,
                        &video_metadata,
                        target,
                        media_type,
                    )? {
                        Some(result) => result,
                        None => return Ok(None),
                    };

                    return Ok(Some((
                        PlanItem {
                            id: Uuid::new_v4().to_string(),
                            status: PlanItemStatus::Pending,
                            source: video.clone(),
                            parsed: ParsedInfo {
                                title: parsed.title,
                                original_title: parsed.original_title,
                                year: parsed.year,
                                confidence: 1.0,
                                raw_response: parsed.raw_response,
                            },
                            movie_metadata: Some(movie_metadata),
                            tv_series_metadata: None,
                            episode_metadata: None,
                            season_metadata: None,
                            video_metadata,
                            target: target_info,
                            operations,
                            poster_download,
                        },
                        None,
                    )));
                }
            } else if media_type == MediaType::TvSeries {
                // For TV shows, try direct lookup using IMDB ID or TMDB ID from path
                // Strategy: If the current directory's IMDB ID fails (e.g., season-specific ID),
                // try looking up parent directories for the show's main ID
                let resolved_tmdb_id = if let Some(tmdb_id) = path_tmdb_id {
                    Some(tmdb_id)
                } else if let Some(ref imdb_id) = path_imdb_id {
                    // Use TMDB's find API to convert IMDB ID to TMDB ID
                    if let Some(client) = &self.tmdb_client {
                        match client.find_tv_by_imdb_id(imdb_id).await {
                            Ok(Some(tv_tmdb_id)) => {
                                tracing::info!(
                                    "[PATH-ID] Resolved TV IMDB {} -> TMDB {}",
                                    imdb_id,
                                    tv_tmdb_id
                                );
                                Some(tv_tmdb_id)
                            }
                            Ok(None) => {
                                tracing::warn!(
                                    "[PATH-ID] No TV show found for IMDB ID: {}",
                                    imdb_id
                                );
                                // SOLUTION B: If IMDB ID lookup failed, try parent directory
                                // This handles cases where season directories have their own IMDB IDs
                                // that are not recognized by TMDB (e.g., S02.tt13660696 for Slow Horses S2)
                                // We should look up the parent directory for the show's main ID
                                self.try_parent_directory_id_lookup(&video.path, imdb_id, client)
                                    .await
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "[PATH-ID] Failed to lookup IMDB ID {}: {}",
                                    imdb_id,
                                    e
                                );
                                None
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(tmdb_id) = resolved_tmdb_id {
                    // Try to use existing organized folder logic
                    if let Some(folder_info) = self.find_organized_tv_series_folder(&video.parent_dir)
                    {
                        tracing::info!(
                            "[PATH-ID] TV show file in folder with ID: {} -> tmdb{}",
                            video.filename,
                            tmdb_id
                        );
                        return self
                            .process_new_file_in_organized_folder(
                                video,
                                target,
                                &folder_info,
                                season_cache,
                                precomputed_ffprobe,
                            )
                            .await;
                    }

                    // If not in organized folder, fetch show details and process
                    if let Some(client) = &self.tmdb_client {
                        // Extract episode info from filename first (needed for parallel queries)
                        let (season_num, episode_num) =
                            parser::extract_episode_from_filename(&video.filename);
                        let season = season_num.unwrap_or(1);
                        let episode = episode_num.unwrap_or(1);

                        // Fetch show details, episode details, and season details IN PARALLEL
                        let (show_result, episode_result, season_result) = tokio::join!(
                            client.get_tv_details(tmdb_id),
                            client.get_episode_details(tmdb_id, season, episode),
                            client.get_season_details(tmdb_id, season),
                        );

                        if let Ok(show_details) = show_result {
                            let show_metadata =
                                self.build_tv_series_metadata_from_details(&show_details);

                            // Get episode metadata from TMDB (already fetched in parallel)
                            let episode_metadata = if let Ok(ep_details) = episode_result {
                                Some(EpisodeMetadata {
                                    name: ep_details.name.clone(),
                                    original_name: None, // Not available in EpisodeDetails
                                    episode_number: ep_details.episode_number,
                                    season_number: ep_details.season_number,
                                    air_date: ep_details.air_date.clone(),
                                    overview: ep_details.overview.clone(),
                                })
                            } else {
                                None
                            };

                            // Get season metadata from TMDB (already fetched in parallel)
                            let season_metadata = if let Ok(season_details) = season_result {
                                Some(SeasonMetadata {
                                    season_number: season_details.season_number,
                                    name: season_details.name,
                                    overview: season_details.overview,
                                    air_date: season_details.air_date,
                                    poster_url: season_details.poster_path.map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)),
                                    episode_count: season_details.episodes.len() as u16,
                                })
                            } else {
                                None
                            };

                            // Build parsed info
                            let parsed = ParsedFilename {
                                title: Some(show_metadata.name.clone()),
                                original_title: Some(show_metadata.original_name.clone()),
                                year: Some(show_metadata.year),
                                season: Some(season),
                                episode: Some(episode),
                                confidence: 1.0,
                                raw_response: Some("path_id_lookup".to_string()),
                            };

                            // Get video metadata
                            let video_metadata = match precomputed_ffprobe {
                                Some(meta) => meta.clone(),
                                None => {
                                    let ffprobe_meta =
                                        ffprobe::extract_metadata(&video.path).unwrap_or_default();
                                    let filename_parsed =
                                        ffprobe::parse_metadata_from_filename(&video.filename);
                                    ffprobe::merge_metadata(ffprobe_meta, filename_parsed)
                                }
                            };

                            // Generate target info - tv_series_metadata needs to be a tuple
                            let tv_series_tuple = (show_metadata.clone(), episode_metadata.clone(), season_metadata.clone());
                            let (target_info, operations, poster_download) = match self.generate_target_info(
                                video,
                                &None,
                                &Some(tv_series_tuple),
                                &parsed,
                                &video_metadata,
                                target,
                                media_type,
                            )? {
                                Some(result) => result,
                                None => return Ok(None),
                            };

                            tracing::info!(
                                "[PATH-ID] Found TV show via path ID: {} S{:02}E{:02} ({})",
                                show_metadata.name,
                                season,
                                episode,
                                video.filename
                            );

                            return Ok(Some((
                                PlanItem {
                                    id: Uuid::new_v4().to_string(),
                                    status: PlanItemStatus::Pending,
                                    source: video.clone(),
                                    parsed: ParsedInfo {
                                        title: parsed.title,
                                        original_title: parsed.original_title,
                                        year: parsed.year,
                                        confidence: 1.0,
                                        raw_response: parsed.raw_response,
                                    },
                                    movie_metadata: None,
                                    tv_series_metadata: Some(show_metadata.clone()),
                                    episode_metadata,
                                    season_metadata,
                                    video_metadata,
                                    target: target_info,
                                    operations,
                                    poster_download,
                                },
                                Some(show_metadata),
                            )));
                        }
                    }
                }
            }
        }

        // ============================================================
        // Step 0: Check if this is an already-organized file
        // If so, parse using regex instead of AI for better accuracy
        // ============================================================
        if parser::is_organized_filename(&video.filename) {
            tracing::debug!(
                "[ORGANIZED] Detected already-organized file: {}",
                video.filename
            );
            return self
                .process_organized_file(
                    video,
                    target,
                    media_type,
                    cached_show,
                    season_cache,
                    precomputed_ffprobe,
                )
                .await;
        }

        // ============================================================
        // Step 1: Extract info from filename
        // ============================================================
        let filename_meta = metadata::extract_from_filename(&video.filename);

        // Try direct ID lookup from filename (in case path extraction missed something)
        if media_type == MediaType::Movies {
            if let Some(movie_metadata) = self.try_direct_id_lookup(&filename_meta).await? {
                tracing::info!(
                    "FILENAME-ID Found movie via ID: {} ({})",
                    movie_metadata.title,
                    video.filename
                );

                // Build parsed info from filename metadata
                let parsed = ParsedFilename {
                    title: filename_meta.chinese_title.clone(),
                    original_title: filename_meta.english_title.clone(),
                    year: filename_meta.year,
                    season: None,
                    episode: None,
                    confidence: 1.0,
                    raw_response: Some("direct_id_lookup".to_string()),
                };

                // Get video metadata (use precomputed or fallback)
                let video_metadata = match precomputed_ffprobe {
                    Some(meta) => meta.clone(),
                    None => {
                        let ffprobe_meta =
                            ffprobe::extract_metadata(&video.path).unwrap_or_default();
                        let filename_parsed =
                            ffprobe::parse_metadata_from_filename(&video.filename);
                        ffprobe::merge_metadata(ffprobe_meta, filename_parsed)
                    }
                };

                // Generate target info and operations
                let (target_info, operations, poster_download) = match self.generate_target_info(
                    video,
                    &Some(movie_metadata.clone()),
                    &None,
                    &parsed,
                    &video_metadata,
                    target,
                    media_type,
                )? {
                    Some(result) => result,
                    None => return Ok(None),
                };

                let plan_item = PlanItem {
                    id: Uuid::new_v4().to_string(),
                    source: video.clone(),
                    parsed: ParsedInfo {
                        title: parsed.title.clone(),
                        original_title: parsed.original_title.clone(),
                        year: parsed.year,
                        confidence: parsed.confidence,
                        raw_response: parsed.raw_response.clone(),
                    },
                    movie_metadata: Some(movie_metadata),
                    tv_series_metadata: None,
                    episode_metadata: None,
                    season_metadata: None,
                    video_metadata: video_metadata.clone(),
                    target: target_info,
                    operations,
                    status: PlanItemStatus::Pending,
                    poster_download,
                };

                return Ok(Some((plan_item, None)));
            }
        }

        // Step 2: Parse filename (AI or regex) - fallback when no ID found
        // First, try local parsing (guessit) to extract title/year
        let (parsed, _) = if media_type == MediaType::TvSeries && cached_show.is_some() {
            // FAST PATH: Extract episode from filename using regex
            let (mut season, episode) = parser::extract_episode_from_filename(&video.filename);
            if episode.is_none() {
                tracing::debug!("Could not extract episode from: {}", video.filename);
                return Ok(None);
            }

            // Try to extract season from parent directory name (e.g., "第一季", "Season 01")
            if season.is_none() || season == Some(1) {
                let parent_name = video
                    .parent_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if let Some(dir_season) = parser::extract_season_from_dirname(parent_name) {
                    tracing::debug!(
                        "Extracted season {} from directory: {}",
                        dir_season,
                        parent_name
                    );
                    season = Some(dir_season);
                }
            }

            let parsed = ParsedFilename {
                title: cached_show.map(|s| s.name.clone()),
                original_title: cached_show.map(|s| s.original_name.clone()),
                year: cached_show.map(|s| s.year),
                season,
                episode,
                confidence: 1.0,
                raw_response: Some("regex_extracted".to_string()),
            };

            // Try TMDB search with local parsed result
            let folder_name = self.get_meaningful_folder_name(&video.parent_dir);
            let (show, _) = self
                .query_tmdb_tv_series_with_folder(&parsed, folder_name.as_deref())
                .await?;

            if let Some(show_meta) = show {
                // TMDB search succeeded, no need for AI
                tracing::info!("TMDB found via local parsing (TV): {}", show_meta.name);
                (parsed, false)
            } else if self.config.ai_enabled {
                // TMDB search failed, try AI parsing
                tracing::info!("TMDB search failed, using AI parsing for: {}", video.filename);
                let parse_input = self.build_parse_input(video);
                let ai_parsed = self.parser.parse(&parse_input, media_type).await?;
                if !self.parser.is_valid(&ai_parsed) {
                    tracing::debug!("Low confidence AI parsing for: {}", video.filename);
                    return Ok(None);
                }

                // Try TMDB search with AI parsed result
                let (ai_show, _) = self
                    .query_tmdb_tv_series_with_folder(&ai_parsed, folder_name.as_deref())
                    .await?;

                if let Some(ai_show_meta) = ai_show {
                    // AI TMDB search succeeded
                    tracing::info!("TMDB found via AI parsing (TV): {}", ai_show_meta.name);
                    (ai_parsed, true)
                } else {
                    // AI TMDB search failed
                    tracing::debug!("TMDB search failed after AI parsing for: {}", video.filename);
                    return Ok(None);
                }
            } else {
                // AI disabled, try folder-based search as fallback
                tracing::debug!("TMDB search failed and AI disabled, trying folder-based search");
                if let Some(folder_title) = self.get_meaningful_folder_name(&video.parent_dir) {
                    let folder_parsed = ParsedFilename {
                        title: Some(folder_title.clone()),
                        original_title: None,
                        year: None,
                        season: parsed.season,
                        episode: parsed.episode,
                        confidence: 0.6,
                        raw_response: Some("folder_fallback".to_string()),
                    };
                    let (show, _) = self
                        .query_tmdb_tv_series_with_folder(&folder_parsed, Some(&folder_title))
                        .await?;
                    if show.is_some() {
                        tracing::info!("TMDB found via folder search (TV): {}", folder_title);
                        (folder_parsed, false)
                    } else {
                        return Ok(None);
                    }
                } else {
                    return Ok(None);
                }
            }
        } else {
            // For movies or first TV episode without cached show, try local parsing first
            let parse_input = self.build_parse_input(video);
            let parsed = self.parser.parse(&parse_input, media_type).await?;
            if !self.parser.is_valid(&parsed) {
                tracing::debug!("Low confidence parsing for: {}", video.filename);
                return Ok(None);
            }

            // Try TMDB search with local parsed result
            if media_type == MediaType::Movies {
                let movie = self.query_tmdb_movie(&parsed).await?;
                if movie.is_some() {
                    // TMDB search succeeded, no need for AI
                    tracing::info!("TMDB found via local parsing (Movie)");
                    (parsed, false)
                } else if self.config.ai_enabled {
                    // TMDB search failed, try AI parsing
                    tracing::info!("TMDB search failed, using AI parsing for: {}", video.filename);
                    let parse_input = self.build_parse_input(video);
                    let ai_parsed = self.parser.parse(&parse_input, media_type).await?;
                    if !self.parser.is_valid(&ai_parsed) {
                        tracing::debug!("Low confidence AI parsing for: {}", video.filename);
                        return Ok(None);
                    }

                    // Try TMDB search with AI parsed result
                    let ai_movie = self.query_tmdb_movie(&ai_parsed).await?;
                    if ai_movie.is_some() {
                        // AI TMDB search succeeded
                        tracing::info!("TMDB found via AI parsing (Movie)");
                        (ai_parsed, true)
                    } else {
                        // AI TMDB search failed
                        tracing::debug!("TMDB search failed after AI parsing for: {}", video.filename);
                        return Ok(None);
                    }
                } else {
                    // AI disabled, try folder-based search as fallback
                    tracing::debug!("TMDB search failed and AI disabled, trying folder-based search");
                    if let Some(folder_title) = self.get_meaningful_folder_name(&video.parent_dir) {
                        let folder_parsed = ParsedFilename {
                            title: Some(folder_title.clone()),
                            original_title: None,
                            year: None,
                            season: None,
                            episode: None,
                            confidence: 0.6,
                            raw_response: Some("folder_fallback".to_string()),
                        };
                        let movie = self.query_tmdb_movie(&folder_parsed).await?;
                        if movie.is_some() {
                            tracing::info!("TMDB found via folder search (Movie): {}", folder_title);
                            (folder_parsed, false)
                        } else {
                            return Ok(None);
                        }
                    } else {
                        return Ok(None);
                    }
                }
            } else {
                // TV Series without cached show - use title from filename_meta if available
                let mut tv_parsed = parsed.clone();
                if tv_parsed.title.is_none() {
                    // Fallback to filename metadata if parser didn't extract title
                    let filename_meta = metadata::extract_from_filename(&video.filename);
                    tv_parsed.title = filename_meta.chinese_title.or(filename_meta.english_title);
                }
                
                let folder_name = self.get_meaningful_folder_name(&video.parent_dir);
                let (show, _) = self
                    .query_tmdb_tv_series_with_folder(&tv_parsed, folder_name.as_deref())
                    .await?;
                
                if show.is_some() {
                    // TMDB search succeeded
                    tracing::info!("TMDB found via local parsing (TV): {}", show.as_ref().unwrap().name);
                    (tv_parsed, false)
                } else if self.config.ai_enabled {
                    // TMDB search failed, try AI parsing
                    tracing::info!("TMDB search failed, using AI parsing for: {}", video.filename);
                    let parse_input = self.build_parse_input(video);
                    let ai_parsed = self.parser.parse(&parse_input, media_type).await?;
                    if !self.parser.is_valid(&ai_parsed) {
                        tracing::debug!("Low confidence AI parsing for: {}", video.filename);
                        return Ok(None);
                    }

                    // Try TMDB search with AI parsed result
                    let (ai_show, _) = self
                        .query_tmdb_tv_series_with_folder(&ai_parsed, folder_name.as_deref())
                        .await?;
                    if ai_show.is_some() {
                        // AI TMDB search succeeded
                        tracing::info!("TMDB found via AI parsing (TV): {}", ai_show.as_ref().unwrap().name);
                        (ai_parsed, true)
                    } else {
                        // AI TMDB search failed
                        tracing::debug!("TMDB search failed after AI parsing for: {}", video.filename);
                        return Ok(None);
                    }
                } else {
                    // AI disabled, try folder-based search as fallback
                    tracing::debug!("TMDB search failed and AI disabled, trying folder-based search");
                    if let Some(folder_title) = folder_name {
                        let folder_parsed = ParsedFilename {
                            title: Some(folder_title.clone()),
                            original_title: None,
                            year: None,
                            season: tv_parsed.season,
                            episode: tv_parsed.episode,
                            confidence: 0.6,
                            raw_response: Some("folder_fallback".to_string()),
                        };
                        let (show, _) = self
                            .query_tmdb_tv_series_with_folder(&folder_parsed, Some(&folder_title))
                            .await?;
                        if show.is_some() {
                            tracing::info!("TMDB found via folder search (TV): {}", folder_title);
                            (folder_parsed, false)
                        } else {
                            return Ok(None);
                        }
                    } else {
                        return Ok(None);
                    }
                }
            }
        };

        // Step 3: Get metadata via title search
        let (movie_metadata, tv_series_metadata, episode_metadata, season_metadata) = match media_type {
            MediaType::Movies => {
                // No direct ID available, use title search
                let movie = self.query_tmdb_movie(&parsed).await?;
                if movie.is_none() {
                    return Ok(None);
                }
                (movie, None, None, None)
            }
            MediaType::TvSeries => {
                if let Some(show_meta) = cached_show {
                    // Use cached show, get episode from season cache
                    let (season, episode) =
                        (parsed.season.unwrap_or(1), parsed.episode.unwrap_or(1));
                    let ep_meta = self
                        .get_episode_from_cache(show_meta.tmdb_id, season, episode, season_cache)
                        .await;
                    // Get season metadata
                    let season_meta = if let Some(client) = &self.tmdb_client {
                        match client.get_season_details(show_meta.tmdb_id, season).await {
                            Ok(season_details) => Some(SeasonMetadata {
                                season_number: season_details.season_number,
                                name: season_details.name,
                                overview: season_details.overview,
                                air_date: season_details.air_date,
                                poster_url: season_details.poster_path.map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)),
                                episode_count: season_details.episodes.len() as u16,
                            }),
                            Err(e) => {
                                tracing::warn!("Failed to get season {} details for {}: {}", season, show_meta.name, e);
                                None
                            }
                        }
                    } else {
                        None
                    };
                    (None, Some(show_meta.clone()), ep_meta, season_meta)
                } else {
                    // First video: get show info and cache season
                    let folder_name = self.get_meaningful_folder_name(&video.parent_dir);
                    let (show, _) = self
                        .query_tmdb_tv_series_with_folder(&parsed, folder_name.as_deref())
                        .await?;
                    if show.is_none() {
                        return Ok(None);
                    }
                    let show_meta = show.unwrap();

                    // Get episode info (with season caching)
                    let (season, episode) = {
                        let (mut s, e) = parser::extract_episode_from_filename(&video.filename);
                        // Try to extract season from parent directory
                        if s.is_none() || s == Some(1) {
                            let parent_name = video
                                .parent_dir
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("");
                            if let Some(dir_s) = parser::extract_season_from_dirname(parent_name) {
                                s = Some(dir_s);
                            }
                        }
                        (
                            s.or(parsed.season).unwrap_or(1),
                            e.or(parsed.episode).unwrap_or(1),
                        )
                    };
                    let ep_meta = self
                        .get_episode_from_cache(show_meta.tmdb_id, season, episode, season_cache)
                        .await;
                    // Get season metadata
                    let season_meta = if let Some(client) = &self.tmdb_client {
                        match client.get_season_details(show_meta.tmdb_id, season).await {
                            Ok(season_details) => Some(SeasonMetadata {
                                season_number: season_details.season_number,
                                name: season_details.name,
                                overview: season_details.overview,
                                air_date: season_details.air_date,
                                poster_url: season_details.poster_path.map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)),
                                episode_count: season_details.episodes.len() as u16,
                            }),
                            Err(e) => {
                                tracing::warn!("Failed to get season {} details for {}: {}", season, show_meta.name, e);
                                None
                            }
                        }
                    } else {
                        None
                    };
                    (None, Some(show_meta), ep_meta, season_meta)
                }
            }
        };

        // Step 3: Get video metadata (use precomputed or fallback)
        let video_metadata = match precomputed_ffprobe {
            Some(meta) => meta.clone(),
            None => {
                let ffprobe_meta = ffprobe::extract_metadata(&video.path).unwrap_or_default();
                let filename_meta = ffprobe::parse_metadata_from_filename(&video.filename);
                ffprobe::merge_metadata(ffprobe_meta, filename_meta)
            }
        };

        // Step 4: Generate target info and operations
        let tv_series_with_episode = tv_series_metadata
            .as_ref()
            .map(|show| (show.clone(), episode_metadata.clone(), season_metadata.clone()));

        let (target_info, operations, poster_download) = match self.generate_target_info(
            video,
            &movie_metadata,
            &tv_series_with_episode,
            &parsed,
            &video_metadata,
            target,
            media_type,
        )? {
            Some(result) => result,
            None => return Ok(None), // Skip: cannot determine country
        };

        let plan_item = PlanItem {
            id: Uuid::new_v4().to_string(),
            source: video.clone(),
            parsed: ParsedInfo {
                title: parsed.title.clone(),
                original_title: parsed.original_title.clone(),
                year: parsed.year,
                confidence: parsed.confidence,
                raw_response: parsed.raw_response.clone(),
            },
            movie_metadata: movie_metadata.clone(),
            tv_series_metadata: tv_series_metadata.clone(),
            episode_metadata: episode_metadata.clone(),
            season_metadata: season_metadata.clone(),
            video_metadata: video_metadata.clone(),
            target: target_info,
            operations,
            status: PlanItemStatus::Pending,
            poster_download,
        };

        Ok(Some((plan_item, tv_series_metadata)))
    }

    /// Process an already-organized file (detected by filename format).
    ///
    /// This handles files that were previously organized by this tool, extracting
    /// metadata directly from the filename format instead of using AI.
    async fn process_organized_file(
        &self,
        video: &VideoFile,
        target: &Path,
        media_type: MediaType,
        cached_show: Option<&TvSeriesMetadata>, // Use cached show to avoid redundant TMDB calls
        season_cache: &SeasonEpisodesCache,
        precomputed_ffprobe: Option<&VideoMetadata>,
    ) -> Result<Option<(PlanItem, Option<TvSeriesMetadata>)>> {
        let (parsed, movie_metadata, tv_series_metadata, episode_metadata, season_metadata) = match media_type {
            MediaType::TvSeries => {
                // Parse organized TV show filename
                let info = match parser::parse_organized_tv_series_filename(&video.filename) {
                    Some(info) => info,
                    None => {
                        tracing::warn!(
                            "[ORGANIZED] Could not parse TV show format: {}",
                            video.filename
                        );
                        return Ok(None);
                    }
                };

                // Try to extract TMDB ID from parent folder names (may be nested like Show/Season 01/)
                let folder_info = self.find_organized_tv_series_folder(&video.parent_dir);

                // OPTIMIZATION: Use cached show if available and TMDB ID matches
                let show_meta = if let Some(cached) = cached_show {
                    // Verify TMDB ID matches (if we have folder info)
                    if let Some(ref folder) = folder_info {
                        if cached.tmdb_id == folder.tmdb_id {
                            tracing::debug!(
                                "[ORGANIZED] Using cached show for: {} S{:02}E{:02}",
                                info.title,
                                info.season,
                                info.episode
                            );
                            cached.clone()
                        } else {
                            // TMDB ID mismatch, fetch fresh data
                            self.fetch_tv_series_by_id(folder.tmdb_id).await?
                        }
                    } else {
                        // No folder info, trust the cache
                        cached.clone()
                    }
                } else if let Some(ref folder) = folder_info {
                    // No cache, fetch by TMDB ID from folder
                    println!(
                        "    [ORGANIZED] Re-indexing TV via ID: {} S{:02}E{:02} (tmdb{})",
                        folder.title, info.season, info.episode, folder.tmdb_id
                    );
                    self.fetch_tv_series_by_id(folder.tmdb_id).await?
                } else {
                    // Fall back to searching by title
                    println!(
                        "    [ORGANIZED] Re-indexing TV: {} S{:02}E{:02}",
                        info.title, info.season, info.episode
                    );

                    let parent_folder = self.get_meaningful_folder_name(&video.parent_dir);
                    let parsed_search = ParsedFilename {
                        title: Some(info.title.clone()),
                        original_title: None,
                        year: None,
                        season: Some(info.season),
                        episode: Some(info.episode),
                        confidence: 1.0,
                        raw_response: Some("organized_format".to_string()),
                    };

                    let (show, _) = self
                        .query_tmdb_tv_series_with_folder(&parsed_search, parent_folder.as_deref())
                        .await?;
                    if show.is_none() {
                        tracing::warn!("[ORGANIZED] TMDB search failed for: {}", info.title);
                        return Ok(None);
                    }
                    show.unwrap()
                };

                let parsed = ParsedFilename {
                    title: Some(info.title.clone()),
                    original_title: None,
                    year: folder_info.as_ref().and_then(|f| f.year),
                    season: Some(info.season),
                    episode: Some(info.episode),
                    confidence: 1.0,
                    raw_response: Some("organized_format".to_string()),
                };

                // Get episode metadata
                let ep_meta = self
                    .get_episode_from_cache(
                        show_meta.tmdb_id,
                        info.season,
                        info.episode,
                        season_cache,
                    )
                    .await;

                // Get season metadata
                let season_meta = if let Some(client) = &self.tmdb_client {
                    match client.get_season_details(show_meta.tmdb_id, info.season).await {
                        Ok(season_details) => Some(SeasonMetadata {
                            season_number: season_details.season_number,
                            name: season_details.name,
                            overview: season_details.overview,
                            air_date: season_details.air_date,
                            poster_url: season_details.poster_path.map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)),
                            episode_count: season_details.episodes.len() as u16,
                        }),
                        Err(e) => {
                            tracing::warn!("Failed to get season {} details for {}: {}", info.season, show_meta.name, e);
                            None
                        }
                    }
                } else {
                    None
                };

                (parsed, None, Some(show_meta), ep_meta, season_meta)
            }
            MediaType::Movies => {
                // Parse organized movie filename
                let mut info = match parser::parse_organized_movie_filename(&video.filename) {
                    Some(info) => info,
                    None => {
                        tracing::warn!(
                            "[ORGANIZED] Could not parse movie format: {}",
                            video.filename
                        );
                        return Ok(None);
                    }
                };

                // If tmdb_id is None, try to extract from parent folder
                // This handles files with technical info format: [Title](Year)-1080p-...
                let tmdb_id = match info.tmdb_id {
                    Some(id) => id,
                    None => {
                        if let Some(folder_info) =
                            self.find_organized_movie_folder(&video.parent_dir)
                        {
                            tracing::debug!(
                                "[ORGANIZED] Extracted TMDB ID {} from parent folder for: {}",
                                folder_info.tmdb_id,
                                video.filename
                            );
                            if info.imdb_id.is_none() {
                                info.imdb_id = folder_info.imdb_id;
                            }
                            folder_info.tmdb_id
                        } else {
                            tracing::warn!(
                                "[ORGANIZED] Could not find TMDB ID for: {}",
                                video.filename
                            );
                            return Ok(None);
                        }
                    }
                };

                println!(
                    "    [ORGANIZED] Re-indexing movie: {} ({}), tmdb{}",
                    info.original_title.as_deref().unwrap_or("?"),
                    info.year,
                    tmdb_id
                );

                // Get parent folder name for guessit parsing
                let folder_name = video.parent_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Parse folder name using guessit to extract title information
                let guessit_parser = GuessItParser::new();
                let guessit_result = guessit_parser.parse_with_type(folder_name, Some("movie")).ok();
                
                // Extract titles from guessit result
                let guessit_title = guessit_result.as_ref().and_then(|r| r.primary_title());
                let guessit_alt_titles = guessit_result.as_ref().and_then(|r| r.alternative_title.clone());

                // Fetch movie details directly using TMDB ID
                let tmdb = match self.tmdb_client.as_ref() {
                    Some(client) => client,
                    None => {
                        tracing::warn!("[ORGANIZED] TMDB client not initialized");
                        return Ok(None);
                    }
                };
                let details = tmdb.get_movie_details(tmdb_id).await?;
                let credits = tmdb.get_movie_credits(tmdb_id).await.ok();

                // Fetch collection details if movie belongs to a collection
                let collection_total = if let Some(ref collection) = details.belongs_to_collection {
                    match tmdb.get_collection_details(collection.id).await {
                        Ok(collection_details) => {
                            tracing::debug!(
                                "[COLLECTION] Fetched {} (tmdb{}): {} movies total",
                                collection.name,
                                collection.id,
                                collection_details.parts.len()
                            );
                            Some(collection_details.parts.len())
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[COLLECTION] Failed to fetch collection {}: {}",
                                collection.id,
                                e
                            );
                            None
                        }
                    }
                } else {
                    None
                };

                // Build movie metadata
                // First, try to get Chinese title from various sources
                let mut fallback_chinese_title: Option<String> = None;
                
                // Priority 1: Check if filename already has Chinese title
                if let Some(title) = &info.title {
                    if chinese::contains_chinese(title) {
                        fallback_chinese_title = Some(title.clone());
                        tracing::info!("[ORGANIZED] Found Chinese title in filename: {}", title);
                    }
                }
                
                // Priority 2: Check guessit alternative titles for Chinese
                if fallback_chinese_title.is_none() {
                    if let Some(alt_titles) = &guessit_alt_titles {
                        for alt_title in alt_titles {
                            if chinese::contains_chinese(alt_title) {
                                fallback_chinese_title = Some(alt_title.to_string());
                                tracing::info!("[ORGANIZED] Found Chinese title from guessit: {}", alt_title);
                                break;
                            }
                        }
                    }
                }
                
                // Priority 3: Search TMDB with guessit title to find Chinese translation
                if fallback_chinese_title.is_none() {
                    // Build list of search candidates from guessit and filename
                    let mut search_candidates: Vec<String> = Vec::new();
                    
                    // Add guessit primary title
                    if let Some(title) = &guessit_title {
                        if !title.is_empty() {
                            search_candidates.push(title.clone());
                        }
                    }
                    
                    // Add filename original title if different
                    if let Some(title) = &info.original_title {
                        if !search_candidates.contains(title) {
                            search_candidates.push(title.clone());
                        }
                    }
                    
                    // Try each candidate with Chinese language to find localized title
                    for candidate in search_candidates {
                        if let Ok(search_results) = tmdb.search_movie_with_language(&candidate, Some(info.year), "zh-CN").await {
                            for result in search_results {
                                if result.id == tmdb_id && chinese::contains_chinese(&result.title) {
                                    tracing::info!(
                                        "[TMDB] Found Chinese title '{}' for '{}' via search",
                                        result.title,
                                        candidate
                                    );
                                    fallback_chinese_title = Some(result.title);
                                    break;
                                }
                            }
                            if fallback_chinese_title.is_some() {
                                break;
                            }
                        }
                    }
                }
                
                // Priority 4: Use TMDB translations API to get Chinese title
                if fallback_chinese_title.is_none() {
                    tracing::info!("[TMDB] Trying translations API for tmdb{}", tmdb_id);
                    match tmdb.get_movie_translations(tmdb_id).await {
                        Ok(translations) => {
                            tracing::info!("[TMDB] Got {} translations for tmdb{}", translations.translations.len(), tmdb_id);
                            // Look for Chinese translation (zh or zh-CN)
                            for translation in &translations.translations {
                                tracing::debug!("[TMDB] Checking translation: {} / {} / '{}'", 
                                    translation.iso_639_1, translation.english_name, translation.data.title);
                                if translation.iso_639_1 == "zh" || translation.iso_639_1 == "zh-CN" {
                                    let chinese_title = &translation.data.title;
                                    tracing::info!("[TMDB] Found zh translation candidate: '{}'", chinese_title);
                                    if !chinese_title.is_empty() && chinese::contains_chinese(chinese_title) {
                                        tracing::info!(
                                            "[TMDB] Found Chinese title '{}' from translations API",
                                            chinese_title
                                        );
                                        fallback_chinese_title = Some(chinese_title.clone());
                                        break;
                                    } else {
                                        tracing::warn!("[TMDB] zh translation title is empty or not Chinese: '{}'", chinese_title);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("[TMDB] Failed to get translations for tmdb{}: {}", tmdb_id, e);
                        }
                    }
                }
                
                let metadata = self.build_movie_metadata_from_details(
                    &details,
                    credits.as_ref(),
                    collection_total,
                    fallback_chinese_title.as_deref(),
                );

                let parsed = ParsedFilename {
                    original_title: Some(metadata.original_title.clone()),
                    title: Some(metadata.title.clone()),
                    year: Some(metadata.year),
                    season: None,
                    episode: None,
                    confidence: 1.0,
                    raw_response: Some("organized_format".to_string()),
                };

                (parsed, Some(metadata), None, None, None)
            }
        };

        // Get video metadata
        let video_metadata = match precomputed_ffprobe {
            Some(meta) => meta.clone(),
            None => {
                let ffprobe_meta = ffprobe::extract_metadata(&video.path).unwrap_or_default();
                let filename_meta = ffprobe::parse_metadata_from_filename(&video.filename);
                ffprobe::merge_metadata(ffprobe_meta, filename_meta)
            }
        };

        // Generate target info
        let tv_series_with_episode = tv_series_metadata
            .as_ref()
            .map(|show| (show.clone(), episode_metadata.clone(), season_metadata.clone()));

        let (target_info, operations, poster_download) = match self.generate_target_info(
            video,
            &movie_metadata,
            &tv_series_with_episode,
            &parsed,
            &video_metadata,
            target,
            media_type,
        )? {
            Some(result) => result,
            None => return Ok(None),
        };

        let plan_item = PlanItem {
            id: Uuid::new_v4().to_string(),
            source: video.clone(),
            parsed: ParsedInfo {
                title: parsed.title.clone(),
                original_title: parsed.original_title.clone(),
                year: parsed.year,
                confidence: parsed.confidence,
                raw_response: parsed.raw_response.clone(),
            },
            movie_metadata: movie_metadata.clone(),
            tv_series_metadata: tv_series_metadata.clone(),
            episode_metadata: episode_metadata.clone(),
            season_metadata: season_metadata.clone(),
            video_metadata: video_metadata.clone(),
            target: target_info,
            operations,
            status: PlanItemStatus::Pending,
            poster_download,
        };

        Ok(Some((plan_item, tv_series_metadata)))
    }

    /// Build MovieMetadata from TMDB MovieDetails (used for organized files).
    ///
    /// The `collection_total_movies` parameter is optional and represents the total number
    /// of movies in the collection (franchise series). If provided, it will be included
    /// in the metadata for NFO generation.
    fn build_movie_metadata_from_details(
        &self,
        details: &MovieDetails,
        credits: Option<&Credits>,
        collection_total_movies: Option<usize>,
        fallback_chinese_title: Option<&str>,
    ) -> MovieMetadata {
        // Extract actor names and roles
        let (actors, actor_roles): (Vec<String>, Vec<String>) = credits
            .map(|c| {
                c.cast
                    .iter()
                    .take(10)
                    .map(|a| (a.name.clone(), a.character.clone().unwrap_or_default()))
                    .unzip()
            })
            .unwrap_or_default();

        let directors: Vec<String> = credits
            .map(|c| {
                c.crew
                    .iter()
                    .filter(|m| m.job == "Director")
                    .map(|m| m.name.clone())
                    .collect()
            })
            .unwrap_or_default();

        let writers: Vec<String> = credits
            .map(|c| {
                c.crew
                    .iter()
                    .filter(|m| matches!(m.job.as_str(), "Writer" | "Screenplay" | "Story"))
                    .map(|m| m.name.clone())
                    .collect()
            })
            .unwrap_or_default();

        // Extract country codes - prioritize origin_country, fallback to production_countries
        let country_codes: Vec<String> = if let Some(ref origin) = details.origin_country {
            if !origin.is_empty() {
                origin.clone()
            } else {
                details
                    .production_countries
                    .as_ref()
                    .map(|countries| countries.iter().map(|c| c.iso_3166_1.clone()).collect())
                    .unwrap_or_default()
            }
        } else {
            details
                .production_countries
                .as_ref()
                .map(|countries| countries.iter().map(|c| c.iso_3166_1.clone()).collect())
                .unwrap_or_default()
        };

        // Extract country names - ALWAYS use country_code_to_name for consistency
        // This ensures country_codes and countries have matching order and format
        let countries: Vec<String> = country_codes
            .iter()
            .map(|c| country_code_to_name(c))
            .collect();

        let genres: Vec<String> = details
            .genres
            .as_ref()
            .map(|genres| genres.iter().map(|g| g.name.clone()).collect())
            .unwrap_or_default();

        let studios: Vec<String> = details
            .production_companies
            .as_ref()
            .map(|companies| companies.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default();

        let poster_urls: Vec<String> = details
            .poster_path
            .as_ref()
            .map(|p| vec![format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)])
            .unwrap_or_default();

        let backdrop_url: Option<String> = details
            .backdrop_path
            .as_ref()
            .map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p));

        // Determine title - use TMDB title, but fallback to parsed Chinese title if TMDB doesn't have translation
        let title: String = {
            let tmdb_title = &details.title;
            let tmdb_original = &details.original_title;
            let titles_same = self.normalize_title(tmdb_title) == self.normalize_title(tmdb_original);
            let title_has_chinese = chinese::contains_chinese(tmdb_title);
            
            // If TMDB has Chinese translation, use it
            if !titles_same || title_has_chinese {
                tmdb_title.clone()
            } else if let Some(fallback) = fallback_chinese_title {
                // Only use fallback if it actually contains Chinese characters
                if chinese::contains_chinese(fallback) {
                    // TMDB doesn't have Chinese translation, use parsed Chinese title from filename
                    tracing::info!(
                        "[TMDB] No Chinese translation for '{}', using fallback: '{}'",
                        tmdb_title,
                        fallback
                    );
                    fallback.to_string()
                } else {
                    // Fallback is also not Chinese, use TMDB title
                    tmdb_title.clone()
                }
            } else {
                tmdb_title.clone()
            }
        };

        MovieMetadata {
            tmdb_id: details.id,
            imdb_id: details.imdb_id.clone(),
            title,
            original_title: details.original_title.clone(),
            original_language: details.original_language.clone(),
            year: details
                .release_date
                .as_ref()
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse().ok())
                .unwrap_or(0),
            release_date: details.release_date.clone(),
            overview: details.overview.clone(),
            tagline: details.tagline.clone(),
            runtime: details.runtime,
            genres,
            countries,
            country_codes,
            studios,
            rating: details.vote_average,
            votes: details.vote_count,
            poster_urls,
            backdrop_url,
            directors,
            writers,
            actors,
            actor_roles,
            certification: None,
            collection_id: details.belongs_to_collection.as_ref().map(|c| c.id),
            collection_name: details
                .belongs_to_collection
                .as_ref()
                .map(|c| c.name.clone()),
            collection_overview: details
                .belongs_to_collection
                .as_ref()
                .and_then(|c| c.overview.clone()),
            collection_total_movies,
        }
    }

    /// Process a new file that was added to an already-organized folder.
    ///
    /// This handles the common case where:
    /// - A TV show folder was already organized (e.g., `[罚罪2](2025)-tt36771056-tmdb296146/`)
    /// - User later added new episode files (e.g., `19.mp4`, `20.mp4`)
    /// - These new files need to be organized using the existing TMDB ID from the folder
    async fn process_new_file_in_organized_folder(
        &self,
        video: &VideoFile,
        target: &Path,
        folder_info: &parser::OrganizedTvSeriesFolderInfo,
        season_cache: &SeasonEpisodesCache,
        precomputed_ffprobe: Option<&VideoMetadata>,
    ) -> Result<Option<(PlanItem, Option<TvSeriesMetadata>)>> {
        // Extract season and episode from filename
        let (mut season, episode) = parser::extract_episode_from_filename(&video.filename);

        if episode.is_none() {
            tracing::warn!(
                "[ORGANIZED-FOLDER] Cannot extract episode from: {} (in folder tmdb{})",
                video.filename,
                folder_info.tmdb_id
            );
            return Ok(None);
        }

        // Try to extract season from parent directory name
        if season.is_none() || season == Some(1) {
            let parent_name = video
                .parent_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if let Some(dir_season) = parser::extract_season_from_dirname(parent_name) {
                season = Some(dir_season);
            }
        }

        let season = season.unwrap_or(1);
        let episode = episode.unwrap();

        println!(
            "    [ORGANIZED-FOLDER] New file: {} -> {} S{:02}E{:02} (tmdb{})",
            video.filename, folder_info.title, season, episode, folder_info.tmdb_id
        );

        // Fetch TV show details using TMDB ID from folder
        let tmdb = match self.tmdb_client.as_ref() {
            Some(client) => client,
            None => {
                tracing::warn!("[ORGANIZED-FOLDER] TMDB client not initialized");
                return Ok(None);
            }
        };

        let show_meta = match tmdb.get_tv_details(folder_info.tmdb_id).await {
            Ok(details) => self.build_tv_series_metadata_from_details(&details),
            Err(e) => {
                tracing::warn!(
                    "[ORGANIZED-FOLDER] Failed to fetch TV details for tmdb{}: {}",
                    folder_info.tmdb_id,
                    e
                );
                return Ok(None);
            }
        };

        // Get episode metadata
        let ep_meta = self
            .get_episode_from_cache(show_meta.tmdb_id, season, episode, season_cache)
            .await;

        // Get season metadata
        let season_meta = match tmdb.get_season_details(folder_info.tmdb_id, season).await {
            Ok(season_details) => Some(SeasonMetadata {
                season_number: season_details.season_number,
                name: season_details.name,
                overview: season_details.overview,
                air_date: season_details.air_date,
                poster_url: season_details.poster_path.map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)),
                episode_count: season_details.episodes.len() as u16,
            }),
            Err(e) => {
                tracing::warn!("[ORGANIZED-FOLDER] Failed to get season {} details for tmdb{}: {}", season, folder_info.tmdb_id, e);
                None
            }
        };

        let parsed = ParsedFilename {
            title: Some(folder_info.title.clone()),
            original_title: None,
            year: folder_info.year,
            season: Some(season),
            episode: Some(episode),
            confidence: 1.0,
            raw_response: Some("organized_folder".to_string()),
        };

        // Get video metadata
        let video_metadata = match precomputed_ffprobe {
            Some(meta) => meta.clone(),
            None => {
                let ffprobe_meta = ffprobe::extract_metadata(&video.path).unwrap_or_default();
                let filename_meta = ffprobe::parse_metadata_from_filename(&video.filename);
                ffprobe::merge_metadata(ffprobe_meta, filename_meta)
            }
        };

        // Generate target info
        let tv_series_with_episode = Some((show_meta.clone(), ep_meta.clone(), season_meta.clone()));

        let (target_info, operations, poster_download) = match self.generate_target_info(
            video,
            &None,
            &tv_series_with_episode,
            &parsed,
            &video_metadata,
            target,
            MediaType::TvSeries,
        )? {
            Some(result) => result,
            None => return Ok(None),
        };

        let plan_item = PlanItem {
            id: Uuid::new_v4().to_string(),
            source: video.clone(),
            parsed: ParsedInfo {
                title: parsed.title.clone(),
                original_title: parsed.original_title.clone(),
                year: parsed.year,
                confidence: parsed.confidence,
                raw_response: parsed.raw_response.clone(),
            },
            movie_metadata: None,
            tv_series_metadata: Some(show_meta.clone()),
            episode_metadata: ep_meta,
            season_metadata: season_meta,
            video_metadata: video_metadata.clone(),
            target: target_info,
            operations,
            status: PlanItemStatus::Pending,
            poster_download,
        };

        Ok(Some((plan_item, Some(show_meta))))
    }

    /// Process organized TV series folders that may not contain video files.
    /// This generates season NFO files for folders that have tvshow.nfo but no videos.
    async fn process_organized_tv_folders(
        &self,
        folders: &[PathBuf],
        _target: &Path,
    ) -> Result<Vec<PlanItem>> {
        let mut items = Vec::new();
        
        if folders.is_empty() {
            return Ok(items);
        }
        
        let tmdb = match self.tmdb_client.as_ref() {
            Some(client) => client,
            None => {
                tracing::warn!("TMDB client not initialized, skipping organized TV folders");
                return Ok(items);
            }
        };
        
        for folder_path in folders {
            // Extract folder info from directory name
            let folder_name = folder_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Some(folder_info) = parser::parse_organized_tv_series_folder(folder_name) {
                tracing::info!(
                    "[ORGANIZED-TV-FOLDER] Processing: {} (tmdb{})",
                    folder_info.title, folder_info.tmdb_id
                );
                
                // Fetch TV show details
                let show_meta = match self.fetch_tv_series_by_id(folder_info.tmdb_id).await {
                    Ok(meta) => meta,
                    Err(e) => {
                        tracing::warn!("Failed to fetch TV series info for tmdb{}: {}", folder_info.tmdb_id, e);
                        continue;
                    }
                };
                
                // Get season directories
                let mut seasons = Vec::new();
                if let Ok(entries) = std::fs::read_dir(folder_path) {
                    for entry in entries.flatten() {
                        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            if let Some(dir_name) = entry.file_name().to_str() {
                                if let Some(season_num) = parser::extract_season_from_dirname(dir_name) {
                                    seasons.push((season_num, entry.path()));
                                }
                            }
                        }
                    }
                }
                
                // If no season directories found, create entries for all seasons
                if seasons.is_empty() {
                    for season_num in 1..=show_meta.number_of_seasons {
                        seasons.push((season_num, folder_path.join(format!("Season {:02}", season_num))));
                    }
                }
                
                // Process each season
                for (season_num, season_path) in seasons {
                    // Fetch season metadata
                    let season_meta = match tmdb.get_season_details(folder_info.tmdb_id, season_num).await {
                        Ok(season_details) => Some(SeasonMetadata {
                            season_number: season_details.season_number,
                            name: season_details.name,
                            overview: season_details.overview,
                            air_date: season_details.air_date,
                            poster_url: season_details.poster_path.map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)),
                            episode_count: season_details.episodes.len() as u16,
                        }),
                        Err(e) => {
                            tracing::warn!("Failed to get season {} details for tmdb{}: {}", season_num, folder_info.tmdb_id, e);
                            continue;
                        }
                    };
                    
                    // Generate season NFO operation
                    if season_meta.is_some() && self.config.generate_nfo && self.config.generate_tv_season_nfo {
                            let season_nfo_name = format!("[{}]-season{:02}.nfo", folder_info.title, season_num);
                            let season_nfo_path = season_path.join(&season_nfo_name);
                            
                            // Create directory if it doesn't exist
                            let mut operations = Vec::new();
                            operations.push(Operation {
                                op: OperationType::Mkdir,
                                from: None,
                                to: season_path.clone(),
                                url: None,
                                content_ref: None,
                            });
                            
                            // Create season NFO
                            operations.push(Operation {
                                op: OperationType::Create,
                                from: None,
                                to: season_nfo_path.clone(),
                                url: None,
                                content_ref: Some("nfo".to_string()),
                            });
                            
                            // Use tvshow.nfo as source file since it exists in organized folders
                            let tvshow_nfo_path = folder_path.join("tvshow.nfo");
                            let source_video = VideoFile {
                                path: tvshow_nfo_path.clone(),
                                filename: "tvshow.nfo".to_string(),
                                parent_dir: folder_path.clone(),
                                size: std::fs::metadata(&tvshow_nfo_path).map(|m| m.len()).unwrap_or(0),
                                modified: chrono::Utc::now(),
                                is_sample: false,
                            };
                            
                            let plan_item = PlanItem {
                                id: Uuid::new_v4().to_string(),
                                source: source_video,
                                parsed: ParsedInfo {
                                    title: Some(folder_info.title.clone()),
                                    original_title: None,
                                    year: folder_info.year,
                                    confidence: 1.0,
                                    raw_response: None,
                                },
                                movie_metadata: None,
                                tv_series_metadata: Some(show_meta.clone()),
                                episode_metadata: None,
                                season_metadata: season_meta,
                                video_metadata: VideoMetadata::default(),
                                target: TargetInfo {
                                    folder: season_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
                                    filename: season_nfo_name.clone(),
                                    full_path: season_nfo_path.clone(),
                                    nfo: season_nfo_name,
                                    poster: None,
                                },
                                operations,
                                status: PlanItemStatus::Pending,
                                poster_download: None,
                            };
                            
                            items.push(plan_item);
                    }
                }
            }
        }
        
        Ok(items)
    }

    /// Find an organized TV show folder by looking up the directory hierarchy.
    ///
    /// Since organized TV shows may have structure like:
    /// `[Show](Year)-ttIMDB-tmdbID/Season 01/[Show]-S01E01-...mp4`
    /// We need to look at parent directories, not just the immediate parent.
    fn find_organized_tv_series_folder(
        &self,
        start_dir: &Path,
    ) -> Option<parser::OrganizedTvSeriesFolderInfo> {
        let mut current = Some(start_dir);

        while let Some(dir) = current {
            if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
                // Try to parse as organized folder
                if let Some(info) = parser::parse_organized_tv_series_folder(name) {
                    tracing::debug!("Found organized TV folder: {} (tmdb{})", name, info.tmdb_id);
                    return Some(info);
                }
            }
            current = dir.parent();
        }

        None
    }

    /// Find an organized movie folder by searching parent directories.
    fn find_organized_movie_folder(
        &self,
        start_dir: &Path,
    ) -> Option<parser::OrganizedMovieFolderInfo> {
        let mut current = Some(start_dir);

        while let Some(dir) = current {
            if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
                // Try to parse as organized folder
                if let Some(info) = parser::parse_organized_movie_folder(name) {
                    tracing::debug!(
                        "Found organized movie folder: {} (tmdb{})",
                        name,
                        info.tmdb_id
                    );
                    return Some(info);
                }
            }
            current = dir.parent();
        }

        None
    }

    /// Fetch TV show metadata by TMDB ID.
    async fn fetch_tv_series_by_id(&self, tmdb_id: u64) -> Result<TvSeriesMetadata> {
        let tmdb = match self.tmdb_client.as_ref() {
            Some(client) => client,
            None => {
                return Err(crate::error::Error::Other(
                    "TMDB client not initialized".to_string(),
                ));
            }
        };

        let details = tmdb.get_tv_details(tmdb_id).await?;
        Ok(self.build_tv_series_metadata_from_details(&details))
    }

    /// Build TvSeriesMetadata from TMDB TvDetails (used for organized files with TMDB ID).
    fn build_tv_series_metadata_from_details(
        &self,
        details: &crate::services::tmdb::TvDetails,
    ) -> TvSeriesMetadata {
        use crate::models::media::Actor;

        let genres: Vec<String> = details
            .genres
            .as_ref()
            .map(|gs| gs.iter().map(|g| g.name.clone()).collect())
            .unwrap_or_default();

        // Always prefer origin_country over production_countries
        // origin_country is more accurate for the content's true origin
        // production_countries may include co-production countries or have TMDB data errors
        // Example: "在劫难逃" has origin_country=["CN"] but production_countries=[{MO}]
        let (mut country_codes, mut countries): (Vec<String>, Vec<String>) =
            if let Some(ref origin) = details.origin_country {
                if !origin.is_empty() {
                    let codes = origin.clone();
                    let names = origin
                        .iter()
                        .map(|code| country_code_to_name(code))
                        .collect();
                    (codes, names)
                } else {
                    (Vec::new(), Vec::new())
                }
            } else {
                (Vec::new(), Vec::new())
            };

        // Fallback: use production_countries if origin_country is empty
        if country_codes.is_empty() {
            if let Some(ref pc) = details.production_countries {
                country_codes = pc.iter().map(|c| c.iso_3166_1.clone()).collect();
                countries = pc.iter().map(|c| c.name.clone()).collect();
            }
        }

        let networks: Vec<String> = details
            .networks
            .as_ref()
            .map(|ns| ns.iter().map(|n| n.name.clone()).collect())
            .unwrap_or_default();

        let creators: Vec<String> = details
            .created_by
            .as_ref()
            .map(|cs| cs.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default();

        let actors: Vec<Actor> = details
            .credits
            .as_ref()
            .and_then(|c| c.cast.as_ref())
            .map(|cast| {
                cast.iter()
                    .take(10)
                    .map(|a| Actor {
                        name: a.name.clone(),
                        role: a.character.clone(),
                        order: a.order,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let imdb_id = details
            .external_ids
            .as_ref()
            .and_then(|e| e.imdb_id.clone());

        let year = details
            .first_air_date
            .as_ref()
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse().ok())
            .unwrap_or(0);

        let poster_urls: Vec<String> = details
            .poster_path
            .as_ref()
            .map(|p| vec![format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)])
            .unwrap_or_default();

        let backdrop_url = details
            .backdrop_path
            .as_ref()
            .map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p));

        TvSeriesMetadata {
            tmdb_id: details.id,
            imdb_id,
            original_name: details.original_name.clone(),
            name: details.name.clone(),
            original_language: details.original_language.clone(),
            year,
            first_air_date: details.first_air_date.clone(),
            overview: details.overview.clone(),
            tagline: details.tagline.clone(),
            genres,
            countries,
            country_codes,
            networks,
            rating: details.vote_average,
            votes: details.vote_count,
            number_of_seasons: details.number_of_seasons,
            number_of_episodes: details.number_of_episodes,
            status: details.status.clone(),
            creators,
            actors,
            poster_urls,
            backdrop_url,
        }
    }

    /// Get episode info from season cache, fetching entire season if not cached.
    async fn get_episode_from_cache(
        &self,
        tmdb_id: u64,
        season: u16,
        episode: u16,
        cache: &SeasonEpisodesCache,
    ) -> Option<EpisodeMetadata> {
        let cache_key = (tmdb_id, season);

        // Check cache first
        {
            let read_cache = cache.read().await;
            if let Some(episodes) = read_cache.get(&cache_key) {
                // Find the episode in cached data
                if let Some(ep_info) = episodes.iter().find(|e| e.episode_number == episode) {
                    return Some(EpisodeMetadata {
                        season_number: season,
                        episode_number: episode,
                        name: ep_info.name.clone(),
                        original_name: None,
                        air_date: ep_info.air_date.clone(),
                        overview: ep_info.overview.clone(),
                    });
                }
            }
        }

        // Cache miss - fetch entire season
        if let Some(client) = &self.tmdb_client {
            match client.get_season_details(tmdb_id, season).await {
                Ok(season_details) => {
                    tracing::info!(
                        "Cached season {} info ({} episodes) for TMDB ID {}",
                        season,
                        season_details.episodes.len(),
                        tmdb_id
                    );

                    // Find the target episode first
                    let target_ep = season_details
                        .episodes
                        .iter()
                        .find(|e| e.episode_number == episode)
                        .map(|ep_info| EpisodeMetadata {
                            season_number: season,
                            episode_number: episode,
                            name: ep_info.name.clone(),
                            original_name: None,
                            air_date: ep_info.air_date.clone(),
                            overview: ep_info.overview.clone(),
                        });

                    // Update cache
                    {
                        let mut write_cache = cache.write().await;
                        write_cache.insert(cache_key, season_details.episodes);
                    }

                    return target_ep;
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch season {} details: {}", season, e);
                }
            }
        }

        // Fallback: return basic episode info
        Some(EpisodeMetadata {
            season_number: season,
            episode_number: episode,
            name: format!("Episode {}", episode),
            original_name: None,
            air_date: None,
            overview: None,
        })
    }

    /// Group videos by their immediate parent directory.
    ///
    /// This is the correct grouping for TV shows:
    /// - /Videos/TV_Series/Show1/01.mp4 -> parent_dir: /Videos/TV_Series/Show1
    /// - /Videos/TV_Series/Collection/ShowA/01.mp4 -> parent_dir: /Videos/TV_Series/Collection/ShowA
    /// - /Videos/TV_Series/Collection/ShowB/01.mp4 -> parent_dir: /Videos/TV_Series/Collection/ShowB
    ///
    /// Each parent directory represents a single TV show/season.
    fn group_by_top_level_dir(
        &self,
        videos: &[VideoFile],
        _source: &Path,
    ) -> std::collections::HashMap<PathBuf, Vec<VideoFile>> {
        let mut groups: std::collections::HashMap<PathBuf, Vec<VideoFile>> =
            std::collections::HashMap::new();

        for video in videos {
            // Use intelligent parent lookup to determine the grouping key
            // This groups files from "4K/", "1080p/", "S01/", "S02/" under their common
            // meaningful parent directory
            let group_key = Self::find_meaningful_parent_dir(video);
            groups.entry(group_key).or_default().push(video.clone());
        }

        groups
    }

    /// Find the meaningful parent directory path (not just name) for grouping.
    ///
    /// This returns the path to the first parent directory that has a meaningful name.
    /// Used for grouping videos that are in technical subdirectories.
    fn find_meaningful_parent_dir(video: &VideoFile) -> PathBuf {
        const MAX_DEPTH: usize = 3;
        let mut current = video.parent_dir.as_path();
        let mut depth = 1;

        while depth <= MAX_DEPTH {
            if let Some(name) = current.file_name().and_then(|n| n.to_str()) {
                // If this directory name is meaningful, use this path
                if !Self::is_meaningless_dirname(name) {
                    return current.to_path_buf();
                }

                // Otherwise, go up one level
                if let Some(parent) = current.parent() {
                    current = parent;
                    depth += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Fallback to immediate parent
        video.parent_dir.clone()
    }

    /// Process a single video file with optional cached TV show metadata.
    /// Returns the PlanItem and the TV show metadata (for caching).
    ///
    /// OPTIMIZATION: For TV shows with cached metadata, we extract episode numbers
    /// from filename using regex instead of calling AI for each file.
    #[allow(dead_code)]
    async fn process_single_video_with_cache(
        &self,
        video: &VideoFile,
        target: &Path,
        media_type: MediaType,
        cached_show: Option<&(TvSeriesMetadata, Option<EpisodeMetadata>)>,
    ) -> Result<Option<(PlanItem, Option<(TvSeriesMetadata, Option<EpisodeMetadata>, Option<SeasonMetadata>)>)>> {
        // Step 1: Parse filename - use regex for TV shows with cache, AI otherwise
        let parsed = if media_type == MediaType::TvSeries && cached_show.is_some() {
            // FAST PATH: Extract episode number from filename using regex (no AI call)
            let (season, episode) = parser::extract_episode_from_filename(&video.filename);
            tracing::debug!(
                "Regex extracted from {}: S{:?}E{:?}",
                video.filename,
                season,
                episode
            );

            if episode.is_none() {
                tracing::debug!("Could not extract episode number from: {}", video.filename);
                return Ok(None);
            }

            // Create a minimal parsed result with episode info
            ParsedFilename {
                title: cached_show.as_ref().map(|(s, _)| s.name.clone()),
                original_title: cached_show.as_ref().map(|(s, _)| s.original_name.clone()),
                year: cached_show.as_ref().map(|(s, _)| s.year),
                season,
                episode,
                confidence: 1.0, // High confidence for regex match
                raw_response: Some("regex_extracted".to_string()),
            }
        } else {
            // NORMAL PATH: Use AI to parse filename
            let parse_input = self.build_parse_input(video);
            let parsed = self.parser.parse(&parse_input, media_type).await?;

            if !self.parser.is_valid(&parsed) {
                tracing::debug!("Low confidence parsing for: {}", video.filename);
                return Ok(None);
            }
            parsed
        };

        // Step 2: Query TMDB based on media type
        // For movies, try to extract IMDB ID from filename for priority lookup
        let filename_imdb_id = metadata::extract_from_filename(&video.filename).imdb_id;

        let (movie_metadata, tv_series_metadata) = match media_type {
            MediaType::Movies => {
                let movie = self
                    .query_tmdb_movie_with_imdb(&parsed, filename_imdb_id.as_deref())
                    .await?;
                if movie.is_none() {
                    tracing::debug!("No TMDB match for movie: {}", video.filename);
                    return Ok(None);
                }
                (movie, None)
            }
            MediaType::TvSeries => {
                // Use cached show metadata if available (same directory = same show)
                if let Some((cached_show_meta, _)) = cached_show {
                    tracing::info!(
                        "Using cached TV show for {}: {} (S{:?}E{:?})",
                        video.filename,
                        cached_show_meta.name,
                        parsed.season,
                        parsed.episode
                    );
                    // Get episode info for this specific file using regex-extracted numbers
                    let (episode, season_metadata) = if let (Some(season), Some(ep)) = (parsed.season, parsed.episode)
                    {
                        let mut season_meta: Option<SeasonMetadata> = None;
                        let episode_meta = if let Some(client) = &self.tmdb_client {
                            // Try to get season details first
                            if let Ok(season_details) = client.get_season_details(cached_show_meta.tmdb_id, season).await {
                                season_meta = Some(SeasonMetadata {
                                    season_number: season_details.season_number,
                                    name: season_details.name,
                                    overview: season_details.overview,
                                    air_date: season_details.air_date,
                                    poster_url: season_details.poster_path.map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)),
                                    episode_count: season_details.episodes.len() as u16,
                                });
                            }
                            
                            match client
                                .get_episode_details(cached_show_meta.tmdb_id, season, ep)
                                .await
                            {
                                Ok(ep_details) => Some(EpisodeMetadata {
                                    season_number: season,
                                    episode_number: ep,
                                    name: ep_details.name,
                                    original_name: None,
                                    air_date: ep_details.air_date,
                                    overview: ep_details.overview,
                                }),
                                Err(_) => Some(EpisodeMetadata {
                                    season_number: season,
                                    episode_number: ep,
                                    name: format!("Episode {}", ep),
                                    original_name: None,
                                    air_date: None,
                                    overview: None,
                                }),
                            }
                        } else {
                            Some(EpisodeMetadata {
                                season_number: season,
                                episode_number: ep,
                                name: format!("Episode {}", ep),
                                original_name: None,
                                air_date: None,
                                overview: None,
                            })
                        };
                        (episode_meta, season_meta)
                    } else {
                        (None, None)
                    };
                    (None, Some((cached_show_meta.clone(), episode, season_metadata)))
                } else {
                    // No cache, query TMDB with folder name as fallback
                    // Try to get meaningful folder name (skip quality descriptors)
                    let folder_name = self.get_meaningful_folder_name(&video.parent_dir);
                    let (show, mut episode) = self
                        .query_tmdb_tv_series_with_folder(&parsed, folder_name.as_deref())
                        .await?;
                    if show.is_none() {
                        tracing::debug!("No TMDB match for TV show: {}", video.filename);
                        return Ok(None);
                    }
                    let show_meta = show.unwrap();

                    // Track season metadata - always try to get it
                    let mut season_metadata: Option<SeasonMetadata> = None;

                    // Determine season number from episode metadata or parsed result
                    let season_num = episode.as_ref().map(|e| e.season_number)
                        .or_else(|| parsed.season)
                        .or_else(|| {
                            // Try to extract from filename
                            let (season, _) = parser::extract_episode_from_filename(&video.filename);
                            season
                        });

                    // Get season metadata if we have a season number
                    if let (Some(season), Some(client)) = (season_num, &self.tmdb_client) {
                        if let Ok(season_details) = client.get_season_details(show_meta.tmdb_id, season).await {
                            season_metadata = Some(SeasonMetadata {
                                season_number: season_details.season_number,
                                name: season_details.name,
                                overview: season_details.overview,
                                air_date: season_details.air_date,
                                poster_url: season_details.poster_path.map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)),
                                episode_count: season_details.episodes.len() as u16,
                            });
                        }
                    }

                    // If episode is None (AI didn't parse season/episode), try regex extraction
                    if episode.is_none() {
                        let (mut regex_season, regex_ep) =
                            parser::extract_episode_from_filename(&video.filename);
                        // Try to get season from parent directory name
                        if regex_season.is_none() || regex_season == Some(1) {
                            let parent_name = video
                                .parent_dir
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("");
                            if let Some(dir_s) = parser::extract_season_from_dirname(parent_name) {
                                regex_season = Some(dir_s);
                            }
                        }
                        tracing::debug!(
                            "Regex extraction for first file {}: S{:?}E{:?}",
                            video.filename,
                            regex_season,
                            regex_ep
                        );

                        if let (Some(season), Some(ep)) = (regex_season, regex_ep) {
                            if let Some(client) = &self.tmdb_client {
                                // Get season details if not already fetched
                                if season_metadata.is_none() {
                                    if let Ok(season_details) = client.get_season_details(show_meta.tmdb_id, season).await {
                                        season_metadata = Some(SeasonMetadata {
                                            season_number: season_details.season_number,
                                            name: season_details.name,
                                            overview: season_details.overview,
                                            air_date: season_details.air_date,
                                            poster_url: season_details.poster_path.map(|p| format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)),
                                            episode_count: season_details.episodes.len() as u16,
                                        });
                                    }
                                }

                                match client
                                    .get_episode_details(show_meta.tmdb_id, season, ep)
                                    .await
                                {
                                    Ok(ep_details) => {
                                        episode = Some(EpisodeMetadata {
                                            season_number: season,
                                            episode_number: ep,
                                            name: ep_details.name,
                                            original_name: None,
                                            air_date: ep_details.air_date,
                                            overview: ep_details.overview,
                                        });
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to get episode details for S{}E{}: {}",
                                            season,
                                            ep,
                                            e
                                        );
                                        episode = Some(EpisodeMetadata {
                                            season_number: season,
                                            episode_number: ep,
                                            name: format!("Episode {}", ep),
                                            original_name: None,
                                            air_date: None,
                                            overview: None,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    (None, Some((show_meta, episode, season_metadata)))
                }
            }
        };

        // Step 3: Extract video metadata with ffprobe + filename parsing
        let ffprobe_metadata = ffprobe::extract_metadata(&video.path).unwrap_or_default();
        let filename_metadata = ffprobe::parse_metadata_from_filename(&video.filename);

        // Merge: prefer ffprobe data, but use filename data as fallback
        let video_metadata = ffprobe::merge_metadata(ffprobe_metadata, filename_metadata);

        tracing::debug!(
            "Video metadata for {}: resolution={}, format={}, codec={}",
            video.filename,
            video_metadata.resolution,
            video_metadata.format,
            video_metadata.video_codec
        );

        // Step 4: Generate target paths
        let (target_info, operations, poster_download) = match self.generate_target_info(
            video,
            &movie_metadata,
            &tv_series_metadata,
            &parsed,
            &video_metadata,
            target,
            media_type,
        )? {
            Some(result) => result,
            None => return Ok(None), // Skip: cannot determine country
        };

        // Step 5: Create plan item
        let item = PlanItem {
            id: Uuid::new_v4().to_string(),
            status: PlanItemStatus::Pending,
            source: video.clone(),
            parsed: ParsedInfo {
                title: parsed.title,
                original_title: parsed.original_title,
                year: parsed.year,
                confidence: parsed.confidence,
                raw_response: parsed.raw_response,
            },
            movie_metadata,
            tv_series_metadata: tv_series_metadata.as_ref().map(|(show, _, _)| show.clone()),
            episode_metadata: tv_series_metadata.as_ref().and_then(|(_, ep, _)| ep.clone()),
            season_metadata: tv_series_metadata.as_ref().and_then(|(_, _, season)| season.clone()),
            video_metadata,
            target: target_info,
            operations,
            poster_download,
        };

        // Return item and tvshow metadata for caching
        Ok(Some((item, tv_series_metadata)))
    }

    /// Get a meaningful folder name from the path, skipping quality descriptors.
    /// Returns the first ancestor directory that looks like a show name.
    fn get_meaningful_folder_name(&self, path: &Path) -> Option<String> {
        // Generic folder names to skip (not actual show titles)
        let is_generic_folder = |name: &str| -> bool {
            let lower = name.to_lowercase();
            matches!(
                lower.as_str(),
                "tv series"
                    | "tv shows"
                    | "tv show"
                    | "series"
                    | "show"
                    | "movies"
                    | "movie"
                    | "films"
                    | "film"
                    | "anime"
                    | "documentary"
                    | "documentaries"
                    | "music"
                    | "music videos"
                    | "concert"
                    | "concerts"
            )
        };

        // Quality descriptor patterns to skip
        let is_quality_desc = |name: &str| -> bool {
            let lower = name.to_lowercase();
            // Skip generic folder names
            if is_generic_folder(&lower) {
                return true;
            }
            // Skip quality descriptors
            if lower.contains("1080")
                || lower.contains("720")
                || lower.contains("2160")
                || lower.contains("4k")
                || lower.contains("内封")
                || lower.contains("外挂")
                || lower.contains("字幕")
            {
                return true;
            }
            // Skip season directories: S1, S01, Season 1, Season.2, 第1季, ShowName.S01, ShowNameS01
            if regex::Regex::new(r"(?i)(^s\d{1,2}$|[^\w]s\d{1,2}[^\w]|[^\w]s\d{1,2}$|s\d{1,2}$)")
                .map(|re| re.is_match(&lower))
                .unwrap_or(false)
            {
                return true;
            }
            if regex::Regex::new(r"^season[\s._-]?\d{1,2}$")
                .map(|re| re.is_match(&lower))
                .unwrap_or(false)
            {
                return true;
            }
            if regex::Regex::new(r"^第\d{1,2}季$")
                .map(|re| re.is_match(name))
                .unwrap_or(false)
            {
                return true;
            }
            // Skip per-episode folders: "NIGEHAJI.E05.720p", "Show.S01E01.WEB"
            // These contain episode numbers and are NOT show titles
            if regex::Regex::new(r"(?i)[\.\s_-]s\d{1,2}e\d{1,3}[\.\s_-]")
                .map(|re| re.is_match(name))
                .unwrap_or(false)
            {
                return true;
            }
            if regex::Regex::new(r"(?i)[\.\s_-]e\d{1,3}[\.\s_-]")
                .map(|re| re.is_match(name))
                .unwrap_or(false)
            {
                return true;
            }
            // Also match at start/end: "E01.720p" or "Show.E05"
            if regex::Regex::new(r"(?i)^e\d{1,3}[\.\s_-]|[\.\s_-]e\d{1,3}$")
                .map(|re| re.is_match(name))
                .unwrap_or(false)
            {
                return true;
            }
            false
        };

        // Try immediate parent first
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if !is_quality_desc(name) {
                return Some(name.to_string());
            }
        }

        // Try grandparent if immediate parent is a quality descriptor
        if let Some(parent) = path.parent() {
            if let Some(name) = parent.file_name().and_then(|n| n.to_str()) {
                if !is_quality_desc(name) {
                    return Some(name.to_string());
                }
            }
        }

        // Try great-grandparent (3 levels up)
        if let Some(parent) = path.parent().and_then(|p| p.parent()) {
            if let Some(name) = parent.file_name().and_then(|n| n.to_str()) {
                if !is_quality_desc(name) {
                    return Some(name.to_string());
                }
            }
        }

        None
    }

    /// Build the input string for AI parsing.
    ///
    /// If the file is in a subdirectory with a meaningful name, include it for better context.
    /// This function now uses intelligent parent directory lookup to skip meaningless
    /// subdirectories like "4K", "S01", "WEB-DL" etc., and removes sorting prefixes.
    fn build_parse_input(&self, video: &VideoFile) -> String {
        // Use intelligent parent lookup to skip meaningless directories
        let (parent_name, depth) = Self::find_meaningful_parent_name(video);

        // Strip sorting prefix from parent name (A_, X_, 01_, etc.)
        let clean_parent = Self::strip_sorting_prefix(&parent_name);

        // Check if filename lacks meaningful title info
        // (e.g., just year + format like "2024 SP.mp4" or "E01.mkv")
        let filename_seems_minimal = self.is_minimal_filename(&video.filename);

        // Check if parent directory has meaningful name (after stripping prefix)
        let parent_has_title = !clean_parent.is_empty()
            && clean_parent != "Movies"
            && clean_parent != "movies"
            && clean_parent != "TvSeries"
            && clean_parent != "tv_series"
            && !clean_parent.starts_with(".")
            && clean_parent.len() > 3;

        // NEW: Check if parent has CJK characters but filename is mostly Latin
        // This handles cases like "逃避虽可耻但有用/NIGEHAJI.E01.720p.FIX字幕侠/..."
        // where the parent dir has the proper CJK title but filename uses romanized name
        let parent_has_cjk = clean_parent.chars().any(|c| {
            matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3040}'..='\u{30ff}' | '\u{ac00}'..='\u{d7af}')
        });

        // Count meaningful CJK characters in filename (excluding common ones like 字幕侠)
        let filename_cjk_count = video.filename.chars()
            .filter(|c| matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3040}'..='\u{30ff}' | '\u{ac00}'..='\u{d7af}'))
            .count();

        // If parent has CJK title but filename is romanized (few CJK chars), use parent context
        // This is important for shows like NIGEHAJI (逃げ恥) where filename uses romanized name
        let filename_is_romanized = filename_cjk_count < 5
            && video
                .filename
                .chars()
                .take(20)
                .filter(|c| c.is_ascii_alphabetic())
                .count()
                >= 5;

        let needs_parent_context =
            filename_seems_minimal || (parent_has_cjk && filename_is_romanized && parent_has_title);

        // If filename already has CJK characters, don't combine with meaningless parent directories
        // This prevents issues where guessit incorrectly parses the parent dir as the title
        let filename_has_cjk = video.filename.chars().any(|c| {
            matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3040}'..='\u{30ff}' | '\u{ac00}'..='\u{d7af}')
        });
        
        if needs_parent_context && parent_has_title && !filename_has_cjk {
            // Combine parent dir name and filename for better context
            tracing::info!(
                "Using parent dir for context: '{}' + '{}' (depth: {})",
                clean_parent,
                video.filename,
                depth
            );
            format!("{} - {}", clean_parent, video.filename)
        } else {
            video.filename.clone()
        }
    }

    /// Extract candidate metadata using the unified metadata extraction system.
    ///
    /// This method implements the new processing flow:
    /// 1. Check if file is already organized format -> extract TMDB ID
    /// 2. Check if file is in organized folder -> extract TMDB ID from folder
    /// 3. Extract info from filename and directory using regex
    /// 4. Determine if AI parsing is needed
    #[allow(dead_code)]
    fn extract_candidate_metadata(&self, video: &VideoFile) -> CandidateMetadata {
        // Phase 1: Check for organized filename
        if parser::is_organized_filename(&video.filename) {
            // Try to parse as organized movie
            if let Some(movie_info) = parser::parse_organized_movie_filename(&video.filename) {
                return CandidateMetadata {
                    english_title: movie_info.original_title,
                    chinese_title: movie_info.title,
                    year: Some(movie_info.year),
                    tmdb_id: movie_info.tmdb_id,
                    imdb_id: movie_info.imdb_id,
                    source: Some(metadata::MetadataSource::OrganizedFilename),
                    confidence: 1.0,
                    ..Default::default()
                };
            }
            // Try to parse as organized TV show
            if let Some(tv_info) = parser::parse_organized_tv_series_filename(&video.filename) {
                return CandidateMetadata {
                    chinese_title: Some(tv_info.title),
                    season: Some(tv_info.season),
                    episode: Some(tv_info.episode),
                    source: Some(metadata::MetadataSource::OrganizedFilename),
                    confidence: 1.0,
                    ..Default::default()
                };
            }
        }

        // Phase 2: Check for organized folder in ancestry
        if let Some((DirectoryType::OrganizedDirectory(info), _path)) =
            metadata::find_title_directory(&video.parent_dir)
        {
            // Extract episode info from filename
            let (season, episode) = parser::extract_episode_from_filename(&video.filename);
            return CandidateMetadata {
                chinese_title: Some(info.title),
                year: info.year,
                tmdb_id: Some(info.tmdb_id),
                imdb_id: info.imdb_id,
                season,
                episode,
                source: Some(metadata::MetadataSource::OrganizedFolder),
                confidence: 1.0,
                ..Default::default()
            };
        }

        // Phase 3: Extract from filename
        let filename_info = metadata::extract_from_filename(&video.filename);

        // Phase 4: Extract from directory
        let mut dir_info = CandidateMetadata::default();
        if let Some(parent_name) = video.parent_dir.file_name().and_then(|n| n.to_str()) {
            let dir_type = metadata::classify_directory(parent_name);
            match dir_type {
                DirectoryType::TitleDirectory(title_info) => {
                    dir_info.chinese_title = title_info.chinese_title;
                    dir_info.english_title = title_info.english_title;
                    dir_info.year = title_info.year;
                    dir_info.source = Some(metadata::MetadataSource::DirectoryName);
                    dir_info.confidence = 0.7;
                }
                DirectoryType::SeasonDirectory(season) => {
                    // Season from directory, look for title in parent
                    dir_info.season = Some(season);
                    if let Some(grandparent) = video.parent_dir.parent() {
                        if let Some(name) = grandparent.file_name().and_then(|n| n.to_str()) {
                            if let DirectoryType::TitleDirectory(title_info) =
                                metadata::classify_directory(name)
                            {
                                dir_info.chinese_title = title_info.chinese_title;
                                dir_info.english_title = title_info.english_title;
                                dir_info.year = title_info.year;
                            }
                        }
                    }
                    dir_info.source = Some(metadata::MetadataSource::DirectoryName);
                    dir_info.confidence = 0.6;
                }
                DirectoryType::OrganizedDirectory(info) => {
                    dir_info.chinese_title = Some(info.title);
                    dir_info.year = info.year;
                    dir_info.tmdb_id = Some(info.tmdb_id);
                    dir_info.imdb_id = info.imdb_id;
                    dir_info.source = Some(metadata::MetadataSource::OrganizedFolder);
                    dir_info.confidence = 1.0;
                }
                _ => {}
            }
        }

        // Phase 5: Merge info (filename takes priority)
        let merged = metadata::merge_info(filename_info.clone(), dir_info);

        // If we still don't have enough info, mark for AI parsing
        if !merged.has_searchable_info() {
            let mut result = merged;
            result.needs_ai_parsing = true;
            return result;
        }

        merged
    }

    /// Check if a filename lacks meaningful title information.
    /// Format country folder name from ISO code and country name.
    /// Returns format like "CN_China", "US_UnitedStates", "KR_SouthKorea".
    /// Format country folder name from ISO code and country name.
    /// Uses original_language to pick the best country for co-productions.
    /// Deduplicate operations across all items.
    ///
    /// This handles cases where:
    /// 1. Multiple videos in the same directory share subtitles (Move operations)
    /// 2. Multiple episodes share the same tvshow.nfo (Create operations)
    /// 3. Multiple episodes share the same poster.jpg (Download operations)
    ///
    /// When two items have the same target file, keep only the first occurrence.
    fn deduplicate_operations(&self, items: &mut [PlanItem]) {
        use std::collections::HashSet;

        // Track seen sources (for Move operations - to avoid moving same file twice)
        let mut seen_sources: HashSet<PathBuf> = HashSet::new();
        // Track seen targets (for Create/Download operations - to avoid creating/downloading same file twice)
        let mut seen_targets: HashSet<PathBuf> = HashSet::new();
        let mut removed_count = 0;

        for item in items.iter_mut() {
            let original_len = item.operations.len();

            item.operations.retain(|op| {
                match op.op {
                    OperationType::Move => {
                        if let Some(ref source) = op.from {
                            // If we've already seen this source file, skip it
                            if seen_sources.contains(source) {
                                return false;
                            }
                            seen_sources.insert(source.clone());
                        }
                    }
                    OperationType::Create | OperationType::Download => {
                        // For Create/Download, deduplicate by target path
                        // This prevents tvshow.nfo and poster.jpg from being created multiple times
                        if seen_targets.contains(&op.to) {
                            return false;
                        }
                        seen_targets.insert(op.to.clone());
                    }
                    _ => {}
                }
                true
            });

            removed_count += original_len - item.operations.len();
        }

        if removed_count > 0 {
            tracing::info!(
                "Deduplicated {} duplicate operations (shared files)",
                removed_count
            );
        }
    }

    /// SAFETY CHECK: Validate that no two items have the same target path.
    /// This prevents data loss from files overwriting each other.
    /// Checks all operation types (Move, Create, Download) for target conflicts.
    /// NOTE: Different file types (e.g., .jpg and .mp4) with the same base name are allowed
    /// because they won't overwrite each other.
    pub fn validate_no_duplicate_targets(&self, items: &mut [PlanItem]) -> Result<()> {
        use std::collections::HashMap;

        let mut target_to_sources: HashMap<PathBuf, Vec<(usize, PathBuf, OperationType)>> = HashMap::new();

        for (idx, item) in items.iter().enumerate() {
            // 只检查 Pending 状态的项目，跳过已经被标记为 Skip/Error 的项目
            // 避免重复调用时重复处理已经处理过的冲突
            if item.status != PlanItemStatus::Pending {
                continue;
            }

            for op in &item.operations {
                // Check all operation types for target conflicts
                // Mkdir operations don't need conflict checking as they just create directories
                match op.op {
                    OperationType::Move | OperationType::Create | OperationType::Download => {
                        // Use the actual source file path from the operation, not the item's source path
                        // This is important because an item can have multiple operations (video, subtitles, posters)
                        // with different source files
                        let source_path = match op.op {
                            OperationType::Move => op.from.clone().unwrap_or_else(|| item.source.path.clone()),
                            OperationType::Create | OperationType::Download => item.source.path.clone(),
                            _ => unreachable!(), // Already filtered above
                        };
                        target_to_sources
                            .entry(op.to.clone())
                            .or_default()
                            .push((idx, source_path, op.op));
                    }
                    OperationType::Mkdir => {
                        // Mkdir operations don't need conflict checking
                    }
                }
            }
        }

        // Filter out false positives: different file types with same base name
        // e.g., .jpg and .mp4 files can coexist
        // Only consider as duplicate if there are multiple sources with the same extension
        let true_duplicates: Vec<_> = target_to_sources
            .iter()
            .filter(|(_, sources)| {
                // If only one source, it's not a duplicate
                if sources.len() <= 1 {
                    return false;
                }

                // Check if all sources have the same extension
                let extensions: Vec<_> = sources
                    .iter()
                    .map(|(_, src, _)| {
                        src.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_lowercase())
                            .unwrap_or_default()
                    })
                    .collect();
                
                // Only a true duplicate if all extensions are the same
                extensions.iter().all(|ext| ext == &extensions[0])
            })
            .collect();

        if !true_duplicates.is_empty() {
            let mut removed_count = 0;
            let mut warning_msg = String::from(
                "⚠️  WARNING: Duplicate target paths detected, marking as unknown to prevent data loss:\n\n",
            );

            for (target, sources) in true_duplicates.iter() {
                warning_msg.push_str(&format!("Target: {:?}\n", target));
                warning_msg.push_str("  These files would collide:\n");
                for (idx, src, _op_type) in sources.iter().skip(1) {
                    warning_msg.push_str(&format!("    - {:?}\n", src));
                    // Mark duplicate items as failed, remove their operations
                    if let Some(item) = items.get_mut(*idx) {
                        item.status = PlanItemStatus::Skip;
                        item.operations.clear();
                        removed_count += 1;
                    }
                }
                warning_msg.push('\n');
            }

            warning_msg.push_str(&format!(
                "⚠️  Marked {} duplicate items as unknown, they will be skipped.\n",
                removed_count
            ));
            warning_msg.push_str("Plan will continue with the remaining valid items.");

            tracing::warn!("{}", warning_msg);
            println!("{}", warning_msg);
            println!();
        } else {
            tracing::info!("Safety check passed: No duplicate target paths (different file types allowed)");
        }

        Ok(())
    }

    /// Add shortened versions of a long title to the queries list.
    /// This helps match titles like "破坏不在场证明 特别篇 钟表店侦探与祖父的不在场证明"
    /// which should match "破坏不在场证明 特别篇" on TMDB.
    fn add_shortened_queries(&self, queries: &mut Vec<String>, title: &str) {
        // Split by common delimiters
        // Common delimiters for splitting long titles (ASCII only for compatibility)
        let delimiters = [" - ", ":", " "];

        for delim in delimiters {
            if let Some(pos) = title.find(delim) {
                let shortened = title[..pos].trim().to_string();
                if shortened.len() >= 4 && !queries.contains(&shortened) {
                    tracing::debug!("Adding shortened query: {}", shortened);
                    queries.push(shortened);
                }
            }
        }

        // For very long titles (>20 chars), try taking just the first part before space
        if title.chars().count() > 20 {
            // Split by space and take progressively longer parts
            let parts: Vec<&str> = title.split_whitespace().collect();
            if parts.len() >= 2 {
                // Try first two parts
                let shortened = parts[..2.min(parts.len())].join(" ");
                if shortened.len() >= 4 && !queries.contains(&shortened) {
                    queries.push(shortened);
                }
            }
        }
    }

    fn is_minimal_filename(&self, filename: &str) -> bool {
        let name = filename.to_lowercase();

        // Remove extension
        let name_without_ext = if let Some(pos) = name.rfind('.') {
            &name[..pos]
        } else {
            &name
        };

        // Count meaningful characters (excluding common technical terms)
        let alphanumeric_count = name_without_ext
            .chars()
            .filter(|c| c.is_alphanumeric())
            .count();

        // If the meaningful part is very short, consider it minimal
        if alphanumeric_count <= 8 {
            return true;
        }

        // Check if filename is mostly technical info (codec, resolution, release group)
        // Pattern: "11.4K.H265.AAC-YYDS" - starts with episode number followed by tech info
        let tech_terms = [
            "4k", "1080p", "720p", "2160p", "h264", "h265", "hevc", "x264", "x265", "aac", "dts",
            "ac3", "flac", "web-dl", "webrip", "bluray", "bdrip", "hdtv", "dvdrip", "remux", "hdr",
            "10bit", "8bit",
        ];

        let parts: Vec<&str> = name_without_ext.split(['.', '-', '_', ' ']).collect();

        // Check if first part is just a number (episode number)
        let first_is_number = parts
            .first()
            .map(|p| p.chars().all(|c| c.is_ascii_digit()) && p.len() <= 3)
            .unwrap_or(false);

        // Count how many parts are technical terms
        let tech_count = parts
            .iter()
            .skip(if first_is_number { 1 } else { 0 })
            .filter(|p| {
                tech_terms.iter().any(|t| p.contains(t))
                    || p.chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            })
            .count();

        // If most parts are technical or release group names, this is minimal
        if first_is_number && tech_count >= parts.len().saturating_sub(2) {
            tracing::debug!("Filename '{}' detected as minimal (tech-only)", filename);
            return true;
        }

        // Check for year-only pattern like "2024 SP"
        if name.contains("sp") || name.contains("ova") || name.contains("特别") {
            let digits: String = name.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() == 4
                && digits
                    .parse::<u16>()
                    .map(|y| (1990..=2030).contains(&y))
                    .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }

    /// Strip sorting prefix from a directory or file name.
    ///
    /// Common sorting prefix patterns:
    /// - Single letter + separator: "A_剧名", "X-电影", "Z.标题"
    /// - Number + separator: "01_剧名", "001-电影", "1.标题"
    /// - These are used for alphabetical/numerical sorting in file managers
    ///
    /// Examples:
    /// - "X_许你耀眼" → "许你耀眼"
    /// - "H_回魂计" → "回魂计"
    /// - "01_电影名" → "电影名"
    /// - "A-剧名" → "剧名"
    /// - "1.标题" → "标题"
    fn strip_sorting_prefix(name: &str) -> &str {
        // Pattern 1: Single ASCII letter + separator (_, -, .)
        // e.g., "A_剧名", "X-电影", "Z.标题"
        if let Ok(re) = regex::Regex::new(r"^[A-Za-z][_\-.]") {
            if re.is_match(name) && name.len() > 2 {
                let stripped = &name[2..];
                if !stripped.is_empty() {
                    tracing::debug!("Stripped sorting prefix from '{}' -> '{}'", name, stripped);
                    return stripped;
                }
            }
        }

        // Pattern 2: Numbers + separator (_, -, .)
        // e.g., "01_剧名", "001-电影", "1.标题"
        if let Ok(re) = regex::Regex::new(r"^(\d{1,3})[_\-.]") {
            if let Some(caps) = re.captures(name) {
                let prefix_len = caps.get(0).map(|m| m.len()).unwrap_or(0);
                if prefix_len > 0 && name.len() > prefix_len {
                    let stripped = &name[prefix_len..];
                    if !stripped.is_empty() {
                        tracing::debug!(
                            "Stripped numeric sorting prefix from '{}' -> '{}'",
                            name,
                            stripped
                        );
                        return stripped;
                    }
                }
            }
        }

        name
    }

    /// Check if a directory name is "meaningless" for title extraction.
    ///
    /// These are common technical/organizational subdirectories that don't contain
    /// the actual media title. When encountered, we should look at parent directories.
    ///
    /// Examples:
    /// - Resolution: "4K", "1080p", "2160p", "720p"
    /// - Season: "S01", "S02", "Season 1", "Season.2"
    /// - Quality: "WEB-DL", "BluRay", "BDRip"
    /// - Technical: "HEVC", "x265", "HDR"
    fn is_meaningless_dirname(name: &str) -> bool {
        let lower = name.to_lowercase();

        // Generic folder names that are not actual show titles
        if matches!(
            lower.as_str(),
            "tv series"
                | "tv shows"
                | "tv show"
                | "series"
                | "show"
                | "movies"
                | "movie"
                | "films"
                | "film"
                | "anime"
                | "documentary"
                | "documentaries"
                | "music"
                | "music videos"
                | "concert"
                | "concerts"
        ) {
            return true;
        }

        // Resolution patterns
        if regex::Regex::new(r"^(4k|1080p|2160p|720p|480p|uhd|hd|sd)$")
            .map(|re| re.is_match(&lower))
            .unwrap_or(false)
        {
            return true;
        }

        // Season patterns: S01, S02, Season 1, Season.2, 第1季, ShowNameS01, ShowName.S01
        if regex::Regex::new(r"(?i)(^s\d{1,2}$|[\._-]s\d{1,2}[\._-]|[\._-]s\d{1,2}$|s\d{1,2}$)")
            .map(|re| re.is_match(&lower))
            .unwrap_or(false)
        {
            return true;
        }
        if regex::Regex::new(r"^season[\s._-]?\d{1,2}$")
            .map(|re| re.is_match(&lower))
            .unwrap_or(false)
        {
            return true;
        }
        if regex::Regex::new(r"^第\d{1,2}季$")
            .map(|re| re.is_match(name))
            .unwrap_or(false)
        {
            return true;
        }

        // Episode folder patterns: folders containing SxxExx or Exx are per-episode folders
        // e.g., "NIGEHAJI.E05.720p.FIX字幕侠", "Show.S01E01.720p", "E01.1080p"
        // These are NOT show titles, skip them to find the actual show name
        if regex::Regex::new(r"(?i)[\.\s_-]s\d{1,2}e\d{1,3}[\.\s_-]")
            .map(|re| re.is_match(name))
            .unwrap_or(false)
        {
            return true;
        }
        if regex::Regex::new(r"(?i)[\.\s_-]e\d{1,3}[\.\s_-]")
            .map(|re| re.is_match(name))
            .unwrap_or(false)
        {
            return true;
        }
        // Also match at start/end: "E01.720p" or "Show.E05"
        if regex::Regex::new(r"(?i)^e\d{1,3}[\.\s_-]|[\.\s_-]e\d{1,3}$")
            .map(|re| re.is_match(name))
            .unwrap_or(false)
        {
            return true;
        }

        // Quality/source patterns
        let quality_patterns = [
            "web-dl", "webrip", "webdl", "web", "bluray", "blu-ray", "bdrip", "brrip", "dvdrip",
            "dvd", "hdtv", "hdtvrip", "remux", "encode", "repack",
        ];
        if quality_patterns.iter().any(|&p| lower == p) {
            return true;
        }

        // Codec patterns (when used as folder name)
        let codec_patterns = [
            "hevc", "h265", "x265", "h264", "x264", "avc", "hdr", "hdr10", "dolby", "dv", "atmos",
        ];
        if codec_patterns.iter().any(|&p| lower == p) {
            return true;
        }

        // Common organizational folders
        let org_patterns = ["video", "videos", "media", "downloads", "new", "temp"];
        if org_patterns.iter().any(|&p| lower == p) {
            return true;
        }

        false
    }

    /// Find the most meaningful parent directory name by skipping technical subdirectories.
    ///
    /// For a path like: `许你耀眼/4K/01 4K.mp4`
    /// Returns: ("许你耀眼", 2) - the meaningful name and depth
    ///
    /// For a path like: `暗夜情报员 The.Night.Agent (2023)/S02/S02E01.mp4`
    /// Returns: ("暗夜情报员 The.Night.Agent (2023)", 2) - the meaningful name and depth
    ///
    /// Max depth is limited to 3 levels to avoid going too far up.
    fn find_meaningful_parent_name(video: &VideoFile) -> (String, usize) {
        const MAX_DEPTH: usize = 3;
        let mut current = video.parent_dir.as_path();
        let mut depth = 1;

        while depth <= MAX_DEPTH {
            if let Some(name) = current.file_name().and_then(|n| n.to_str()) {
                // If this directory name is meaningful, use it
                if !Self::is_meaningless_dirname(name) {
                    tracing::debug!(
                        "Found meaningful parent at depth {}: '{}' for '{}'",
                        depth,
                        name,
                        video.filename
                    );
                    return (name.to_string(), depth);
                }

                // Otherwise, go up one level
                if let Some(parent) = current.parent() {
                    tracing::debug!(
                        "Skipping meaningless dirname '{}', going up to parent",
                        name
                    );
                    current = parent;
                    depth += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Fallback to immediate parent
        video
            .parent_dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| (s.to_string(), 1))
            .unwrap_or_default()
    }

    /// Find the most meaningful parent directory name and extract season info from path.
    ///
    /// Returns: (meaningful_name, depth, extracted_season)
    ///
    /// For a path like: `今夜我用身体恋爱/S1/file.mp4`
    /// Returns: ("今夜我用身体恋爱", 2, Some(1))
    #[allow(dead_code)]
    fn find_meaningful_parent_with_season(video: &VideoFile) -> (String, usize, Option<u16>) {
        const MAX_DEPTH: usize = 3;
        let mut current = video.parent_dir.as_path();
        let mut depth = 1;
        let mut extracted_season: Option<u16> = None;

        while depth <= MAX_DEPTH {
            if let Some(name) = current.file_name().and_then(|n| n.to_str()) {
                // Try to extract season from this directory name before deciding if it's meaningful
                if extracted_season.is_none() {
                    extracted_season = Self::extract_season_from_dirname(name);
                }

                // If this directory name is meaningful, use it
                if !Self::is_meaningless_dirname(name) {
                    tracing::debug!(
                        "Found meaningful parent at depth {}: '{}' for '{}', season={:?}",
                        depth,
                        name,
                        video.filename,
                        extracted_season
                    );
                    return (name.to_string(), depth, extracted_season);
                }

                // Otherwise, go up one level
                if let Some(parent) = current.parent() {
                    tracing::debug!(
                        "Skipping meaningless dirname '{}', going up to parent (season extracted: {:?})",
                        name, extracted_season
                    );
                    current = parent;
                    depth += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Fallback to immediate parent
        let name = video
            .parent_dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        (name, 1, extracted_season)
    }

    /// Extract season number from directory name.
    /// Handles: S01, S1, Season 1, Season.2, 第1季
    #[allow(dead_code)]
    fn extract_season_from_dirname(name: &str) -> Option<u16> {
        let lower = name.to_lowercase();

        // Pattern: S01, S1
        if let Ok(re) = regex::Regex::new(r"^s(\d{1,2})$") {
            if let Some(caps) = re.captures(&lower) {
                return caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
        }

        // Pattern: Season 1, Season.2, Season_3
        if let Ok(re) = regex::Regex::new(r"^season[\s._-]?(\d{1,2})$") {
            if let Some(caps) = re.captures(&lower) {
                return caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
        }

        // Pattern: 第1季
        if let Ok(re) = regex::Regex::new(r"^第(\d{1,2})季$") {
            if let Some(caps) = re.captures(name) {
                return caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
        }

        None
    }

    /// Try to get movie metadata directly using TMDB ID or IMDB ID from filename.
    ///
    /// This is the highest priority lookup - if we have an ID, skip AI parsing entirely.
    /// Returns None if no ID found or lookup fails.
    async fn try_direct_id_lookup(
        &self,
        filename_meta: &metadata::CandidateMetadata,
    ) -> Result<Option<MovieMetadata>> {
        let client = match &self.tmdb_client {
            Some(c) => c,
            None => return Ok(None),
        };

        // Run TMDB ID and IMDB ID lookups in parallel
        let tmdb_id = filename_meta.tmdb_id;
        let imdb_id = filename_meta.imdb_id.clone();

        let tmdb_lookup = async {
            if let Some(id) = tmdb_id {
                tracing::debug!("FILENAME-ID Trying TMDB ID: {}", id);
                match self.get_movie_details(client, id, None).await {
                    Ok(Some(movie)) => {
                        tracing::info!(
                            "FILENAME-ID Found movie via TMDB ID {}: {}",
                            id,
                            movie.title
                        );
                        Some(movie)
                    }
                    Ok(None) => {
                        tracing::warn!("FILENAME-ID TMDB ID {} returned no results", id);
                        None
                    }
                    Err(e) => {
                        tracing::warn!("FILENAME-ID TMDB ID {} lookup failed: {}", id, e);
                        None
                    }
                }
            } else {
                None
            }
        };

        let imdb_lookup = async {
            if let Some(ref id) = imdb_id {
                tracing::debug!("FILENAME-ID Trying IMDB ID: {}", id);
                match client.find_movie_by_imdb_id(id).await {
                    Ok(Some(tmdb_id)) => {
                        tracing::info!("FILENAME-ID IMDB {} -> TMDB {}", id, tmdb_id);
                        match self.get_movie_details(client, tmdb_id, None).await {
                            Ok(Some(movie)) => {
                                tracing::info!(
                                    "FILENAME-ID Found movie via IMDB ID {}: {}",
                                    id,
                                    movie.title
                                );
                                Some(movie)
                            }
                            Ok(None) => {
                                tracing::warn!(
                                    "FILENAME-ID TMDB ID {} (from IMDB {}) returned no results",
                                    tmdb_id,
                                    id
                                );
                                None
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "FILENAME-ID TMDB lookup for IMDB {} failed: {}",
                                    id,
                                    e
                                );
                                None
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("FILENAME-ID No TMDB match for IMDB ID: {}", id);
                        None
                    }
                    Err(e) => {
                        tracing::warn!("FILENAME-ID IMDB ID {} lookup failed: {}", id, e);
                        None
                    }
                }
            } else {
                None
            }
        };

        let (tmdb_result, imdb_result) = tokio::join!(tmdb_lookup, imdb_lookup);

        // Priority: TMDB ID result first (highest priority), then IMDB ID result
        if let Some(movie) = tmdb_result {
            return Ok(Some(movie));
        }
        if let Some(movie) = imdb_result {
            return Ok(Some(movie));
        }

        Ok(None)
    }

    /// Try to find a TV show TMDB ID by looking at parent directories.
    ///
    /// This is used when the current directory's IMDB ID is not found in TMDB.
    /// For example, season-specific IMDB IDs (tt13660696 for Slow Horses S2) are not
    /// recognized by TMDB's find API, but the parent directory might have the show's
    /// main IMDB ID (tt5875444 for Slow Horses).
    ///
    /// Returns Some(tmdb_id) if found, None otherwise.
    async fn try_parent_directory_id_lookup(
        &self,
        file_path: &std::path::Path,
        failed_imdb_id: &str,
        client: &TmdbClient,
    ) -> Option<u64> {
        // Get the parent directory of the file
        let parent_dir = file_path.parent()?;

        // Look for IDs starting from the parent's parent (skip the current season directory)
        // e.g., for /L_流人/S02.tt13660696/file.mp4, we want to look at /L_流人/
        let (parent_tmdb_id, parent_imdb_id) = metadata::extract_ids_from_path_starting_at(
            file_path, parent_dir, // Start from parent of parent (skip season dir)
        );

        // If we found a TMDB ID directly, use it
        if let Some(tmdb_id) = parent_tmdb_id {
            tracing::info!(
                "[PARENT-ID] Found TMDB ID {} in parent directory (after {} failed)",
                tmdb_id,
                failed_imdb_id
            );
            return Some(tmdb_id);
        }

        // If we found a different IMDB ID, try to resolve it
        if let Some(ref imdb_id) = parent_imdb_id {
            if imdb_id != failed_imdb_id {
                match client.find_tv_by_imdb_id(imdb_id).await {
                    Ok(Some(tmdb_id)) => {
                        tracing::info!(
                            "[PARENT-ID] Resolved parent IMDB {} -> TMDB {} (after {} failed)",
                            imdb_id,
                            tmdb_id,
                            failed_imdb_id
                        );
                        return Some(tmdb_id);
                    }
                    Ok(None) => {
                        tracing::debug!(
                            "[PARENT-ID] Parent IMDB {} also not found in TMDB",
                            imdb_id
                        );
                    }
                    Err(e) => {
                        tracing::debug!(
                            "[PARENT-ID] Failed to lookup parent IMDB {}: {}",
                            imdb_id,
                            e
                        );
                    }
                }
            }
        }

        None
    }

    /// Query TMDB for movie metadata (convenience wrapper without IMDB ID).
    #[allow(dead_code)]
    async fn query_tmdb_movie(&self, parsed: &ParsedFilename) -> Result<Option<MovieMetadata>> {
        self.query_tmdb_movie_with_imdb(parsed, None).await
    }

    /// Query TMDB for movie metadata with optional IMDB ID.
    ///
    /// Priority:
    /// 1. If IMDB ID is provided, use find API to get TMDB ID directly (highest priority)
    /// 2. Otherwise, search by title with various strategies
    async fn query_tmdb_movie_with_imdb(
        &self,
        parsed: &ParsedFilename,
        imdb_id: Option<&str>,
    ) -> Result<Option<MovieMetadata>> {
        let client = match &self.tmdb_client {
            Some(c) => c,
            None => return Ok(None),
        };

        // Extract Chinese and English titles, filtering out meaningless ones
        let chinese_title = parsed
            .title
            .clone()
            .filter(|t| !t.is_empty())
            .filter(|t| self.is_meaningful_title(t));
        let english_title = parsed
            .original_title
            .clone()
            .filter(|t| !t.is_empty())
            .filter(|t| self.is_meaningful_title(t));

        // If both titles are meaningless, we can't search
        if chinese_title.is_none() && english_title.is_none() && imdb_id.is_none() {
            tracing::warn!(
                "Both titles are meaningless, cannot search TMDB: chinese={:?}, english={:?}",
                parsed.title,
                parsed.original_title
            );
            return Ok(None);
        }

        tracing::debug!(
            "TMDB movie search: chinese={:?}, english={:?}, year={:?}, imdb={:?}",
            chinese_title,
            english_title,
            parsed.year,
            imdb_id
        );

        // Run IMDB ID lookup and title searches ALL in parallel
        let chinese_title_clone = chinese_title.clone();
        let english_title_clone = english_title.clone();
        let search_year = parsed.year;
        let imdb_id_owned = imdb_id.map(|s| s.to_string());

        let tmdb_search_start = std::time::Instant::now();
        let (imdb_result, chinese_results, english_results) = tokio::join!(
            // IMDB ID lookup (runs in parallel with title searches)
            async {
                if let Some(ref imdb) = imdb_id_owned {
                    tracing::debug!("Trying IMDB ID lookup: {}", imdb);
                    match client.find_movie_by_imdb_id(imdb).await {
                        Ok(Some(tmdb_id)) => {
                            tracing::info!("TMDB found via IMDB ID {}: tmdb{}", imdb, tmdb_id);
                            self.get_movie_details(client, tmdb_id, chinese_title.as_deref()).await.ok().flatten()
                        }
                        Ok(None) => {
                            tracing::debug!("No TMDB match for IMDB ID: {}", imdb);
                            None
                        }
                        Err(e) => {
                            tracing::warn!("IMDB lookup failed for {}: {}", imdb, e);
                            None
                        }
                    }
                } else {
                    None
                }
            },
            // Chinese title search
            async {
                let mut results: Vec<crate::services::tmdb::MovieSearchItem> = Vec::new();
                if let Some(ref title) = chinese_title_clone {
                    let r = if let Some(year) = search_year {
                        client.search_movie(title, Some(year)).await
                    } else {
                        client.search_movie(title, None).await
                    };
                    if let Ok(r) = r {
                        results = r;
                    }
                }
                results
            },
            // English title search
            async {
                let mut results: Vec<crate::services::tmdb::MovieSearchItem> = Vec::new();
                if let Some(ref title) = english_title_clone {
                    let r = if let Some(year) = search_year {
                        client.search_movie(title, Some(year)).await
                    } else {
                        client.search_movie(title, None).await
                    };
                    if let Ok(r) = r {
                        results = r;
                    }
                }
                results
            },
        );

        let tmdb_search_time = tmdb_search_start.elapsed();
        tracing::debug!("TMDB movie search took {:.2}s", tmdb_search_time.as_secs_f64());

        // Priority 0: IMDB ID result (highest priority - direct lookup)
        if let Some(movie) = imdb_result {
            return Ok(Some(movie));
        }

        // Priority 1: Chinese title results (最高优先级 - 中文结果)
        if !chinese_results.is_empty() {
            let query = chinese_title.as_deref().unwrap_or("");
            if let Some(best) = self.select_best_movie_match(&chinese_results, query, parsed.year) {
                let tmdb_year = Self::extract_year_from_release_date(&best.release_date);
                tracing::debug!("Priority 1: Selected title='{}' tmdb_year={:?}", best.title, tmdb_year);
                if self.is_reasonable_match_with_year(
                    query,
                    &best.title,
                    &best.original_title,
                    parsed.year,
                    tmdb_year,
                ) {
                    tracing::info!("TMDB found (Chinese match): {}", best.title);
                    return self.get_movie_details(client, best.id, chinese_title.as_deref()).await;
                }
            }
        }

        // Priority 2: Find common results (intersection by TMDB ID)
        if !chinese_results.is_empty() && !english_results.is_empty() {
            let chinese_ids: std::collections::HashSet<u64> =
                chinese_results.iter().map(|r| r.id).collect();

            let common: Vec<_> = english_results
                .iter()
                .filter(|r| chinese_ids.contains(&r.id))
                .collect();

            if !common.is_empty() {
                let query = english_title.as_deref().unwrap_or("");
                if let Some(best) = self.select_best_movie_match_ref(&common, query, parsed.year) {
                    tracing::info!(
                        "TMDB found (common match): {} - matches both '{}' and '{}'",
                        best.title,
                        chinese_title.as_deref().unwrap_or(""),
                        english_title.as_deref().unwrap_or("")
                    );
                    return self.get_movie_details(client, best.id, chinese_title.as_deref()).await;
                }
            }
        }

        // Priority 3: English title results (国际电影 fallback)
        if !english_results.is_empty() {
            let query = english_title.as_deref().unwrap_or("");
            if let Some(best) = self.select_best_movie_match(&english_results, query, parsed.year) {
                let tmdb_year = Self::extract_year_from_release_date(&best.release_date);
                if self.is_reasonable_match_with_year(
                    query,
                    &best.title,
                    &best.original_title,
                    parsed.year,
                    tmdb_year,
                ) {
                    tracing::info!("TMDB found (English match): {}", best.title);
                    return self.get_movie_details(client, best.id, chinese_title.as_deref()).await;
                }
            }
        }

        // Fallback: Try shortened queries for long Chinese titles
        if let Some(ref title) = chinese_title {
            let mut shortened_queries: Vec<String> = Vec::new();
            self.add_shortened_queries(&mut shortened_queries, title);

            for query in &shortened_queries {
                let results = match client.search_movie(query, parsed.year).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("TMDB search failed for shortened query '{}': {}", query, e);
                        continue;
                    }
                };
                if !results.is_empty() {
                    if let Some(best) = self.select_best_movie_match(&results, query, parsed.year) {
                        let tmdb_year = Self::extract_year_from_release_date(&best.release_date);
                        if self.is_reasonable_match_with_year(
                            query,
                            &best.title,
                            &best.original_title,
                            parsed.year,
                            tmdb_year,
                        ) {
                            tracing::info!("TMDB found (shortened query): {}", best.title);
                            return self.get_movie_details(client, best.id, chinese_title.as_deref()).await;
                        }
                    }
                }
            }
        }

        tracing::warn!(
            "TMDB: No match found for chinese={:?}, english={:?}",
            chinese_title,
            english_title
        );
        Ok(None)
    }

    /// Select best movie match from a slice of references.
    /// Returns None if match is ambiguous.
    fn select_best_movie_match_ref<'a>(
        &self,
        results: &[&'a crate::services::tmdb::MovieSearchItem],
        query_title: &str,
        target_year: Option<u16>,
    ) -> Option<&'a crate::services::tmdb::MovieSearchItem> {
        use chrono::Datelike;
        let current_year = chrono::Utc::now().year() as u16;
        let query_normalized = self.normalize_title(query_title);

        let mut scored_results: Vec<(usize, i64, bool)> = Vec::new(); // (idx, score, is_exact)

        for (i, movie) in results.iter().enumerate() {
            let year: u16 = movie
                .release_date
                .as_ref()
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse().ok())
                .unwrap_or(0);

            // Skip future movies
            if year > current_year + 1 {
                continue;
            }

            let mut score = movie.vote_count.unwrap_or(0) as i64;
            if year > 0 {
                score += 100;
            }

            // Bonus for title match
            let title_normalized = self.normalize_title(&movie.title);
            let original_normalized = self.normalize_title(&movie.original_title);

            let is_exact =
                title_normalized == query_normalized || original_normalized == query_normalized;
            if is_exact {
                score += 10000;
            } else if title_normalized.contains(&query_normalized)
                || original_normalized.contains(&query_normalized)
            {
                score += 1000;
            }

            // Year match bonus: prioritize matching the target year
            // - Exact year match: +50000
            // - Year diff = 1: +5000
            // - Year diff = 2: +1000
            // - Year diff > 2: no bonus
            let year_match_bonus: i64 = if let Some(target) = target_year {
                if year == target {
                    50000
                } else if year > 0 {
                    let diff = (year as i32 - target as i32).abs();
                    if diff == 1 {
                        5000
                    } else if diff == 2 {
                        1000
                    } else {
                        0
                    }
                } else {
                    0
                }
            } else {
                0
            };
            score += year_match_bonus;

            scored_results.push((i, score, is_exact));
        }

        if scored_results.is_empty() {
            return None;
        }

        scored_results.sort_by(|a, b| b.1.cmp(&a.1));
        let (best_idx, best_score, best_exact) = scored_results[0];

        // Ambiguity check for non-exact matches
        if !best_exact && scored_results.len() > 1 {
            let (_, second_score, _) = scored_results[1];
            if best_score - second_score < 1000 {
                tracing::warn!(
                    "Ambiguous movie match (ref): '{}' vs '{}' - skipping",
                    results[best_idx].title,
                    results[scored_results[1].0].title
                );
                return None;
            }
        }

        Some(results[best_idx])
    }

    /// Check if a title is meaningful for TMDB search.
    /// Filters out single-character Chinese titles, common placeholder words, etc.
    fn is_meaningful_title(&self, title: &str) -> bool {
        let trimmed = title.trim();

        // Empty or whitespace only
        if trimmed.is_empty() {
            return false;
        }

        // Single character (especially problematic for CJK)
        let char_count = trimmed.chars().count();
        if char_count <= 1 {
            tracing::debug!("Filtering meaningless single-char title: '{}'", trimmed);
            return false;
        }

        // Common placeholder/meaningless words
        const MEANINGLESS_WORDS: &[&str] = &[
            "无", "是", "的", "了", "在", "有", "和", "与", "null", "none", "unknown", "untitled",
            "n/a",
        ];

        let lower = trimmed.to_lowercase();
        if MEANINGLESS_WORDS.iter().any(|w| lower == *w) {
            tracing::debug!("Filtering meaningless placeholder title: '{}'", trimmed);
            return false;
        }

        // Very short titles that look like technical info
        if char_count <= 3 {
            // Check if it looks like resolution/format
            let patterns = ["4k", "hd", "sd", "mp4", "mkv", "avi", "web", "blu"];
            if patterns.iter().any(|p| lower.contains(p)) {
                tracing::debug!("Filtering short technical-looking title: '{}'", trimmed);
                return false;
            }
        }

        true
    }

    /// Check if the TMDB match is reasonable with year validation.
    /// query_year: the year from parsed filename
    /// tmdb_year: the year from TMDB result
    fn is_reasonable_match_with_year(
        &self,
        query: &str,
        tmdb_title: &str,
        tmdb_orig: &str,
        query_year: Option<u16>,
        tmdb_year: Option<u16>,
    ) -> bool {
        // YEAR VALIDATION: If both years are known, they should be close
        if let (Some(qy), Some(ty)) = (query_year, tmdb_year) {
            let diff = (qy as i32 - ty as i32).abs();
            if diff > 1 {
                tracing::debug!(
                    "Year mismatch too large: query={}, tmdb={}, diff={}",
                    qy,
                    ty,
                    diff
                );
                return false;
            }
        }

        let query_lower = query.to_lowercase();
        let title_lower = tmdb_title.to_lowercase();
        let orig_lower = tmdb_orig.to_lowercase();

        // Check if query appears in either title
        if title_lower.contains(&query_lower) || query_lower.contains(&title_lower) {
            return true;
        }
        if orig_lower.contains(&query_lower) || query_lower.contains(&orig_lower) {
            return true;
        }

        // Check for significant word overlap (for CJK languages)
        // But require more overlap for short queries to avoid false matches
        let query_chars: std::collections::HashSet<char> = query
            .chars()
            .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
            .collect();
        let title_chars: std::collections::HashSet<char> = tmdb_title
            .chars()
            .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
            .collect();

        let common = query_chars.intersection(&title_chars).count();
        let query_len = query_chars.len();
        let title_len = title_chars.len();
        let min_len = query_len.min(title_len);

        // For very short queries (<=3 chars), require exact match
        if query_len <= 3 {
            if common == query_len && query_len == title_len {
                return true;
            }
            return false;
        }

        // For longer queries, require at least 50% character overlap
        if min_len > 0 && common * 2 >= min_len {
            return true;
        }

        false
    }

    /// Query TMDB for TV show metadata.
    #[allow(dead_code)]
    async fn query_tmdb_tv_series(
        &self,
        parsed: &ParsedFilename,
    ) -> Result<(Option<TvSeriesMetadata>, Option<EpisodeMetadata>)> {
        self.query_tmdb_tv_series_with_folder(parsed, None).await
    }

    /// Query TMDB for TV show metadata with optional folder name as fallback.
    async fn query_tmdb_tv_series_with_folder(
        &self,
        parsed: &ParsedFilename,
        folder_name: Option<&str>,
    ) -> Result<(Option<TvSeriesMetadata>, Option<EpisodeMetadata>)> {
        let client = match &self.tmdb_client {
            Some(c) => c,
            None => return Ok((None, None)),
        };

        // Helper to clean up search query
        let clean_query = |s: &str| -> String { s.replace(['.', '_'], " ").trim().to_string() };

        // Extract Chinese and English titles from parsed result
        let mut chinese_title = parsed
            .title
            .as_ref()
            .map(|t| clean_query(t))
            .filter(|t| !t.is_empty());
        let mut english_title = parsed
            .original_title
            .as_ref()
            .map(|t| clean_query(t))
            .filter(|t| !t.is_empty());
        let mut folder_year: Option<u16> = parsed.year;

        // Smart parsing of folder name to extract Chinese/English titles and year
        // Format: "中文标题 English Title (2022)" or similar variations
        if let Some(folder) = folder_name {
            tracing::debug!("[FOLDER] Processing folder name: '{}'", folder);

            let is_quality_desc = folder.contains("1080")
                || folder.contains("720")
                || folder.contains("2160")
                || folder.contains("4K")
                || folder.contains("内封")
                || folder.contains("外挂");

            if !is_quality_desc {
                // Extract year from parentheses: (2022)
                let year_re = regex::Regex::new(r"\((\d{4})\)").unwrap();
                if folder_year.is_none() {
                    if let Some(caps) = year_re.captures(folder) {
                        folder_year = caps.get(1).and_then(|m| m.as_str().parse().ok());
                        tracing::debug!("[FOLDER] Extracted year: {:?}", folder_year);
                    }
                }

                // Remove year and brackets, then split Chinese and English
                let cleaned = year_re.replace_all(folder, "").to_string();
                // Strip sorting prefix (A_, X_, 01_, etc.)
                let cleaned = Self::strip_sorting_prefix(&cleaned);
                // Replace separators with spaces for better parsing
                let cleaned = cleaned.replace(['.', '_', '-'], " ").trim().to_string();

                tracing::debug!("[FOLDER] Cleaned folder name: '{}'", cleaned);

                // Extract Chinese portion (consecutive CJK characters and punctuation)
                let chinese_re =
                    regex::Regex::new(r"[\u4e00-\u9fff\u3000-\u303f\u00b7\uff01-\uff5e]+").unwrap();
                let folder_chinese: String = chinese_re
                    .find_iter(&cleaned)
                    .map(|m| m.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string();

                // Extract English portion (remaining non-CJK characters)
                let folder_english: String = chinese_re
                    .replace_all(&cleaned, " ")
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");

                tracing::debug!(
                    "[FOLDER] Extracted Chinese: '{}', English: '{}'",
                    folder_chinese,
                    folder_english
                );

                // Use folder-extracted titles as fallback if AI didn't provide them
                if chinese_title.is_none() && !folder_chinese.is_empty() {
                    tracing::info!("[FOLDER] Using folder Chinese title: '{}'", folder_chinese);
                    chinese_title = Some(folder_chinese.clone());
                }
                if english_title.is_none() && !folder_english.is_empty() {
                    tracing::info!("[FOLDER] Using folder English title: '{}'", folder_english);
                    english_title = Some(folder_english.clone());
                }
            } else {
                tracing::debug!("[FOLDER] Skipped quality descriptor folder: '{}'", folder);
            }
        }

        // Use extracted year
        let search_year = folder_year.or(parsed.year);

        tracing::debug!(
            "TMDB TV search: chinese={:?}, english={:?}, year={:?}",
            chinese_title,
            english_title,
            search_year
        );

        // Strategy: Search with both titles and find intersection first
        // Priority: 1) Common results (both titles match) - most reliable
        //           2) English title results
        //           3) Chinese title results

        // Debug: print what we're searching for
        tracing::debug!("TMDB TV search - chinese_title: {:?}, english_title: {:?}, year: {:?}", chinese_title, english_title, search_year);

        // Search with Chinese and English titles in parallel
        let chinese_title_clone = chinese_title.clone();
        let english_title_clone = english_title.clone();

        let tmdb_search_start = std::time::Instant::now();
        let (chinese_results, english_results) = tokio::join!(
            async {
                let mut results: Vec<crate::services::tmdb::TvSearchItem> = Vec::new();
                if let Some(ref title) = chinese_title_clone {
                    if let Ok(r) = client.search_tv(title, search_year).await {
                        if r.is_empty() {
                            if let Ok(r2) = client.search_tv(title, None).await {
                                results = r2;
                            }
                        } else {
                            results = r;
                        }
                    }
                }
                results
            },
            async {
                let mut results: Vec<crate::services::tmdb::TvSearchItem> = Vec::new();
                if let Some(ref title) = english_title_clone {
                    if let Ok(r) = client.search_tv(title, search_year).await {
                        if r.is_empty() {
                            if let Ok(r2) = client.search_tv(title, None).await {
                                results = r2;
                            }
                        } else {
                            results = r;
                        }
                    }
                }
                results
            },
        );

        let tmdb_search_time = tmdb_search_start.elapsed();
        tracing::debug!("TMDB TV search took {:.2}s", tmdb_search_time.as_secs_f64());

        // Priority 1: Find common results (intersection by TMDB ID)
        if !chinese_results.is_empty() && !english_results.is_empty() {
            let chinese_ids: std::collections::HashSet<u64> =
                chinese_results.iter().map(|r| r.id).collect();

            let common: Vec<_> = english_results
                .iter()
                .filter(|r| chinese_ids.contains(&r.id))
                .cloned()
                .collect();

            if !common.is_empty() {
                let query = english_title.as_deref().unwrap_or("");
                if let Some(best) = self.select_best_tv_match(query, &common) {
                    tracing::info!(
                        "TMDB TV found (common match): {} - matches both '{}' and '{}'",
                        best.name,
                        chinese_title.as_deref().unwrap_or(""),
                        english_title.as_deref().unwrap_or("")
                    );
                    return self.get_tv_series_details(client, best.id, parsed).await;
                }
            }
        }

        // Priority 2: Chinese title results (preferred for Asian content)
        // Chinese title is more specific and less likely to match wrong international content
        // e.g., "神探伽利略" will correctly match the Japanese show,
        // while "Galileo" might match the German show
        if !chinese_results.is_empty() {
            let query = chinese_title.as_deref().unwrap_or("");
            if let Some(best) = self.select_best_tv_match(query, &chinese_results) {
                tracing::info!("TMDB TV found (Chinese match): {}", best.name);
                return self.get_tv_series_details(client, best.id, parsed).await;
            }
        }

        // Priority 3: English title results (fallback)
        if !english_results.is_empty() {
            let query = english_title.as_deref().unwrap_or("");
            if let Some(best) = self.select_best_tv_match(query, &english_results) {
                tracing::info!("TMDB TV found (English match): {}", best.name);
                return self.get_tv_series_details(client, best.id, parsed).await;
            }
        }

        tracing::warn!(
            "TMDB TV: No match found for chinese={:?}, english={:?}, year={:?}",
            chinese_title,
            english_title,
            search_year
        );
        Ok((None, None))
    }

    /// Select the best TV show match from search results.
    /// Prioritizes: exact match > shorter prefix match > contains match
    /// Returns None if match is ambiguous (multiple candidates with similar scores).
    /// Principle: prefer skipping over wrong match.
    fn select_best_tv_match<'a>(
        &self,
        query: &str,
        results: &'a [crate::services::tmdb::TvSearchItem],
    ) -> Option<&'a crate::services::tmdb::TvSearchItem> {
        if results.is_empty() {
            return None;
        }

        // SPECIAL CASE: For pure CJK queries, if TMDB returns only one result,
        // trust TMDB's matching even if the result name is in a different language.
        // This handles cases like searching "人生复本" returning "Dark Matter".
        // TMDB internally maps Chinese titles to original titles.
        let is_pure_cjk = query.chars().all(|c| {
            c.is_whitespace()
                || ('\u{4E00}'..='\u{9FFF}').contains(&c) // CJK Unified Ideographs
                || ('\u{3400}'..='\u{4DBF}').contains(&c) // CJK Extension A
                || ('\u{AC00}'..='\u{D7AF}').contains(&c) // Korean Hangul
                || ('\u{3040}'..='\u{30FF}').contains(&c) // Japanese Hiragana/Katakana
        }) && query.chars().any(|c| !c.is_whitespace());

        if is_pure_cjk && results.len() == 1 {
            tracing::info!(
                "TMDB single result for CJK query '{}': trusting result '{}'",
                query,
                results[0].name
            );
            return Some(&results[0]);
        }

        let query_lower = query.to_lowercase();
        let mut scored_results: Vec<(usize, i32)> = Vec::new();

        for (i, show) in results.iter().enumerate() {
            let name_lower = show.name.to_lowercase();
            let orig_lower = show.original_name.to_lowercase();

            let mut score: i32 = 0;

            // Exact match gets highest score
            if name_lower == query_lower || orig_lower == query_lower {
                score += 1000;
            }
            // Query is a prefix of the result name - prefer SHORTEST match (most specific)
            else if name_lower.starts_with(&query_lower) || orig_lower.starts_with(&query_lower) {
                // Shorter result name = better match (e.g., "战地青春之歌" < "战地青春：直击...")
                score += 500;
                // BONUS for shorter names (closer to query length)
                let len_diff = show.name.chars().count() as i32 - query.chars().count() as i32;
                score -= len_diff * 10; // Penalize longer names heavily
            }
            // Result name is contained in query (query is more specific)
            else if query_lower.contains(&name_lower) || query_lower.contains(&orig_lower) {
                score += 400;
            }
            // Query is contained in result name
            else if name_lower.contains(&query_lower) || orig_lower.contains(&query_lower) {
                // Base score for contains match
                score += 100;

                // IMPROVED: Prioritize results whose title length is close to query length
                // This helps "流人" (query) match "流人" (result) better than "千古风流人物" (result)
                // If result name length is within 2 chars of query, it's likely an exact or near-exact match
                let query_len = query.chars().count();
                let name_len = show.name.chars().count();
                let len_diff = (name_len as i32 - query_len as i32).abs();

                if len_diff <= 2 {
                    // Near-exact length match - boost significantly
                    score += 800; // Total: 900, just below exact match
                    tracing::debug!(
                        "Near-exact length match: '{}' (len {}) vs query '{}' (len {})",
                        show.name,
                        name_len,
                        query,
                        query_len
                    );
                } else if len_diff <= 5 {
                    // Reasonably close length - moderate boost
                    score += 300; // Total: 400
                }
                // Long names containing query get penalized (they might be partial matches)
                else {
                    score -= (len_diff - 5) * 20; // Penalize very long names
                }
            }
            // Check character overlap
            else {
                let query_chars: std::collections::HashSet<char> = query.chars().collect();
                let name_chars: std::collections::HashSet<char> = show.name.chars().collect();
                let common = query_chars.intersection(&name_chars).count();
                let min_len = query_chars.len().min(name_chars.len());
                if min_len > 0 && common * 2 >= min_len {
                    score += 50;
                } else {
                    continue; // Not a good match
                }
            }

            tracing::debug!("TV match candidate: {} (score: {})", show.name, score);

            scored_results.push((i, score));
        }

        if scored_results.is_empty() {
            return None;
        }

        // Sort by score descending
        scored_results.sort_by(|a, b| b.1.cmp(&a.1));

        let (best_idx, best_score) = scored_results[0];

        // AMBIGUITY CHECK: If there are multiple candidates with the same score tier,
        // the match is ambiguous - skip rather than risk wrong match.
        // Score tiers: 1000 (exact), 400-500 (prefix/contains), <100 (weak)
        if scored_results.len() > 1 {
            let (_, second_score) = scored_results[1];

            // Check if both are in the same score tier (within 100 points)
            // OR if best is not an exact match and second is close
            let is_ambiguous = if best_score >= 1000 {
                // Exact match is unambiguous
                false
            } else if best_score >= 400 && second_score >= 400 {
                // Both are prefix/contains matches - ambiguous!
                tracing::warn!(
                    "Ambiguous TV match: '{}' (score {}) vs '{}' (score {}) - skipping",
                    results[best_idx].name,
                    best_score,
                    results[scored_results[1].0].name,
                    second_score
                );
                true
            } else if best_score - second_score < 100 && best_score < 500 {
                // Scores too close and not high enough - ambiguous
                tracing::warn!(
                    "Ambiguous TV match (close scores): '{}' ({}) vs '{}' ({}) - skipping",
                    results[best_idx].name,
                    best_score,
                    results[scored_results[1].0].name,
                    second_score
                );
                true
            } else {
                false
            };

            if is_ambiguous {
                return None;
            }
        }

        // Require a minimum score for confidence
        if best_score < 100 {
            tracing::warn!(
                "TV match score too low: '{}' (score {}) - skipping",
                results[best_idx].name,
                best_score
            );
            return None;
        }

        tracing::debug!(
            "Selected best TV match: {} (score: {})",
            results[best_idx].name,
            best_score
        );
        Some(&results[best_idx])
    }

    /// Get TV show details from TMDB.
    async fn get_tv_series_details(
        &self,
        client: &TmdbClient,
        tv_id: u64,
        parsed: &ParsedFilename,
    ) -> Result<(Option<TvSeriesMetadata>, Option<EpisodeMetadata>)> {
        let details = client.get_tv_details(tv_id).await?;

        // Extract year from first_air_date
        let year = details
            .first_air_date
            .as_ref()
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse().ok())
            .unwrap_or(0);

        // Get poster URL
        let poster_urls = details
            .poster_path
            .as_ref()
            .map(|p| vec![format!("https://image.tmdb.org/t/p/{}{}", self.config.poster_size, p)])
            .unwrap_or_default();

        // Get backdrop URL
        let backdrop_url = details
            .backdrop_path
            .as_ref()
            .map(|p| client.get_poster_url(p, &self.config.poster_size));

        // Extract genres
        let genres = details
            .genres
            .as_ref()
            .map(|g| g.iter().map(|x| x.name.clone()).collect())
            .unwrap_or_default();

        // Always prefer origin_country over production_countries
        // origin_country is more accurate for the content's true origin
        // production_countries may include co-production countries or have TMDB data errors
        // Example: "在劫难逃" has origin_country=["CN"] but production_countries=[{MO}]
        // This matches the logic in build_tv_series_metadata_from_details for consistency
        let (country_codes, countries): (Vec<String>, Vec<String>) =
            if let Some(ref origin) = details.origin_country {
                if !origin.is_empty() {
                    let codes = origin.clone();
                    let names = origin
                        .iter()
                        .map(|code| country_code_to_name(code))
                        .collect();
                    (codes, names)
                } else {
                    (Vec::new(), Vec::new())
                }
            } else {
                (Vec::new(), Vec::new())
            };

        // Fallback: use production_countries if origin_country is empty
        let (country_codes, countries) = if country_codes.is_empty() {
            if let Some(ref pc) = details.production_countries {
                if !pc.is_empty() {
                    let codes = pc.iter().map(|x| x.iso_3166_1.clone()).collect();
                    let names = pc.iter().map(|c| country_code_to_name(&c.iso_3166_1)).collect();
                    (codes, names)
                } else {
                    (country_codes, countries)
                }
            } else {
                (country_codes, countries)
            }
        } else {
            (country_codes, countries)
        };

        // Extract networks
        let networks = details
            .networks
            .as_ref()
            .map(|n| n.iter().map(|x| x.name.clone()).collect())
            .unwrap_or_default();

        // Extract creators
        let creators = details
            .created_by
            .as_ref()
            .map(|c| c.iter().map(|x| x.name.clone()).collect())
            .unwrap_or_default();

        // Extract actors (top 10)
        let actors = details
            .credits
            .as_ref()
            .and_then(|c| c.cast.as_ref())
            .map(|cast| {
                cast.iter()
                    .take(10)
                    .map(|c| crate::models::media::Actor {
                        name: c.name.clone(),
                        role: c.character.clone(),
                        order: c.order,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let show = TvSeriesMetadata {
            tmdb_id: details.id,
            imdb_id: details.external_ids.and_then(|e| e.imdb_id),
            original_name: details.original_name,
            name: details.name,
            original_language: details.original_language,
            year,
            first_air_date: details.first_air_date,
            overview: details.overview,
            tagline: details.tagline,
            genres,
            countries,
            country_codes,
            networks,
            rating: details.vote_average,
            votes: details.vote_count,
            number_of_seasons: details.number_of_seasons,
            number_of_episodes: details.number_of_episodes,
            status: details.status,
            creators,
            actors,
            poster_urls,
            backdrop_url,
        };

        // If we have season/episode info, get episode details
        // Note: parsed.season/episode may be None for the first file which uses AI parsing.
        // The actual episode info will be extracted by regex in process_single_video_with_cache.
        // So we return None here - the caller will handle getting episode details.
        let episode = if let (Some(season), Some(ep)) = (parsed.season, parsed.episode) {
            match client.get_episode_details(tv_id, season, ep).await {
                Ok(ep_details) => Some(EpisodeMetadata {
                    season_number: season,
                    episode_number: ep,
                    name: ep_details.name,
                    original_name: None,
                    air_date: ep_details.air_date,
                    overview: ep_details.overview,
                }),
                Err(_) => Some(EpisodeMetadata {
                    season_number: season,
                    episode_number: ep,
                    name: format!("Episode {}", ep),
                    original_name: None,
                    air_date: None,
                    overview: None,
                }),
            }
        } else {
            // Season/episode not parsed from input - return None
            // The caller should extract episode info from filename using regex
            None
        };

        Ok((Some(show), episode))
    }

    /// Select the best movie match from search results.
    /// Prioritizes: 1) exact title match, 2) already released movies with most votes.
    /// Returns None if match is ambiguous or uncertain.
    /// Principle: prefer skipping over wrong match.
    fn select_best_movie_match<'a>(
        &self,
        results: &'a [crate::services::tmdb::MovieSearchItem],
        query_title: &str,
        target_year: Option<u16>,
    ) -> Option<&'a crate::services::tmdb::MovieSearchItem> {
        use chrono::Datelike;
        let current_year = chrono::Utc::now().year() as u16;

        // Normalize query title for comparison
        let query_normalized = self.normalize_title(query_title);

        // Calculate scores for all valid candidates
        let mut scored_results: Vec<(usize, i64, bool)> = Vec::new(); // (idx, score, is_exact)

        for (i, movie) in results.iter().enumerate() {
            // Extract year from release_date
            let year: u16 = movie
                .release_date
                .as_ref()
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse().ok())
                .unwrap_or(0);

            // Skip future movies (year > current year + 1)
            if year > current_year + 1 {
                tracing::debug!("Skipping far future movie: {} ({})", movie.title, year);
                continue;
            }

            // Check for exact title match (highest priority)
            let title_normalized = self.normalize_title(&movie.title);
            let orig_title_normalized = self.normalize_title(&movie.original_title);

            let exact_match =
                title_normalized == query_normalized || orig_title_normalized == query_normalized;

            // Score calculation
            let exact_match_bonus: i64 = if exact_match { 100000 } else { 0 };
            let vote_count = movie.vote_count.unwrap_or(0) as i64;
            let date_bonus: i64 = if year > 0 { 100 } else { 0 };

            // Year match bonus: prioritize matching the target year
            // - Exact year match: +50000
            // - Year diff = 1: +5000
            // - Year diff = 2: +1000
            // - Year diff > 2: no bonus
            let year_match_bonus: i64 = if let Some(target) = target_year {
                if year == target {
                    50000
                } else if year > 0 {
                    let diff = (year as i32 - target as i32).abs();
                    if diff == 1 {
                        5000
                    } else if diff == 2 {
                        1000
                    } else {
                        0
                    }
                } else {
                    0
                }
            } else {
                0
            };

            let score = exact_match_bonus + vote_count + date_bonus + year_match_bonus;

            tracing::debug!(
                "Movie candidate: {} (year={}, votes={}, exact={}, year_match_bonus={}, total_score={})",
                movie.title,
                year,
                vote_count,
                exact_match,
                year_match_bonus,
                score
            );

            scored_results.push((i, score, exact_match));
        }

        if scored_results.is_empty() {
            return None;
        }

        // Sort by score descending
        scored_results.sort_by(|a, b| b.1.cmp(&a.1));

        let (best_idx, best_score, best_exact) = scored_results[0];

        // AMBIGUITY CHECK: If best is not exact match and there are multiple candidates
        // with similar scores, the match is ambiguous
        if !best_exact && scored_results.len() > 1 {
            let (_, second_score, _) = scored_results[1];

            // If scores are within 1000 of each other (both have similar vote counts)
            // and neither is an exact match, it's ambiguous
            if best_score - second_score < 1000 {
                tracing::warn!(
                    "Ambiguous movie match: '{}' (score {}) vs '{}' (score {}) - skipping",
                    results[best_idx].title,
                    best_score,
                    results[scored_results[1].0].title,
                    second_score
                );
                return None;
            }
        }

        // Require minimum vote count for non-exact matches to ensure quality
        if !best_exact {
            let vote_count = results[best_idx].vote_count.unwrap_or(0);
            
            // 提高最低投票数阈值，拒绝冷门/不存在的匹配结果
            if vote_count < 50 {
                tracing::warn!(
                    "Movie match too uncertain (low votes): '{}' ({} votes) - skipping",
                    results[best_idx].title,
                    vote_count
                );
                return None;
            }
            
            // 非精确匹配时，要求分数显著高于第二名
            if scored_results.len() > 1 {
                let (_, second_score, _) = scored_results[1];
                if best_score - second_score < 5000 {
                    tracing::warn!(
                        "Ambiguous movie match: '{}' (score {}) vs '{}' (score {}) - skipping",
                        results[best_idx].title,
                        best_score,
                        results[scored_results[1].0].title,
                        second_score
                    );
                    return None;
                }
            }
        }

        tracing::debug!(
            "Selected best match: {} (score: {}, exact: {})",
            results[best_idx].title,
            best_score,
            best_exact
        );

        Some(&results[best_idx])
    }

    /// Extract year from TMDB release_date format (YYYY-MM-DD).
    fn extract_year_from_release_date(release_date: &Option<String>) -> Option<u16> {
        release_date
            .as_ref()
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse().ok())
    }

    /// Normalize title for comparison (lowercase, remove punctuation/spaces).
    fn normalize_title(&self, title: &str) -> String {
        title
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase()
    }

    /// Get movie details from TMDB.
    async fn get_movie_details(
        &self,
        client: &TmdbClient,
        movie_id: u64,
        fallback_chinese_title: Option<&str>,
    ) -> Result<Option<MovieMetadata>> {
        let details = client.get_movie_details(movie_id).await?;

        // Extract year from release date
        let year = details
            .release_date
            .as_ref()
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse().ok())
            .unwrap_or(0);

        // Extract credits from details (now included via append_to_response)
        let credits = details.credits.as_ref();

        // Extract directors
        let directors = credits
            .map(|c| {
                c.crew
                    .iter()
                    .filter(|m| m.job == "Director")
                    .map(|m| m.name.clone())
                    .collect()
            })
            .unwrap_or_default();

        // Extract writers
        let writers = credits
            .map(|c| {
                c.crew
                    .iter()
                    .filter(|m| m.job == "Writer" || m.job == "Screenplay")
                    .take(5)
                    .map(|m| m.name.clone())
                    .collect()
            })
            .unwrap_or_default();

        // Extract actors and their roles
        let (actors, actor_roles): (Vec<String>, Vec<String>) = credits
            .map(|c| {
                c.cast
                    .iter()
                    .take(15)
                    .map(|m| (m.name.clone(), m.character.clone().unwrap_or_default()))
                    .unzip()
            })
            .unwrap_or_default();

        // Extract genres
        let genres = details
            .genres
            .as_ref()
            .map(|g| g.iter().map(|x| x.name.clone()).collect())
            .unwrap_or_default();

        // Extract country codes - prioritize origin_country, fallback to production_countries
        let country_codes: Vec<String> = if let Some(ref origin) = details.origin_country {
            if !origin.is_empty() {
                origin.clone()
            } else {
                details
                    .production_countries
                    .as_ref()
                    .map(|c| c.iter().map(|x| x.iso_3166_1.clone()).collect())
                    .unwrap_or_default()
            }
        } else {
            details
                .production_countries
                .as_ref()
                .map(|c| c.iter().map(|x| x.iso_3166_1.clone()).collect())
                .unwrap_or_default()
        };

        // Extract country names - ALWAYS use country_code_to_name for consistency
        // This ensures country_codes and countries have matching order and format
        let countries: Vec<String> = country_codes
            .iter()
            .map(|c| country_code_to_name(c))
            .collect();

        // Extract studios
        let studios = details
            .production_companies
            .as_ref()
            .map(|c| c.iter().take(3).map(|x| x.name.clone()).collect())
            .unwrap_or_default();

        // Extract certification (from release_dates)
        let certification = details.release_dates.as_ref().and_then(|rd| {
            // Try to find US certification first, then CN
            for country in &["US", "CN"] {
                if let Some(c) = rd.results.iter().find(|r| r.iso_3166_1 == *country) {
                    if let Some(cert) = c
                        .release_dates
                        .iter()
                        .filter_map(|r| r.certification.as_ref())
                        .find(|c| !c.is_empty())
                    {
                        return Some(cert.clone());
                    }
                }
            }
            None
        });

        // Build poster URLs
        let mut poster_urls = Vec::new();
        if let Some(ref poster_path) = details.poster_path {
            poster_urls.push(client.get_poster_url(poster_path, &self.config.poster_size));
        }

        // Build backdrop URL
        let backdrop_url = details
            .backdrop_path
            .as_ref()
            .map(|p| client.get_poster_url(p, &self.config.poster_size));

        // Extract collection info (for movie series like "Pirates of the Caribbean")
        let (collection_id, collection_name, collection_overview, collection_total_movies) =
            if let Some(ref collection) = details.belongs_to_collection {
                // Fetch collection details to get total movies count
                let total = match client.get_collection_details(collection.id).await {
                    Ok(collection_details) => {
                        tracing::debug!(
                            "[COLLECTION] Fetched {} (tmdb{}): {} movies total",
                            collection.name,
                            collection.id,
                            collection_details.parts.len()
                        );
                        Some(collection_details.parts.len())
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[COLLECTION] Failed to fetch collection {}: {}",
                            collection.id,
                            e
                        );
                        None
                    }
                };
                (
                    Some(collection.id),
                    Some(collection.name.clone()),
                    collection.overview.clone(),
                    total,
                )
            } else {
                (None, None, None, None)
            };

        // Determine title - use TMDB title, but fallback to parsed Chinese title if TMDB doesn't have translation
        let mut title: String = {
            let tmdb_title = &details.title;
            let tmdb_original = &details.original_title;
            let titles_same = self.normalize_title(tmdb_title) == self.normalize_title(tmdb_original);
            let title_has_chinese = chinese::contains_chinese(tmdb_title);
            
            // If TMDB has Chinese translation, use it
            if !titles_same || title_has_chinese {
                tmdb_title.clone()
            } else if let Some(fallback) = fallback_chinese_title {
                // TMDB doesn't have Chinese translation, use parsed Chinese title from filename
                tracing::info!(
                    "[TMDB] No Chinese translation for '{}', using fallback: '{}'",
                    tmdb_title,
                    fallback
                );
                fallback.to_string()
            } else {
                tmdb_title.clone()
            }
        };
        
        // If title is still not Chinese, try translations API
        if !chinese::contains_chinese(&title) {
            tracing::info!("[TMDB] Title '{}' is not Chinese, trying translations API", title);
            match client.get_movie_translations(movie_id).await {
                Ok(translations) => {
                    tracing::info!("[TMDB] Got {} translations for tmdb{}", translations.translations.len(), movie_id);
                    
                    // Collect all valid Chinese translations
                    let chinese_candidates: Vec<(String, String)> = translations.translations
                        .iter()
                        .filter(|t| t.iso_639_1 == "zh" || t.iso_639_1 == "zh-CN")
                        .filter(|t| !t.data.title.is_empty() && chinese::contains_chinese(&t.data.title))
                        .map(|t| (t.iso_3166_1.clone(), t.data.title.clone()))
                        .collect();
                    
                    // Priority order: CN (Simplified) > SG (Simplified) > HK (Traditional) > TW (Traditional)
                    let region_priority = ["CN", "SG", "HK", "TW"];
                    
                    // First pass: try in priority order
                    for priority_region in &region_priority {
                        if let Some((_region, chinese_title)) = chinese_candidates
                            .iter()
                            .find(|(r, _)| r == priority_region)
                        {
                            tracing::info!(
                                "[TMDB] Found {} title '{}' ({} region, priority {})",
                                if *priority_region == "CN" || *priority_region == "SG" { "Simplified Chinese" } else { "Traditional Chinese" },
                                chinese_title, 
                                priority_region,
                                priority_region
                            );
                            title = chinese_title.clone();
                            break;
                        }
                    }
                    
                    // Final fallback: use any available Chinese translation
                    if !chinese::contains_chinese(&title) {
                        if let Some((region, chinese_title)) = chinese_candidates.first() {
                            tracing::info!(
                                "[TMDB] Found Chinese title '{}' ({} region, final fallback)",
                                chinese_title, region
                            );
                            title = chinese_title.clone();
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("[TMDB] Failed to get translations for tmdb{}: {}", movie_id, e);
                }
            }
        }

        Ok(Some(MovieMetadata {
            tmdb_id: details.id,
            imdb_id: details.imdb_id,
            original_title: details.original_title,
            title,
            original_language: details.original_language,
            year,
            release_date: details.release_date,
            overview: details.overview,
            tagline: details.tagline,
            runtime: details.runtime,
            genres,
            countries,
            country_codes,
            studios,
            rating: details.vote_average,
            votes: details.vote_count,
            poster_urls,
            backdrop_url,
            directors,
            writers,
            actors,
            actor_roles,
            certification,
            collection_id,
            collection_name,
            collection_overview,
            collection_total_movies,
        }))
    }

    /// Process a sibling movie file using already-matched metadata from the same directory.
    ///
    /// When multiple video files exist in the same directory (e.g., different resolutions),
    /// this function uses the cached movie metadata from the first matched file.
    #[allow(dead_code)]
    async fn process_sibling_movie(
        &self,
        video: &VideoFile,
        target: &Path,
        cached_movie: &MovieMetadata,
        precomputed_ffprobe: Option<&VideoMetadata>,
    ) -> Result<PlanItem> {
        tracing::debug!(
            "[SIBLING] Using cached metadata for: {} -> {}",
            video.filename,
            cached_movie.title
        );

        // Get video metadata
        let video_metadata = match precomputed_ffprobe {
            Some(meta) => meta.clone(),
            None => ffprobe::parse_metadata_from_filename(&video.filename),
        };

        // Create a dummy parsed filename (we don't need AI parsing since we have cached metadata)
        let parsed = ParsedFilename {
            title: Some(cached_movie.title.clone()),
            original_title: Some(cached_movie.original_title.clone()),
            year: Some(cached_movie.year),
            confidence: 1.0,
            raw_response: Some("sibling_movie_cached".to_string()),
            ..Default::default()
        };

        // Generate target info using the cached movie metadata
        let (target_info, operations, poster_download) = self
            .generate_target_info(
                video,
                &Some(cached_movie.clone()),
                &None,
                &parsed,
                &video_metadata,
                target,
                MediaType::Movies,
            )?
            .ok_or_else(|| {
                crate::Error::other("Failed to generate target info for sibling movie")
            })?;

        Ok(PlanItem {
            id: uuid::Uuid::new_v4().to_string(),
            status: PlanItemStatus::Pending,
            source: video.clone(),
            parsed: ParsedInfo {
                title: parsed.title,
                original_title: parsed.original_title,
                year: parsed.year,
                confidence: 1.0,
                raw_response: parsed.raw_response,
            },
            movie_metadata: Some(cached_movie.clone()),
            tv_series_metadata: None,
            episode_metadata: None,
            season_metadata: None,
            video_metadata,
            target: target_info,
            operations,
            poster_download,
        })
    }

    /// Generate target path information and operations.
    /// Returns None if country information cannot be determined (skip rather than wrong match).
    #[allow(clippy::too_many_arguments)]
    fn generate_target_info(
        &self,
        video: &VideoFile,
        movie_metadata: &Option<MovieMetadata>,
        tv_series_metadata: &Option<(TvSeriesMetadata, Option<EpisodeMetadata>, Option<SeasonMetadata>)>,
        parsed: &ParsedFilename,
        video_metadata: &VideoMetadata,
        target: &Path,
        media_type: MediaType,
    ) -> Result<Option<(TargetInfo, Vec<Operation>, Option<PosterDownloadStatus>)>> {
        let mut operations = Vec::new();

        // Get file extension
        let extension = video
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mkv");

        let (folder_name, filename, nfo_name, season_folder) = match media_type {
            MediaType::Movies => {
                let metadata = movie_metadata
                    .as_ref()
                    .ok_or_else(|| crate::Error::other("Missing movie metadata"))?;

                let folder = gen_folder::generate_movie_folder(metadata, None);

                // Extract disc identifier from source filename (cd1, cd2, part1, part2, etc.)
                let disc_id = gen_filename::extract_disc_identifier(&video.filename);
                if disc_id.is_some() {
                    tracing::debug!(
                        "[MULTI-DISC] Detected disc identifier '{}' in: {}",
                        disc_id.as_ref().unwrap(),
                        video.filename
                    );
                }

                let filename = gen_filename::generate_movie_filename_with_disc(
                    metadata,
                    video_metadata,
                    None,
                    disc_id.as_deref(),
                    extension,
                );
                let nfo = filename.replace(&format!(".{}", extension), ".nfo");

                (folder, filename, nfo, None)
            }
            MediaType::TvSeries => {
                let (show, episode, _season) = tv_series_metadata
                    .as_ref()
                    .ok_or_else(|| crate::Error::other("Missing TV show metadata"))?;

                // TV show folder: ShowName (Year)
                let folder = gen_folder::generate_tv_series_folder(show);

                // Season folder: Season XX
                let season_num = parsed.season.unwrap_or(1);
                let season_folder_name = format!("Season {:02}", season_num);

                // Episode filename
                let ep_num = parsed.episode.unwrap_or(1);
                let ep_meta = episode.clone().unwrap_or_else(|| EpisodeMetadata {
                    season_number: season_num,
                    episode_number: ep_num,
                    name: format!("Episode {}", ep_num),
                    original_name: None,
                    air_date: None,
                    overview: None,
                });
                let filename = gen_filename::generate_episode_filename(
                    show,
                    &ep_meta,
                    video_metadata,
                    extension,
                );

                // For TV shows, use tvshow.nfo in root folder (not per-episode)
                // Jellyfin/Kodi will fetch episode info automatically
                let nfo = filename.replace(&format!(".{}", extension), ".nfo");

                (folder, filename, nfo, Some(season_folder_name))
            }
        };

        // Get language folder name (e.g., "ZH_Chinese", "EN_English", "JA_Japanese")
        // Uses original_language from TMDB for classification
        let language_folder = match media_type {
            MediaType::Movies => {
                movie_metadata
                    .as_ref()
                    .map(|m| format_language_folder(&m.original_language))
            }
            MediaType::TvSeries => {
                tv_series_metadata
                    .as_ref()
                    .map(|(show, _, _)| format_language_folder(&show.original_language))
            }
        };

        // Principle: prefer skipping over wrong classification
        let language_folder = match language_folder {
            Some(folder) => folder,
            None => {
                tracing::warn!(
                    "Skipping {}: cannot determine language (prefer skip over wrong match)",
                    video.filename
                );
                return Ok(None);
            }
        };

        // Build target paths with language folder layer
        let language_path = target.join(&language_folder);
        let show_folder = language_path.join(&folder_name);
        let target_folder = if let Some(ref season_dir) = season_folder {
            show_folder.join(season_dir)
        } else {
            show_folder.clone()
        };
        let target_file = target_folder.join(&filename);

        // NFO goes in same folder as video file
        let target_nfo = target_folder.join(&nfo_name);

        // Operation 1: Create directory (including parent dirs)
        operations.push(Operation {
            op: OperationType::Mkdir,
            from: None,
            to: target_folder.clone(),
            url: None,
            content_ref: None,
        });

        // Operation 2: Move video file
        operations.push(Operation {
            op: OperationType::Move,
            from: Some(video.path.clone()),
            to: target_file.clone(),
            url: None,
            content_ref: None,
        });

        // Operation 2.5: Move subtitle, sample, extras, and poster files (keep original names)
        let media_titles = match media_type {
            MediaType::Movies => {
                movie_metadata.as_ref().map(|m| {
                    let chinese_title = &m.title;
                    let original_title = &m.original_title;
                    (chinese_title.as_str(), original_title.as_str())
                })
            }
            MediaType::TvSeries => {
                tv_series_metadata.as_ref().map(|(show, _, _)| {
                    let chinese_title = &show.name;
                    let original_title = &show.original_name;
                    (chinese_title.as_str(), original_title.as_str())
                })
            }
        };
        self.add_auxiliary_operations(&video.parent_dir, &target_folder, &mut operations, media_titles);

        // Operation 3: Create NFO file
        match media_type {
            MediaType::Movies => {
                if self.config.generate_nfo && self.config.generate_movie_nfo {
                    operations.push(Operation {
                        op: OperationType::Create,
                        from: None,
                        to: target_nfo.clone(),
                        url: None,
                        content_ref: Some("nfo".to_string()),
                    });
                }
            }
            MediaType::TvSeries => {
                // Create episode NFO
                if self.config.generate_nfo && self.config.generate_tv_episode_nfo {
                    operations.push(Operation {
                        op: OperationType::Create,
                        from: None,
                        to: target_nfo.clone(),
                        url: None,
                        content_ref: Some("nfo".to_string()),
                    });
                }

                // Create season NFO in season folder
                if self.config.generate_nfo && self.config.generate_tv_season_nfo {
                    if season_folder.is_some() {
                        let (show, _, _) = tv_series_metadata.as_ref().unwrap();
                        let season_nfo_name = format!("[{}]-season{:02}.nfo", show.name, parsed.season.unwrap_or(1));
                        let season_nfo_path = target_folder.join(season_nfo_name);
                        operations.push(Operation {
                            op: OperationType::Create,
                            from: None,
                            to: season_nfo_path,
                            url: None,
                            content_ref: Some("nfo".to_string()),
                        });
                    }
                }
            }
        }

        // Operation 4: Download poster
        // Only download if:
        // 1. download_posters config is enabled
        // 2. No local image file with the same name will be moved (to avoid conflicts)
        let mut poster_download_status: Option<PosterDownloadStatus> = None;
        
        if self.config.download_posters {
            // Poster goes in same folder as video file
            let poster_folder = target_folder.clone();

            let poster_url = movie_metadata
                .as_ref()
                .and_then(|m| m.poster_urls.first().cloned())
                .or_else(|| {
                    tv_series_metadata
                        .as_ref()
                        .and_then(|(s, _, _)| s.poster_urls.first().cloned())
                });

            if let Some(url) = poster_url {
                // For movies: use video filename as poster name
                // For TV series: use [show.name]-seasonXX.jpg (same naming as NFO)
                let poster_filename = match media_type {
                    MediaType::Movies => filename.replace(&format!(".{}", extension), ".jpg"),
                    MediaType::TvSeries => {
                        // Use [show.name]-seasonXX.jpg for TV series to ensure only one poster per season
                        // and keep consistent naming with season NFO
                        let (show, _, _) = tv_series_metadata.as_ref().unwrap();
                        format!("[{}]-season{:02}.jpg", show.name, parsed.season.unwrap_or(1))
                    }
                };
                let poster_path = poster_folder.join(&poster_filename);

                // Check if a local image with the same name will be moved
                // If yes, skip downloading to avoid duplicate target path conflicts
                let has_local_image = operations.iter().any(|op| {
                    if op.op == OperationType::Move {
                        if let Some(ref from_path) = op.from {
                            // Check if the moved file has the same filename as the poster
                            from_path.file_name() == poster_path.file_name()
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                });

                if !has_local_image {
                    operations.push(Operation {
                        op: OperationType::Download,
                        from: None,
                        to: poster_path,
                        url: Some(url),
                        content_ref: None,
                    });
                    poster_download_status = Some(PosterDownloadStatus::Download);
                } else {
                    tracing::info!(
                        "[POSTER] Skipping download: local image with same name exists: {}",
                        poster_filename
                    );
                    poster_download_status = Some(PosterDownloadStatus::SkippedLocalExists);
                }
            } else {
                poster_download_status = Some(PosterDownloadStatus::NotAvailable);
            }
        }

        let display_folder = if let Some(ref season_dir) = season_folder {
            format!("{}/{}", folder_name, season_dir)
        } else {
            folder_name.clone()
        };

        let target_info = TargetInfo {
            folder: display_folder,
            filename,
            full_path: target_file,
            nfo: nfo_name,
            poster: Some("poster.jpg".to_string()),
        };

        Ok(Some((target_info, operations, poster_download_status)))
    }

    /// Add operations to move subtitle, sample, extras, and poster files.
    ///
    /// Detects and moves:
    /// - Subtitle folders: Sub, Subs, Subtitle, Subtitles, etc.
    /// - Subtitle files: .srt, .ass, .ssa, .sub, .idx, .vtt, .sup
    /// - Sample videos: files/folders with "sample" in the name
    /// - Extras folders: Extras, Bonus, Deleted Scenes, etc.
    /// - Poster images: poster.jpg, folder.jpg, etc.
    ///
    /// Files/folders are moved without renaming.
    /// Note: Duplicates are handled by deduplicate_operations() at the plan level.
    fn add_auxiliary_operations(
        &self,
        source_dir: &Path,
        target_folder: &Path,
        operations: &mut Vec<Operation>,
        media_titles: Option<(&str, &str)>,
    ) {
        // Subtitle folder names (case-insensitive)
        const SUBTITLE_FOLDERS: &[&str] = &["sub", "subs", "subtitle", "subtitles", "字幕"];

        // Subtitle file extensions
        const SUBTITLE_EXTENSIONS: &[&str] =
            &["srt", "ass", "ssa", "sub", "idx", "vtt", "sup", "smi"];

        // Poster image extensions
        const POSTER_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

        // Poster file names (case-insensitive)
        const POSTER_FILENAMES: &[&str] = &[
            "poster",
            "folder",
            "cover",
            "thumb",
            "thumbnail",
            "海报",
            "封面",
        ];

        // Extras folder patterns (case-insensitive)
        const EXTRAS_PATTERNS: &[&str] = &[
            "extras",
            "extra",
            "featurettes",
            "featurette",
            "behind the scenes",
            "behindthescenes",
            "deleted scenes",
            "deletedscenes",
            "making of",
            "makingof",
            "bonus",
            "bonuses",
            "special features",
            "specialfeatures",
            "sample",
            "samples",
        ];

        // OST (Original Soundtrack) folder patterns (case-insensitive)
        // Note: These patterns check for EXACT matches or containment
        // "音乐" can match "音乐" but not "Extras" due to word boundary checks
        const OST_PATTERNS: &[&str] = &[
            "ost",
            "soundtrack",
            " soundtrack",
            "原声带",
            "原声音乐",
            "audio",
            "music",
            "score",
        ];

        // Chinese keywords that indicate OST folders (require exact match or suffix)
        const OST_CHINESE_PATTERNS: &[&str] = &[
            "音乐",
        ];

        // Read source directory
        let entries = match std::fs::read_dir(source_dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::debug!("Could not read source dir for auxiliary files: {}", e);
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            let name_lower = name.to_lowercase();

            // Check for folders
            if path.is_dir() {
                // Check for subtitle folders
                if self.config.move_subtitles
                    && SUBTITLE_FOLDERS.iter().any(|&f| name_lower == f)
                {
                    let target_path = target_folder.join(name);
                    tracing::debug!(
                        "Adding subtitle folder move: {} -> {}",
                        path.display(),
                        target_path.display()
                    );

                    operations.push(Operation {
                        op: OperationType::Move,
                        from: Some(path.clone()),
                        to: target_path,
                        url: None,
                        content_ref: None,
                    });
                    continue;
                }

                // Check for extras folders (exact match or pattern match)
                if self.config.move_extras {
                    let is_extras = EXTRAS_PATTERNS.iter().any(|&p| name_lower == p)
                        || name_lower.contains(".extras")
                        || name_lower.contains("-extras")
                        || name_lower.contains("_extras")
                        || name_lower.contains(".featurette")
                        || name_lower.contains("-featurette")
                        || name_lower.contains(".sample")
                        || name_lower.contains("-sample");

                    if is_extras {
                        let target_path = target_folder.join(name);
                        tracing::debug!(
                            "Adding extras folder move: {} -> {}",
                            path.display(),
                            target_path.display()
                        );

                        operations.push(Operation {
                            op: OperationType::Move,
                            from: Some(path.clone()),
                            to: target_path,
                            url: None,
                            content_ref: None,
                        });
                        continue;
                    }
                }

                // Check for sample directories
                if self.config.move_samples {
                    if name_lower == "sample" || name_lower == "samples" {
                        let target_path = target_folder.join(name);
                        tracing::debug!(
                            "Adding sample folder move: {} -> {}",
                            path.display(),
                            target_path.display()
                        );

                        operations.push(Operation {
                            op: OperationType::Move,
                            from: Some(path.clone()),
                            to: target_path,
                            url: None,
                            content_ref: None,
                        });
                        continue;
                    }
                }

                // Check for poster folders (e.g., "poster/", "posters/")
                if self.config.move_posters {
                    if name_lower == "poster" || name_lower == "posters"
                        || name_lower == "folder"
                        || name_lower == "cover"
                    {
                        let target_path = target_folder.join(name);
                        tracing::debug!(
                            "Adding poster folder move: {} -> {}",
                            path.display(),
                            target_path.display()
                        );

                        operations.push(Operation {
                            op: OperationType::Move,
                            from: Some(path.clone()),
                            to: target_path,
                            url: None,
                            content_ref: None,
                        });
                    }
                }

                // Check for OST (Original Soundtrack) folders
                if self.config.move_ost {
                    // English patterns: exact match OR contains the pattern (but not as substring of another word)
                    let is_english_ost = OST_PATTERNS.iter().any(|&p| {
                        name_lower == p
                            || name_lower.ends_with(p)
                            || name_lower.starts_with(&format!("{}-", p))
                            || name_lower.starts_with(&format!("{}_", p))
                            || name_lower.starts_with(&format!("{} ", p))
                            || name_lower.contains(&format!("-{}", p))
                            || name_lower.contains(&format!("_{}", p))
                            || name_lower.contains(&format!(" {}", p))
                    });

                    // Chinese patterns: exact match only (to avoid "音乐" matching "Extras")
                    let is_chinese_ost = OST_CHINESE_PATTERNS.iter().any(|&p| name_lower == p);

                    let is_ost = is_english_ost || is_chinese_ost;

                    if is_ost {
                        let target_path = target_folder.join(name);
                        tracing::debug!(
                            "Adding OST folder move: {} -> {}",
                            path.display(),
                            target_path.display()
                        );

                        operations.push(Operation {
                            op: OperationType::Move,
                            from: Some(path.clone()),
                            to: target_path,
                            url: None,
                            content_ref: None,
                        });
                        continue;
                    }
                }
            }
            // Check for files
            else if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();

                // Check for subtitle files
                if self.config.move_subtitles
                    && SUBTITLE_EXTENSIONS.iter().any(|&e| ext == e)
                {
                    let target_path = target_folder.join(name);
                    tracing::debug!(
                        "Adding subtitle file move: {} -> {}",
                        path.display(),
                        target_path.display()
                    );

                    operations.push(Operation {
                        op: OperationType::Move,
                        from: Some(path.clone()),
                        to: target_path,
                        url: None,
                        content_ref: None,
                    });
                    continue;
                }

                // Check for poster images
                if self.config.move_posters
                    && POSTER_EXTENSIONS.iter().any(|&e| ext == e)
                {
                    // Check if filename matches poster patterns
                    // Supports: poster.jpg, folder.png, cover.webp
                    // Also supports: video-name-poster.jpg, movie-fanart.png, etc.
                    let is_poster = POSTER_FILENAMES
                        .iter()
                        .any(|&p| name_lower.starts_with(p))
                        || name_lower.contains("-poster")
                        || name_lower.contains("-fanart")
                        || name_lower.contains("-cover")
                        || name_lower.contains("-thumb")
                        || name_lower.contains("-thumbnail")
                        || name_lower.contains("-clearlogo");

                    // NEW: Check if filename contains media title (Chinese or original)
                    let contains_media_title = if let Some((chinese_title, original_title)) = media_titles {
                        // Sanitize titles to match sanitized filenames (special chars -> _)
                        let sanitize_for_match = |s: &str| {
                            s.chars()
                                .map(|c| match c {
                                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                                    _ => c,
                                })
                                .collect::<String>()
                        };
                        let sanitized_chinese = sanitize_for_match(chinese_title).to_lowercase();
                        let sanitized_original = sanitize_for_match(original_title).to_lowercase();
                        name_lower.contains(&sanitized_chinese)
                            || name_lower.contains(&sanitized_original)
                    } else {
                        false
                    };

                    if is_poster || contains_media_title {
                        let target_path = target_folder.join(name);
                        tracing::debug!(
                            "Adding poster image move: {} -> {}",
                            path.display(),
                            target_path.display()
                        );

                        operations.push(Operation {
                            op: OperationType::Move,
                            from: Some(path.clone()),
                            to: target_path,
                            url: None,
                            content_ref: None,
                        });
                        continue;
                    }
                }

                // Check for sample video files (files with "sample" in filename)
                if self.config.move_samples {
                    let is_video = [
                        "mkv", "mp4", "avi", "mov", "wmv", "m4v", "ts", "m2ts", "flv", "webm",
                    ]
                    .iter()
                    .any(|&e| ext == e);
                    let is_sample =
                        name_lower.contains("sample") && !name_lower.contains("sampler");

                    if is_video && is_sample {
                        let target_path = target_folder.join(name);
                        tracing::debug!(
                            "Adding sample video file move: {} -> {}",
                            path.display(),
                            target_path.display()
                        );

                        operations.push(Operation {
                            op: OperationType::Move,
                            from: Some(path.clone()),
                            to: target_path,
                            url: None,
                            content_ref: None,
                        });
                    }
                }
            }
        }
    }

    /// Process sample files.
    fn process_samples(
        &self,
        samples: &[VideoFile],
        items: &[PlanItem],
        _target: &Path,
    ) -> Vec<SampleItem> {
        samples
            .iter()
            .filter_map(|sample| {
                // Try to find a matching item by parent directory
                let matching_item = items.iter().find(|item| {
                    sample.parent_dir == item.source.parent_dir
                        || sample.parent_dir.starts_with(&item.source.parent_dir)
                });

                matching_item.map(|item| {
                    // Use the full target path's parent to get the correct directory
                    // (includes language folder like EN_English/MovieFolder)
                    let target_folder = item
                        .target
                        .full_path
                        .parent()
                        .unwrap_or(_target)
                        .join("Sample");
                    let target_file = target_folder.join(&sample.filename);

                    SampleItem {
                        source: sample.path.clone(),
                        target: target_file,
                    }
                })
            })
            .collect()
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::new().expect("Failed to create default planner")
    }
}

/// Generate a plan for organizing videos (convenience function).
pub async fn generate_plan(source: &Path, target: &Path, media_type: MediaType) -> Result<Plan> {
    let planner = Planner::new()?;
    planner.generate(source, target, media_type).await
}

/// Save a plan to a JSON file.
pub fn save_plan(plan: &Plan, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(plan)?;

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::File::create(path)?;
    file.write_all(json.as_bytes())?;

    tracing::info!("Plan saved to {:?}", path);
    Ok(())
}

/// Load a plan from a JSON file.
pub fn load_plan(path: &Path) -> Result<Plan> {
    let content = fs::read_to_string(path)?;
    let plan: Plan = serde_json::from_str(&content)?;
    Ok(plan)
}

/// Get the default plan output path.
/// Saves to target directory if provided, otherwise to source directory.
pub fn default_plan_path(source: &Path, target: Option<&Path>) -> PathBuf {
    let filename = format!("plan_{}.json", Utc::now().format("%Y%m%d_%H%M%S"));
    // Prefer target directory, fallback to source
    let base_dir = target.unwrap_or(source);
    base_dir.join(filename)
}

/// Get the sessions directory.
pub fn sessions_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| crate::Error::other("Cannot find home directory"))?;
    let dir = home
        .join(".config")
        .join("media_organizer")
        .join("sessions");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Save plan to sessions directory.
pub fn save_to_sessions(plan: &Plan) -> Result<PathBuf> {
    let session_id = format!(
        "{}_{}",
        Utc::now().format("%Y%m%d_%H%M%S"),
        &Uuid::new_v4().to_string()[..8]
    );

    let sessions = sessions_dir()?;
    let session_dir = sessions.join(&session_id);
    fs::create_dir_all(&session_dir)?;

    let plan_path = session_dir.join("plan.json");
    save_plan(plan, &plan_path)?;

    tracing::info!("Session saved: {}", session_id);
    Ok(session_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_config_default() {
        let config = PlannerConfig::default();
        assert_eq!(config.min_confidence, 0.7); // Higher threshold: prefer skipping over wrong matches
        assert!(config.download_posters);
        assert!(config.generate_nfo);
        assert_eq!(config.poster_size, "w500");
    }

    #[test]
    fn test_default_plan_path() {
        let source = Path::new("/tmp/movies");
        let target = Path::new("/tmp/movies_organized");

        // Test with target
        let path = default_plan_path(source, Some(target));
        assert!(path.to_string_lossy().contains("plan_"));
        assert!(path.to_string_lossy().ends_with(".json"));
        assert!(path.starts_with(target));

        // Test without target (falls back to source)
        let path = default_plan_path(source, None);
        assert!(path.starts_with(source));
    }

    // test_save_and_load_plan moved to tests/io_tests.rs

    #[test]
    fn test_language_code_to_name() {
        // Major languages
        assert_eq!(language_code_to_name("en"), "English");
        assert_eq!(language_code_to_name("zh"), "Chinese");
        assert_eq!(language_code_to_name("ja"), "Japanese");
        assert_eq!(language_code_to_name("ko"), "Korean");
        assert_eq!(language_code_to_name("fr"), "French");
        assert_eq!(language_code_to_name("de"), "German");
        assert_eq!(language_code_to_name("es"), "Spanish");
        assert_eq!(language_code_to_name("it"), "Italian");

        // Case insensitive
        assert_eq!(language_code_to_name("EN"), "English");
        assert_eq!(language_code_to_name("ZH"), "Chinese");

        // Asian languages
        assert_eq!(language_code_to_name("th"), "Thai");
        assert_eq!(language_code_to_name("vi"), "Vietnamese");
        assert_eq!(language_code_to_name("id"), "Indonesian");

        // Chinese variants
        assert_eq!(language_code_to_name("cn"), "Chinese");
        assert_eq!(language_code_to_name("yue"), "Cantonese");

        // Unknown language - returns uppercase code
        assert_eq!(language_code_to_name("xx"), "XX");
        assert_eq!(language_code_to_name("unknown"), "UNKNOWN");
    }

    #[test]
    fn test_format_language_folder() {
        // Standard cases
        assert_eq!(format_language_folder("en"), "EN_English");
        assert_eq!(format_language_folder("zh"), "ZH_Chinese");
        assert_eq!(format_language_folder("ja"), "JA_Japanese");
        assert_eq!(format_language_folder("ko"), "KO_Korean");
        assert_eq!(format_language_folder("fr"), "FR_French");

        // Case insensitive input
        assert_eq!(format_language_folder("EN"), "EN_English");
        assert_eq!(format_language_folder("ZH"), "ZH_Chinese");

        // TMDB quirk: "cn" should normalize to "zh"
        assert_eq!(format_language_folder("cn"), "ZH_Chinese");
        assert_eq!(format_language_folder("CN"), "ZH_Chinese");

        // Unknown language
        assert_eq!(format_language_folder("xx"), "XX_XX");
    }

    #[test]
    fn test_normalize_language_code() {
        assert_eq!(normalize_language_code("cn"), "zh");
        assert_eq!(normalize_language_code("CN"), "zh");
        assert_eq!(normalize_language_code("zh"), "zh");
        assert_eq!(normalize_language_code("en"), "en");
        assert_eq!(normalize_language_code("ja"), "ja");
    }

    #[test]
    fn test_country_code_to_name() {
        // Major countries (used for NFO metadata)
        assert_eq!(country_code_to_name("US"), "United States");
        assert_eq!(country_code_to_name("CN"), "China");
        assert_eq!(country_code_to_name("JP"), "Japan");
        assert_eq!(country_code_to_name("KR"), "South Korea");
        assert_eq!(country_code_to_name("GB"), "United Kingdom");
        assert_eq!(country_code_to_name("ID"), "Indonesia");

        // Case insensitive
        assert_eq!(country_code_to_name("us"), "United States");
        assert_eq!(country_code_to_name("cn"), "China");

        // Unknown country - returns uppercase code
        assert_eq!(country_code_to_name("XX"), "XX");
    }

    #[test]
    fn test_add_auxiliary_operations_with_media_titles() {
        use std::fs;
        
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        // Create test files
        // 会被移动：包含标题或海报关键字
        fs::File::create(source_dir.join("黑暗骑士-poster.jpg")).unwrap();     // -poster
        fs::File::create(source_dir.join("The Dark Knight-cover.png")).unwrap(); // -cover
        fs::File::create(source_dir.join("黑暗骑士_剧照.webp")).unwrap();         // 包含标题
        fs::File::create(source_dir.join("The Dark Knight fanart.jpg")).unwrap(); // -fanart
        fs::File::create(source_dir.join("other-movie-poster.jpg")).unwrap();     // -poster（原有逻辑）
        fs::File::create(source_dir.join("poster.jpg")).unwrap();                  // 标准海报
        fs::File::create(source_dir.join("folder.png")).unwrap();                  // 标准海报
        
        // 不会被移动
        fs::File::create(source_dir.join("random-image.png")).unwrap();            // 不匹配任何条件

        let mut planner = Planner::new().unwrap();
        planner.config.move_posters = true;
        
        let mut operations = Vec::new();
        let media_titles = Some(("黑暗骑士", "The Dark Knight"));
        
        planner.add_auxiliary_operations(&source_dir, &target_dir, &mut operations, media_titles);

        // 7 个文件会被移动（包括 other-movie-poster.jpg，因为包含 -poster）
        assert_eq!(operations.len(), 7);
        
        let moved_files: Vec<String> = operations
            .iter()
            .filter(|op| op.op == OperationType::Move)
            .map(|op| op.to.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        
        assert!(moved_files.iter().any(|s| s == "黑暗骑士-poster.jpg"));
        assert!(moved_files.iter().any(|s| s == "The Dark Knight-cover.png"));
        assert!(moved_files.iter().any(|s| s == "黑暗骑士_剧照.webp"));
        assert!(moved_files.iter().any(|s| s == "The Dark Knight fanart.jpg"));
        assert!(moved_files.iter().any(|s| s == "other-movie-poster.jpg")); // 原有逻辑
        assert!(moved_files.iter().any(|s| s == "poster.jpg"));
        assert!(moved_files.iter().any(|s| s == "folder.png"));
        assert!(!moved_files.iter().any(|s| s == "random-image.png"));
    }

    #[test]
    fn test_add_auxiliary_operations_without_media_titles() {
        use std::fs;
        
        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        // "黑暗骑士-poster.jpg" 会因为包含 "-poster" 被移动（原有逻辑）
        fs::File::create(source_dir.join("黑暗骑士-poster.jpg")).unwrap();
        fs::File::create(source_dir.join("poster.jpg")).unwrap();
        fs::File::create(source_dir.join("folder.png")).unwrap();
        // 这个不会被移动，因为既不是标准海报也不包含标题
        fs::File::create(source_dir.join("黑暗骑士.jpg")).unwrap();

        let mut planner = Planner::new().unwrap();
        planner.config.move_posters = true;
        
        let mut operations = Vec::new();
        
        planner.add_auxiliary_operations(&source_dir, &target_dir, &mut operations, None);

        // 3 个文件会被移动：黑暗骑士-poster.jpg（因为包含-poster）、poster.jpg、folder.png
        assert_eq!(operations.len(), 3);
        
        let moved_files: Vec<String> = operations
            .iter()
            .filter(|op| op.op == OperationType::Move)
            .map(|op| op.to.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        
        assert!(moved_files.iter().any(|s| s == "黑暗骑士-poster.jpg"));
        assert!(moved_files.iter().any(|s| s == "poster.jpg"));
        assert!(moved_files.iter().any(|s| s == "folder.png"));
        // "黑暗骑士.jpg" 不会被移动，因为没有媒体标题匹配
        assert!(!moved_files.iter().any(|s| s == "黑暗骑士.jpg"));
    }

    #[test]
    fn test_add_auxiliary_operations_hebrew_movie() {
        use std::fs;
        
        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        fs::File::create(source_dir.join("危墙-poster.jpg")).unwrap();
        fs::File::create(source_dir.join("חיתוך-fanart.png")).unwrap();
        fs::File::create(source_dir.join("危墙_剧照.webp")).unwrap();

        let mut planner = Planner::new().unwrap();
        planner.config.move_posters = true;
        
        let mut operations = Vec::new();
        let media_titles = Some(("危墙", "חיתוך"));
        
        planner.add_auxiliary_operations(&source_dir, &target_dir, &mut operations, media_titles);

        assert_eq!(operations.len(), 3);
        
        let moved_files: Vec<String> = operations
            .iter()
            .filter(|op| op.op == OperationType::Move)
            .map(|op| op.to.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        
        assert!(moved_files.iter().any(|s| s == "危墙-poster.jpg"));
        assert!(moved_files.iter().any(|s| s == "חיתוך-fanart.png"));
        assert!(moved_files.iter().any(|s| s == "危墙_剧照.webp"));
    }

    #[test]
    fn test_add_auxiliary_operations_tv_series() {
        use std::fs;
        
        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        // 创建包含标题的图片文件
        fs::File::create(source_dir.join("绝命毒师.jpg")).unwrap();
        fs::File::create(source_dir.join("Breaking Bad.png")).unwrap();
        // 创建不包含标题的图片文件
        fs::File::create(source_dir.join("other-show.jpg")).unwrap();

        let mut planner = Planner::new().unwrap();
        planner.config.move_posters = true;
        
        let mut operations = Vec::new();
        let media_titles = Some(("绝命毒师", "Breaking Bad"));

        planner.add_auxiliary_operations(&source_dir, &target_dir, &mut operations, media_titles);

        // 2 个文件会被移动：绝命毒师.jpg 和 Breaking Bad.png（因为包含媒体标题）
        assert_eq!(operations.len(), 2);

        let moved_files: Vec<String> = operations
            .iter()
            .filter(|op| op.op == OperationType::Move)
            .map(|op| op.to.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(moved_files.iter().any(|s| s == "绝命毒师.jpg"));
        assert!(moved_files.iter().any(|s| s == "Breaking Bad.png"));
        assert!(!moved_files.iter().any(|s| s == "other-show.jpg"));
    }

    #[test]
    fn test_add_auxiliary_operations_with_special_chars_in_title() {
        use std::fs;
        
        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        // 电影标题包含冒号: "Kim Dotcom: Caught in the Web"
        // 文件名中冒号被替换为下划线: "Kim Dotcom_ Caught in the Web"
        fs::File::create(source_dir.join("[K][Kim Dotcom_ Caught in the Web](2017)-1920x1080.jpg")).unwrap();
        fs::File::create(source_dir.join("Kim Dotcom_ Caught in the Web-poster.jpg")).unwrap();
        
        // 包含其他特殊字符的标题
        fs::File::create(source_dir.join("Test*Movie.jpg")).unwrap();
        fs::File::create(source_dir.join("Test?Question.jpg")).unwrap();

        let mut planner = Planner::new().unwrap();
        planner.config.move_posters = true;
        
        let mut operations = Vec::new();
        // 原始标题包含特殊字符
        let media_titles = Some(("Kim Dotcom: Caught in the Web", "Kim Dotcom: Caught in the Web"));
        
        planner.add_auxiliary_operations(&source_dir, &target_dir, &mut operations, media_titles);

        // 2 个文件会被移动：包含 sanitize 后的标题
        assert_eq!(operations.len(), 2);
        
        let moved_files: Vec<String> = operations
            .iter()
            .filter(|op| op.op == OperationType::Move)
            .map(|op| op.to.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        
        assert!(moved_files.iter().any(|s| s == "[K][Kim Dotcom_ Caught in the Web](2017)-1920x1080.jpg"));
        assert!(moved_files.iter().any(|s| s == "Kim Dotcom_ Caught in the Web-poster.jpg"));
        // 不匹配的文件不会被移动
        assert!(!moved_files.iter().any(|s| s == "Test*Movie.jpg"));
        assert!(!moved_files.iter().any(|s| s == "Test?Question.jpg"));
    }

    #[test]
    fn test_validate_no_duplicate_targets_allows_different_file_types() {
        // Test that the duplicate detection logic correctly identifies that
        // .jpg and .mp4 files with the same base name are NOT duplicates
        use std::collections::HashMap;

        let mut target_to_sources: HashMap<PathBuf, Vec<(usize, PathBuf)>> = HashMap::new();

        // Simulate a scenario where:
        // - A .jpg file is moved (local image)
        // - A .mp4 file is moved (video)
        // Both have the same base name but different extensions
        target_to_sources.insert(
            PathBuf::from("/target/movie.jpg"),
            vec![(0, PathBuf::from("/source/movie.jpg"))],
        );
        target_to_sources.insert(
            PathBuf::from("/target/movie.mp4"),
            vec![(1, PathBuf::from("/source/movie.mp4"))],
        );

        // Filter logic: only consider as duplicate if:
        // 1. Multiple sources (len > 1)
        // 2. All sources have the same extension
        let true_duplicates: Vec<_> = target_to_sources
            .iter()
            .filter(|(_, sources)| {
                // If only one source, it's not a duplicate
                if sources.len() <= 1 {
                    return false;
                }

                // Check if all sources have the same extension
                let extensions: Vec<_> = sources
                    .iter()
                    .map(|(_, src)| {
                        src.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_lowercase())
                            .unwrap_or_default()
                    })
                    .collect();

                // Only a true duplicate if all extensions are the same
                extensions.iter().all(|ext| ext == &extensions[0])
            })
            .collect();

        // There should be no true duplicates since each target has only one source
        assert_eq!(true_duplicates.len(), 0);

        // Now test with actual duplicates: two operations moving to the same .jpg file
        let mut target_to_sources_with_duplicates: HashMap<PathBuf, Vec<(usize, PathBuf)>> = HashMap::new();

        // Two different sources trying to move to the same .jpg file
        target_to_sources_with_duplicates.insert(
            PathBuf::from("/target/movie.jpg"),
            vec![
                (0, PathBuf::from("/source/image1.jpg")),
                (1, PathBuf::from("/source/image2.jpg")),
            ],
        );

        let true_duplicates: Vec<_> = target_to_sources_with_duplicates
            .iter()
            .filter(|(_, sources)| {
                if sources.len() <= 1 {
                    return false;
                }

                let extensions: Vec<_> = sources
                    .iter()
                    .map(|(_, src)| {
                        src.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_lowercase())
                            .unwrap_or_default()
                    })
                    .collect();

                extensions.iter().all(|ext| ext == &extensions[0])
            })
            .collect();

        // There should be one true duplicate (.jpg -> .jpg conflict)
        assert_eq!(true_duplicates.len(), 1);
    }

    #[test]
    fn test_poster_download_status_enum() {
        // Test the PosterDownloadStatus enum
        let status_download = PosterDownloadStatus::Download;
        let status_skipped = PosterDownloadStatus::SkippedLocalExists;
        let status_not_available = PosterDownloadStatus::NotAvailable;

        // Just verify they can be created and are different
        assert_ne!(status_download, status_skipped);
        assert_ne!(status_download, status_not_available);
        assert_ne!(status_skipped, status_not_available);
    }

    #[test]
    fn test_poster_stats_struct() {
        // Test the PosterStats struct
        let stats = PosterStats {
            download_count: 2,
            skipped_count: 3,
        };

        assert_eq!(stats.download_count, 2);
        assert_eq!(stats.skipped_count, 3);
    }

    #[test]
    fn test_plan_item_with_poster_download_status() {
        // Test creating a PlanItem with poster_download status
        let plan_item = PlanItem {
            id: "test-id".to_string(),
            status: PlanItemStatus::Pending,
            source: VideoFile {
                path: PathBuf::from("/test/video.mp4"),
                filename: "video.mp4".to_string(),
                parent_dir: PathBuf::from("/test"),
                size: 1000,
                modified: chrono::Utc::now(),
                is_sample: false,
            },
            parsed: ParsedInfo {
                title: Some("Test Movie".to_string()),
                original_title: Some("Test Original".to_string()),
                year: Some(2024),
                confidence: 1.0,
                raw_response: None,
            },
            movie_metadata: None,
            tv_series_metadata: None,
            episode_metadata: None,
            season_metadata: None,
            video_metadata: VideoMetadata::default(),
            target: TargetInfo {
                folder: "Test Folder".to_string(),
                filename: "video.mp4".to_string(),
                full_path: PathBuf::from("/target/video.mp4"),
                nfo: "video.nfo".to_string(),
                poster: Some("poster.jpg".to_string()),
            },
            operations: Vec::new(),
            poster_download: Some(PosterDownloadStatus::SkippedLocalExists),
        };

        assert_eq!(plan_item.poster_download, Some(PosterDownloadStatus::SkippedLocalExists));
    }

    #[test]
    fn test_poster_stats_calculation() {
        // Test poster stats calculation logic
        let mut items = Vec::new();

        // Item 1: poster will be downloaded
        items.push(PlanItem {
            id: "1".to_string(),
            status: PlanItemStatus::Pending,
            source: VideoFile {
                path: PathBuf::from("/test/video1.mp4"),
                filename: "video1.mp4".to_string(),
                parent_dir: PathBuf::from("/test"),
                size: 1000,
                modified: chrono::Utc::now(),
                is_sample: false,
            },
            parsed: ParsedInfo::default(),
            movie_metadata: None,
            tv_series_metadata: None,
            episode_metadata: None,
            season_metadata: None,
            video_metadata: VideoMetadata::default(),
            target: TargetInfo::default(),
            operations: Vec::new(),
            poster_download: Some(PosterDownloadStatus::Download),
        });

        // Item 2: poster skipped (local exists)
        items.push(PlanItem {
            id: "2".to_string(),
            status: PlanItemStatus::Pending,
            source: VideoFile {
                path: PathBuf::from("/test/video2.mp4"),
                filename: "video2.mp4".to_string(),
                parent_dir: PathBuf::from("/test"),
                size: 1000,
                modified: chrono::Utc::now(),
                is_sample: false,
            },
            parsed: ParsedInfo::default(),
            movie_metadata: None,
            tv_series_metadata: None,
            episode_metadata: None,
            season_metadata: None,
            video_metadata: VideoMetadata::default(),
            target: TargetInfo::default(),
            operations: Vec::new(),
            poster_download: Some(PosterDownloadStatus::SkippedLocalExists),
        });

        // Item 3: poster skipped (local exists)
        items.push(PlanItem {
            id: "3".to_string(),
            status: PlanItemStatus::Pending,
            source: VideoFile {
                path: PathBuf::from("/test/video3.mp4"),
                filename: "video3.mp4".to_string(),
                parent_dir: PathBuf::from("/test"),
                size: 1000,
                modified: chrono::Utc::now(),
                is_sample: false,
            },
            parsed: ParsedInfo::default(),
            movie_metadata: None,
            tv_series_metadata: None,
            episode_metadata: None,
            season_metadata: None,
            video_metadata: VideoMetadata::default(),
            target: TargetInfo::default(),
            operations: Vec::new(),
            poster_download: Some(PosterDownloadStatus::SkippedLocalExists),
        });

        // Item 4: poster will be downloaded
        items.push(PlanItem {
            id: "4".to_string(),
            status: PlanItemStatus::Pending,
            source: VideoFile {
                path: PathBuf::from("/test/video4.mp4"),
                filename: "video4.mp4".to_string(),
                parent_dir: PathBuf::from("/test"),
                size: 1000,
                modified: chrono::Utc::now(),
                is_sample: false,
            },
            parsed: ParsedInfo::default(),
            movie_metadata: None,
            tv_series_metadata: None,
            episode_metadata: None,
            season_metadata: None,
            video_metadata: VideoMetadata::default(),
            target: TargetInfo::default(),
            operations: Vec::new(),
            poster_download: Some(PosterDownloadStatus::Download),
        });

        // Item 5: no poster info (status not set)
        items.push(PlanItem {
            id: "5".to_string(),
            status: PlanItemStatus::Pending,
            source: VideoFile {
                path: PathBuf::from("/test/video5.mp4"),
                filename: "video5.mp4".to_string(),
                parent_dir: PathBuf::from("/test"),
                size: 1000,
                modified: chrono::Utc::now(),
                is_sample: false,
            },
            parsed: ParsedInfo::default(),
            movie_metadata: None,
            tv_series_metadata: None,
            episode_metadata: None,
            season_metadata: None,
            video_metadata: VideoMetadata::default(),
            target: TargetInfo::default(),
            operations: Vec::new(),
            poster_download: None,
        });

        // Calculate stats (same logic as in planner.rs)
        let (downloaded, skipped) = items.iter()
            .filter(|item| item.status == PlanItemStatus::Pending)
            .fold((0, 0), |(d, s), item| {
                match item.poster_download {
                    Some(PosterDownloadStatus::Download) => (d + 1, s),
                    Some(PosterDownloadStatus::SkippedLocalExists) => (d, s + 1),
                    _ => (d, s),
                }
            });

        // Expected: 2 downloaded, 2 skipped, 1 not counted (None)
        assert_eq!(downloaded, 2);
        assert_eq!(skipped, 2);
    }

    #[test]
    fn test_plan_with_poster_stats() {
        // Test creating a Plan with poster_stats
        let plan = Plan {
            version: "1.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            media_type: Some(MediaType::Movies),
            source_path: PathBuf::from("/source"),
            target_path: PathBuf::from("/target"),
            items: Vec::new(),
            samples: Vec::new(),
            unknown: Vec::new(),
            poster_stats: Some(PosterStats {
                download_count: 5,
                skipped_count: 3,
            }),
        };

        assert!(plan.poster_stats.is_some());
        assert_eq!(plan.poster_stats.as_ref().unwrap().download_count, 5);
        assert_eq!(plan.poster_stats.as_ref().unwrap().skipped_count, 3);
    }

    #[test]
    fn test_add_auxiliary_operations_ost_folder() {
        use std::fs;

        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        // 创建各种 OST 文件夹
        fs::create_dir_all(source_dir.join("OST")).unwrap();
        fs::create_dir_all(source_dir.join("Soundtrack")).unwrap();
        fs::create_dir_all(source_dir.join("原声带")).unwrap();
        fs::create_dir_all(source_dir.join("Music")).unwrap();
        fs::create_dir_all(source_dir.join("audio")).unwrap();
        // 创建一个普通视频文件
        fs::File::create(source_dir.join("video.mp4")).unwrap();

        let mut planner = Planner::new().unwrap();
        planner.config.move_ost = true;
        planner.config.move_extras = false;  // 禁用 extras 移动

        let mut operations = Vec::new();
        planner.add_auxiliary_operations(&source_dir, &target_dir, &mut operations, None);

        // 应该移动 5 个 OST 相关文件夹
        // OST, Soundtrack, 原声带, Music, audio
        let move_ops: Vec<_> = operations
            .iter()
            .filter(|op| op.op == OperationType::Move)
            .collect();

        assert_eq!(move_ops.len(), 5, "Expected 5 OST folders to be moved, got {}", move_ops.len());

        let moved_folders: Vec<String> = move_ops
            .iter()
            .map(|op| op.to.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(moved_folders.iter().any(|s| s == "OST"));
        assert!(moved_folders.iter().any(|s| s == "Soundtrack"));
        assert!(moved_folders.iter().any(|s| s == "原声带"));
        assert!(moved_folders.iter().any(|s| s == "Music"));
        assert!(moved_folders.iter().any(|s| s == "audio"));
    }

    #[test]
    fn test_add_auxiliary_operations_ost_disabled() {
        use std::fs;

        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        // 创建 OST 文件夹
        fs::create_dir_all(source_dir.join("OST")).unwrap();
        fs::create_dir_all(source_dir.join("Soundtrack")).unwrap();

        let mut planner = Planner::new().unwrap();
        planner.config.move_ost = false;  // 禁用 OST 移动

        let mut operations = Vec::new();
        planner.add_auxiliary_operations(&source_dir, &target_dir, &mut operations, None);

        // 当 move_ost = false 时，不应该移动任何 OST 文件夹
        let move_ops: Vec<_> = operations
            .iter()
            .filter(|op| op.op == OperationType::Move)
            .collect();

        assert_eq!(move_ops.len(), 0, "Expected 0 OST folders when move_ost is disabled");
    }

    #[test]
    fn test_add_auxiliary_operations_ost_tv_series() {
        use std::fs;

        let temp_dir = tempfile::tempdir().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        // 创建 TV 系列的 OST 文件夹
        fs::create_dir_all(source_dir.join("Breaking Bad OST")).unwrap();
        fs::create_dir_all(source_dir.join("绝命毒师原声带")).unwrap();
        fs::create_dir_all(source_dir.join("Soundtrack")).unwrap();

        let mut planner = Planner::new().unwrap();
        planner.config.move_ost = true;

        let mut operations = Vec::new();
        planner.add_auxiliary_operations(&source_dir, &target_dir, &mut operations, None);

        // 应该移动 3 个 OST 文件夹
        let move_ops: Vec<_> = operations
            .iter()
            .filter(|op| op.op == OperationType::Move)
            .collect();

        assert_eq!(move_ops.len(), 3, "Expected 3 OST folders for TV series");

        let moved_folders: Vec<String> = move_ops
            .iter()
            .map(|op| op.to.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(moved_folders.iter().any(|s| s == "Breaking Bad OST"));
        assert!(moved_folders.iter().any(|s| s == "绝命毒师原声带"));
        assert!(moved_folders.iter().any(|s| s == "Soundtrack"));
    }

    // =============================================================================
    // Tests for select_best_movie_match with year weighting
    // =============================================================================

    fn create_movie_search_item(
        id: u64,
        title: &str,
        original_title: &str,
        release_date: Option<&str>,
        vote_count: u32,
    ) -> crate::services::tmdb::MovieSearchItem {
        crate::services::tmdb::MovieSearchItem {
            id,
            title: title.to_string(),
            original_title: original_title.to_string(),
            release_date: release_date.map(|s| s.to_string()),
            overview: None,
            poster_path: None,
            vote_count: Some(vote_count),
            vote_average: None,
        }
    }

    #[test]
    fn test_select_best_movie_match_exact_year_match() {
        // Test: When searching for "Aladdin" with year 2019, should prefer 2019 version over 1992
        let planner = Planner::new().unwrap();

        let movies = vec![
            create_movie_search_item(812, "Aladdin", "Aladdin", Some("1992-11-25"), 5000),
            create_movie_search_item(420817, "Aladdin", "Aladdin", Some("2019-05-22"), 3000),
        ];

        // Query with year 2019
        let result = planner.select_best_movie_match(&movies, "Aladdin", Some(2019));

        assert!(result.is_some());
        // Should select 2019 version (id: 420817) due to exact year match
        assert_eq!(result.unwrap().id, 420817);
    }

    #[test]
    fn test_select_best_movie_match_year_diff_1() {
        // Test: Year difference of 1 should get +5000 bonus
        let planner = Planner::new().unwrap();

        let movies = vec![
            create_movie_search_item(1, "Some Movie", "Some Movie", Some("2020-01-01"), 100),
            create_movie_search_item(2, "Some Movie", "Some Movie", Some("2021-01-01"), 100),
        ];

        // Query with year 2021 - should prefer 2021 version (+5000 year bonus)
        let result = planner.select_best_movie_match(&movies, "Some Movie", Some(2021));

        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 2);
    }

    #[test]
    fn test_select_best_movie_match_year_diff_2() {
        // Test: Year difference of 2 should get +1000 bonus
        let planner = Planner::new().unwrap();

        let movies = vec![
            create_movie_search_item(1, "Some Movie", "Some Movie", Some("2019-01-01"), 100),
            create_movie_search_item(2, "Some Movie", "Some Movie", Some("2021-01-01"), 100),
        ];

        // Query with year 2021 - should prefer 2021 version (+1000 year bonus)
        let result = planner.select_best_movie_match(&movies, "Some Movie", Some(2021));

        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 2);
    }

    #[test]
    fn test_select_best_movie_match_no_year_preference() {
        // Test: Without year info, should prefer higher votes
        let planner = Planner::new().unwrap();

        let movies = vec![
            create_movie_search_item(812, "Aladdin", "Aladdin", Some("1992-11-25"), 5000),
            create_movie_search_item(420817, "Aladdin", "Aladdin", Some("2019-05-22"), 3000),
        ];

        // Query without year - should prefer higher votes
        let result = planner.select_best_movie_match(&movies, "Aladdin", None);

        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 812); // Higher votes
    }

    #[test]
    fn test_select_best_movie_match_exact_title_match_takes_precedence() {
        // Test: Exact title match should still take precedence over year bonus
        let planner = Planner::new().unwrap();

        let movies = vec![
            create_movie_search_item(1, "Different Movie", "Different Movie", Some("2019-01-01"), 100),
            create_movie_search_item(2, "Aladdin", "Aladdin", Some("2020-01-01"), 100),
        ];

        // Query for "Aladdin" with year 2019 - should select exact title match despite year diff
        let result = planner.select_best_movie_match(&movies, "Aladdin", Some(2019));

        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 2); // Exact title match
    }

    #[test]
    fn test_select_best_movie_match_year_diff_greater_than_2() {
        // Test: Year difference > 2 should not get bonus, title match wins
        let planner = Planner::new().unwrap();

        let movies = vec![
            create_movie_search_item(1, "Aladdin", "Aladdin", Some("1990-01-01"), 100),
            create_movie_search_item(2, "Aladdin", "Aladdin", Some("2020-01-01"), 100),
        ];

        // Query with year 2019 - neither matches exactly, 2020 is closer
        let result = planner.select_best_movie_match(&movies, "Aladdin", Some(2019));

        assert!(result.is_some());
        // 2020 is 1 year away from 2019, 1990 is 29 years away
        // So 2020 should win with +5000 bonus
        assert_eq!(result.unwrap().id, 2);
    }

    #[test]
    fn test_select_best_movie_match_empty_results() {
        let planner = Planner::new().unwrap();
        let movies: Vec<crate::services::tmdb::MovieSearchItem> = vec![];

        let result = planner.select_best_movie_match(&movies, "Aladdin", Some(2019));
        assert!(result.is_none());
    }

    #[test]
    fn test_select_best_movie_match_ref_exact_year_match() {
        // Test select_best_movie_match_ref with year matching
        let planner = Planner::new().unwrap();

        let movies = vec![
            create_movie_search_item(812, "Aladdin", "Aladdin", Some("1992-11-25"), 5000),
            create_movie_search_item(420817, "Aladdin", "Aladdin", Some("2019-05-22"), 3000),
        ];

        // Convert to references as the function expects
        let movie_refs: Vec<&crate::services::tmdb::MovieSearchItem> = movies.iter().collect();

        // Query with year 2019
        let result = planner.select_best_movie_match_ref(&movie_refs, "Aladdin", Some(2019));

        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 420817);
    }

    #[test]
    fn test_select_best_movie_match_ref_year_diff_1() {
        let planner = Planner::new().unwrap();

        let movies = vec![
            create_movie_search_item(1, "Movie", "Movie", Some("2020-01-01"), 100),
            create_movie_search_item(2, "Movie", "Movie", Some("2021-01-01"), 100),
        ];
        let movie_refs: Vec<&crate::services::tmdb::MovieSearchItem> = movies.iter().collect();

        // Query with year 2021
        let result = planner.select_best_movie_match_ref(&movie_refs, "Movie", Some(2021));

        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 2);
    }

    #[test]
    fn test_select_best_movie_match_ref_empty_results() {
        let planner = Planner::new().unwrap();
        let movies: Vec<&crate::services::tmdb::MovieSearchItem> = vec![];

        let result = planner.select_best_movie_match_ref(&movies, "Aladdin", Some(2019));
        assert!(result.is_none());
    }

    #[test]
    fn test_aladdin_2019_vs_1992_real_scenario() {
        // Real scenario from TMDB: Searching "阿拉丁" (Aladdin) with year 2019
        // Should return 2019 version, not 1992
        let planner = Planner::new().unwrap();

        // TMDB search results for "阿拉丁" with year 2019
        let movies = vec![
            // 1992 animated version
            create_movie_search_item(812, "阿拉丁", "Aladdin", Some("1992-11-25"), 4500),
            // 2019 live-action version
            create_movie_search_item(420817, "阿拉丁", "Aladdin", Some("2019-05-22"), 3500),
            // Another 2019 movie with similar name
            create_movie_search_item(602411, "阿拉丁与神灯", "Adventures of Aladdin", Some("2019-05-14"), 500),
        ];

        // Query with Chinese title and year 2019
        let result = planner.select_best_movie_match(&movies, "阿拉丁", Some(2019));

        assert!(result.is_some());
        // Should select 2019 version due to exact year match (+50000)
        assert_eq!(result.unwrap().id, 420817);
    }

    #[test]
    fn test_spider_man_no_way_home_year_match() {
        // Real scenario: Spider-Man: No Way Home 2021
        let planner = Planner::new().unwrap();

        // TMDB search results (simplified)
        let movies = vec![
            create_movie_search_item(634649, "Spider-Man: No Way Home", "Spider-Man: No Way Home", Some("2021-12-15"), 15000),
            create_movie_search_item(453395, "Doctor Strange in the Multiverse of Madness", "Doctor Strange in the Multiverse of Madness", Some("2022-05-06"), 12000),
        ];

        // Query with exact title and year
        let result = planner.select_best_movie_match(&movies, "Spider-Man: No Way Home", Some(2021));

        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 634649);
    }
}
