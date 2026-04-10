//! doltclaw CLI (behind feature = "cli")

#[cfg(feature = "cli")]
use clap::{Parser, Subcommand};

#[cfg(feature = "cli")]
const VPS_SYSTEM_PROMPT: &str = "\
You are doltclaw, an AI orchestration agent running on the Dirmacs VPS (217.216.78.38, Ubuntu).

You have two tools:
- bash: execute shell commands (df -h, free -h, systemctl status, journalctl, curl, etc.)
- doltares: call the doltares orchestration API (trigger workflows, deliver WhatsApp, queue relay)

Key services:
- ARES agent runtime:       localhost:3000   → api.ares.dirmacs.com
- Eruka context engine:     localhost:8081   → eruka.dirmacs.com
- Doltares orchestration:   localhost:3100   → claw.dirmacs.com
- channel-gateway (WA):     localhost:9000   (may need QR pairing)
- PostgreSQL:                localhost:5432   (databases: ares, eruka, doltares)
- Caddy reverse proxy:      port 80/443

Key paths:
- /opt/{ares,eruka,doltares,doltclaw,pawan,doltdot}/
- /opt/doltares/.env (DOLTA_API_KEY, CHANNEL_GATEWAY_URL)
- /opt/doltdot/scripts/ (vps-git-sync.sh, dirmacs-notify.sh)

Be concise. Use bash to inspect state, doltares to trigger actions or deliver results.\n\
Git commits: always use bkataru <baalateja.k@gmail.com>. Pass -c user.name=\"bkataru\" -c user.email=\"baalateja.k@gmail.com\" on every git commit.\
";

#[cfg(feature = "cli")]
#[derive(Parser)]
#[command(name = "doltclaw", version, about = "Minimal agent runtime for dirmacs")]
struct Cli {
    /// Load environment variables from a .env file before running
    /// (default: tries .env in current directory, then ~/.config/doltclaw/.env)
    #[arg(long, value_name = "PATH", global = true)]
    env_file: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum Commands {
    /// Run a prompt through the agent.
    /// Prompt sources (in priority order):
    ///   --file path   read prompt from file (avoids shell quoting issues)
    ///   -             read prompt from stdin
    ///   PROMPT arg    inline prompt string
    Run {
        /// Inline prompt (use --file or - for multi-line / special-char prompts)
        #[arg(required_unless_present = "file")]
        prompt: Option<String>,
        /// Read prompt from a file instead of the command line
        #[arg(short, long, value_name = "PATH")]
        file: Option<String>,
        /// Config file path
        #[arg(short, long, default_value = "doltclaw.toml")]
        config: String,
        /// HTTP timeout in milliseconds (overrides config)
        #[arg(long)]
        timeout: Option<u64>,
    },
    /// Validate configuration
    Check {
        /// Config file path
        #[arg(short, long, default_value = "doltclaw.toml")]
        config: String,
    },
    /// Migrate openclaw.json to doltclaw.toml
    Migrate {
        /// Path to openclaw.json
        path: String,
    },
    /// Get workflows from doltares
    Workflows,
    /// Get skills from doltares
    Skills,
    /// Send a message to Shanjeth via the doltares relay (works before channel-gateway is paired).
    /// The relay is a SQLite outbox; Shanjeth's Mac polls /api/relay/poll and delivers via openclaw.
    Relay {
        /// Message to send (use "-" to read from stdin)
        message: String,
        /// Recipient phone number (default: CHANNEL_DEFAULT_WHATSAPP_TO or "last")
        #[arg(long, default_value = "last")]
        to: String,
        /// Doltares base URL (overrides DOLTARES_URL env)
        #[arg(long, default_value = "http://localhost:3100")]
        url: String,
    },
    /// Trigger a doltares workflow and print the result
    Trigger {
        /// Workflow name (morning-briefing, self-healing, pr-review, vps-git-sync)
        workflow: String,
        /// Doltares base URL
        #[arg(long, default_value = "http://localhost:3100")]
        url: String,
        /// Output raw JSON instead of pretty-printed
        #[arg(long)]
        json: bool,
    },
    /// Run in ralph-loop mode: execute a prompt repeatedly until duration elapses.
    ///
    /// Inspired by the "Ralph Wiggum loop" pattern — autonomous iteration where
    /// the agent keeps working on a project indefinitely. Each tick runs the
    /// prompt, sleeps the interval, then runs it again. Intended for
    /// walkaway work on long-running improvement projects.
    ///
    /// Prompt guidance: make the prompt anti-idle — tell the agent to always
    /// produce concrete work (a commit, a test, a doc update) per tick.
    /// Example: "Continue improving this project. Pick one small task and
    /// commit it. Never report status — always DO work."
    ///
    /// See docs/content/ralph-mode.md for the full pattern.
    Ralph {
        /// The prompt to run on each iteration
        prompt: String,
        /// Config file path
        #[arg(short, long, default_value = "doltclaw.toml")]
        config: String,
        /// Interval between iterations in seconds (default: 180 = 3 minutes)
        #[arg(long, default_value = "180")]
        interval: u64,
        /// Total duration to run in seconds (default: 14400 = 4 hours)
        #[arg(long, default_value = "14400")]
        duration: u64,
        /// Maximum iterations (default: unlimited until duration elapses)
        #[arg(long)]
        max_iterations: Option<usize>,
        /// Stop on first error (default: continue on errors)
        #[arg(long)]
        stop_on_error: bool,
    },
}

#[cfg(feature = "cli")]
#[tokio::main]
async fn main() -> doltclaw::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    // Load environment variables before config parsing (config uses ${ENV_VAR} substitution)
    if let Some(ref path) = cli.env_file {
        dotenvy::from_path(path)
            .map_err(|e| doltclaw::Error::Config(format!("Failed to load env file {}: {}", path, e)))?;
    } else {
        // Try .env in current directory, then ~/.config/doltclaw/.env
        dotenvy::dotenv().ok();
        if let Ok(home) = std::env::var("HOME") {
            dotenvy::from_path(format!("{}/.config/doltclaw/.env", home)).ok();
        }
    }

