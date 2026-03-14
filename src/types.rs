//! Core types for doltclaw agent communication

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Callback for receiving streaming tokens
pub type TokenCallback = Box<dyn Fn(&str) + Send + Sync>;

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<ToolResult>,
}

/// Role of a message sender
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A request to call a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Result from a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: Value,
    pub success: bool,
}

/// LLM response from a generation request
#[derive(Debug, Clone)]
pub struct Response {
    pub content: String,
    pub tool_calls: Vec<ToolCallRequest>,
    pub finish_reason: String,
    pub usage: Option<TokenUsage>,
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// Record of a tool call execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub result: Value,
    pub success: bool,
    pub duration_ms: u64,
}

/// Tool definition for the LLM
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_message_serialization() {
        let msg = Message {
            role: Role::User,
            content: "Hello".to_string(),
            tool_calls: vec![],
            tool_result: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
        // Empty tool_calls should be omitted
        assert!(!json.contains("tool_calls"));
    }

    #[test]
    fn test_tool_call_request() {
        let tc = ToolCallRequest {
            id: "tc_1".to_string(),
            name: "read_file".to_string(),
            arguments: json!({"path": "test.rs"}),
        };
        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains("read_file"));
        assert!(json.contains("test.rs"));
    }

    #[test]
    fn test_role_serde() {
        let role: Role = serde_json::from_str("\"assistant\"").unwrap();
        assert_eq!(role, Role::Assistant);
    }
}
