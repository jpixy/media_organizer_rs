//! Media-related data models.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Media type enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Movies,
    /// TV series (formerly "tv_series"). Accepts both "tv_series" and "tv_series" for backward compatibility.
    #[serde(alias = "tv_series")]
    TvSeries,
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaType::Movies => write!(f, "movies"),
            MediaType::TvSeries => write!(f, "tv_series"),
        }
    }
}

/// Actor with role information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Actor {
    /// Actor name.
    pub name: String,
    /// Character/role name.
    pub role: Option<String>,
    /// Display order.
    pub order: Option<u32>,
}

/// Crew member (director, writer, etc.).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrewMember {
    /// Crew member name.
    pub name: String,
    /// Job title (e.g., "Director", "Writer").
    pub job: String,
    /// Department (e.g., "Directing", "Writing").
    pub department: String,
}

/// Video file information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFile {
    /// Full path to the file.
    pub path: PathBuf,
    /// File name without path.
    pub filename: String,
    /// File size in bytes.
    pub size: u64,
    /// Last modified time.
    pub modified: chrono::DateTime<chrono::Utc>,
    /// Whether this is a sample file.
    pub is_sample: bool,
    /// Parent directory.
    pub parent_dir: PathBuf,
}

/// Video metadata extracted from ffprobe.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoMetadata {
    /// Video width in pixels.
    pub width: u32,
    /// Video height in pixels.
    pub height: u32,
    /// Resolution category (e.g., "2160p", "1080p").
    pub resolution: String,
    /// Video format (e.g., "BluRay", "WEB-DL").
    pub format: String,
    /// Video codec (e.g., "hevc", "h264").
    pub video_codec: String,
    /// Bit depth (e.g., 8, 10).
    pub bit_depth: u8,
    /// Audio codec (e.g., "dts", "ac3", "aac").
    pub audio_codec: String,
    /// Audio channels (e.g., "5.1", "7.1").
    pub audio_channels: String,
}

/// TMDB metadata for a movie.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MovieMetadata {
    /// TMDB ID.
    pub tmdb_id: u64,
    /// IMDB ID.
    pub imdb_id: Option<String>,
    /// Original title (usually English).
    pub original_title: String,
    /// Localized title.
    pub title: String,
    /// Original language.
    pub original_language: String,
    /// Release year.
    pub year: u16,
    /// Full release date (YYYY-MM-DD).
    pub release_date: Option<String>,
    /// Overview/synopsis.
    pub overview: Option<String>,
    /// Tagline.
    pub tagline: Option<String>,
    /// Runtime in minutes.
    pub runtime: Option<u32>,
    /// Genres.
    pub genres: Vec<String>,
    /// Production countries (names only).
    pub countries: Vec<String>,
    /// Production country codes (ISO 3166-1, e.g., "CN", "US").
    pub country_codes: Vec<String>,
    /// Production companies/studios.
    pub studios: Vec<String>,
    /// User rating (0-10).
    pub rating: Option<f32>,
    /// Vote count.
    pub votes: Option<u32>,
    /// Poster URLs.
    pub poster_urls: Vec<String>,
    /// Backdrop URL.
    pub backdrop_url: Option<String>,
    /// Directors.
    pub directors: Vec<String>,
    /// Writers.
    pub writers: Vec<String>,
    /// Main actors with roles.
    pub actors: Vec<String>,
    /// Actor roles (parallel to actors).
    pub actor_roles: Vec<String>,
    /// Actor information with name, role, and order.
    #[serde(default)]
    pub actors_info: Vec<Actor>,
    /// Certification/rating (e.g., "PG-13").
    pub certification: Option<String>,
    /// Collection/Set ID (for movie series like "Pirates of the Caribbean").
    pub collection_id: Option<u64>,
    /// Collection/Set name.
    pub collection_name: Option<String>,
    /// Collection/Set overview.
    pub collection_overview: Option<String>,
    /// Total number of movies in the collection.
    pub collection_total_movies: Option<usize>,
}

