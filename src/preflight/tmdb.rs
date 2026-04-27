//! TMDB API preflight check.

use super::{CheckResult, CheckSeverity};
use crate::models::config::Config;
use crate::services::tmdb::TmdbClient;

/// Check if TMDB API is accessible.
pub async fn check(config: &Config) -> CheckResult {
    if let Some(api_key) = &config.tmdb.api_key {
        let tmdb_config = crate::services::tmdb::TmdbConfig {
            api_key: api_key.clone(),
            language: config.tmdb.language.clone(),
            use_bearer: api_key.starts_with("eyJ"),
        };
        
        let client = TmdbClient::new(tmdb_config);
        
        match client.verify_api_key().await {
            Ok(true) => CheckResult::ok("TMDB API", "connected", CheckSeverity::Required),
            Ok(false) => CheckResult::fail(
                "TMDB API",
                "invalid API key",
                "Check your TMDB API key in config file",
                CheckSeverity::Required
            ),
            Err(_) => CheckResult::fail(
                "TMDB API",
                "connection failed",
                "Check your network connection",
                CheckSeverity::Required
            ),
        }
    } else {
        CheckResult::fail(
            "TMDB API",
            "API key not configured",
            "Configure TMDB API key in ~/.config/media_organizer/config.toml",
            CheckSeverity::Required
        )
    }
}
