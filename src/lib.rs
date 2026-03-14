//! doltclaw — Minimal agent runtime for dirmacs
//!
//! Provides LLM inference with model fallback, tool calling, and TOML-based
//! configuration. Part of the dirmacs OSS stack (ares, pawan, aegis, nimakai).
//!
//! # Quick Start
//!
//! ```no_run
//! use doltclaw::{Agent, Config};
//!
//! #[tokio::main]
//! async fn main() -> doltclaw::Result<()> {
//!     let config = Config::load("doltclaw.toml".as_ref())?;
//!     let mut agent = Agent::from_config(config)?;
//!     let response = agent.execute("Hello!").await?;
//!     println!("{}", response.content);
//!     Ok(())
//! }
//! ```

pub mod agent;
pub mod backend;
pub mod config;
pub mod tools;
pub mod types;

pub use agent::{Agent, AgentResponse};
pub use config::Config;
pub use types::{Message, Response, Role};

/// doltclaw error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config: {0}")]
    Config(String),
    #[error("llm: {0}")]
    Llm(String),
    #[error("tool: {0}")]
    Tool(String),
    #[error("agent: {0}")]
    Agent(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// doltclaw result type
pub type Result<T> = std::result::Result<T, Error>;
