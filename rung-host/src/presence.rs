//! The `Presence` ladder and the host that drives it.
//!
//! ```text
//! Waking(Recovered) => Boundary(Edge) => { Again -> Boundary | Halted(Why) }
//! ```
//!
//! There is no resting rung: a boundary either runs a turn (or waits out a
//! world-imposed `degraded` interval) and goes `Again`, or the stop
//! authority halts it. A boundary's [`Edge`] is sealed: only this module
//! mints one, and admitting stimuli consumes it, so admission happens only
//! at a boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rung::ladder;
use rung_agent_core::engine::TurnCtl;
use serde_json::{Value, json};

use crate::calendar::{Entry, Missed, Origin, When};
use crate::canon;
use crate::clock::{Clock, Millis};
use crate::core::{Core, HostConfig};
use crate::desk::admit::{Admit, AdmitChoice};
use crate::desk::consolidate::Consolidate;
use crate::desk::inject::{Cue, Inject, InjectChoice, InjectCtx};
use crate::desk::pack::{Action, Pack as PackFamily, PackInput};
use crate::desk::tools::Tools;
use crate::desk::{By, DecisionDesk, HostQuestion};
use crate::engine::{Ended, EngineTurn, TurnEngine, TurnRequest};
use crate::governor::{self, Wait};
use crate::inbox::{Item, ItemKind, Role, Source};
use crate::kernel::{self, COPY_LOOP_TURNS, COPY_SIMILARITY, TurnKind};
use crate::memory::MemoryHost;
use crate::notify::Notifier;
use crate::pack::Pack;
use crate::record::{Line, Record};
use crate::registers::{ExpState, Judge};
use crate::render;
use crate::state::State;
use crate::stop::{StopAuthority, Why};
use crate::toolbox::{self, HostTools, NoWeb, WebReader};

/// A boundary's sealed token. Minted only here; consumed by admission.
#[derive(Debug)]
pub struct Edge {
    n: u64,
}

impl Edge {
    fn mint(n: u64) -> Self {
        Self { n }
    }

    pub fn n(&self) -> u64 {
        self.n
    }
}

/// What waking found in the record.
#[derive(Debug, Clone)]
pub struct Recovered {
    /// Lines already in the record when the host opened it.
    pub lines: usize,
    /// The time of the last of them.
    pub last_at: Option<Millis>,
    pub torn_bytes: u64,
}

/// When the host stops by itself (tests and bounded runs).
#[derive(Debug, Clone, Copy, Default)]
pub struct Limits {
    pub max_turns: Option<u64>,
    pub until: Option<Millis>,
}

/// Everything a host is built from.
pub struct HostBuilder {
    pub config: HostConfig,
    pub state_dir: PathBuf,
    pub clock: Arc<dyn Clock>,
    pub stop: Arc<StopAuthority>,
    pub notifier: Notifier,
    pub engine: Arc<dyn TurnEngine>,
    pub desk: DecisionDesk,
    pub memory: Option<Arc<MemoryHost>>,
    pub sources: Vec<Box<dyn Source>>,
    pub judge: Option<Arc<dyn Judge>>,
    pub web: Arc<dyn WebReader>,
    pub limits: Limits,
    /// Record segment size (rotation).
    pub segment_bytes: u64,
    /// Calendar entries the operator seeds (added once).
    pub seed_calendar: Vec<Entry>,
    /// Lists the router's models for the ladder's filter; `None`: the
    /// configured ladder is walked as it is.
    pub lister: Option<Arc<dyn crate::ladder::Lister>>,
}

impl HostBuilder {
    pub fn new(
        config: HostConfig,
        state_dir: &Path,
        clock: Arc<dyn Clock>,
        engine: Arc<dyn TurnEngine>,
    ) -> Self {
        Self {
            config,
            state_dir: state_dir.to_path_buf(),
            clock,
            stop: Arc::new(StopAuthority::default()),
            notifier: Notifier::none(),
            engine,
            desk: DecisionDesk::rule_only(),
            memory: None,
            sources: Vec::new(),
            judge: None,
            web: Arc::new(NoWeb),
            limits: Limits::default(),
            segment_bytes: crate::record::SEGMENT_BYTES,
            seed_calendar: Vec::new(),
            lister: None,
        }
    }
}

/// One continuous host.
pub struct Host {
    pub core: Arc<Core>,
    engine: Arc<dyn TurnEngine>,
    pub desk: DecisionDesk,
    pack: Mutex<Pack>,
    memory: Option<Arc<MemoryHost>>,
    sources: Mutex<Vec<Box<dyn Source>>>,
    judge: Option<Arc<dyn Judge>>,
    web: Arc<dyn WebReader>,
    limits: Limits,
    seed_calendar: Vec<Entry>,
    lister: Option<Arc<dyn crate::ladder::Lister>>,
    /// Set on waking after a gap: when the host stopped running.
    down_since: Mutex<Option<Millis>>,
    /// The next model call follows a host-made change: (cause, what stayed valid).
    reset: Mutex<Option<(String, Validity)>>,
    /// A model switch waits for the next boundary's rollover.
    switch_pending: Mutex<Option<String>>,
    /// The previous call's prompt tokens (same epoch and model), for the
    /// cache efficiency expectation.
    last_call: Mutex<Option<(u64, String, u64, Millis)>>,
    /// When this boundary's decisions must be made by.
    desk_deadline: Mutex<Instant>,
    /// Retains decided at the boundary, done after the turn.
    deferred_retain: Mutex<Vec<crate::desk::Candidate>>,
    /// The running turn's cancel flag (an owner may cut a shared turn).
    turn_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

/// What survives a host-made change, for the next call's expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Validity {
    Stable,
    Nothing,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host").field("core", &self.core).finish()
    }
}

ladder!(Presence {
    carry {
        host: Arc<Host>,
    }

    Waking(Recovered) => Boundary(Edge) => {
          Again -> Boundary
        | Halted(Why)
    }
} impl {
    boundary = |waking| {
        let carry = waking.carry().clone();
        carry.host.wake(&waking.payload);
        // Boundaries count on across restarts.
        let n = carry.host.core.state().boundary + 1;
        Boundary::new(Edge::mint(n), carry)
    },
    step = |b| {
        let carry = b.carry().clone();
        match carry.host.step(b.payload) {
            Next::Again(edge) => Ok(StepOutcome::Again(Boundary::new(edge, carry))),
            Next::Halt(why) => Ok(StepOutcome::Halted(Halted::new(why))),
        }
    },
});

/// What a boundary leads to.
pub enum Next {
    Again(Edge),
    Halt(Why),
}

/// The `calendar.added` body of a one-shot entry (agent origin dates).
pub(crate) fn calendar_body(id: &str, at: Millis, origin: Origin, text: &str) -> Value {
    let e = Entry {
        id: id.into(),
        when: When::At(at),
        origin,
        text: text.into(),
        firm: false,
        missed: Missed::OnceLate,
    };
    crate::calendar::added_body(&e)
}

