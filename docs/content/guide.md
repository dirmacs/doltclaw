+++
title = "Guide"
+++

# Getting Started

## Install

```bash
git clone https://github.com/dirmacs/doltclaw && cd doltclaw
cargo install --path . --features cli
```

## Set up NVIDIA NIM

Get a free API key from [build.nvidia.com](https://build.nvidia.com):

```bash
export NVIDIA_API_KEY=nvapi-...
```

## Create config

```bash
# If migrating from openclaw:
doltclaw migrate ~/.openclaw/openclaw.json > doltclaw.toml

# Or create manually — see doltclaw.toml in the repo
```

## Verify

```bash
doltclaw check
```

Output:
```
Config OK
  nvidia-nim/qwen/qwen3.5-122b-a10b -> Qwen 3.5 122B (128k ctx, 16k max, reasoning=false)
    API: https://integrate.api.nvidia.com/v1
  nvidia-nim/z-ai/glm4.7 -> GLM 4.7 (125k ctx, 16k max, reasoning=true)
    API: https://integrate.api.nvidia.com/v1
```

## Run a prompt

```bash
doltclaw run "Hello, what models are you?"
```

## Use as a library

```rust
use doltclaw::{Agent, Config};
use std::sync::Arc;

#[tokio::main]
async fn main() -> doltclaw::Result<()> {
    let config = Config::load("doltclaw.toml".as_ref())?;
    let mut agent = Agent::from_config(config)?;

    // Register your own tools
    // agent.register_tool(Arc::new(MyCustomTool));

    let response = agent.execute("Explain Rust's ownership model").await?;
    println!("{}", response.content);
    println!("Model: {} | Iterations: {}", response.model_used, response.iterations);
    Ok(())
}
```

## Model fallback

If the primary model (Qwen 3.5) fails with a retryable error (429, 5xx, timeout), doltclaw automatically tries the next model in the chain. Non-retryable errors (401, 403, 404) abort immediately.

```toml
[agent]
primary = "nvidia-nim/qwen/qwen3.5-122b-a10b"
fallbacks = [
  "nvidia-nim/stepfun-ai/step-3.5-flash",
  "nvidia-nim/z-ai/glm4.7",
]
```

## Environment variable substitution

Use `${ENV_VAR}` in your TOML config:

```toml
[providers.nvidia-nim]
api_key = "${NVIDIA_API_KEY}"
```

Variables are substituted at load time. Missing variables become empty strings.
