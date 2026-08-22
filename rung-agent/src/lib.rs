//! Headless agent — composes rung-std blocks. Not a coding product;
//! coding is one thing it can do when given write/shell tools.
//!
//! Tool access is a **scope** (`--tools` / config `tools:`). `--toolset
//! explore|implement|review` names a preset. `--type` is an alias of `--toolset`.
//!
//! ```text
//! rung-agent [--tools none|read,python,web,…] [--toolset explore|implement|review]
//!            [--isolation none|worktree] [--background] [PROMPT]
//! rung-agent --acp
//! ```
//!
//! LLM config: `$XDG_CONFIG_HOME/rung/config.yaml` (`llm:` block), then env
//! (`RUNG_BASE_URL`, `RUNG_API_KEY` / `XAI_API_KEY`, `RUNG_MODEL`). Env wins.
//! The file may name `api_key_env`; it does not hold the key. No key is
//! required — LAN llama.cpp sends no Authorization header.

pub mod acp;
pub mod args;
pub mod background;
pub mod catalog;
pub mod config;
pub mod isolation;
pub mod run;
pub mod session;
pub mod stream;

pub use args::{Args, IsolationMode};
pub use catalog::{Kind, Scope};
pub use run::{Outcome, run_job};
pub use session::{Line, Session, SessionStore};
