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
        
        // Debug: print what type of auth we're using (before moving tmdb_config)
        let use_bearer = tmdb_config.use_bearer;
        tracing::debug!("TMDB auth type: {}", if use_bearer { "Bearer Token" } else { "API Key" });
        
        let client = TmdbClient::new(tmdb_config);
        
        // Use a more reliable validation method: try to search for a well-known show
        match client.search_tv("House of Cards", None).await {
            Ok(results) => {
                tracing::debug!("TMDB search results count: {}", results.len());
                if !results.is_empty() {
                    CheckResult::ok("TMDB API", "connected", CheckSeverity::Required)
                } else {
                    CheckResult::fail(
                        "TMDB API",
                        "search returned empty results",
                        "Check if your API key has proper permissions",
                        CheckSeverity::Required
                    )
                }
            }
            Err(e) => {
                tracing::error!("TMDB API error: {}", e);
                CheckResult::fail(
                    "TMDB API",
                    &format!("connection failed: {}", e),
                    "Check your network connection and API key",
                    CheckSeverity::Required
                )
            }
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
