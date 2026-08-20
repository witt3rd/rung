//! Headless task agent — the product that composes rung-std blocks.
//!
//! Kernel stays generic: [`rung_std::agent::run`] drives one loop, `task` admits
//! a nested child, file tools resolve against the process cwd. This crate is
//! what OpenCode / grok keep in the session layer:
//!
//! - named [`catalog`] profiles (`explore`, `implement`, `review`)
//! - [`session`] files under `.rung/sessions` (resume by `task_id`)
//! - [`isolation`] git worktrees (`{repo}.wt/rung-task--{id}`)
//! - a [`background`] child of this binary
//!
//! ```text
//! rung-agent [--task-id ID] [--type explore|implement|review]
//!            [--isolation none|worktree] [--background] [PROMPT]
//! ```
//!
//! LLM config is environment-only: `RUNG_BASE_URL` (default `https://api.x.ai/v1`),
//! `RUNG_API_KEY` or `XAI_API_KEY`, `RUNG_MODEL` (default `grok-4`).

pub mod args;
pub mod background;
pub mod catalog;
pub mod config;
pub mod isolation;
pub mod run;
pub mod session;

pub use args::{Args, IsolationMode};
pub use catalog::Kind;
pub use run::{Outcome, run_job};
pub use session::{Line, Session, SessionStore};
