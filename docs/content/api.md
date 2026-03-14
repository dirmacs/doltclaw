+++
title = "API"
+++

# Public API

## Core Types

| Type | Description |
|------|-------------|
| `Config` | Top-level config loaded from TOML |
| `Agent` | Runs tool-calling loops with model fallback |
| `AgentResponse` | Result of an agent execution |
| `Message` | Conversation message (role + content + tool calls) |
| `Response` | Raw LLM response |
| `Role` | System, User, Assistant, Tool |
| `Error` / `Result` | Error handling |

## Traits

| Trait | Description |
|-------|-------------|
| `Backend` | LLM generation interface (`async fn generate`) |
| `Tool` | Tool interface (`fn name`, `fn description`, `fn parameters_schema`, `async fn execute`) |

## Config Types

| Type | Description |
|------|-------------|
| `ProviderConfig` | API endpoint, key, models |
| `ModelConfig` | Model ID, name, reasoning flag, context/token limits |
| `AgentConfig` | Primary model, fallbacks, max iterations |
| `InferenceParams` | Temperature, top_p, system prompt |
| `ModelRef` | Parsed "provider/model-id" reference |

## Tool System

| Type | Description |
|------|-------------|
| `ToolRegistry` | Manages registered tools, executes by name |
| `ToolDefinition` | Name + description + JSON schema (sent to LLM) |
| `ToolCallRequest` | LLM's request to call a tool |
| `ToolCallRecord` | Execution record with timing |
| `ToolResult` | Tool execution result |
| `TokenUsage` | Prompt/completion/total token counts |
| `TokenCallback` | Streaming token callback (`Box<dyn Fn(&str)>`) |
