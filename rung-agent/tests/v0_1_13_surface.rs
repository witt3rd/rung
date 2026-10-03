//! The library surface a dependent pinned at v0.1.13 used still resolves at
//! the same `rung_agent::` paths, now that the agent lives in
//! `rung-agent-core` and this crate re-exports it.
//!
//! The list is every top-level public item of every public module at
//! v0.1.13 (`c0e23a8`), plus the crate-root re-exports. A path that stopped
//! resolving is a compile error here. The signatures of the entry points a
//! host calls are pinned too.

use std::path::Path;

#[allow(unused_imports)]
use rung_agent::acp::run;
#[allow(unused_imports)]
use rung_agent::args::{Args, IsolationMode, usage};
#[allow(unused_imports)]
use rung_agent::background::{CHILD_ENV, Launch, child_args, in_child, spawn_child};
#[allow(unused_imports)]
use rung_agent::catalog::{Kind, Scope};
#[allow(unused_imports)]
use rung_agent::config::{
    MemorySettings, TurnCheckBackend, TurnCheckSettings, config_dir, config_dir_from, config_path,
    dummy, load, load_from_path, load_memory, load_tool_groups, load_tool_groups_from_path,
    load_tool_images, load_tool_images_from_path, load_turn_check, load_turn_check_from_path,
};
#[allow(unused_imports)]
use rung_agent::isolation::{Worktree, ensure, primary_clone};
#[allow(unused_imports)]
use rung_agent::mcp::{
    CancelScopeGuard, DEFAULT_TIMEOUT, HttpWire, MAX_ATTEMPTS, McpRoster, McpSpec, McpToolError,
    SessionSinkGuard, WithMcp, is_session_cancelled, parse_rpc_body, redact, register_secret,
    set_session_cancel, set_session_sink,
};
#[allow(unused_imports)]
use rung_agent::memory::{
    CONTEXT_CHARS, CONTEXT_ITEMS, Hooks, Layered, MARKER, MAX_CHARS, MAX_RECORDS, McpProvider,
    MemoryReport, PROMPT_CHARS, RECALL_TOOL, RETAIN_TOOL, TURN_CHARS, Turnover, default_scope,
    inject, mcp_spec, registry, repo_root,
};
#[allow(unused_imports)]
use rung_agent::memory_fixture::{Clause, FIXTURE_COST_USD, check, report, serve};
#[allow(unused_imports)]
use rung_agent::run::{
    CatalogSpawn, JobError, JobEx, Outcome, Status, WrapTools, run_job, run_job_ex,
};
#[allow(unused_imports)]
use rung_agent::session::{Line, Session, SessionStore, check_id, new_id};
#[allow(unused_imports)]
use rung_agent::stream::{Emitter, NotifyingToolset, ObservingToolset, ToolNotify};
#[allow(unused_imports)]
use rung_agent::turn_check::{
    Arm, Checked, Completion, Facts, Flagged, Gate, NUDGE, Nudged, STATE_TOKEN_LIMIT, Turn,
    TurnCheckReport, TurnReading, Unjudged, Unread, arm, prior_actions, questions, turn_ask,
    turn_state, turncheck,
};

#[allow(unused_imports)]
use rung_agent::{
    Args as RootArgs, IsolationMode as RootIsolation, Kind as RootKind, Line as RootLine,
    Outcome as RootOutcome, Scope as RootScope, Session as RootSession, SessionStore as RootStore,
    Status as RootStatus, run_job as root_run_job,
};

#[test]
fn the_v0_1_13_entry_points_keep_their_signatures() {
    let _: fn(&Args, &Path) -> Result<Outcome, String> = run_job;
    let _: fn(&Args, &Path, JobEx) -> Result<Outcome, JobError> = run_job_ex;
    let _: fn(Args) -> Result<(), String> = rung_agent::acp::run;
    let _: fn() -> &'static str = rung_agent::args::usage;
    let _: fn(&[String]) -> Result<(), String> = rung_agent::memory_fixture::serve;
    let _: fn(&RootArgs, &Path) -> Result<RootOutcome, String> = root_run_job;
    let _: fn(&str) = rung_agent::mcp::register_secret;
    let _: fn(&str) -> String = rung_agent::mcp::redact;
}
