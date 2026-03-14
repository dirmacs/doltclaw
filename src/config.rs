//! Configuration loading and types
//!
//! Loads `doltclaw.toml` with `${ENV_VAR}` substitution.

use crate::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

/// Top-level doltclaw configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// LLM providers (keyed by name, e.g. "nvidia-nim")
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,

    /// Agent configuration
    #[serde(default)]
    pub agent: AgentConfig,
}

/// Configuration for an LLM provider
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// Base URL for the API (e.g. "https://integrate.api.nvidia.com/v1")
    pub base_url: String,

    /// API key (supports ${ENV_VAR} substitution)
    pub api_key: Option<String>,

    /// API compatibility mode (default: "openai-completions")
    #[serde(default = "default_api")]
    pub api: String,

    /// Models available from this provider
    #[serde(default)]
    pub models: Vec<ModelConfig>,
}

/// Configuration for a model
#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    /// Model ID (e.g. "qwen/qwen3.5-122b-a10b")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Whether this model supports reasoning/thinking mode
    #[serde(default)]
    pub reasoning: bool,

    /// Context window size in tokens
    #[serde(default = "default_context_window")]
    pub context_window: usize,

    /// Maximum output tokens
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
}

/// Agent behavior configuration
#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    /// Primary model reference (e.g. "nvidia-nim/qwen/qwen3.5-122b-a10b")
    pub primary: String,

    /// Fallback models in priority order
    #[serde(default)]
    pub fallbacks: Vec<String>,

    /// Maximum tool-calling iterations per execute()
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,

    /// HTTP request timeout in milliseconds (default: 30000)
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Inference parameters
    #[serde(default)]
    pub params: InferenceParams,
}

/// LLM inference parameters
#[derive(Debug, Clone, Deserialize)]
pub struct InferenceParams {
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    #[serde(default = "default_top_p")]
    pub top_p: f32,

    /// System prompt injected before conversation
    pub system_prompt: Option<String>,
}

/// Parsed model reference: "provider/model-id"
#[derive(Debug, Clone)]
pub struct ModelRef {
    pub provider: String,
    pub model_id: String,
}

// --- Defaults ---

fn default_api() -> String {
    "openai-completions".to_string()
}
fn default_context_window() -> usize {
    128000
}
fn default_max_tokens() -> usize {
    16384
}
fn default_max_iterations() -> usize {
    50
}
fn default_timeout_ms() -> u64 {
    30000
}
fn default_temperature() -> f32 {
    1.0
}
fn default_top_p() -> f32 {
    0.95
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            primary: String::new(),
            fallbacks: Vec::new(),
            max_iterations: default_max_iterations(),
            timeout_ms: default_timeout_ms(),
            params: InferenceParams::default(),
        }
    }
}

impl Default for InferenceParams {
    fn default() -> Self {
        Self {
            temperature: default_temperature(),
            top_p: default_top_p(),
            system_prompt: None,
        }
    }
}

impl ModelRef {
    /// Parse "provider/model-id" — splits on first `/` only.
    /// e.g. "nvidia-nim/qwen/qwen3.5-122b-a10b" → provider="nvidia-nim", model_id="qwen/qwen3.5-122b-a10b"
    pub fn parse(s: &str) -> Result<Self> {
        let slash = s.find('/').ok_or_else(|| {
            Error::Config(format!(
                "Invalid model ref '{}': expected 'provider/model-id'",
                s
            ))
        })?;
        Ok(Self {
            provider: s[..slash].to_string(),
            model_id: s[slash + 1..].to_string(),
        })
    }
}

impl Config {
    /// Load configuration from a TOML file with `${ENV_VAR}` substitution.
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read {}: {}", path.display(), e)))?;

        let expanded = substitute_env_vars(&raw);

        toml::from_str(&expanded)
            .map_err(|e| Error::Config(format!("Failed to parse {}: {}", path.display(), e)))
    }



    /// Resolve a model reference to its provider and model config.
    pub fn resolve_model(&self, ref_str: &str) -> Result<(&ProviderConfig, &ModelConfig)> {
        let model_ref = ModelRef::parse(ref_str)?;
        let provider = self.providers.get(&model_ref.provider).ok_or_else(|| {
            Error::Config(format!("Provider '{}' not found", model_ref.provider))
        })?;
        let model = provider
            .models
            .iter()
            .find(|m| m.id == model_ref.model_id)
            .ok_or_else(|| {
                Error::Config(format!(
                    "Model '{}' not found in provider '{}'",
                    model_ref.model_id, model_ref.provider
                ))
            })?;
        Ok((provider, model))
    }

    /// Get all model references in priority order (primary first, then fallbacks).
    pub fn model_chain(&self) -> Vec<&str> {
        let mut chain = vec![self.agent.primary.as_str()];
        for fb in &self.agent.fallbacks {
            chain.push(fb.as_str());
        }
        chain
    }
}

