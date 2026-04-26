//! Ollama API client.
//!
//! Configuration can be set via environment variables for easy integration
//! with local-ai-starter:
//! - `OLLAMA_HOST`: Ollama service URL (default: http://localhost:11434)
//! - `OLLAMA_MODEL`: Model to use (default: qwen2.5:7b)
//! - `OLLAMA_TIMEOUT`: Request timeout in seconds (default: 120)

use crate::Result;
use serde::{Deserialize, Serialize};

const DEFAULT_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "qwen2.5:7b";
// CPU 推理 7B 模型可能需要 3-5 分钟，设置足够长的超时
const DEFAULT_TIMEOUT_SECS: u64 = 300;

/// Ollama client configuration.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl OllamaConfig {
    /// Create configuration from environment variables.
    /// Falls back to defaults if not set.
    ///
    /// Environment variables:
    /// - `OLLAMA_HOST`: Service URL (default: http://localhost:11434)
    /// - `OLLAMA_MODEL`: Model name (default: qwen2.5:7b)
    /// - `OLLAMA_TIMEOUT`: Timeout in seconds (default: 120)
    pub fn from_env() -> Self {
        let base_url =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        let timeout_secs = std::env::var("OLLAMA_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        Self {
            base_url,
            model,
            timeout_secs,
        }
    }

    /// Create configuration from loaded application config.
    /// Environment variables override values from config file.
    pub fn from_config(config: &crate::models::config::OllamaConfig) -> Self {
        // Environment variables take precedence over config file
        let base_url = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| format!("http://{}:{}", config.host, config.port));

        let model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| config.model.clone());

        let timeout_secs = std::env::var("OLLAMA_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(config.timeout);

        Self {
            base_url,
            model,
            timeout_secs,
        }
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

/// Ollama API client.
pub struct OllamaClient {
    config: OllamaConfig,
    client: reqwest::Client,
}

/// Options for generation.
#[derive(Debug, Serialize)]
struct GenerateOptions {
    /// Temperature for sampling (0 = deterministic, 1 = creative)
    temperature: f32,
    /// Random seed for reproducibility
    seed: u32,
}

/// Generate request payload.
#[derive(Debug, Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    /// Generation options (temperature, seed, etc.)
    options: GenerateOptions,
}

/// Generate response.
#[derive(Debug, Deserialize)]
pub struct GenerateResponse {
    pub response: String,
    pub model: String,
    pub done: bool,
}

/// Models list response.
#[derive(Debug, Deserialize)]
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
}

/// Model information.
#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
}

impl OllamaClient {
    /// Create a new Ollama client with default configuration.
    pub fn new() -> Self {
        Self::with_config(OllamaConfig::default())
    }

    /// Create a new Ollama client with custom configuration.
    pub fn with_config(config: OllamaConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    /// Check if Ollama service is available.
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.config.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// List available models.
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", self.config.base_url);
        let resp: ModelsResponse = self.client.get(&url).send().await?.json().await?;
        Ok(resp.models)
    }

    /// Generate text from a prompt.
    pub async fn generate(&self, prompt: &str) -> Result<GenerateResponse> {
        self.generate_with_format(prompt, None).await
    }

    /// Generate text with specified format (e.g., "json").
    pub async fn generate_with_format(
        &self,
        prompt: &str,
        format: Option<&str>,
    ) -> Result<GenerateResponse> {
        let url = format!("{}/api/generate", self.config.base_url);

        let request = GenerateRequest {
            model: self.config.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            format: format.map(|s| s.to_string()),
            // Set temperature=0 and fixed seed for deterministic output
            // This ensures same input always produces same output
            options: GenerateOptions {
                temperature: 0.0,
                seed: 42,
            },
        };

        let resp = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}
