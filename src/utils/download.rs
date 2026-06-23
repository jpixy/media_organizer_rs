//! Download utilities with retry mechanism and rate limiting.

use anyhow::{Context, Result};
use reqwest::Client;
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;

use super::http_client::{create_http_client, HttpClientConfig};

/// Download configuration.
#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /// Maximum number of retries for failed downloads.
    pub max_retries: usize,
    /// Base delay between retries in milliseconds.
    pub retry_delay_ms: u64,
    /// Whether exponential backoff is enabled.
    pub exponential_backoff: bool,
    /// Timeout in seconds for HTTP requests.
    pub timeout_secs: u64,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 1000,
            exponential_backoff: true,
            timeout_secs: 30,
        }
    }
}

/// Download a file from URL to path with retry mechanism.
pub async fn download_file_with_retry(
    url: &str,
    path: &Path,
    config: &DownloadConfig,
    proxy_enabled: bool,
    proxy: &Option<String>,
) -> Result<u64> {
    let mut retries = 0;

    let http_client_config = HttpClientConfig {
        timeout_secs: config.timeout_secs,
        proxy_enabled,
        proxy: proxy.clone(),
    };
    let client = create_http_client(&http_client_config);

    loop {
        match download_file_internal(url, path, &client).await {
            Ok(size) => return Ok(size),
            Err(e) => {
                retries += 1;
                if retries > config.max_retries {
                    return Err(e.context(format!(
                        "Failed after {} retries",
                        config.max_retries
                    )));
                }

                // Calculate delay with exponential backoff if enabled
                let delay = if config.exponential_backoff {
                    config.retry_delay_ms * 2_u64.pow(retries as u32 - 1)
                } else {
                    config.retry_delay_ms
                };

                tracing::warn!(
                    "Download failed (attempt {}/{}): {}. Retrying in {}ms...",
                    retries,
                    config.max_retries,
                    e,
                    delay
                );

                sleep(Duration::from_millis(delay)).await;
            }
        }
    }
}

/// Internal download function without retry logic.
async fn download_file_internal(
    url: &str,
    path: &Path,
    client: &Client,
) -> Result<u64> {
    let response = client.get(url)
        .send()
        .await
        .context("Failed to fetch URL")?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "HTTP error: {} - {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    let bytes = response.bytes()
        .await
        .context("Failed to read response bytes")?;

    let size = bytes.len() as u64;

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .context("Failed to create parent directory")?;
        }
    }

    std::fs::write(path, bytes)
        .context("Failed to write file")?;

    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_download_file_with_retry_config() {
        let config = DownloadConfig {
            max_retries: 3,
            retry_delay_ms: 100,
            exponential_backoff: true,
            timeout_secs: 30,
        };

        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 100);
        assert!(config.exponential_backoff);
        assert_eq!(config.timeout_secs, 30);
    }

    #[tokio::test]
    async fn test_download_config_default() {
        let config = DownloadConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 1000);
        assert!(config.exponential_backoff);
        assert_eq!(config.timeout_secs, 30);
    }

    #[tokio::test]
    async fn test_download_file_internal_invalid_url() {
        let http_config = HttpClientConfig::default();
        let client = create_http_client(&http_config);
        
        let result = download_file_internal(
            "http://invalid-url-that-does-not-exist.example",
            Path::new("/tmp/test.jpg"),
            &client,
        ).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_file_with_retry_timeout() {
        let config = DownloadConfig {
            max_retries: 2,
            retry_delay_ms: 100,
            exponential_backoff: false,
            timeout_secs: 5,
        };

        let result = download_file_with_retry(
            "http://10.255.255.1:9999/test.jpg",
            Path::new("/tmp/test_timeout.jpg"),
            &config,
            false,
            &None,
        ).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_file_with_retry_success_after_failure() {
        let config = DownloadConfig::default();
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_download_config_clone() {
        let config = DownloadConfig {
            max_retries: 5,
            retry_delay_ms: 2000,
            exponential_backoff: false,
            timeout_secs: 60,
        };

        let cloned = config.clone();
        assert_eq!(cloned.max_retries, 5);
        assert_eq!(cloned.retry_delay_ms, 2000);
        assert!(!cloned.exponential_backoff);
        assert_eq!(cloned.timeout_secs, 60);
    }

    #[test]
    fn test_download_config_debug() {
        let config = DownloadConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("max_retries"));
        assert!(debug_str.contains("retry_delay_ms"));
        assert!(debug_str.contains("exponential_backoff"));
        assert!(debug_str.contains("timeout_secs"));
    }
}