impl FromStr for Config {
    type Err = Error;

    /// Parse a Config from a TOML string with environment variable substitution.
    fn from_str(s: &str) -> Result<Self> {
        let expanded = substitute_env_vars(s);
        toml::from_str(&expanded).map_err(|e| Error::Config(format!("Failed to parse TOML: {}", e)))
    }
}

/// Replace `${ENV_VAR}` patterns with environment variable values.
fn substitute_env_vars(input: &str) -> String {
    let mut result = input.to_string();
    // Find all ${...} patterns
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let end = start + end;
            let var_name = &result[start + 2..end];
            let value = std::env::var(var_name).unwrap_or_default();
            result = format!("{}{}{}", &result[..start], value, &result[end + 1..]);
        } else {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_ref_parse() {
        let r = ModelRef::parse("nvidia-nim/qwen/qwen3.5-122b-a10b").unwrap();
        assert_eq!(r.provider, "nvidia-nim");
        assert_eq!(r.model_id, "qwen/qwen3.5-122b-a10b");
    }

    #[test]
    fn test_model_ref_parse_error() {
        assert!(ModelRef::parse("no-slash").is_err());
    }

    #[test]
    fn test_env_substitution() {
        std::env::set_var("DOLTCLAW_TEST_KEY", "secret123");
        let result = substitute_env_vars("key = \"${DOLTCLAW_TEST_KEY}\"");
        assert_eq!(result, "key = \"secret123\"");
        std::env::remove_var("DOLTCLAW_TEST_KEY");
    }

    #[test]
    fn test_env_substitution_missing_var() {
        let result = substitute_env_vars("key = \"${DOLTCLAW_NONEXISTENT}\"");
        assert_eq!(result, "key = \"\"");
    }

    #[test]
    fn test_config_from_str() {
        let toml = r#"
[providers.nvidia-nim]
base_url = "https://integrate.api.nvidia.com/v1"
api = "openai-completions"

[[providers.nvidia-nim.models]]
id = "qwen/qwen3.5-122b-a10b"
name = "Qwen 3.5 122B"
reasoning = false
context_window = 131072
max_tokens = 16384

[agent]
primary = "nvidia-nim/qwen/qwen3.5-122b-a10b"
fallbacks = []
"#;
        let config: Config = toml.parse().unwrap();
        assert_eq!(config.providers.len(), 1);
        let provider = config.providers.get("nvidia-nim").unwrap();
        assert_eq!(provider.models.len(), 1);
        assert_eq!(provider.models[0].id, "qwen/qwen3.5-122b-a10b");
        assert_eq!(provider.models[0].context_window, 131072);
    }

    #[test]
    fn test_resolve_model() {
        let toml = r#"
[providers.nvidia-nim]
base_url = "https://example.com/v1"

[[providers.nvidia-nim.models]]
id = "qwen/qwen3.5-122b-a10b"
name = "Qwen 3.5 122B"

[[providers.nvidia-nim.models]]
id = "z-ai/glm4.7"
name = "GLM 4.7"
reasoning = true

[agent]
primary = "nvidia-nim/qwen/qwen3.5-122b-a10b"
fallbacks = ["nvidia-nim/z-ai/glm4.7"]
"#;
        let config: Config = toml.parse().unwrap();
        let (provider, model) = config
            .resolve_model("nvidia-nim/qwen/qwen3.5-122b-a10b")
            .unwrap();
        assert_eq!(provider.base_url, "https://example.com/v1");
        assert_eq!(model.name, "Qwen 3.5 122B");

        let (_, glm) = config.resolve_model("nvidia-nim/z-ai/glm4.7").unwrap();
        assert!(glm.reasoning);
    }

    #[test]
    fn test_model_chain() {
        let toml = r#"
[agent]
primary = "nvidia-nim/qwen/qwen3.5-122b-a10b"
fallbacks = ["nvidia-nim/stepfun-ai/step-3.5-flash", "nvidia-nim/z-ai/glm4.7"]
"#;
        let config: Config = toml.parse().unwrap();
        let chain = config.model_chain();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], "nvidia-nim/qwen/qwen3.5-122b-a10b");
        assert_eq!(chain[2], "nvidia-nim/z-ai/glm4.7");
    }
}
