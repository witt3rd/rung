//! Headless agent — composes rung-std blocks. Not a coding product;
//! coding is one thing it can do when given write/shell tools.
//!
//! This crate is the agent as a library. The `rung-agent` binary is a thin
//! shell over it (argv in, exit code out); a host imports it directly.
//!
//! Turn engine ([`engine`]): [`engine::Engine`] is built from values
//! ([`engine::EngineSpec`]) and runs one turn on a caller-assembled thread
//! ([`engine::Engine::turn`]), with no session-store side effects, its
//! diagnostics to the caller's sink and a typed provider failure on the
//! report. [`run::run_job`] is the CLI's job around it: load the session,
//! assemble the thread, run the turn, save.
//!
//! Tool access is a **scope** (`--tools` / config `tools:`). `--toolset
//! explore|implement|review` names a preset. `--type` is an alias of `--toolset`.
//!
//! ```text
//! rung-agent [--tools none|read,python,web,…] [--toolset explore|implement|review]
//!            [--isolation none|worktree] [--background] [PROMPT]
//! rung-agent --acp
//! rung-agent --acp-http [ADDR]
//! ```
//!
//! LLM config: `$XDG_CONFIG_HOME/rung/config.yaml` (`llm:` block), then env
//! (`RUNG_BASE_URL`, `RUNG_API_KEY` / `XAI_API_KEY`, `RUNG_MODEL`). Env wins.
//! The file may name `api_key_env`; it does not hold the key. No key is
//! required — LAN llama.cpp sends no Authorization header.
//!
//! Turn check ([`turn_check`]): off by default. `turn_check.backend: jev` in
//! the same file, or `RUNG_TURN_CHECK=jev`, has Jev read each finished turn
//! before it is reported `completed`.
//!
//! Memory ([`memory`]): off by default. `--memory`, `RUNG_MEMORY` or
//! `memory.provider` picks `external` (the caller owns memory; rung keeps
//! none), `baseline` (lexical, local, offline), or `mcp:<url or command>` (a
//! provider process speaking the `rung-memory/1` contract,
//! `docs/rung-memory.md`).

pub mod acp;
pub mod acp_http;
pub mod args;
pub mod background;
pub mod catalog;
pub mod config;
pub mod engine;
pub mod isolation;
pub mod mcp;
pub mod memory;
pub mod memory_fixture;
pub mod memory_score;
pub mod redact;
pub mod run;
pub mod session;
pub mod stream;
pub mod turn_check;

pub use args::{Args, IsolationMode};
pub use catalog::{Kind, Scope};
pub use run::{Outcome, Status, run_job};
pub use session::{Line, Session, SessionStore};
