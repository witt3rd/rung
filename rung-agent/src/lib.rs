//! `rung-agent` is a thin shell over [`rung_agent_core`], which is the agent
//! itself. The binary (`src/main.rs`) parses argv and maps the result to an
//! exit code. This library re-exports the core at the paths it had before the
//! split, so `rung_agent::run::run_job`, `rung_agent::acp::run` and every
//! other path still resolve.

pub use rung_agent_core::*;
