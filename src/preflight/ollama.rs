//! Ollama preflight check.

use super::{CheckResult, CheckSeverity};
use crate::services::ollama::OllamaClient;

/// Check if Ollama service is running.
pub async fn check() -> CheckResult {
    let client = OllamaClient::new();

    match client.health_check().await {
        Ok(true) => {
            // Try to get model list
            match client.list_models().await {
                Ok(models) => {
                    let model_names: Vec<_> = models.iter().map(|m| m.name.as_str()).collect();
                    if models.is_empty() {
            CheckResult::fail(
                "Ollama",
                "running but no models",
                "Pull a model for AI enhanced parsing: ollama pull qwen2.5:7b",
                CheckSeverity::Optional
            )
                    } else {
            CheckResult::ok(
                "Ollama",
                &format!("running (models: {})", model_names.join(", ")),
                CheckSeverity::Optional
            )
                    }
                }
                Err(_) => CheckResult::ok("Ollama", "running", CheckSeverity::Optional),
            }
        }
        Ok(false) | Err(_) => {
            CheckResult::fail(
                "Ollama",
                "not running (AI features disabled)",
                "Start Ollama for AI enhanced parsing: ollama serve",
                CheckSeverity::Optional
            )
        }
    }
}
