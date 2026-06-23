use reqwest::{Client, Proxy};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub timeout_secs: u64,
    pub proxy_enabled: bool,
    pub proxy: Option<String>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            proxy_enabled: false,
            proxy: None,
        }
    }
}

pub fn create_http_client(config: &HttpClientConfig) -> Client {
    let mut client_builder = Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs));

    if config.proxy_enabled {
        if let Some(ref proxy_url) = config.proxy {
            match Proxy::all(proxy_url) {
                Ok(proxy) => {
                    client_builder = client_builder.proxy(proxy);
                    tracing::info!("HTTP client using proxy: {}", proxy_url);
                }
                Err(e) => {
                    tracing::error!("Failed to configure proxy {}: {}", proxy_url, e);
                }
            }
        }
    }

    client_builder.build().unwrap_or_else(|_| Client::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_http_client_default() {
        let config = HttpClientConfig::default();
        let client = create_http_client(&config);
        let _ = client.get("http://example.com");
    }

    #[test]
    fn test_create_http_client_with_proxy() {
        let config = HttpClientConfig {
            timeout_secs: 60,
            proxy_enabled: true,
            proxy: Some("http://localhost:8080".to_string()),
        };
        let client = create_http_client(&config);
        let _ = client.get("http://example.com");
    }

    #[test]
    fn test_create_http_client_custom_timeout() {
        let config = HttpClientConfig {
            timeout_secs: 120,
            ..Default::default()
        };
        let client = create_http_client(&config);
        let _ = client.get("http://example.com");
    }
}