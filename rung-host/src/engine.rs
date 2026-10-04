//! The seam between the host and whatever runs a turn.
//!
//! [`TurnEngine::turn`] runs one bounded turn on the thread the host
//! assembled, with the host's tools and `rung-agent-core`'s
//! [`TurnCtl`] (cancel flag, sink), and reports every model call. Two
//! engines implement it: the scripted [`crate::sim::MockEngine`] and the
//! real [`crate::adapter::AgentEngine`] over
//! `rung_agent_core::engine::Engine`, which maps a `TurnReport` (its
//! `calls`, `failure` and messages) onto [`EngineTurn`].

use std::sync::Arc;

use rung_agent_core::engine::{CallUsage, ProviderClass, ProviderFailure, TurnCtl};
use rung_std::agent::Thread;
use rung_std::llm::ChatMessage;
use rung_std::tools::Toolset;
use serde::Serialize;
use serde_json::{Value, json};

use crate::clock::Millis;

/// Who refused a call: the model's provider, or the routing platform in
/// front of it (an account quota). Only a provider failure steps down the
/// model ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Provider,
    Platform,
}

/// A provider failure that stopped a turn, with where it came from and
/// when a platform quota resets.
#[derive(Debug, Clone, PartialEq)]
pub struct HostFailure {
    pub failure: ProviderFailure,
    pub origin: Origin,
    /// `X-RateLimit-Reset`, when the platform sent it.
    pub reset_at: Option<Millis>,
    /// The router has no endpoint for this model that the account's data
    /// policy or guardrails allow (its reasons, as it named them). Another
    /// rung may route: the ladder steps down.
    pub unroutable: Option<Vec<String>>,
}

impl HostFailure {
    pub fn class_name(&self) -> &'static str {
        match self.failure.class {
            ProviderClass::RateLimit => "rate_limit",
            ProviderClass::Overloaded => "overloaded",
            ProviderClass::Transport => "transport",
            ProviderClass::Timeout => "timeout",
            ProviderClass::Auth => "auth",
            ProviderClass::Quota => "quota",
            ProviderClass::Config => "config",
            ProviderClass::Refused => "refused",
            ProviderClass::Overflow => "overflow",
            ProviderClass::Invalid => "invalid",
            ProviderClass::Output => "output",
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "class": self.class_name(),
            "origin": self.origin,
            "retry_after_ms": self.failure.retry_after_ms,
            "reset_at": self.reset_at,
            "unroutable": self.unroutable,
        })
    }
}

/// One model call as the engine saw it.
#[derive(Debug, Clone)]
pub struct CallRecord {
    /// The served model and the provider's usage.
    pub usage: CallUsage,
    pub provider: String,
    pub latency_ms: Millis,
}

/// What a turn is given.
pub struct TurnRequest {
    pub turn: u64,
    pub thread: Thread,
    pub tools: Arc<dyn Toolset>,
    pub ctl: TurnCtl,
    /// The model the host asks for (the ladder's current rung).
    pub model: String,
    /// The epoch's session id (sticky routing on a real provider).
    pub session: String,
    /// The turn's hard end on the host clock.
    pub deadline: Millis,
    /// Model calls the turn may make.
    pub step_cap: u32,
}

/// How a turn ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ended {
    Completed,
    /// A provider failure stopped it.
    Failed,
    /// The stop authority raised its cancel flag.
    Cancelled,
    /// It reached its wall-clock bound or step cap without an answer.
    Bounded,
}

/// What one turn did.
#[derive(Debug, Clone)]
pub struct EngineTurn {
    /// The messages the turn added after the thread it was given, verbatim.
    pub messages: Vec<ChatMessage>,
    pub final_text: Option<String>,
    pub calls: Vec<CallRecord>,
    pub failure: Option<HostFailure>,
    pub ended: Ended,
    /// The engine rewrote what it was given inside the turn (a context
    /// overflow's elision, its last resort): that call's prefix broke.
    pub rewritten: bool,
}

/// Runs one turn. Send + Sync so the host can hold it in its carry.
pub trait TurnEngine: Send + Sync {
    /// `mock`, or the adapter's name.
    fn name(&self) -> &str;
    fn turn(&self, req: TurnRequest) -> EngineTurn;
}
