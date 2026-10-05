//! The host's core: the record, the state it projects, the clock, the stop
//! authority and the configuration, shared by the loop and the tools.
//!
//! Every change is `Core::emit`: append one record line, then apply it to
//! the state, under one lock, so the state is always exactly the record's
//! projection.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use serde_json::Value;

use crate::clock::{Clock, MINUTE, Millis, SECOND};
use crate::governor::GovConfig;
use crate::notify::Notifier;
use crate::record::{BatchLine, Line, Record};
use crate::state::State;
use crate::stop::StopAuthority;

/// Lock a host mutex. The host's one poison policy: **recover**. A panic
/// while a lock is held is a bug that surfaces once, where it happened; it
/// must not cascade into every later boundary or outward call. Recovering
/// is sound because what a host lock guards is either the projection of
/// the record (rebuilt from it on every start) or boundary scratch (the
/// pack, the next call's cache expectation, a cancel flag) that the next
/// rollover or turn overwrites.
///
/// The record's writer lock is the one exception and keeps `expect`: a
/// panic mid-write can leave a torn line, and appending after it would
/// bury the tear mid-file, which no open can cut.
pub(crate) fn lock<T: ?Sized>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// A record line only its own module may build (`kernel.commit`,
/// `kernel.release`, `expectation.settled`).
pub(crate) trait Sealed {
    fn kind(&self) -> &'static str;
    fn into_body(self) -> Value;
}

impl Sealed for crate::registers::Settlement {
    fn kind(&self) -> &'static str {
        "expectation.settled"
    }
    fn into_body(self) -> Value {
        crate::registers::Settlement::into_body(self)
    }
}

/// The operator's configuration of one host.
#[derive(Debug, Clone, Serialize)]
pub struct HostConfig {
    /// `mock` (the scripted engine) or `agent` (the real adapter).
    pub engine: String,
    /// The identity seed (the operator's text; rung ships none).
    pub identity: String,
    /// Operator-pinned facts (the stable layer's last part).
    pub pinned: Vec<String>,
    /// The operator ceiling: tool groups the agent may ever have.
    pub ceiling: BTreeSet<String>,
    /// The model ladder, best first. With a lister set, the listing filter
    /// decides which rungs stand ([`crate::ladder`]).
    pub ladder: Vec<String>,
    pub epoch_budget_tokens: usize,
    /// A turn's wall-clock bound.
    pub turn_bound_ms: Millis,
    /// One tool call's deadline.
    pub tool_deadline_ms: Millis,
    pub governor: GovConfig,
    /// The owner's channel name.
    pub owner_channel: String,
    /// Add [`crate::render::FREE_TIME_IDLE_RULE`] to the free-time rules
    /// (off by default).
    pub free_time_idle_rule: bool,
    /// Projects the operator seeds (id, title, why).
    pub seed_projects: Vec<(String, String, String)>,
    /// Write queued outbox messages here as `*.msg` files, if set.
    #[serde(skip)]
    pub outbox_dir: Option<PathBuf>,
    /// The sandbox workspace.
    #[serde(skip)]
    pub workspace: PathBuf,
    /// A provider cache's idle time-to-live (to tell a cold cache from a
    /// host bug).
    pub cache_ttl_ms: Millis,
    /// A seed for the host's own jitter (backoff).
    pub seed: u64,
}

impl HostConfig {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            engine: "mock".into(),
            identity: "You are an agent with a continuous existence in this host.".into(),
            pinned: Vec::new(),
            ceiling: ["core", "memory", "read", "workspace_write", "web_read"]
                .into_iter()
                .map(String::from)
                .collect(),
            ladder: vec!["mock/primary".into()],
            epoch_budget_tokens: 128_000,
            turn_bound_ms: 120 * SECOND,
            tool_deadline_ms: 30 * SECOND,
            governor: GovConfig::default(),
            owner_channel: "owner".into(),
            free_time_idle_rule: false,
            seed_projects: Vec::new(),
            outbox_dir: None,
            workspace,
            cache_ttl_ms: 10 * MINUTE,
            seed: 0,
        }
    }
}

