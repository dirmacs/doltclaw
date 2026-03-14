//! Agent: tool-calling loop with model fallback chain

use crate::backend::openai_compat::OpenAiCompatBackend;
use crate::backend::Backend;
use crate::config::Config;
use crate::tools::ToolRegistry;
use crate::types::{
    Message, Role, ToolCallRecord, ToolResult, TokenCallback, TokenUsage,
};
use crate::{Error, Result};
use serde_json::json;
use std::sync::Arc;

/// Result from a complete agent execution
#[derive(Debug)]
pub struct AgentResponse {
    /// Final text response
    pub content: String,
    /// All tool calls made during execution
    pub tool_calls: Vec<ToolCallRecord>,
    /// Number of iterations taken
    pub iterations: usize,
    /// Cumulative token usage
    pub usage: TokenUsage,
    /// Which model actually succeeded (after fallbacks)
    pub model_used: String,
}

/// The doltclaw agent — runs tool-calling loops with model fallback
pub struct Agent {
    config: Config,
    tools: ToolRegistry,
    history: Vec<Message>,
    /// Backends in priority order: primary first, then fallbacks
    backends: Vec<(String, Box<dyn Backend>)>,
}

impl Agent {
    /// Build from config. Creates OpenAiCompatBackend for primary + all fallbacks.
    pub fn from_config(config: Config) -> Result<Self> {
        let mut backends: Vec<(String, Box<dyn Backend>)> = Vec::new();

        for model_ref_str in config.model_chain() {
            let (provider, model) = config.resolve_model(model_ref_str)?;
            let backend = OpenAiCompatBackend::new(provider, model, &config.agent.params);
            backends.push((model_ref_str.to_string(), Box::new(backend)));
        }

        if backends.is_empty() {
            return Err(Error::Config("No models configured".into()));
        }

        Ok(Self {
            config,
            tools: ToolRegistry::new(),
            history: Vec::new(),
            backends,
        })
    }

    /// Build with custom backends (for consumers who implement Backend themselves)
    pub fn with_backends(config: Config, backends: Vec<(String, Box<dyn Backend>)>) -> Self {
        Self {
            config,
            tools: ToolRegistry::new(),
            history: Vec::new(),
            backends,
        }
    }

    /// Register a tool
    pub fn register_tool(&mut self, tool: Arc<dyn crate::tools::Tool>) {
        self.tools.register(tool);
    }

    /// Get mutable access to the tool registry
    pub fn tools_mut(&mut self) -> &mut ToolRegistry {
        &mut self.tools
    }

    /// Run the agent loop (non-streaming)
    pub async fn execute(&mut self, prompt: &str) -> Result<AgentResponse> {
        self.execute_inner(prompt, None).await
    }

    /// Run the agent loop with streaming token callback
    pub async fn execute_streaming(
        &mut self,
        prompt: &str,
        on_token: TokenCallback,
    ) -> Result<AgentResponse> {
        self.execute_inner(prompt, Some(on_token)).await
    }

    /// Core execution loop with model fallback
    async fn execute_inner(
        &mut self,
        prompt: &str,
        on_token: Option<TokenCallback>,
    ) -> Result<AgentResponse> {
        // Add user message
        self.history.push(Message {
            role: Role::User,
            content: prompt.to_string(),
            tool_calls: vec![],
            tool_result: None,
        });

        let mut all_tool_calls = Vec::new();
        let mut total_usage = TokenUsage::default();
        let mut iterations = 0;
        let max_iterations = self.config.agent.max_iterations;
        let mut model_used = self.backends[0].0.clone();

        loop {
            iterations += 1;
            if iterations > max_iterations {
                return Err(Error::Agent(format!(
                    "Max iterations ({}) exceeded",
                    max_iterations
                )));
            }

            let tool_defs = self.tools.definitions();

            // Try each backend in priority order (fallback chain)
            let response = self
                .generate_with_fallback(&tool_defs, on_token.as_ref(), &mut model_used)
                .await?;

            // Accumulate token usage
            if let Some(ref usage) = response.usage {
                total_usage.prompt_tokens += usage.prompt_tokens;
                total_usage.completion_tokens += usage.completion_tokens;
                total_usage.total_tokens += usage.total_tokens;
            }

            // No tool calls → final response
            if response.tool_calls.is_empty() {
                self.history.push(Message {
                    role: Role::Assistant,
                    content: response.content.clone(),
                    tool_calls: vec![],
                    tool_result: None,
                });

                return Ok(AgentResponse {
                    content: response.content,
                    tool_calls: all_tool_calls,
                    iterations,
                    usage: total_usage,
                    model_used,
                });
            }

            // Add assistant message with tool calls
            self.history.push(Message {
                role: Role::Assistant,
                content: response.content.clone(),
                tool_calls: response.tool_calls.clone(),
                tool_result: None,
            });

            // Execute each tool call
            for tool_call in &response.tool_calls {
                let start = std::time::Instant::now();

                let result = self
                    .tools
                    .execute(&tool_call.name, tool_call.arguments.clone())
                    .await;

                let duration_ms = start.elapsed().as_millis() as u64;

                let (result_value, success) = match result {
                    Ok(v) => (v, true),
                    Err(e) => (json!({"error": e.to_string()}), false),
                };

                let record = ToolCallRecord {
                    id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                    result: result_value.clone(),
                    success,
                    duration_ms,
                };
                all_tool_calls.push(record);

                self.history.push(Message {
                    role: Role::Tool,
                    content: serde_json::to_string(&result_value).unwrap_or_default(),
                    tool_calls: vec![],
                    tool_result: Some(ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content: result_value,
                        success,
                    }),
                });
            }
        }
    }

    /// Try generating with primary backend, fall back on retryable errors
    async fn generate_with_fallback(
        &self,
        tool_defs: &[crate::types::ToolDefinition],
        on_token: Option<&TokenCallback>,
        model_used: &mut String,
    ) -> Result<crate::types::Response> {
        let mut last_err = None;

        for (name, backend) in &self.backends {
            match backend.generate(&self.history, tool_defs, on_token).await {
                Ok(response) => {
                    *model_used = name.clone();
                    return Ok(response);
                }
                Err(e) => {
                    let err_str = e.to_string();
                    tracing::warn!(model = %name, error = %err_str, "Backend failed, trying fallback");

                    // Check if retryable by looking for status codes in error message
                    let is_retryable = err_str.contains("429")
                        || err_str.contains("500")
                        || err_str.contains("502")
                        || err_str.contains("503")
                        || err_str.contains("504")
                        || err_str.contains("timeout")
                        || err_str.contains("Stream error");

                    if !is_retryable {
                        return Err(e);
                    }

                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| Error::Agent("All backends failed".into())))
    }

    /// Access conversation history
    pub fn history(&self) -> &[Message] {
        &self.history
    }

    /// Clear conversation history
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Get the config
    pub fn config(&self) -> &Config {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_agent_from_config() {
        let toml = r#"
[providers.nvidia-nim]
base_url = "https://integrate.api.nvidia.com/v1"
api_key = "nvapi-test"

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
        let config = Config::from_str(toml).unwrap();
        let agent = Agent::from_config(config).unwrap();
        assert_eq!(agent.backends.len(), 2);
        assert!(agent.history().is_empty());
    }

    #[test]
    fn test_agent_no_models_error() {
        let toml = r#"
[agent]
primary = "missing/model"
"#;
        let config = Config::from_str(toml).unwrap();
        assert!(Agent::from_config(config).is_err());
    }
}
