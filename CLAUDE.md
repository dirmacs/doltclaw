# Doltclaw

Minimal Rust agent runtime for dirmacs. Single crate (lib + optional CLI binary). Size target: under 3MB release binary. Used as the inference/tool-calling layer in doltares.

## Build & Test

```bash
cargo build --release              # release binary (~2.9MB stripped)
cargo build --features cli         # build CLI binary
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

## Architecture

Single crate with optional `cli` feature:

```
src/
  lib.rs      — Public API: Agent, Config, Message, Response, Role
  agent.rs    — Agent: tool-calling loop, execute()
  backend/    — LlmBackend trait + openai_compat impl
  config.rs   — Config: loads doltclaw.toml with ${ENV_VAR} substitution
  tools.rs    — Tool trait + ToolRegistry
  types.rs    — Message, Response, Role, ToolCall
  main.rs     — CLI binary (feature-gated behind "cli")
```

## Config

```toml
# doltclaw.toml
[providers.nvidia-nim]
base_url = "https://integrate.api.nvidia.com/v1"
api_key = "${NVIDIA_API_KEY}"

[[providers.nvidia-nim.models]]
id = "stepfun-ai/step-3.5-flash"
name = "StepFun Flash"

[agent]
primary = "nvidia-nim/stepfun-ai/step-3.5-flash"
fallbacks = ["nvidia-nim/z-ai/glm4.7"]
max_iterations = 50
```

## Key Rules

- **10-dependency ceiling** — keep the dep count minimal; check `cargo tree` before adding deps
- **`cli` feature is opt-in** — `clap` and `tracing-subscriber` are behind `features = ["cli"]`
- **`${ENV_VAR}` substitution** — handled in `config.rs`, not at runtime; no shell expansion
- **Release profile strips symbols** — `opt-level = "z"`, `lto = true`, `strip = true` for minimal binary size
- **No dirmacs-internal deps** — doltclaw must be usable standalone without ares/eruka/pawan

## Git Author

```bash
git -c user.name="bkataru" -c user.email="baalateja.k@gmail.com" commit
```
