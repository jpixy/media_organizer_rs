//! Error types for the media organizer.

use thiserror::Error;

/// Result type alias using our Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for the media organizer.
#[derive(Error, Debug)]
pub enum Error {
    // Preflight errors
    #[error("ffprobe not found. Install FFmpeg: sudo apt install ffmpeg")]
    FfprobeNotFound,

    #[error("Ollama service not running. Start with: ollama serve")]
    OllamaNotRunning,

    #[error("TMDB API key not configured. Set TMDB_API_KEY environment variable")]
    TmdbApiKeyMissing,

    #[error("TMDB API key invalid")]
    TmdbApiKeyInvalid,

    // File system errors
    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Not a directory: {0}")]
    NotADirectory(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("File already exists: {0}")]
    FileAlreadyExists(String),

    // Parse errors
    #[error("Failed to parse filename: {0}")]
    ParseError(String),

    #[error("AI parsing failed: {0}")]
    AiParseError(String),

    // TMDB errors
    #[error("TMDB search failed: {0}")]
    TmdbSearchError(String),

    #[error("Movie not found on TMDB: {0}")]
    MovieNotFound(String),

    #[error("TV show not found on TMDB: {0}")]
    TvSeriesNotFound(String),

    // Plan/Execute errors
    #[error("Invalid plan file: {0}")]
    InvalidPlanFile(String),

    #[error("Plan validation failed: {0}")]
    PlanValidationError(String),

    #[error("Execute operation failed: {0}")]
    ExecuteError(String),

    // Rollback errors
    #[error("Invalid rollback file: {0}")]
    InvalidRollbackFile(String),

    #[error("Rollback conflict: {0}")]
    RollbackConflict(String),

    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // HTTP errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    // JSON errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // Generic errors
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Create a generic error from a string.
    pub fn other<S: Into<String>>(msg: S) -> Self {
        Error::Other(msg.into())
    }
}
