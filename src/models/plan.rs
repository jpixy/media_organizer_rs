//! Plan data model.

use super::media::{
    EpisodeMetadata, MediaType, MovieMetadata, TvSeriesMetadata, VideoFile, VideoMetadata,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Plan file structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Plan {
    /// Plan version.
    pub version: String,
    /// Creation timestamp.
    pub created_at: String,
    /// Media type (movies or tv_series).
    pub media_type: Option<MediaType>,
    /// Source directory.
    pub source_path: PathBuf,
    /// Target directory.
    pub target_path: PathBuf,
    /// Plan items.
    pub items: Vec<PlanItem>,
    /// Sample files.
    pub samples: Vec<SampleItem>,
    /// Unknown/failed files.
    pub unknown: Vec<UnknownItem>,
}

/// A single item in the plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    /// Unique item ID.
    pub id: String,
    /// Item status.
    pub status: PlanItemStatus,
    /// Source file information.
    pub source: VideoFile,
    /// AI parsed information.
    pub parsed: ParsedInfo,
    /// TMDB metadata (for movies).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movie_metadata: Option<MovieMetadata>,
    /// TMDB metadata (for TV shows).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tv_series_metadata: Option<TvSeriesMetadata>,
    /// Episode metadata (for TV shows).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_metadata: Option<EpisodeMetadata>,
    /// Video technical metadata.
    pub video_metadata: VideoMetadata,
    /// Target information.
    pub target: TargetInfo,
    /// Operations to perform.
    pub operations: Vec<Operation>,
}

/// Plan item status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanItemStatus {
    Pending,
    Skip,
    Error,
}

/// AI parsed information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedInfo {
    /// Detected title.
    pub title: Option<String>,
    /// Detected original title.
    pub original_title: Option<String>,
    /// Detected year.
    pub year: Option<u16>,
    /// Confidence score.
    pub confidence: f32,
    /// Raw AI response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_response: Option<String>,
}

/// Target path information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TargetInfo {
    /// Target folder name.
    pub folder: String,
    /// Target file name.
    pub filename: String,
    /// Full target path.
    pub full_path: PathBuf,
    /// NFO file name.
    pub nfo: String,
    /// Poster file name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster: Option<String>,
}

/// Operation to perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// Operation type.
    pub op: OperationType,
    /// Source path (for move operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<PathBuf>,
    /// Target path.
    pub to: PathBuf,
    /// URL (for download operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Content reference (for create operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_ref: Option<String>,
}

/// Operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Mkdir,
    Move,
    Create,
    Download,
}

/// Sample file item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleItem {
    /// Source path.
    pub source: PathBuf,
    /// Target path.
    pub target: PathBuf,
}

/// Unknown/failed file item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnknownItem {
    /// Source file.
    pub source: VideoFile,
    /// Reason for failure.
    pub reason: String,
}
