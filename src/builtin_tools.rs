//! Built-in tools for doltclaw CLI
//!
//! Registered automatically by `doltclaw run` unless --no-tools is passed.
//!
//! - BashTool: execute shell commands on the host
//! - DoltaresTool: call doltares API (deliver, trigger, relay)

use crate::tools::Tool;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// Execute bash commands on the host system.
pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command on the VPS. Runs with cwd=/. \
         Use for: df -h (disk), free -h (memory), systemctl status, journalctl, \
         cat /opt/*/logs, curl localhost:PORT/health, etc."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 60)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value) -> crate::Result<Value> {
        let command = args["command"]
            .as_str()
            .ok_or_else(|| crate::Error::Tool("command is required".into()))?;
        let timeout_secs = args["timeout_secs"].as_u64().unwrap_or(60);

        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(command)
            .current_dir("/")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let result: Result<crate::Result<_>, _> = timeout(Duration::from_secs(timeout_secs), async {
            let mut child = cmd.spawn().map_err(crate::Error::Io)?;
            let mut stdout = String::new();
            let mut stderr = String::new();
            if let Some(mut h) = child.stdout.take() {
                h.read_to_string(&mut stdout).await.ok();
            }
            if let Some(mut h) = child.stderr.take() {
                h.read_to_string(&mut stderr).await.ok();
            }
            let status = child.wait().await.map_err(crate::Error::Io)?;
            Ok((status, stdout, stderr))
        })
        .await;

        match result {
            Ok(Ok((status, stdout, stderr))) => {
                let truncate = |s: String| -> String {
                    if s.len() > 20_000 {
                        format!("{}...[truncated, {} bytes total]", &s[..20_000], s.len())
                    } else {
                        s
                    }
                };
                Ok(json!({
                    "success": status.success(),
                    "exit_code": status.code().unwrap_or(-1),
                    "stdout": truncate(stdout),
                    "stderr": truncate(stderr),
                }))
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(crate::Error::Agent(format!(
                "Command timed out after {}s: {}",
                timeout_secs, command
            ))),
        }
    }
}

/// Call the doltares orchestration API: deliver messages, trigger workflows, queue relay.
pub struct DoltaresTool {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl DoltaresTool {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client");
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            client,
        }
    }
}

#[async_trait]
impl Tool for DoltaresTool {
    fn name(&self) -> &str {
        "doltares"
    }

    fn description(&self) -> &str {
        "Call the doltares orchestration API. \
         Actions: \
         'deliver' — send a WhatsApp message (requires message, optional to); \
         'trigger' — run a workflow (requires workflow: morning-briefing | self-healing | pr-review | vps-git-sync); \
         'relay' — queue a relay message for delivery via Shanjeth's Mac (requires message, optional to)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["deliver", "trigger", "relay"],
                    "description": "What to do"
                },
                "workflow": {
                    "type": "string",
                    "description": "Workflow name for 'trigger' (morning-briefing, self-healing, pr-review, vps-git-sync)"
                },
                "message": {
                    "type": "string",
                    "description": "Message text for 'deliver' or 'relay'"
                },
                "to": {
                    "type": "string",
                    "description": "Recipient for deliver/relay (default: 'last' = CHANNEL_DEFAULT_WHATSAPP_TO)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> crate::Result<Value> {
        let action = args["action"]
            .as_str()
            .ok_or_else(|| crate::Error::Tool("action is required".into()))?;

        let (url, body) = match action {
            "deliver" => {
                let msg = args["message"].as_str().unwrap_or("");
                let to = args["to"].as_str().unwrap_or("last");
                (
                    format!("{}/api/deliver", self.base_url),
                    json!({ "channel": "whatsapp", "to": to, "message": msg }),
                )
            }
            "trigger" => {
                let wf = args["workflow"]
                    .as_str()
                    .ok_or_else(|| crate::Error::Tool("workflow is required for trigger".into()))?;
                (
                    format!("{}/api/workflow/{}", self.base_url, wf),
                    json!({}),
                )
            }
            "relay" => {
                let msg = args["message"].as_str().unwrap_or("");
                let to = args["to"].as_str().unwrap_or("last");
                (
                    format!("{}/api/relay", self.base_url),
                    json!({ "to": to, "message": msg }),
                )
            }
            _ => return Err(crate::Error::Tool(format!("Unknown action: {}", action))),
        };

        let res = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::Error::Agent(format!("doltares request failed: {}", e)))?;

        let http_status = res.status().as_u16();
        let json: Value = res
            .json()
            .await
            .unwrap_or(json!({ "error": "non-JSON response" }));

        Ok(json!({ "http_status": http_status, "response": json }))
    }
}