fn mode_value(st: &State) -> Value {
    json!(st.kernel.mode_label())
}

impl Host {
    /// Open the record, replay it, and build the host. Returns the host and
    /// what waking found.
    pub fn open(b: HostBuilder) -> std::io::Result<(Arc<Host>, Recovered)> {
        std::fs::create_dir_all(&b.state_dir)?;
        let opened = Record::open_with(b.state_dir.join("record"), b.segment_bytes)?;
        let state = State::replay(&opened.lines);
        let recovered = Recovered {
            lines: opened.lines.len(),
            last_at: opened.lines.last().map(|l| l.at),
            torn_bytes: opened.torn_bytes,
        };
        let tools = toolbox::superset(b.memory.is_some());
        let system = render::system(&b.config);
        let pack = Pack::new(tools, system, b.config.epoch_budget_tokens);
        let core = Arc::new(Core::new(
            opened.record,
            state,
            b.clock,
            b.stop,
            b.notifier,
            b.config,
        ));
        let host = Arc::new(Host {
            core,
            engine: b.engine,
            desk: b.desk,
            pack: Mutex::new(pack),
            memory: b.memory,
            sources: Mutex::new(b.sources),
            judge: b.judge,
            web: b.web,
            limits: b.limits,
            seed_calendar: b.seed_calendar,
            lister: b.lister,
            down_since: Mutex::new(None),
            reset: Mutex::new(None),
            switch_pending: Mutex::new(None),
            last_call: Mutex::new(None),
            deferred_retain: Mutex::new(Vec::new()),
            turn_cancel: Mutex::new(None),
            desk_deadline: Mutex::new(Instant::now()),
        });
        Ok((host, recovered))
    }

    /// Run the ladder until the stop authority halts it.
    pub fn run(self: &Arc<Self>, recovered: Recovered) -> Why {
        let w = presence::Waking::new(recovered, presence::Carry { host: self.clone() });
        let mut b = presence::boundary(w);
        loop {
            match presence::step(b) {
                Ok(presence::StepOutcome::Again(next)) => b = next,
                Ok(presence::StepOutcome::Halted(h)) => return h.into_payload(),
                // The step has no error path of its own.
                Err(f) => {
                    let why = Why::Stopped {
                        by: format!("ladder failure: {}", f.error),
                    };
                    self.halt(why.clone());
                    return why;
                }
            }
        }
    }

    /// The canonical bytes of the request the pack would send now.
    pub fn request_bytes(&self) -> Vec<u8> {
        let p = self.pack.lock().expect("pack");
        crate::pack::request_bytes(p.tools(), p.system(), &p.thread().messages)
    }

    pub fn record_lines(&self) -> std::io::Result<Vec<Line>> {
        Record::read_dir(self.core.record_dir())
    }

    fn cfg(&self) -> &HostConfig {
        &self.core.config
    }

    fn rung_model(&self, rung: usize) -> String {
        self.cfg()
            .ladder
            .get(rung)
            .cloned()
            .unwrap_or_else(|| "none".into())
    }

    // ─── Outward channels ────────────────────────────────────────────────
    //
    // What an outward surface (ACP) may do to the host. Each change is a
    // record line, durable before it returns.

    /// Accept a stimulus from outside: recorded and fsynced before this
    /// returns. An owner control (`stop`, `release`) acts at once.
    pub fn accept_external(&self, item: &Item) {
        self.accept(item);
    }

    /// Withdraw a stimulus that no boundary has admitted yet: its one
    /// disposition is `withdrawn`. False when it was admitted (or unknown).
    pub fn withdraw(&self, id: &str) -> bool {
        let turn = self.core.state().turn;
        let done = self.core.emit_if(
            "stimulus.disposed",
            json!({"id": id, "disposition": "withdrawn", "turn": turn}),
            |st| st.inbox.pending.contains_key(id),
        );
        if done.is_some() {
            self.core.sync();
        }
        done.is_some()
    }

