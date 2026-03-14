# Doltclaw — Agent Context

Doltclaw is a minimal Rust agent runtime. Single crate providing LLM inference, model fallback chains, and tool calling. Used as the tool-execution layer inside doltares. Size-optimized: ~2.9MB release binary.

## Architecture

```
src/
  lib.rs      — Public API exports (Agent, Config, Message, Response, Role)
  agent.rs    — PawanAgent equivalent: tool-calling agentic loop
                execute(prompt) → iterates: LLM call → parse tool calls → execute → repeat
  backend/    — LlmBackend trait
    openai_compat.rs  — OpenAI-compatible HTTP (NIM, OpenAI, any OAI endpoint)
  config.rs   — Config struct, TOML loading, ${ENV_VAR} substitution at load time
  tools.rs    — Tool trait, ToolRegistry, built-in tools
  types.rs    — Message, Response, Role, ToolCall, InferenceParams
  main.rs     — CLI entry point (feature = "cli" only)
```

## Common Tasks

**Add a new tool:**
1. Implement `tools::Tool` trait in `src/tools.rs`
2. Register in `ToolRegistry::default()` or via `registry.register(tool)`
3. The agent loop handles tool dispatch automatically via `ToolRegistry::execute(name, args)`

**Add a new model provider:**
1. Implement `backend::LlmBackend` trait (async `complete()` + `stream()`)
2. Add to `Config` provider deserialization in `config.rs`
3. Wire in `Agent::from_config()` in `agent.rs`

**Change fallback behavior:**
- Fallback order is `config.agent.fallbacks` — tried in order when primary fails
- Failure triggers: HTTP error, timeout, empty response
- Currently no retry count limit per-model; uses `max_iterations` as total cap

## Key Decisions

- **Single crate, not a workspace** — keeps it small and easy to use as a path dependency
- **`cli` behind feature flag** — avoids clap/tracing-subscriber in lib users' dep trees
- **TOML config only** — no CLI flags for model selection; all via `doltclaw.toml`
- **rustls not native-tls** — `reqwest` uses `rustls-tls` feature, no OpenSSL dep needed
- **10-dep ceiling** — hard limit to keep binary small; check before adding any dep

## Library Usage (in doltares)

```toml
# Cargo.toml of consumer
doltclaw = { path = "/opt/doltclaw" }
```

```rust
use doltclaw::{Agent, Config};

let config = Config::load("doltclaw.toml".as_ref())?;
let mut agent = Agent::from_config(config)?;
let response = agent.execute("List files in the workspace").await?;
println!("{}", response.content);
```

## Environment

- `NVIDIA_API_KEY` — substituted into `${NVIDIA_API_KEY}` in config at load time
- `OLLAMA_BASE_URL` — if using local Ollama backend
- `RUST_LOG` — tracing filter (only active when `cli` feature is enabled)