/// TMDB metadata for a TV show.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TvSeriesMetadata {
    /// TMDB ID.
    pub tmdb_id: u64,
    /// IMDB ID.
    pub imdb_id: Option<String>,
    /// Original name.
    pub original_name: String,
    /// Localized name.
    pub name: String,
    /// Original language.
    pub original_language: String,
    /// First air year.
    pub year: u16,
    /// First air date.
    pub first_air_date: Option<String>,
    /// Overview/synopsis.
    pub overview: Option<String>,
    /// Tagline.
    pub tagline: Option<String>,
    /// Genres.
    pub genres: Vec<String>,
    /// Production countries (names only).
    pub countries: Vec<String>,
    /// Production country codes (ISO 3166-1, e.g., "CN", "US").
    pub country_codes: Vec<String>,
    /// Studios/Networks.
    pub networks: Vec<String>,
    /// Rating (0-10).
    pub rating: Option<f32>,
    /// Vote count.
    pub votes: Option<u32>,
    /// Number of seasons.
    pub number_of_seasons: u16,
    /// Number of episodes.
    pub number_of_episodes: u16,
    /// Status (Returning Series, Ended, etc.)
    pub status: Option<String>,
    /// Created by.
    pub creators: Vec<String>,
    /// Main cast with roles.
    pub actors: Vec<Actor>,
    /// Poster URLs.
    pub poster_urls: Vec<String>,
    /// Backdrop/fanart URL.
    pub backdrop_url: Option<String>,
}

/// TMDB metadata for a TV episode.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EpisodeMetadata {
    /// Season number.
    pub season_number: u16,
    /// Episode number.
    pub episode_number: u16,
    /// Episode name.
    pub name: String,
    /// Original episode name.
    pub original_name: Option<String>,
    /// Air date.
    pub air_date: Option<String>,
    /// Overview.
    pub overview: Option<String>,
    /// Cast members.
    pub cast: Vec<Actor>,
    /// Crew members (directors, writers, etc.).
    pub crew: Vec<CrewMember>,
}

impl EpisodeMetadata {
    /// Build EpisodeMetadata from TMDB EpisodeDetails response.
    ///
    /// This is the single source of truth for constructing EpisodeMetadata,
    /// eliminating field-by-field duplication and cast/crew mapping across all call sites.
    pub fn from_tmdb_details(
        ep_details: &crate::services::tmdb::EpisodeDetails,
        season_number: u16,
        episode_number: u16,
    ) -> Self {
        Self {
            season_number,
            episode_number,
            name: ep_details.name.clone(),
            original_name: None, // Not available in EpisodeDetails
            air_date: ep_details.air_date.clone(),
            overview: ep_details.overview.clone(),
            cast: ep_details
                .credits
                .as_ref()
                .map(|c| c.cast.iter().take(10).map(|a| Actor {
                    name: a.name.clone(),
                    role: a.character.clone(),
                    order: a.order,
                }).collect())
                .unwrap_or_default(),
            crew: ep_details
                .credits
                .as_ref()
                .map(|c| c.crew.iter().map(|cr| CrewMember {
                    name: cr.name.clone(),
                    job: cr.job.clone(),
                    department: cr.department.clone(),
                }).collect())
                .unwrap_or_default(),
        }
    }
}

/// TMDB metadata for a TV season.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeasonMetadata {
    /// Season number.
    pub season_number: u16,
    /// Season name.
    pub name: String,
    /// Season overview.
    pub overview: Option<String>,
    /// Air date.
    pub air_date: Option<String>,
    /// Poster URL.
    pub poster_url: Option<String>,
    /// Number of episodes.
    pub episode_count: u16,
    /// TMDB Season ID.
    pub tmdb_id: u64,
    /// IMDB ID (important for anthology series where each season has its own IMDB ID)
    /// Falls back to TV Show's IMDB ID if not available.
    pub imdb_id: Option<String>,
}

impl SeasonMetadata {
    /// Build SeasonMetadata from TMDB SeasonDetails response.
    ///
    /// This is the single source of truth for constructing SeasonMetadata,
    /// eliminating field-by-field duplication across all call sites.
    pub fn from_tmdb_details(
        season_details: &crate::services::tmdb::SeasonDetails,
        imdb_id: Option<String>,
        poster_size: &str,
    ) -> Self {
        Self {
            season_number: season_details.season_number.unwrap_or_default(),
            name: season_details.name.clone().unwrap_or_default(),
            overview: season_details.overview.clone(),
            air_date: season_details.air_date.clone(),
            poster_url: season_details
                .poster_path
                .as_ref()
                .map(|p| format!("https://image.tmdb.org/t/p/{}{}", poster_size, p)),
            episode_count: season_details.episodes.as_ref().map(|e| e.len() as u16).unwrap_or_default(),
            tmdb_id: season_details.id.unwrap_or_default(),
            imdb_id,
        }
    }
}