    /// Raise the running turn's cancel flag (the owner cuts a shared turn).
    /// False when no turn is running.
    pub fn cut_turn(&self) -> bool {
        match &*self.turn_cancel.lock().expect("turn cancel") {
            Some(c) => {
                c.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    /// Add a standing calendar entry (the owner's origin).
    pub fn add_calendar(&self, entry: &Entry) {
        self.core
            .emit("calendar.added", crate::calendar::added_body(entry));
        self.core.sync();
    }

    /// Record a channel opened by an outward client.
    pub fn open_channel(&self, session: &str, role: Role, channel: &str) {
        self.core.emit(
            "channel.opened",
            json!({"session": session, "role": role, "channel": channel}),
        );
        self.core.sync();
    }

    /// The now set: what `_rung/status` reports.
    pub fn status(&self) -> Value {
        let st = self.core.state();
        let cfg = self.cfg();
        let rung = st.governor.rung;
        let quota = cfg.governor.quota.as_ref().map(|q| {
            json!({"rpd": q.rpd, "rpm": q.rpm, "left_today": q.rpd.saturating_sub(st.governor.requests_today)})
        });
        let ladder: Vec<Value> = cfg
            .ladder
            .iter()
            .enumerate()
            .map(|(i, m)| json!({"rung": i, "model": m, "available": !st.governor.unavailable.contains(&i)}))
            .collect();
        json!({
            "now": self.core.now(),
            "turn": st.turn,
            "boundary": st.boundary,
            "mode": st.kernel.mode_label(),
            "project": st.kernel.commitment().map(|c| c.project.clone()),
            "epoch": st.pack.epoch,
            "rung": rung,
            "model": self.rung_model(rung),
            "ladder": ladder,
            "quota": quota,
            "degraded": st.governor.degraded,
            "desk": {"backend": self.desk.backend(), "mode": self.desk.mode, "spent_today_usd": st.desk.spent_today},
            "pending": st.inbox.pending.len(),
            "in_flight": st.inbox.in_flight.len(),
            "stopping": self.core.stop.raised(),
        })
    }

    // ─── Waking ──────────────────────────────────────────────────────────

    fn wake(&self, r: &Recovered) {
        let core = &*self.core;
        let cfg_v = serde_json::to_value(self.cfg()).unwrap_or(Value::Null);
        let mut pre: Vec<(&'static str, Value)> = vec![(
            "host.start",
            json!({"pid": std::process::id(), "config": cfg_v, "desk": self.desk.backend(),
                   "desk_mode": self.desk.mode, "memory": self.memory.as_ref().map(|m| m.name().to_string())}),
        )];
        let now = core.now();
        let first = r.lines == 0;
        let (epoch, rung) = {
            let st = core.state();
            (st.pack.epoch + 1, st.governor.rung)
        };
        let mut l1 = render::epoch_line(epoch, now, &self.rung_model(rung), rung);
        let mut gap = None;
        if first {
            for (id, title, why) in &self.cfg().seed_projects {
                pre.push((
                    "project.added",
                    json!({"id": id, "title": title, "why": why, "status": "seed", "turn": 0}),
                ));
            }
            for e in &self.seed_calendar {
                pre.push(("calendar.added", crate::calendar::added_body(e)));
            }
        } else {
            let last = r.last_at.unwrap_or(now);
            let (requeue, last_turn, mode) = {
                let st = core.state();
                (st.inbox.in_flight_ids(), st.turn, st.kernel.mode_label())
            };
            pre.push((
                "recovered",
                json!({"gap_ms": now - last, "last_at": last, "torn_bytes": r.torn_bytes,
                       "requeued": requeue, "mode": mode, "last_turn": last_turn}),
            ));
            if !requeue.is_empty() {
                pre.push((
                    "stimulus.requeued",
                    json!({"ids": requeue, "why": "interrupted_by_restart"}),
                ));
            }
            // A wait in progress when the host died is over.
            if core.state().governor.degraded.is_some() {
                pre.push((
                    "degraded.ended",
                    json!({"class": "interrupted", "waited_ms": 0}),
                ));
            }
            *self.down_since.lock().expect("down") = Some(last);
            l1.push('\n');
            l1.push_str(&render::recovered_line(last, now, last_turn, requeue.len()));
            gap = Some(now - last);
        }
        // The waking lines are one write: a kill cannot land between them.
        self.rollover(
            if first { "start" } else { "wake" },
            &[],
            &By::Rule(crate::desk::Why::NothingToAsk),
            Some(l1),
            gap,
            pre,
        );
        core.sync();
        core.notifier.ready();
    }

    fn halt(&self, why: Why) -> Next {
        let core = &*self.core;
        core.emit("halted", json!({"why": why}));
        core.sync();
        core.notifier.stopping();
        Next::Halt(why)
    }

    // ─── One boundary ────────────────────────────────────────────────────

    fn desk_deadline(&self) -> Instant {
        *self.desk_deadline.lock().expect("deadline")
    }

    fn step(&self, edge: Edge) -> Next {
        let started = Instant::now();
        // Every ask at this boundary shares one budget.
        *self.desk_deadline.lock().expect("deadline") = started + self.desk.timeout;
        let core = self.core.clone();
        let now = core.now();
        let turn_next = core.state().turn + 1;
        if self.limits.max_turns.is_some_and(|m| turn_next > m)
            || self.limits.until.is_some_and(|u| now >= u)
        {
            core.stop.request(Why::Stopped { by: "limit".into() });
        }
        if let Some(why) = core.stop.check() {
            return self.halt(why);
        }
        core.notifier.alive();
        let n = edge.n;
        {
            let (pending, mode) = {
                let st = core.state();
                (st.inbox.pending.len(), mode_value(&st))
            };
            core.emit(
                "boundary",
                json!({"n": n, "mode": mode, "pending": pending}),
            );
        }
        self.poll_sources();
        if let Some(why) = core.stop.check() {
            return self.halt(why);
        }
        self.fire_calendar();
        self.settle();
        self.list_ladder();
        self.probe_up();
        let (admit, inject, tools, kind) = self.decide(n, turn_next);
        // The governor: a world-imposed wait, never rest.
        let wait = {
            let st = core.state();
            governor::must_wait(&st.governor, &self.cfg().governor, kind, core.now())
        };
        if let Some(w) = wait {
            self.wait(w);
            return Next::Again(Edge::mint(n + 1));
        }
        let (now_items, digests) = self.admit(edge, &admit, turn_next);
        let note_line = self.consolidate_and_pack(n, turn_next, kind);
        self.turn(
            n, turn_next, kind, now_items, digests, inject, tools, note_line, started,
        );
        Next::Again(Edge::mint(n + 1))
    }

    fn poll_sources(&self) {
        let core = &*self.core;
        let now = core.now();
        let mut sources = self.sources.lock().expect("sources");
        for src in sources.iter_mut() {
            let seen = core.state().inbox.seen.clone();
            for item in src.poll(now, &seen) {
                self.accept(&item);
                src.accepted(&item);
            }
        }
    }

    /// Record an item, durable before anything else.
    fn accept(&self, item: &Item) {
        let core = &*self.core;
        // An id already accepted is never recorded twice: a second line
        // would overwrite the pending or in-flight copy and break
        // one-disposal-per-item.
        if core.state().inbox.seen.contains(&item.id) {
            return;
        }
        core.emit("stimulus.accepted", crate::inbox::accepted_body(item));
        core.sync();
        if let (Some(c), Role::Owner) = (&item.control, item.role) {
            let turn = core.state().turn;
            core.emit(
                "stimulus.disposed",
                json!({"id": item.id, "disposition": "control", "turn": turn}),
            );
            match c.as_str() {
                "stop" => core.stop.request(Why::Stopped { by: "owner".into() }),
                "release" => {
                    let _ = kernel::owner_release(core, &item.text);
                }
                _ => {}
            }
        }
    }

    fn fire_calendar(&self) {
        let core = &*self.core;
        let now = core.now();
        let down = self.down_since.lock().expect("down").take();
        let due = core.state().calendar.due(now, down);
        for f in due {
            if !f.fire {
                core.emit("calendar.skipped", json!({"id": f.id, "due": f.due}));
                continue;
            }
            let item_id = format!("cal-{}-{}", f.id, f.due);
            core.emit(
                "calendar.fired",
                json!({"id": f.id, "due": f.due, "late_by_ms": f.late_by_ms, "missed": f.missed,
                       "firm": f.firm, "item_id": item_id}),
            );
            let item = Item {
                id: item_id,
                kind: ItemKind::Calendar,
                role: Role::Host,
                channel: "calendar".into(),
                at: now,
                due: Some(f.due),
                urgency: None,
                firm: f.firm,
                text: f.text,
                fact: None,
                control: None,
            };
            self.accept(&item);
        }
    }

    fn settle(&self) {
        let core = &*self.core;
        loop {
            let s = {
                let st = core.state();
                st.registers
                    .settle(core.now(), self.judge.as_deref())
                    .into_iter()
                    .next()
            };
            let Some(s) = s else { break };
            let (id, state, surprise) = (s.id().to_string(), s.state().to_string(), s.surprise());
            core.emit_sealed(s);
            if state != "void" {
                let item = Item {
                    id: format!("exp-{id}"),
                    kind: ItemKind::Expectation,
                    role: Role::Host,
                    channel: "expectations".into(),
                    at: core.now(),
                    due: None,
                    urgency: None,
                    firm: false,
                    text: format!(
                        "expectation {id} {state} (surprise {})",
                        surprise.map(|x| x.to_string()).unwrap_or_default()
                    ),
                    fact: None,
                    control: None,
                };
                self.accept(&item);
            }
        }
    }

    /// List the router's models when due (at the first boundary, then
    /// every six hours; 15 minutes after a failure) and switch off a rung
    /// the listing took away.
    fn list_ladder(&self) {
        let Some(lister) = &self.lister else { return };
        let core = &*self.core;
        let now = core.now();
        let rungs = self.cfg().ladder.len();
        let previous: Vec<bool> = {
            let st = core.state();
            if st.governor.next_listing_at.is_some_and(|t| now < t) {
                return;
            }
            if st.governor.next_listing_at.is_none() {
                Vec::new()
            } else {
                (0..rungs)
                    .map(|r| !st.governor.unavailable.contains(&r))
                    .collect()
            }
        };
        let body = crate::ladder::list(lister.as_ref(), &self.cfg().ladder, &previous, now);
        let line = core.emit("ladder.listed", body);
        let to = {
            let st = core.state();
            governor::after_listing(&st.governor, rungs, core.now())
                .map(|to| (st.governor.rung, to))
        };
        if let Some((from, to)) = to {
            let gone = line.get("rungs")[from]["why"]
                .as_str()
                .unwrap_or("unavailable")
                .to_string();
            let direction = if to > from { "down" } else { "up" };
            self.switch(from, to, direction, &format!("listing: {gone}"), 0);
        }
    }

    fn probe_up(&self) {
        let core = &*self.core;
        let up = {
            let st = core.state();
            governor::probe_up(&st.governor, core.now()).map(|to| (st.governor.rung, to))
        };
        if let Some((from, to)) = up {
            self.switch(from, to, "up", "probe: the rung above has cooled down", 0);
        }
        let pending = self.switch_pending.lock().expect("switch").take();
        if pending.is_some() {
            self.rollover(
                "model_switch",
                &[],
                &By::Rule(crate::desk::Why::NothingToAsk),
                None,
                None,
                Vec::new(),
            );
        }
    }

    fn switch(&self, from: usize, to: usize, direction: &str, why: &str, cooldown_ms: Millis) {
        self.core.emit(
            "model.switch",
            json!({"from": self.rung_model(from), "to": self.rung_model(to), "rung_from": from,
                   "rung_to": to, "direction": direction, "why": why, "cooldown_ms": cooldown_ms}),
        );
        *self.switch_pending.lock().expect("switch") = Some(direction.into());
    }

    /// The boundary ask: Admit, Inject and Tools in one.
    fn decide(&self, n: u64, turn: u64) -> (AdmitChoice, InjectChoice, Vec<String>, TurnKind) {
        let core = &*self.core;
        let k = &self.desk.knobs;
        let now = core.now();
        let mem = self
            .memory
            .as_ref()
            .map(|m| m.name().to_string())
            .unwrap_or_else(|| "off".into());
        let (ain, actx, iin, tin, tctx, spent, first_committed) = {
            let st = core.state();
            let (ain, actx) = crate::desk::admit::build(&st, now, k);
            let iin = crate::desk::inject::build(&st, now, &mem);
            // The ask goes out before Admit is composed: the Tools state
            // carries the kind the Admit rule would give.
            let preview = Admit::guard(&ain, Admit::rule(&ain, k, &actx), k, &actx);
            let pkind = st.kernel.next(!preview.now().is_empty());
            let (tin, tctx) = crate::desk::tools::build(&st, &self.cfg().ceiling, pkind, k);
            let first = st
                .kernel
                .commitment()
                .is_some_and(|c| c.since_turn + 1 == turn);
            (ain, actx, iin, tin, tctx, st.desk.spent_today, first)
        };
        let mut qs = Admit::questions(&ain);
        qs.extend(Inject::questions(&iin));
        qs.extend(Tools::questions(&tin));
        let state = json!({"admit": ain, "inject": iin, "tools": tin});
        let asked = self.desk.ask_until(state, qs, spent, self.desk_deadline());
        core.emit(
            "desk.ask",
            asked.line(n, &["admit", "inject", "tools"], self.desk.backend()),
        );
        let a = self.desk.decide::<Admit>(&ain, &actx, &asked, n, turn);
        core.emit("decision.admit", a.line().clone());
        let kind = core.state().kernel.next(!a.choice().now().is_empty());
        let ictx = InjectCtx {
            kind,
            first_committed: first_committed && kind == TurnKind::Committed,
            turn,
        };
        let i = self.desk.decide::<Inject>(&iin, &ictx, &asked, n, turn);
        core.emit("decision.inject", i.line().clone());
        let tctx = crate::desk::tools::ToolsCtx { kind, ..tctx };
        let t = self.desk.decide::<Tools>(&tin, &tctx, &asked, n, turn);
        core.emit("decision.tools", t.line().clone());
        (
            a.into_choice(),
            i.into_choice(),
            t.into_choice().enabled,
            kind,
        )
    }

    /// Wait out a world-imposed interval.
    fn wait(&self, w: Wait) {
        let core = &*self.core;
        let from_state = core.state().governor.degraded.as_ref() == Some(&w);
        if !from_state {
            core.emit(
                "degraded",
                json!({"class": w.class, "until": w.until, "why": w.why, "owner_wakes": w.owner_wakes}),
            );
        }
        let start = core.now();
        let next = || {
            let src = self.sources.lock().expect("sources");
            let a = src.iter().filter_map(|s| s.next_at()).min();
            let b = core.state().calendar.next_due();
            match (a, b) {
                (Some(x), Some(y)) => Some(x.min(y)),
                (x, y) => x.or(y),
            }
        };
        let mut wake = |_now: Millis| {
            core.notifier.alive();
            if core.stop.raised() {
                return true;
            }
            self.poll_sources();
            if core.stop.raised() {
                return true;
            }
            w.owner_wakes
                && core
                    .state()
                    .inbox
                    .pending
                    .values()
                    .any(|p| p.item.role == Role::Owner)
        };
        let _ = core.clock.wait(w.until, &next, &mut wake);
        core.emit(
            "degraded.ended",
            json!({"class": w.class, "waited_ms": core.now() - start}),
        );
    }

    /// Admit the batch: consumes the boundary's edge.
    fn admit(&self, edge: Edge, a: &AdmitChoice, turn: u64) -> (Vec<String>, Vec<String>) {
        let (now, digests) = (a.now(), a.digests());
        if !now.is_empty() || !digests.is_empty() {
            self.core.emit(
                "stimulus.admitted",
                json!({"turn": turn, "boundary": edge.n, "ids": now, "digests": digests}),
            );
        }
        (now, digests)
    }

    fn pack_input(&self, at_break: bool) -> PackInput {
        let core = &*self.core;
        let k = &self.desk.knobs;
        let st = core.state();
        let pack = self.pack.lock().expect("pack");
        let commit_turns = st
            .kernel
            .commitment()
            .map(|c| (c.since_turn, c.last_progress_turn));
        let exp_turns: Vec<u64> = st
            .registers
            .expectations
            .values()
            .filter(|e| e.state == ExpState::Open)
            .map(|e| e.turn)
            .collect();
        let is_ref = |first: u64, last: u64| {
            let c = commit_turns
                .is_some_and(|(a, b)| (first..=last).contains(&a) || (first..=last).contains(&b));
            let e = exp_turns.iter().any(|t| (first..=last).contains(t));
            (c, e)
        };
        let mut input = PackInput {
            epoch_tokens: pack.tokens(),
            header_reserve: pack.header_reserve(),
            budget: pack.budget,
            turns_in_epoch: pack.turns_in_epoch(),
            at_break,
            mode: st.kernel.mode_label(),
            cache_read_ratio_last10: canon::fixed(st.desk.cache_ratio()),
            copy_flag: st.kernel.copy_streak >= COPY_LOOP_TURNS,
            segments: Vec::new(),
            last_turn: pack.spans().last().map(|s| s.turn).unwrap_or(0),
        };
        // Segments only matter when the gate opens.
        if input.gate_open(k) {
            input.segments = pack.segments(k.max_segments, &is_ref);
        }
        input
    }

    /// The rollover ask (when the pack's gate opens) and Consolidate.
    /// Returns whether the next header offers a note update.
    fn consolidate_and_pack(&self, n: u64, turn: u64, _kind: TurnKind) -> bool {
        let core = &*self.core;
        let k = &self.desk.knobs;
        let at_break = core.state().kernel.at_break();
        let pin = self.pack_input(at_break);
        let gate = pin.gate_open(k);
        let periodic = turn.is_multiple_of(k.consolidate_every);
        if !gate && !periodic {
            return false;
        }
        let imminent = pin.copy_flag || pin.fraction() >= k.pack_rule_break;
        let (cin, cands) = {
            let st = core.state();
            crate::desk::consolidate::build(&st, k, imminent)
        };
        let mut qs = Consolidate::questions(&cin);
        if gate {
            qs.extend(PackFamily::questions(&pin));
        }
        let spent = core.state().desk.spent_today;
        let state = if gate {
            json!({"pack": pin, "consolidate": cin})
        } else {
            json!({"consolidate": cin})
        };
        let asked = self.desk.ask_until(state, qs, spent, self.desk_deadline());
        let families: &[&str] = if gate {
            &["pack", "consolidate"]
        } else {
            &["consolidate"]
        };
        core.emit("desk.ask", asked.line(n, families, self.desk.backend()));
        let p = gate.then(|| {
            let d = self.desk.decide::<PackFamily>(&pin, &(), &asked, n, turn);
            core.emit("decision.pack", d.line().clone());
            d
        });
        let c = self.desk.decide::<Consolidate>(&cin, &(), &asked, n, turn);
        core.emit("decision.consolidate", c.line().clone());
        let chosen: Vec<_> = cands
            .into_iter()
            .filter(|x| c.choice().retain.contains(&x.id))
            .collect();
        let rolling = p
            .as_ref()
            .is_some_and(|p| p.choice().action == Action::Rollover);
        if rolling {
            // Retain before anything is evicted.
            self.retain(turn, chosen);
        } else {
            // Nothing is evicted: retain after the turn, off the boundary's
            // path (the candidates are already in the record).
            *self.deferred_retain.lock().expect("retain") = chosen;
        }
        if let Some(p) = p
            && p.choice().action == Action::Rollover
        {
            let cause = p.choice().cause.clone();
            if cause == "copy_loop" {
                let streak = core.state().kernel.copy_streak;
                core.emit("copy.loop", json!({"turn": turn, "streak": streak}));
                self.outbox(
                    turn,
                    &self.cfg().owner_channel.clone(),
                    "The repetition guard fired: the agent's last turns repeated earlier text. The host rolled its context over (its topic is its own).",
                    "host:copy_loop",
                );
            }
            let keep = p.choice().keep.clone();
            self.rollover(&cause, &keep, p.by(), None, None, Vec::new());
        }
        c.choice().note_line
    }

    fn retain(&self, turn: u64, chosen: Vec<crate::desk::Candidate>) {
        let Some(m) = &self.memory else { return };
        for cand in chosen {
            let attrs = BTreeMap::from([
                ("source".to_string(), format!("host:{}", cand.kind)),
                ("turn".to_string(), cand.turn.to_string()),
            ]);
            let report = m.retain(&cand.text, attrs);
            self.core.emit(
                "memory.retain",
                json!({"turn": turn, "candidate": cand.id, "candidate_kind": cand.kind, "report": report}),
            );
        }
    }

    pub(crate) fn outbox(&self, turn: u64, channel: &str, text: &str, source: &str) {
        let line = self.core.emit(
            "outbox.queued",
            json!({"turn": turn, "channel": channel, "text": text, "source": source}),
        );
        write_outbox(&self.core, &line);
    }

    /// Start a new epoch. The record is fsynced first: log before forget.
    fn rollover(
        &self,
        cause: &str,
        keep: &[String],
        by: &By,
        l1: Option<String>,
        gap: Option<Millis>,
        pre: Vec<(&'static str, Value)>,
    ) {
        let core = &*self.core;
        // Log before forget: what the epoch held is on disk first.
        if !matches!(cause, "start" | "wake") {
            core.sync();
        }
        let k = &self.desk.knobs;
        let mut pack = self.pack.lock().expect("pack");
        let kept = pack.segment_text(k.max_segments, keep);
        let tokens_before = pack.tokens();
        let st = core.state();
        let to = st.pack.epoch + 1;
        let rung = st.governor.rung;
        let l1 =
            l1.unwrap_or_else(|| render::epoch_line(to, core.now(), &self.rung_model(rung), rung));
        let note = st.registers.note.as_ref().map(|(_, t)| t.as_str());
        // The outline of the epoch that is ending.
        let outline = st.pack.outline.clone();
        let slow = render::slow(&l1, note, &st.registers, &st.kernel, &outline, &kept);
        let first_turn = st.turn + 1;
        let from = st.pack.epoch;
        drop(st);
        pack.rollover(to, slow);
        let slow_tokens = pack.slow_tokens();
        drop(pack);
        let mut body = json!({"from": from, "to": to, "cause": cause, "by": by.to_value(), "kept": keep,
                              "tokens_before": tokens_before, "slow_tokens": slow_tokens,
                              "first_turn": first_turn, "l1": l1});
        if let Some(g) = gap {
            body["gap_ms"] = g.into();
        }
        let mut lines = pre;
        lines.push(("epoch.rollover", body));
        core.emit_many(lines);
        let validity = if cause == "model_switch" {
            Validity::Nothing
        } else {
            Validity::Stable
        };
        *self.reset.lock().expect("reset") = Some((
            if cause == "model_switch" {
                "model_switch".into()
            } else {
                "rollover".into()
            },
            validity,
        ));
    }

    #[allow(clippy::too_many_arguments)]
    fn turn(
        &self,
        n: u64,
        turn: u64,
        kind: TurnKind,
        now_ids: Vec<String>,
        digest_ids: Vec<String>,
        inject: InjectChoice,
        enabled: Vec<String>,
        note_line: bool,
        started: Instant,
    ) {
        let core = self.core.clone();
        let cfg = self.cfg();
        let now = core.now();
        // What the header shows.
        let (admitted, digests, header, recall_on) = {
            let recall = self.recall(turn, &inject, &now_ids);
            let st = core.state();
            let item = |id: &String| st.inbox.in_flight.get(id).map(|(_, p)| p.item.clone());
            let admitted: Vec<Item> = now_ids.iter().filter_map(item).collect();
            let digests: Vec<Item> = digest_ids.iter().filter_map(item).collect();
            let quota = cfg
                .governor
                .quota
                .as_ref()
                .map(|q| (q.rpd.saturating_sub(st.governor.requests_today), q.rpd));
            let rung = st.governor.rung;
            let model = self.rung_model(rung);
            let mut notices = Vec::new();
            if let Some(w) = &st.governor.degraded {
                notices.push(format!(
                    "the last wait was the world's: {} ({})",
                    w.class, w.why
                ));
            }
            let resumed = kind == TurnKind::Committed
                && st
                    .pack
                    .outline
                    .last()
                    .is_some_and(|o| o.contains(" responding "));
            let exp_digest = inject.expectations.then(|| {
                let due: Vec<String> = st
                    .registers
                    .expectations
                    .iter()
                    .filter(|(_, e)| e.state == ExpState::Open && e.due <= now + crate::clock::HOUR)
                    .take(5)
                    .map(|(id, e)| format!("{id} due {}", crate::clock::iso(e.due)))
                    .collect();
                format!(
                    "expectations due within 1h: {}",
                    if due.is_empty() {
                        "none".into()
                    } else {
                        due.join(", ")
                    }
                )
            });
            let cal_digest = inject.calendar.then(|| {
                let ahead: Vec<String> = st
                    .calendar
                    .entries
                    .values()
                    .filter(|s| {
                        s.next_due
                            .is_some_and(|d| d >= now && d <= now + 2 * crate::clock::HOUR)
                    })
                    .take(5)
                    .map(|s| {
                        format!(
                            "{} at {}",
                            s.entry.id,
                            crate::clock::iso(s.next_due.unwrap_or(0))
                        )
                    })
                    .collect();
                format!(
                    "calendar within 2h: {}",
                    if ahead.is_empty() {
                        "none".into()
                    } else {
                        ahead.join(", ")
                    }
                )
            });
            let h = render::HeaderCtx {
                turn,
                kind,
                now,
                since_external: st.inbox.last_external_at,
                quota,
                model: &model,
                rung,
                enabled: &enabled,
                admitted: admitted.iter().collect(),
                digests: digests.iter().collect(),
                commitment: st.kernel.commitment(),
                turns_since_progress: st.kernel.turns_since_progress,
                resumed,
                material: (kind == TurnKind::Free)
                    .then(|| render::material(&st.registers, &st.kernel, now)),
                recall: None,
                expectations: exp_digest,
                calendar: cal_digest,
                note_line,
                notices,
            };
            let base = render::header(&h);
            let full = match &recall {
                Some(block) => format!("{base}\n{block}"),
                None => base.clone(),
            };
            (admitted, digests, (base, full), recall.is_some())
        };
        let (header_logged, header_full) = header;
        let (rung, model, epoch, pack_tokens_before, thread, header_tokens) = {
            let st = core.state();
            let mut pack = self.pack.lock().expect("pack");
            let before = pack.tokens();
            pack.begin_turn(turn, kind.as_str(), header_full.clone());
            let ht = pack.tokens() - before;
            (
                st.governor.rung,
                self.rung_model(st.governor.rung),
                pack.epoch,
                before,
                pack.thread(),
                ht,
            )
        };
        let deadline = now + cfg.turn_bound_ms;
        let enabled_set: BTreeSet<String> = enabled.iter().cloned().collect();
        let tools = Arc::new(HostTools::new(
            core.clone(),
            turn,
            enabled_set,
            deadline,
            self.memory.clone(),
            self.web.clone(),
            cfg.workspace.clone(),
        ));
        let (mode, project) = {
            let st = core.state();
            (
                st.kernel.mode_label(),
                st.kernel.commitment().map(|c| c.project.clone()),
            )
        };
        let pack_tokens = pack_tokens_before + header_tokens;
        core.emit(
            "turn.started",
            json!({"turn": turn, "boundary": n, "turn_kind": kind.as_str(), "mode": mode, "project": project,
                   "model": model, "rung": rung, "epoch": epoch, "pack_tokens": pack_tokens,
                   "header_tokens": header_tokens, "enabled": enabled,
                   "wall_boundary_us": started.elapsed().as_micros() as u64}),
        );
        let cancel = Arc::new(AtomicBool::new(false));
        *self.turn_cancel.lock().expect("turn cancel") = Some(cancel.clone());
        let watcher =
            (!core.clock.is_sim()).then(|| spawn_watcher(core.clone(), cancel.clone(), deadline));
        let session = format!("epoch-{epoch}");
        let out = self.engine.turn(TurnRequest {
            turn,
            thread,
            tools,
            ctl: TurnCtl {
                cancel: Some(cancel.clone()),
                ..TurnCtl::default()
            },
            model: model.clone(),
            session,
            deadline,
            step_cap: cfg.governor.step_cap,
        });
        *self.turn_cancel.lock().expect("turn cancel") = None;
        if let Some((done, h)) = watcher {
            done.store(true, Ordering::SeqCst);
            let _ = h.join();
        }
        let post = Instant::now();
        self.finish(
            turn,
            kind,
            rung,
            &model,
            epoch,
            pack_tokens_before,
            out,
            header_logged,
            recall_on,
            now,
            admitted,
            digests,
            post,
        );
    }

    fn recall(&self, turn: u64, inject: &InjectChoice, now_ids: &[String]) -> Option<String> {
        let m = self.memory.as_ref()?;
        if !inject.recall {
            return None;
        }
        let core = &*self.core;
        let (prompt, context) = {
            let st = core.state();
            let prompt = match inject.cue {
                Cue::Stimulus => now_ids
                    .iter()
                    .filter_map(|id| st.inbox.in_flight.get(id).map(|(_, p)| p.item.text.clone()))
                    .collect::<Vec<_>>()
                    .join("\n"),
                Cue::Commitment => st
                    .kernel
                    .commitment()
                    .map(|c| format!("{} {}", c.title, c.next_step.clone().unwrap_or_default()))
                    .unwrap_or_default(),
                Cue::Note => st
                    .registers
                    .note
                    .as_ref()
                    .map(|(_, t)| t.clone())
                    .unwrap_or_default(),
                Cue::None => String::new(),
            };
            let context = st.kernel.recent.iter().map(|(_, t)| t.clone()).collect();
            (prompt, context)
        };
        if prompt.trim().is_empty() {
            return None;
        }
        let (report, block) = m.recall(&prompt, context);
        core.emit(
            "memory.recall",
            json!({"turn": turn, "cue": inject.cue, "report": report}),
        );
        block
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(
        &self,
        turn: u64,
        kind: TurnKind,
        rung: usize,
        model: &str,
        epoch: u64,
        before_header: usize,
        out: EngineTurn,
        header: String,
        recall_on: bool,
        started_at: Millis,
        admitted: Vec<Item>,
        digests: Vec<Item>,
        post: Instant,
    ) {
        let core = self.core.clone();
        let cfg = self.cfg();
        let (s_hash, l_hash, stable_tokens) = {
            let p = self.pack.lock().expect("pack");
            (
                p.s_hash().to_string(),
                p.l_hash().to_string(),
                p.stable_tokens() as u64,
            )
        };
        // Every model call, with the cache expectation.
        let mut reset = self.reset.lock().expect("reset").take();
        // The provider caches the previous request of this epoch and model.
        let mut prev_prompt: Option<u64> = {
            let lc = self.last_call.lock().expect("last call");
            lc.as_ref()
                .filter(|(e, m, _, _)| *e == epoch && m == model)
                .map(|x| x.2)
        };
        let mut last_at = self
            .last_call
            .lock()
            .expect("last call")
            .as_ref()
            .map(|x| x.3);
        let mut cost = 0.0;
        let (mut prompt_sum, mut cached_sum) = (0u64, 0u64);
        for (i, c) in out.calls.iter().enumerate() {
            let u = c.usage.usage.clone().unwrap_or_default();
            let (break_cause, expected) = match reset.take() {
                Some((cause, Validity::Stable)) => (Some(cause), stable_tokens),
                Some((cause, Validity::Nothing)) => (Some(cause), 0),
                None => (None, prev_prompt.unwrap_or(stable_tokens)),
            };
            let at = core.now();
            let mut body = json!({
                "turn": turn, "call": i + 1, "epoch": epoch, "rung": rung,
                "model_requested": model, "model_served": c.usage.model, "provider": c.provider,
                "prompt_tokens": u.input_tokens, "cached_tokens": u.cache_read_input_tokens,
                "cache_write_tokens": u.cache_creation_input_tokens,
                "completion_tokens": u.output_tokens, "reasoning_tokens": u.thinking_tokens,
                "cost_usd": canon::fixed(u.cost_usd.unwrap_or(0.0)), "latency_ms": c.latency_ms,
                "ttft_ms": u.ttft_ms.map(|x| x as i64), "turn_kind": kind.as_str(),
                "prefix": {"s_hash": s_hash, "l_hash": l_hash, "log_len_bytes": before_header,
                           "expected_cached_tokens": expected},
            });
            if i > 0 {
                body["prefix"]["log_len_bytes"] = Value::Null;
            }
            core.emit("llm.call", body);
            cost += u.cost_usd.unwrap_or(0.0);
            prompt_sum += u64::from(u.input_tokens);
            cached_sum += u64::from(u.cache_read_input_tokens);
            if let Some(cause) = break_cause {
                core.emit(
                    "cache.break",
                    json!({"turn": turn, "call": i + 1, "cause": cause}),
                );
            } else if expected > 0 && (u.cache_read_input_tokens as f64) < 0.5 * expected as f64 {
                let idle = last_at.map(|t| at - t).unwrap_or(0);
                let cause = if idle > cfg.cache_ttl_ms {
                    "ttl"
                } else {
                    "provider"
                };
                core.emit(
                    "cache.cold",
                    json!({"turn": turn, "call": i + 1, "cause": cause, "idle_ms": idle}),
                );
            }
            prev_prompt = Some(u64::from(u.input_tokens));
            last_at = Some(at);
        }
        if let Some(p) = prev_prompt.filter(|_| !out.calls.is_empty()) {
            *self.last_call.lock().expect("last call") =
                Some((epoch, model.to_string(), p, core.now()));
        }
        let deferred = std::mem::take(&mut *self.deferred_retain.lock().expect("retain"));
        self.retain(turn, deferred);
        // The turn's messages, verbatim, before the pack moves on.
        core.emit(
            "turn.log",
            json!({"turn": turn, "header": header, "recall": recall_on,
                   "messages": serde_json::to_value(&out.messages).unwrap_or(Value::Null)}),
        );
        self.pack
            .lock()
            .expect("pack")
            .end_turn(out.messages.clone());
        // The batch.
        let ids: Vec<String> = admitted
            .iter()
            .chain(digests.iter())
            .map(|i| i.id.clone())
            .collect();
        let stopped = core.stop.raised();
        let status = match out.ended {
            Ended::Completed => "completed",
            Ended::Failed => "failed",
            Ended::Cancelled if stopped => "cancelled",
            Ended::Cancelled | Ended::Bounded => "bounded",
        };
        match out.ended {
            Ended::Completed | Ended::Bounded => {
                for i in &admitted {
                    core.emit(
                        "stimulus.disposed",
                        json!({"id": i.id, "disposition": "answered", "turn": turn}),
                    );
                }
                for i in &digests {
                    core.emit(
                        "stimulus.disposed",
                        json!({"id": i.id, "disposition": "digested", "turn": turn}),
                    );
                }
            }
            Ended::Cancelled if !stopped => {
                for i in &admitted {
                    core.emit(
                        "stimulus.disposed",
                        json!({"id": i.id, "disposition": "answered", "turn": turn}),
                    );
                }
                for i in &digests {
                    core.emit(
                        "stimulus.disposed",
                        json!({"id": i.id, "disposition": "digested", "turn": turn}),
                    );
                }
            }
            _ => {
                if !ids.is_empty() {
                    core.emit("stimulus.requeued", json!({"ids": ids, "why": status}));
                }
            }
        }
        // The copy guard on the final answer.
        let final_text = out.final_text.clone().unwrap_or_default();
        let ans_sim = if final_text.is_empty() {
            0.0
        } else {
            core.state().kernel.similarity(&final_text)
        };
        if ans_sim > COPY_SIMILARITY {
            core.emit(
                "copy.guard",
                json!({"turn": turn, "similarity": canon::fixed(ans_sim), "of": "answer"}),
            );
        }
        let copied = core.state().kernel.last_guard_turn == Some(turn);
        let failure = out.failure.clone();
        let elapsed = core.now() - started_at;
        let mut body = json!({
            "turn": turn, "turn_kind": kind.as_str(), "status": status, "calls": out.calls.len(),
            "elapsed_ms": elapsed, "rung": rung, "model": model,
            "final_text": final_text.chars().take(2_000).collect::<String>(),
            "copied": copied,
            "cost": {"calls": out.calls.len(), "prompt": prompt_sum, "cached": cached_sum,
                     "cost_usd": canon::fixed(cost), "jev_usd": 0.0},
            "wall_post_us": post.elapsed().as_micros() as u64,
        });
        if let Some(f) = &failure {
            body["failure"] = f.to_value();
        }
        if out.rewritten {
            body["rewritten"] = true.into();
        }
        if let Some(cap) = cfg.governor.spend_cap_usd_day {
            let spent = core.state().governor.paid_spent_today;
            if spent >= cap {
                core.stop.request(Why::SpendCap {
                    spent_usd: spent,
                    cap_usd: cap,
                });
            }
        }
        core.emit_hashed("turn.ended", body);
        // A failure: the governor's plan, once the turn is on record.
        if let Some(f) = &failure {
            let (plan, cooldown) = {
                let st = core.state();
                let jitter = jitter(cfg.seed, turn);
                let plan = governor::on_failure(
                    &st.governor,
                    &cfg.governor,
                    f,
                    cfg.ladder.len(),
                    core.now(),
                    jitter,
                );
                let cd = plan
                    .step_down
                    .map(|(from, _)| governor::cooldown_for(&st.governor, &cfg.governor, from));
                (plan, cd)
            };
            if let Some(w) = &plan.wait {
                core.emit(
                    "degraded",
                    json!({"class": w.class, "until": w.until, "why": w.why, "owner_wakes": w.owner_wakes,
                           "incident_start": plan.incident_start, "failure": f.to_value()}),
                );
            }
            if plan.incident_start {
                self.outbox(
                    turn,
                    &cfg.owner_channel.clone(),
                    &format!("The host is blocked: the provider refused its credentials ({}). It keeps probing every 15 minutes and will not exit.", f.class_name()),
                    "host:blocked",
                );
            }
            if let Some((from, to)) = plan.step_down {
                self.switch(
                    from,
                    to,
                    "down",
                    &format!("provider {}", f.class_name()),
                    cooldown.unwrap_or(0),
                );
            }
        }
        core.sync();
    }
}

/// The deterministic jitter for a backoff: a hash of the seed and turn.
fn jitter(seed: u64, turn: u64) -> f64 {
    let h = canon::hash(format!("{seed}:{turn}").as_bytes());
    let x = u64::from_str_radix(&h[..12], 16).unwrap_or(0);
    (x % 10_000) as f64 / 10_000.0
}

/// Raise `cancel` when the stop authority is raised or the deadline passes.
fn spawn_watcher(
    core: Arc<Core>,
    cancel: Arc<AtomicBool>,
    deadline: Millis,
) -> (Arc<AtomicBool>, std::thread::JoinHandle<()>) {
    let done = Arc::new(AtomicBool::new(false));
    let d2 = done.clone();
    let h = std::thread::spawn(move || {
        while !d2.load(Ordering::SeqCst) {
            if core.stop.raised() || core.now() >= deadline {
                cancel.store(true, Ordering::SeqCst);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    (done, h)
}

/// Write a queued outbox line as a `*.msg` file, when an outbox dir is set.
pub(crate) fn write_outbox(core: &Core, line: &Line) {
    if let Some(dir) = &core.config.outbox_dir {
        let _ = std::fs::create_dir_all(dir);
        // Write then rename: a reader of `*.msg` never sees a partial file.
        let name = format!("{:010}", line.seq);
        let tmp = dir.join(format!("{name}.tmp"));
        if std::fs::write(&tmp, line.text()).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join(format!("{name}.msg")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(name: &str, outbox: Option<PathBuf>) -> (Arc<Host>, crate::sim::TempDir) {
        let guard = crate::sim::temp_dir_guard(name);
        let mut sc = crate::sim::Scenario::new(guard.path(), 1);
        sc.config.outbox_dir = outbox;
        let (h, _, _) = crate::sim::build(sc);
        (h, guard)
    }

    /// Poison `m`: a thread panics while it holds the lock.
    fn poison<T: Send>(m: &Mutex<T>) {
        std::thread::scope(|s| {
            let _ = s
                .spawn(|| {
                    let _g = m.lock();
                    panic!("a panic while the lock is held");
                })
                .join();
        });
        assert!(m.is_poisoned());
    }

    /// One panicked thread does not cascade: with every host lock poisoned,
    /// the outward calls and later boundaries run, as they do when the
    /// state lock (`Core::state`) is poisoned.
    #[test]
    fn a_poisoned_host_lock_does_not_cascade() {
        let guard = crate::sim::temp_dir_guard("poison");
        let mut sc = crate::sim::Scenario::new(guard.path(), 1);
        sc.max_turns = Some(3);
        let (h, rec, _) = crate::sim::build(sc);
        poison(&h.pack);
        poison(&h.sources);
        poison(&h.down_since);
        poison(&h.reset);
        poison(&h.switch_pending);
        poison(&h.last_call);
        poison(&h.desk_deadline);
        poison(&h.deferred_retain);
        poison(&h.turn_cancel);
        let _ = h.request_bytes();
        assert!(!h.cut_turn());
        let why = h.run(rec);
        assert!(matches!(why, Why::Stopped { .. }), "{why:?}");
        let turns = h
            .record_lines()
            .unwrap()
            .iter()
            .filter(|l| l.kind == "turn.ended")
            .count();
        assert_eq!(turns, 3);
    }

    #[test]
    fn a_duplicate_id_is_accepted_once() {
        let (h, _g) = host("dup-accept", None);
        h.accept(&Item::message("dup1", Role::Peer, "peer:x", 1, "hi"));
        h.accept(&Item::message("dup1", Role::Peer, "peer:x", 2, "again"));
        let n = h
            .record_lines()
            .unwrap()
            .iter()
            .filter(|l| l.kind == "stimulus.accepted")
            .count();
        assert_eq!(n, 1);
    }

    #[test]
    fn the_outbox_write_leaves_a_whole_msg_and_no_tmp() {
        let g = crate::sim::temp_dir_guard("outbox-atomic");
        let out = g.path().join("out");
        let (h, _g) = host("outbox-host", Some(out.clone()));
        let line = h.core.emit("note.written", json!({"text": "hello"}));
        write_outbox(&h.core, &line);
        let name = format!("{:010}", line.seq);
        let body = std::fs::read_to_string(out.join(format!("{name}.msg"))).unwrap();
        assert_eq!(Line::parse(&body).unwrap().seq, line.seq);
        let names: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
    }

    #[test]
    fn a_huge_until_s_saturates() {
        let (h, _g) = host("huge-until", None);
        let input = json!({"new": {"title": "t", "why": "w"}, "done_when": "d",
            "until_s": i64::MAX});
        assert!(kernel::tool_commit(&h.core, 1, &input).is_ok());
    }
}
