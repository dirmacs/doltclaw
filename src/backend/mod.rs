//! LLM backend trait and implementations

pub mod openai_compat;

use crate::types::{Response, ToolDefinition, TokenCallback};
use crate::{Message, Result};
use async_trait::async_trait;

/// Trait for LLM backends
#[async_trait]
pub trait Backend: Send + Sync {
    /// Generate a response from the LLM
    async fn generate(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        on_token: Option<&TokenCallback>,
    ) -> Result<Response>;
}
