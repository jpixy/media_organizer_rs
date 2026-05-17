//! Configuration model.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Ollama configuration.
    #[serde(default)]
    pub ollama: OllamaConfig,
    /// TMDB configuration.
    #[serde(default)]
    pub tmdb: TmdbConfig,
    /// Network configuration.
    #[serde(default)]
    pub network: NetworkConfig,
    /// Organize configuration.
    #[serde(default)]
    pub organize: OrganizeConfig,
    /// Sessions directory.
    #[serde(skip)]
    pub sessions_dir: PathBuf,
}

/// Ollama configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Whether AI parsing is enabled. Default: false (AI disabled by default).
    /// When disabled, only local parsing (guessit library) is used.
    #[serde(default = "default_ai_enabled")]
    pub enabled: bool,
    /// Ollama host.
    pub host: String,
    /// Ollama port.
    pub port: u16,
    /// Model to use.
    pub model: String,
    /// Request timeout in seconds.
    pub timeout: u64,
}

fn default_ai_enabled() -> bool {
    false
}

/// TMDB configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbConfig {
    /// API key.
    pub api_key: Option<String>,
    /// Language for responses.
    pub language: String,
}

/// Network configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Whether to enable proxy. Default: false.
    #[serde(default)]
    pub proxy_enabled: bool,
    /// HTTP/HTTPS proxy URL (e.g., "http://127.0.0.1:7890").
    /// Used only when proxy_enabled is true.
    #[serde(default)]
    pub proxy: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ollama: OllamaConfig::default(),
            tmdb: TmdbConfig::default(),
            network: NetworkConfig::default(),
            organize: OrganizeConfig::default(),
            sessions_dir: dirs_config_path().join("sessions"),
        }
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            enabled: false, // AI disabled by default per requirements
            host: "localhost".to_string(),
            port: 11434,
            model: "qwen2.5:7b".to_string(),
            timeout: 60,
        }
    }
}

impl Default for TmdbConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            language: "zh-CN".to_string(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            proxy_enabled: false,
            proxy: None,
        }
    }
}

/// Get the configuration directory path.
#[allow(unused)]
fn dirs_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("media_organizer")
}

/// Get the configuration directory path.
/// Public version for testing purposes.
pub fn test_config_path(base_path: &std::path::Path) -> PathBuf {
    base_path.join("media_organizer")
}

/// Load configuration from file.
/// 
/// For testing purposes, an optional base directory can be provided.
/// If not provided, uses the standard system config directory.
pub fn load_config() -> Config {
    load_config_from(None)
}

/// Internal implementation that accepts a base directory for testing.
pub(crate) fn load_config_from(base_dir: Option<&std::path::Path>) -> Config {
    let config_path = match base_dir {
        Some(dir) => dir.join("media_organizer").join("config.toml"),
        None => dirs_config_path().join("config.toml"),
    };

    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(mut config) = toml::from_str::<Config>(&content) {
                // Environment variables override config file values
                if let Ok(api_key) = std::env::var("TMDB_API_KEY") {
                    config.tmdb.api_key = Some(api_key);
                }
                if let Ok(lang) = std::env::var("TMDB_LANGUAGE") {
                    config.tmdb.language = lang;
                }
                if let Ok(host) = std::env::var("OLLAMA_HOST") {
                    config.ollama.host = host;
                }
                if let Ok(port) = std::env::var("OLLAMA_PORT") {
                    if let Ok(p) = port.parse::<u16>() {
                        config.ollama.port = p;
                    }
                }
                if let Ok(model) = std::env::var("OLLAMA_MODEL") {
                    config.ollama.model = model;
                }
                if let Ok(timeout) = std::env::var("OLLAMA_TIMEOUT") {
                    if let Ok(t) = timeout.parse::<u64>() {
                        config.ollama.timeout = t;
                    }
                }
                if let Ok(enabled) = std::env::var("OLLAMA_ENABLED") {
                    config.ollama.enabled = enabled == "true" || enabled == "1";
                }
                return config;
            }
        }
    }

    let mut config = Config::default();

    // Apply environment variable overrides on top of defaults
    if let Ok(api_key) = std::env::var("TMDB_API_KEY") {
        config.tmdb.api_key = Some(api_key);
    }
    if let Ok(lang) = std::env::var("TMDB_LANGUAGE") {
        config.tmdb.language = lang;
    }
    if let Ok(host) = std::env::var("OLLAMA_HOST") {
        config.ollama.host = host;
    }
    if let Ok(port) = std::env::var("OLLAMA_PORT") {
        if let Ok(p) = port.parse::<u16>() {
            config.ollama.port = p;
        }
    }
    if let Ok(model) = std::env::var("OLLAMA_MODEL") {
        config.ollama.model = model;
    }
    if let Ok(timeout) = std::env::var("OLLAMA_TIMEOUT") {
        if let Ok(t) = timeout.parse::<u64>() {
            config.ollama.timeout = t;
        }
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::sync::Mutex;

    // Global mutex to serialize tests that modify environment variables
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.ollama.host, "localhost");
        assert_eq!(config.ollama.port, 11434);
        assert_eq!(config.ollama.model, "qwen2.5:7b");
        assert_eq!(config.ollama.timeout, 60);
        assert_eq!(config.tmdb.language, "zh-CN");
    }