pub struct Core {
    record: Record,
    state: Mutex<State>,
    pub clock: Arc<dyn Clock>,
    pub stop: Arc<StopAuthority>,
    pub notifier: Notifier,
    pub config: HostConfig,
    /// Called with every line, in seq order, while the line is applied (an
    /// outward channel's view of the record). An observer must not block
    /// or emit.
    observers: Mutex<Vec<Observer>>,
}

/// A record observer ([`Core::observe`]).
pub type Observer = Box<dyn Fn(&Line) + Send + Sync>;

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core")
            .field("record", &self.record)
            .finish()
    }
}

impl Core {
    pub(crate) fn new(
        record: Record,
        state: State,
        clock: Arc<dyn Clock>,
        stop: Arc<StopAuthority>,
        notifier: Notifier,
        config: HostConfig,
    ) -> Self {
        Self {
            record,
            state: Mutex::new(state),
            clock,
            stop,
            notifier,
            config,
            observers: Mutex::new(Vec::new()),
        }
    }

    /// See every line from now on, in seq order.
    pub fn observe(&self, f: Observer) {
        let _st = self.state();
        lock(&self.observers).push(f);
    }

    fn notify(&self, line: &Line) {
        let obs = lock(&self.observers);
        for f in obs.iter() {
            f(line);
        }
    }

    /// The state, locked. Hold it briefly: every emit waits for it.
    pub fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    /// Append a line and apply it.
    pub(crate) fn emit(&self, kind: &str, body: Value) -> Line {
        let mut st = self.state();
        let line = self.record.append(self.clock.now(), kind, body);
        st.apply(&line);
        self.notify(&line);
        line
    }

    /// Append a line and apply it, under one lock, only when `when` holds
    /// of the state just before it.
    pub(crate) fn emit_if(
        &self,
        kind: &str,
        body: Value,
        when: impl FnOnce(&State) -> bool,
    ) -> Option<Line> {
        let mut st = self.state();
        if !when(&st) {
            return None;
        }
        let line = self.record.append(self.clock.now(), kind, body);
        st.apply(&line);
        self.notify(&line);
        Some(line)
    }

    /// Append several lines in one write and apply them, under one lock.
    pub(crate) fn emit_many(&self, lines: Vec<(&str, Value)>) -> Vec<Line> {
        let mut st = self.state();
        let out = self.record.append_many(self.clock.now(), lines);
        for l in &out {
            st.apply(l);
            self.notify(l);
        }
        out
    }

    /// Decide on the state and write the decision, under one lock: `f`
    /// sees the state, and the lines it returns go out in one write and
    /// are applied in order before any other emit can run. So a check and
    /// the lines it licenses cannot be split by another writer or by a
    /// crash. `f` must not lock the state (it already holds it).
    pub(crate) fn transact<T, E>(
        &self,
        f: impl FnOnce(&State) -> Result<(Vec<BatchLine>, T), E>,
    ) -> Result<(Vec<Line>, T), E> {
        let mut st = self.state();
        let (batch, out) = f(&st)?;
        let lines = if batch.is_empty() {
            Vec::new()
        } else {
            self.record.append_batch(self.clock.now(), batch)
        };
        for l in &lines {
            st.apply(l);
            self.notify(l);
        }
        Ok((lines, out))
    }

    /// Append a sealed line and apply it.
    pub(crate) fn emit_sealed(&self, s: impl Sealed) -> Line {
        let mut st = self.state();
        let kind = s.kind();
        let line = self
            .record
            .append_sealed(self.clock.now(), kind, s.into_body());
        st.apply(&line);
        self.notify(&line);
        line
    }

    /// Emit with the state hash taken just before the line (a `turn.ended`).
    pub(crate) fn emit_hashed(&self, kind: &str, mut body: Value) -> Line {
        let mut st = self.state();
        body["projection"] = st.hash().into();
        let line = self.record.append(self.clock.now(), kind, body);
        st.apply(&line);
        self.notify(&line);
        line
    }

    /// fsync the record.
    pub fn sync(&self) {
        self.record.sync();
    }

    pub fn record_dir(&self) -> PathBuf {
        self.record.dir().to_path_buf()
    }

    pub fn now(&self) -> Millis {
        self.clock.now()
    }
}
