//! TMDB API preflight check.

use super::{CheckResult, CheckSeverity};
use crate::services::tmdb::TmdbClient;

/// Check if TMDB API is accessible.
pub async fn check() -> CheckResult {
    match TmdbClient::from_env() {
        Ok(client) => match client.verify_api_key().await {
            Ok(true) => CheckResult::ok("TMDB API", "connected", CheckSeverity::Required),
            Ok(false) => CheckResult::fail(
                "TMDB API",
                "invalid API key",
                "Check your TMDB_API_KEY environment variable or config file",
                CheckSeverity::Required
            ),
            Err(_) => CheckResult::fail(
                "TMDB API",
                "connection failed",
                "Check your network connection",
                CheckSeverity::Required
            ),
        },
        Err(_) => CheckResult::fail(
            "TMDB API",
            "API key not configured",
            "Set TMDB_API_KEY environment variable or configure in ~/.config/media_organizer/config.toml",
            CheckSeverity::Required
        ),
    }
}