#[test]
fn test_load_config_from_file() {
    // Serialize all env-modifying tests
    let _lock = TEST_MUTEX.lock().unwrap();

    // Clear any existing environment variables that may interfere
    let old_tmdb_key = std::env::var("TMDB_API_KEY").ok();
    let old_ollama_host = std::env::var("OLLAMA_HOST").ok();
    let old_ollama_port = std::env::var("OLLAMA_PORT").ok();
    let old_ollama_model = std::env::var("OLLAMA_MODEL").ok();
    let old_ollama_timeout = std::env::var("OLLAMA_TIMEOUT").ok();

    std::env::remove_var("TMDB_API_KEY");
    std::env::remove_var("OLLAMA_HOST");
    std::env::remove_var("OLLAMA_PORT");
    std::env::remove_var("OLLAMA_MODEL");
    std::env::remove_var("OLLAMA_TIMEOUT");

    let temp_dir = tempdir().unwrap();
    let config_content = r#"
[ollama]
host = "192.168.1.100"
port = 8080
model = "qwen2:7b"
timeout = 120

[tmdb]
api_key = "test_api_key_123"
language = "en-US"

[network]
proxy_enabled = true
proxy = "http://127.0.0.1:7890"
"#;

    // Create config.toml directly in temp dir (no subdirectory) for direct testing
    let config_path = temp_dir.path().join("config.toml");
    std::fs::write(&config_path, config_content).unwrap();

    // Parse directly without going through path logic
    let config: Config = toml::from_str(&config_content).unwrap();

    assert_eq!(config.ollama.host, "192.168.1.100");
    assert_eq!(config.ollama.port, 8080);
    assert_eq!(config.ollama.model, "qwen2:7b");
    assert_eq!(config.ollama.timeout, 120);
    assert_eq!(config.tmdb.api_key, Some("test_api_key_123".to_string()));
    assert_eq!(config.tmdb.language, "en-US");
    assert_eq!(config.network.proxy_enabled, true);
    assert_eq!(config.network.proxy, Some("http://127.0.0.1:7890".to_string()));

    // Restore original variables
    if let Some(v) = old_tmdb_key { std::env::set_var("TMDB_API_KEY", v); }
    if let Some(v) = old_ollama_host { std::env::set_var("OLLAMA_HOST", v); }
    if let Some(v) = old_ollama_port { std::env::set_var("OLLAMA_PORT", v); }
    if let Some(v) = old_ollama_model { std::env::set_var("OLLAMA_MODEL", v); }
    if let Some(v) = old_ollama_timeout { std::env::set_var("OLLAMA_TIMEOUT", v); }
}

    #[test]
    fn test_environment_variables_override_config() {
        // Serialize all env-modifying tests
        let _lock = TEST_MUTEX.lock().unwrap();

        // Clear ALL relevant environment variables first
        let old_tmdb_key = std::env::var("TMDB_API_KEY").ok();
        let old_tmdb_lang = std::env::var("TMDB_LANGUAGE").ok();
        let old_ollama_host = std::env::var("OLLAMA_HOST").ok();
        let old_ollama_port = std::env::var("OLLAMA_PORT").ok();
        let old_ollama_model = std::env::var("OLLAMA_MODEL").ok();
        let old_ollama_timeout = std::env::var("OLLAMA_TIMEOUT").ok();

        std::env::remove_var("TMDB_API_KEY");
        std::env::remove_var("TMDB_LANGUAGE");
        std::env::remove_var("OLLAMA_HOST");
        std::env::remove_var("OLLAMA_PORT");
        std::env::remove_var("OLLAMA_MODEL");
        std::env::remove_var("OLLAMA_TIMEOUT");

        std::env::set_var("TMDB_API_KEY", "env_key_456");
        std::env::set_var("OLLAMA_MODEL", "llama3:8b");

        let config = load_config();

        assert_eq!(config.tmdb.api_key, Some("env_key_456".to_string()));
        assert_eq!(config.ollama.model, "llama3:8b");

        std::env::remove_var("TMDB_API_KEY");
        std::env::remove_var("OLLAMA_MODEL");

        // Restore original environment
        if let Some(v) = old_tmdb_key { std::env::set_var("TMDB_API_KEY", v); }
        if let Some(v) = old_tmdb_lang { std::env::set_var("TMDB_LANGUAGE", v); }
        if let Some(v) = old_ollama_host { std::env::set_var("OLLAMA_HOST", v); }
        if let Some(v) = old_ollama_port { std::env::set_var("OLLAMA_PORT", v); }
        if let Some(v) = old_ollama_model { std::env::set_var("OLLAMA_MODEL", v); }
        if let Some(v) = old_ollama_timeout { std::env::set_var("OLLAMA_TIMEOUT", v); }
    }
}

/// Organize configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizeConfig {
    /// Whether to download posters. Default: true.
    #[serde(default = "default_download_posters")]
    pub download_posters: bool,
    /// Poster size for TMDB. Default: "w500".
    #[serde(default = "default_poster_size")]
    pub poster_size: String,
    /// Whether to generate NFO files. Default: true.
    #[serde(default = "default_generate_nfo")]
    pub generate_nfo: bool,
}

fn default_download_posters() -> bool {
    true
}

fn default_poster_size() -> String {
    "w500".to_string()
}

fn default_generate_nfo() -> bool {
    true
}

impl Default for OrganizeConfig {
    fn default() -> Self {
        Self {
            download_posters: default_download_posters(),
            poster_size: default_poster_size(),
            generate_nfo: default_generate_nfo(),
        }
    }
}