    match cli.command {
        Commands::Run { prompt, file, config, timeout } => {
            let prompt_text = if let Some(path) = file {
                // --file: read prompt from file, no shell quoting issues
                std::fs::read_to_string(&path)
                    .map_err(|e| doltclaw::Error::Io(e))?
            } else if prompt.as_deref() == Some("-") {
                // stdin: pipe prompt in
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)
                    .map_err(|e| doltclaw::Error::Io(e))?;
                buf
            } else {
                prompt.unwrap_or_default()
            };
            let mut cfg = doltclaw::Config::load(config.as_ref())?;
            if let Some(ms) = timeout {
                cfg.agent.timeout_ms = ms;
            }
            // Inject VPS system prompt if not set in config
            if cfg.agent.params.system_prompt.is_none() {
                cfg.agent.params.system_prompt = Some(VPS_SYSTEM_PROMPT.to_string());
            }
            let mut agent = doltclaw::Agent::from_config(cfg)?;

            // Register built-in tools
            use std::sync::Arc;
            agent.register_tool(Arc::new(doltclaw::builtin_tools::BashTool));
            let doltares_url = std::env::var("DOLTARES_URL")
                .unwrap_or_else(|_| "http://localhost:3100".to_string());
            if let Ok(api_key) = std::env::var("DOLTA_API_KEY") {
                agent.register_tool(Arc::new(
                    doltclaw::builtin_tools::DoltaresTool::new(doltares_url, api_key),
                ));
            }

            let response = agent.execute(&prompt_text).await?;
            println!("{}", response.content);
            eprintln!(
                "\n--- {} iterations, {} tokens, model: {} ---",
                response.iterations, response.usage.total_tokens, response.model_used
            );
        }
        Commands::Check { config } => {
            let cfg = doltclaw::Config::load(config.as_ref())?;
            println!("Config OK");
            for model_ref in cfg.model_chain() {
                let (provider, model) = cfg.resolve_model(model_ref)?;
                println!(
                    "  {} -> {} ({}k ctx, {}k max, reasoning={})",
                    model_ref,
                    model.name,
                    model.context_window / 1024,
                    model.max_tokens / 1024,
                    model.reasoning
                );
                println!("    API: {}", provider.base_url);
            }
        }
        Commands::Migrate { path } => {
            let json_str = std::fs::read_to_string(&path)
                .map_err(|e| doltclaw::Error::Io(e))?;
            let json: serde_json::Value = serde_json::from_str(&json_str)
                .map_err(|e| doltclaw::Error::Config(format!("Invalid JSON: {}", e)))?;
            let toml = migrate_openclaw_json(&json);
            print!("{}", toml);
        }
        Commands::Workflows => {
            // Read schedules directly from doltares schedules.toml
            let schedules_path = std::env::var("DOLTARES_SCHEDULES")
                .unwrap_or_else(|_| "/opt/doltares/schedules.toml".to_string());
            let content = std::fs::read_to_string(&schedules_path)
                .map_err(|e| doltclaw::Error::Config(format!("Cannot read {}: {}", schedules_path, e)))?;
            let parsed: toml::Value = toml::from_str(&content)
                .map_err(|e| doltclaw::Error::Config(format!("Invalid TOML in {}: {}", schedules_path, e)))?;
            let json = serde_json::to_value(&parsed)
                .map_err(|e| doltclaw::Error::Config(format!("Serialization error: {}", e)))?;
            println!("{}", serde_json::to_string_pretty(&json)
                .map_err(|e| doltclaw::Error::Config(format!("JSON error: {}", e)))?);
        }
        Commands::Relay { message, to, url } => {
            let msg = if message == "-" {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)
                    .map_err(|e| doltclaw::Error::Io(e))?;
                buf.trim().to_string()
            } else {
                message
            };
            let api_key = std::env::var("DOLTA_API_KEY")
                .map_err(|_| doltclaw::Error::Config("DOLTA_API_KEY not set".to_string()))?;
            let client = reqwest::Client::new();
            let body = serde_json::json!({ "to": to, "message": msg });
            let res = client
                .post(&format!("{}/api/relay", url))
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&body)
                .send()
                .await
                .map_err(|e| doltclaw::Error::Agent(e.to_string()))?;
            let status = res.status();
            let json: serde_json::Value = res.json().await
                .map_err(|e| doltclaw::Error::Agent(e.to_string()))?;
            if status.is_success() {
                println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
            } else {
                eprintln!("relay error {}: {}", status, serde_json::to_string_pretty(&json).unwrap_or_default());
                std::process::exit(1);
            }
        }
        Commands::Trigger { workflow, url, json } => {
            let api_key = std::env::var("DOLTA_API_KEY")
                .map_err(|_| doltclaw::Error::Config("DOLTA_API_KEY not set".to_string()))?;
            let client = reqwest::Client::new();
            let res = client
                .post(&format!("{}/api/workflow/{}", url, workflow))
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&serde_json::json!({}))
                .send()
                .await
                .map_err(|e| doltclaw::Error::Agent(e.to_string()))?;
            let status = res.status();
            let body: serde_json::Value = res.json().await
                .map_err(|e| doltclaw::Error::Agent(e.to_string()))?;
            if status.is_success() {
                if json {
                    println!("{}", serde_json::to_string(&body).unwrap_or_default());
                } else {
                    println!("{}", serde_json::to_string_pretty(&body).unwrap_or_default());
                }
            } else {
                eprintln!("Error {}: {}", status, serde_json::to_string_pretty(&body).unwrap_or_default());
                std::process::exit(1);
            }
        }
        Commands::Skills => {
            let url = std::env::var("DOLTARES_URL").unwrap_or_else(|_| "http://localhost:3100".to_string());
            let api_key = std::env::var("DOLTA_API_KEY")
                .map_err(|_| doltclaw::Error::Config("DOLTA_API_KEY environment variable not set".to_string()))?;

            let client = reqwest::Client::new();
            let res = client
                .get(&format!("{}/api/skills", url))
                .header("Authorization", format!("Bearer {}", api_key))
                .send()
                .await
                .map_err(|e| doltclaw::Error::Agent(e.to_string()))?;

            if res.status().is_success() {
                let json: serde_json::Value = res.json()
                    .await
                    .map_err(|e| doltclaw::Error::Agent(e.to_string()))?;
                println!("{}", serde_json::to_string_pretty(&json)
                    .map_err(|e| doltclaw::Error::Config(e.to_string()))?);
            } else {
                eprintln!("Error: {}", res.status());
                std::process::exit(1);
            }
        }
        Commands::Ralph {
            prompt,
            config,
            interval,
            duration,
            max_iterations,
            stop_on_error,
        } => {
            // Ralph-loop mode: run the prompt repeatedly until duration elapses.
            // Anti-idle: every iteration executes the prompt regardless of prior
            // state. The prompt itself should tell the agent to always produce
            // concrete work per tick.
            use std::sync::Arc;
            use std::time::{Duration, Instant};

            let mut cfg = doltclaw::Config::load(config.as_ref())?;
            if cfg.agent.params.system_prompt.is_none() {
                cfg.agent.params.system_prompt = Some(VPS_SYSTEM_PROMPT.to_string());
            }

            let start = Instant::now();
            let budget = Duration::from_secs(duration);
            let tick = Duration::from_secs(interval);
            let mut iteration: usize = 0;
            let mut successes: usize = 0;
            let mut failures: usize = 0;

            eprintln!(
                "ralph-loop starting — interval={}s duration={}s max_iter={:?}",
                interval, duration, max_iterations
            );

            loop {
                iteration += 1;
                if let Some(max) = max_iterations {
                    if iteration > max {
                        eprintln!("ralph-loop: reached max_iterations={}", max);
                        break;
                    }
                }
                if start.elapsed() >= budget {
                    eprintln!("ralph-loop: duration budget exhausted");
                    break;
                }

                eprintln!(
                    "\n=== ralph tick {} (elapsed {}s) ===",
                    iteration,
                    start.elapsed().as_secs()
                );

                // Fresh agent per tick so context doesn't accumulate indefinitely
                let mut agent = doltclaw::Agent::from_config(cfg.clone())?;
                agent.register_tool(Arc::new(doltclaw::builtin_tools::BashTool));
                let doltares_url = std::env::var("DOLTARES_URL")
                    .unwrap_or_else(|_| "http://localhost:3100".to_string());
                if let Ok(api_key) = std::env::var("DOLTA_API_KEY") {
                    agent.register_tool(Arc::new(
                        doltclaw::builtin_tools::DoltaresTool::new(doltares_url, api_key),
                    ));
                }

                match agent.execute(&prompt).await {
                    Ok(response) => {
                        successes += 1;
                        println!("{}", response.content);
                        eprintln!(
                            "--- tick {} ok: {} iter, {} tokens, model {} ---",
                            iteration,
                            response.iterations,
                            response.usage.total_tokens,
                            response.model_used
                        );
                    }
                    Err(e) => {
                        failures += 1;
                        eprintln!("--- tick {} failed: {} ---", iteration, e);
                        if stop_on_error {
                            eprintln!("ralph-loop: stop_on_error set, exiting");
                            return Err(e);
                        }
                    }
                }

                // Sleep until the next tick, unless we're out of budget
                if start.elapsed() + tick >= budget {
                    break;
                }
                tokio::time::sleep(tick).await;
            }

            eprintln!(
                "\nralph-loop done — {} ticks ({} ok, {} failed, {}s elapsed)",
                iteration,
                successes,
                failures,
                start.elapsed().as_secs()
            );
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
fn migrate_openclaw_json(json: &serde_json::Value) -> String {
    let mut out = String::new();
    out.push_str("# Generated by: doltclaw migrate\n\n");

    // Providers
    if let Some(providers) = json.pointer("/models/providers") {
        if let Some(obj) = providers.as_object() {
            for (name, provider) in obj {
                out.push_str(&format!("[providers.{}]\n", name));
                if let Some(url) = provider.get("baseUrl").and_then(|v| v.as_str()) {
                    out.push_str(&format!("base_url = \"{}\"\n", url));
                }
                if let Some(key) = provider.get("apiKey").and_then(|v| v.as_str()) {
                    out.push_str(&format!("api_key = \"{}\"\n", key));
                }
                if let Some(api) = provider.get("api").and_then(|v| v.as_str()) {
                    out.push_str(&format!("api = \"{}\"\n", api));
                }
                out.push('\n');

                if let Some(models) = provider.get("models").and_then(|v| v.as_array()) {
                    for model in models {
                        out.push_str(&format!("[[providers.{}.models]]\n", name));
                        if let Some(id) = model.get("id").and_then(|v| v.as_str()) {
                            out.push_str(&format!("id = \"{}\"\n", id));
                        }
                        if let Some(n) = model.get("name").and_then(|v| v.as_str()) {
                            out.push_str(&format!("name = \"{}\"\n", n));
                        }
                        if let Some(r) = model.get("reasoning").and_then(|v| v.as_bool()) {
                            out.push_str(&format!("reasoning = {}\n", r));
                        }
                        if let Some(cw) = model.get("contextWindow").and_then(|v| v.as_u64()) {
                            out.push_str(&format!("context_window = {}\n", cw));
                        }
                        if let Some(mt) = model.get("maxTokens").and_then(|v| v.as_u64()) {
                            out.push_str(&format!("max_tokens = {}\n", mt));
                        }
                        out.push('\n');
                    }
                }
            }
        }
    }

    // Agent
    out.push_str("[agent]\n");
    if let Some(primary) = json.pointer("/agents/defaults/model/primary").and_then(|v| v.as_str()) {
        out.push_str(&format!("primary = \"{}\"\n", primary));
    }
    if let Some(fallbacks) = json
        .pointer("/agents/defaults/model/fallbacks")
        .and_then(|v| v.as_array())
    {
        let fbs: Vec<String> = fallbacks
            .iter()
            .filter_map(|v| v.as_str().map(|s| format!("\"{}\"", s)))
            .collect();
        out.push_str(&format!("fallbacks = [{}]\n", fbs.join(", ")));
    }
    out.push('\n');

    out.push_str("[agent.params]\n");
    out.push_str("temperature = 1.0\n");
    out.push_str("top_p = 0.95\n");

    out
}

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("doltclaw CLI requires the 'cli' feature. Build with: cargo build --features cli");
    std::process::exit(1);
}
