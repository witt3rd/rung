//! The host's acceptance gates: slice 1's, frozen before the first run of
//! the host, and slice 2's (below the slice-1 gates), frozen before the
//! first run of what they gate.
//!
//! Each gate is a pure function over what a run leaves behind: the record
//! (its [`Line`]s) and, where the record cannot hold the evidence, what the
//! test harness measured from outside (a captured provider request log,
//! stop timings, a second process's bytes). The thresholds are the
//! constants below, copied from the design's gate table; a gate that a run
//! cannot meet is reported as failed, never loosened.
//!
//! The record vocabulary these gates read is documented in
//! `docs/rung-host.md` ("The record"). Every evaluator returns a
//! [`GateResult`] with the measured numbers, so a test prints the evidence
//! whether it passes or not.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value, json};

use crate::record::Line;

// ─── Thresholds (the design's gate table) ────────────────────────────────────

/// G-a: idle wall time outside turns, excluding declared `degraded` waits.
pub const G_A_IDLE_MAX: f64 = 0.02;
/// G-a: boundary → next turn start, p99, ms.
pub const G_A_BOUNDARY_P99_MS: f64 = 50.0;
/// G-a: the run's simulated length.
pub const G_A_RUN_MS: i64 = 30 * 60 * 1000;
/// G-b: owner admission p95 may exceed the p95 turn by at most this, ms.
pub const G_B_ADMISSION_SLACK_MS: f64 = 100.0;
/// G-b / G-h: what a refused long tool call tells the agent.
pub const LONG_WORK_MESSAGE: &str =
    "too long for one turn; break it into steps, or commit to it as a project.";
/// G-c: scripted turns.
pub const G_C_TURNS: u64 = 2_000;
/// G-c: random register scores under which the free-time material must
/// keep one order.
pub const G_C_SCORE_PERMUTATIONS: usize = 100;
/// G-g: the backoff cap, ms.
pub const G_G_BACKOFF_CAP_MS: i64 = 15 * 60 * 1000;
/// G-h: a stop mid-turn may take the remaining mock call plus this, ms.
pub const G_H_TURN_SLACK_MS: u64 = 1_000;
/// G-h: a stop from any wait, ms.
pub const G_H_WAIT_MS: u64 = 1_000;
/// G-i: random `kill -9`s.
pub const G_I_KILLS: usize = 50;
/// G-j / G-l: scripted turns.
pub const G_J_TURNS: u64 = 10_000;
/// G-j: the rollover hard ceiling, as a fraction of the epoch budget.
pub const G_J_CEILING: f64 = 0.85;
/// G-j: linear growth: the largest 1,000-turn window of record bytes over
/// the smallest (first window excluded).
pub const G_J_GROWTH_RATIO_MAX: f64 = 1.5;
/// G-l: mock cache efficiency outside recorded breaks.
pub const G_L_EFFICIENCY_MIN: f64 = 0.98;
/// G-m: the most one boundary's decisions may cost, ms, when the decider
/// stalls for [`G_M_DELAY_MS`].
pub const G_M_BOUNDARY_COST_MS: f64 = 2_000.0;
pub const G_M_DELAY_MS: u64 = 3_000;
/// G-m: rollover soft floor, as a fraction of the epoch budget.
pub const G_M_SOFT_FLOOR: f64 = 0.40;
/// The decision families.
pub const FAMILIES: [&str; 5] = ["admit", "inject", "tools", "pack", "consolidate"];

// ─── Result ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct GateResult {
    pub id: &'static str,
    pub pass: bool,
    /// Every failed check, by name.
    pub failures: Vec<String>,
    /// What was measured.
    pub measured: Value,
}

impl GateResult {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            pass: true,
            failures: Vec::new(),
            measured: json!({}),
        }
    }

    fn check(&mut self, ok: bool, what: impl Into<String>) {
        if !ok {
            self.pass = false;
            self.failures.push(what.into());
        }
    }

    fn put(&mut self, k: &str, v: impl Into<Value>) {
        self.measured[k] = v.into();
    }

    /// One line for a report.
    pub fn summary(&self) -> String {
        format!(
            "{} {} {}",
            self.id,
            if self.pass { "PASS" } else { "FAIL" },
            crate::canon::string(&self.measured)
        )
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn of<'a>(lines: &'a [Line], kind: &'a str) -> impl Iterator<Item = &'a Line> + 'a {
    lines.iter().filter(move |l| l.kind == kind)
}

/// The `q`-quantile (nearest rank) of `xs`; 0 for none.
pub fn quantile(xs: &[f64], q: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let rank = ((q * v.len() as f64).ceil() as usize).clamp(1, v.len());
    v[rank - 1]
}

/// Turns as `(started, ended)` pairs, by turn number. A turn that never
/// ended (a crash) has no pair.
fn turns(lines: &[Line]) -> Vec<(&Line, &Line)> {
    let mut started: BTreeMap<u64, &Line> = BTreeMap::new();
    let mut out = Vec::new();
    for l in lines {
        match l.kind.as_str() {
            "turn.started" => {
                started.insert(l.u64("turn"), l);
            }
            "turn.ended" => {
                if let Some(s) = started.remove(&l.u64("turn")) {
                    out.push((s, l));
                }
            }
            _ => {}
        }
    }
    out
}

fn ids(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn host_config(lines: &[Line]) -> &Value {
    of(lines, "host.start")
        .next()
        .map(|l| l.get("config"))
        .unwrap_or(&Value::Null)
}

// ─── G-a no rest ─────────────────────────────────────────────────────────────

/// G-a over a run with no stimuli and no quota pressure.
pub fn g_a(lines: &[Line]) -> GateResult {
    let mut g = GateResult::new("G-a");
    let boundaries: Vec<&Line> = of(lines, "boundary").collect();
    let ts = turns(lines);
    let (Some(first), Some(last)) = (boundaries.first(), lines.last()) else {
        g.check(false, "no boundaries");
        return g;
    };
    let span = (last.at - first.at) as f64;
    let turn_ms: f64 = ts.iter().map(|(s, e)| (e.at - s.at) as f64).sum();
    let declared: f64 = of(lines, "degraded.ended")
        .map(|l| l.f64("waited_ms"))
        .sum();
    let wall_ms: f64 = ts
        .iter()
        .map(|(s, e)| (s.f64("wall_boundary_us") + e.f64("wall_post_us")) / 1000.0)
        .sum();
    let virtual_gap = (span - turn_ms - declared).max(0.0);
    let idle = (virtual_gap + wall_ms) / (span + wall_ms).max(1.0);
    // Boundary → next turn: the virtual gap since the previous turn ended
    // (none after a declared wait) plus the host's measured work.
    let mut lat = Vec::new();
    let mut prev_end: Option<&Line> = None;
    let mut waited_since = false;
    for l in lines {
        match l.kind.as_str() {
            "degraded.ended" => waited_since = true,
            "turn.started" => {
                let gap = match (prev_end, waited_since) {
                    (Some(p), false) => (l.at - p.at) as f64,
                    _ => 0.0,
                };
                lat.push(gap + l.f64("wall_boundary_us") / 1000.0);
                waited_since = false;
            }
            "turn.ended" => prev_end = Some(l),
            _ => {}
        }
    }
    let p99 = quantile(&lat, 0.99);
    // Every boundary carries its decisions.
    let decided: BTreeSet<u64> = lines
        .iter()
        .filter(|l| l.kind.starts_with("decision."))
        .map(|l| l.u64("boundary"))
        .collect();
    let bare: Vec<u64> = boundaries
        .iter()
        .map(|b| b.u64("n"))
        .filter(|n| !decided.contains(n))
        .collect();
    g.put("span_ms", span);
    g.put("turns", ts.len());
    g.put("idle_fraction", idle);
    g.put("boundary_to_turn_p99_ms", p99);
    g.put("boundaries", boundaries.len());
    g.put("boundaries_without_decisions", bare.len());
    g.check(span >= G_A_RUN_MS as f64, "run shorter than 30 minutes");
    g.check(idle <= G_A_IDLE_MAX, "idle fraction over 2%");
    g.check(p99 <= G_A_BOUNDARY_P99_MS, "boundary → turn p99 over 50 ms");
    g.check(bare.is_empty(), "a boundary has no decision lines");
    g
}

// ─── G-b responsiveness ──────────────────────────────────────────────────────

/// G-b over a run with owner stimuli under load and scripted long work.
pub fn g_b(lines: &[Line]) -> GateResult {
    let mut g = GateResult::new("G-b");
    let bound = host_config(lines)["turn_bound_ms"].as_f64().unwrap_or(0.0);
    let ts = turns(lines);
    let durations: Vec<f64> = ts.iter().map(|(s, e)| (e.at - s.at) as f64).collect();
    let mut arrived: BTreeMap<String, i64> = BTreeMap::new();
    for l in of(lines, "stimulus.accepted") {
        let item = l.get("item");
        if item["role"] == "owner" {
            arrived.insert(
                item["id"].as_str().unwrap_or("").into(),
                item["at"].as_i64().unwrap_or(l.at),
            );
        }
    }
    let mut admission = Vec::new();
    for l in of(lines, "stimulus.admitted") {
        for id in ids(l.get("ids")) {
            if let Some(at) = arrived.remove(&id) {
                admission.push((l.at - at) as f64);
            }
        }
    }
    let p95_turn = quantile(&durations, 0.95);
    let p95_admit = quantile(&admission, 0.95);
    let over = durations.iter().filter(|d| **d > bound).count();
    let long: Vec<&Line> = of(lines, "tool.refused")
        .filter(|l| l.str("why") == "overran")
        .collect();
    let long_ok = long.iter().all(|l| l.str("message") == LONG_WORK_MESSAGE);
    g.put("owner_stimuli", admission.len());
    g.put("owner_never_admitted", arrived.len());
    g.put("admission_p95_ms", p95_admit);
    g.put("turn_p95_ms", p95_turn);
    g.put("turns_over_bound", over);
    g.put("turn_bound_ms", bound);
    g.put("long_work_refused", long.len());
    g.check(!admission.is_empty(), "no owner stimuli");
    g.check(arrived.is_empty(), "an owner stimulus was never admitted");
    g.check(
        p95_admit <= p95_turn + G_B_ADMISSION_SLACK_MS,
        "admission p95 over p95 turn + 100 ms",
    );
    g.check(bound > 0.0 && over == 0, "a turn ran past its bound");
    g.check(!long.is_empty(), "no long work was attempted");
    g.check(
        long_ok,
        "long work refused without the commit-or-steps message",
    );
    g
}

// ─── G-c free-time kernel ────────────────────────────────────────────────────

/// G-c's record checks over a 2,000-turn run. The no-ranking property and
/// the trybuild pin are separate tests over the same thresholds.
pub fn g_c(lines: &[Line]) -> GateResult {
    let mut g = GateResult::new("G-c");
    let mut now_admitted: BTreeSet<u64> = BTreeSet::new();
    for l in of(lines, "stimulus.admitted") {
        if !ids(l.get("ids")).is_empty() {
            now_admitted.insert(l.u64("turn"));
        }
    }
    let (mut free, mut committed, mut responding) = (0u64, 0u64, 0u64);
    let mut wrong = Vec::new();
    let mut in_commit = false;
    // The agent's successful tool calls, by turn (a call is recorded once
    // it has returned, so after the lines it wrote).
    let mut tool_calls: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    for l in of(lines, "tool.call").filter(|l| l.get("ok") == &Value::Bool(true)) {
        tool_calls
            .entry(l.u64("turn"))
            .or_default()
            .insert(l.str("name").into());
    }
    let mut unauthorised = 0;
    let (mut commits, mut releases) = (0, 0);
    for l in lines {
        match l.kind.as_str() {
            "kernel.commit" => {
                commits += 1;
                in_commit = true;
                let via_tool = l.str("via") == "tool:commit"
                    && tool_calls
                        .get(&l.u64("turn"))
                        .is_some_and(|s| s.contains("commit"));
                if !via_tool {
                    unauthorised += 1;
                }
            }
            "kernel.release" => {
                releases += 1;
                in_commit = false;
                let ok = match l.str("via") {
                    "tool:release" => tool_calls
                        .get(&l.u64("turn"))
                        .is_some_and(|s| s.contains("release")),
                    "owner:_rung/release" => l.str("released_by") == "owner",
                    _ => false,
                };
                if !ok {
                    unauthorised += 1;
                }
            }
            "turn.started" => {
                let turn = l.u64("turn");
                let kind = l.str("turn_kind");
                let expect = if now_admitted.contains(&turn) {
                    "responding"
                } else if in_commit {
                    "committed"
                } else {
                    "free"
                };
                match kind {
                    "free" => free += 1,
                    "committed" => committed += 1,
                    "responding" => responding += 1,
                    _ => {}
                }
                if kind != expect && wrong.len() < 5 {
                    wrong.push(json!({"turn": turn, "kind": kind, "expected": expect}));
                }
                if kind != expect {
                    g.pass = false;
                }
            }
            _ => {}
        }
    }
    let total = free + committed + responding;
    g.put("turns", total);
    g.put("free", free);
    g.put("committed", committed);
    g.put("responding", responding);
    g.put("commits", commits);
    g.put("releases", releases);
    g.put("wrong_kind", Value::Array(wrong.clone()));
    g.put("kernel_lines_not_from_agent_or_owner", unauthorised);
    g.check(total >= G_C_TURNS, "fewer than 2,000 turns");
    g.check(wrong.is_empty(), "a turn ran in the wrong mode");
    g.check(
        free > 0 && committed > 0 && responding > 0,
        "a mode never ran",
    );
    g.check(commits > 0 && releases > 0, "no commit or release");
    g.check(unauthorised == 0, "a kernel line without the agent's tool");
    g
}

// ─── G-d schedule ────────────────────────────────────────────────────────────

/// G-d: each due item fired at the first boundary at or after its due time
/// (once, with lateness), firm items admitted at that boundary.
pub fn g_d(lines: &[Line]) -> GateResult {
    let mut g = GateResult::new("G-d");
    let mut first_boundary_after: Vec<(i64, u64)> = Vec::new(); // (at, seq) of boundaries
    let mut runs: Vec<u64> = Vec::new(); // seq of each host.start
    for l in lines {
        match l.kind.as_str() {
            "boundary" => first_boundary_after.push((l.at, l.seq)),
            "host.start" => runs.push(l.seq),
            _ => {}
        }
    }
    let mut seen: BTreeSet<(String, i64)> = BTreeSet::new();
    let (mut fired, mut late, mut missed, mut dupes, mut early, mut firm_late) = (0, 0, 0, 0, 0, 0);
    let mut not_first = 0;
    for (i, l) in lines.iter().enumerate() {
        if l.kind != "calendar.fired" {
            continue;
        }
        fired += 1;
        let id = l.str("id").to_string();
        let due = l.i64("due");
        if !seen.insert((id.clone(), due)) {
            dupes += 1;
        }
        if l.at < due {
            early += 1;
        }
        if l.i64("late_by_ms") != (l.at - due).max(0) {
            late += 1;
        }
        if l.get("missed") == &Value::Bool(true) {
            missed += 1;
        }
        // The run this fire belongs to started at the last host.start before it.
        let run_start = runs
            .iter()
            .rev()
            .find(|s| **s < l.seq)
            .copied()
            .unwrap_or(0);
        let first = first_boundary_after
            .iter()
            .find(|(at, seq)| *seq > run_start && *at >= due)
            .map(|(_, s)| *s);
        // The fire belongs to the boundary just before it.
        let mine = first_boundary_after
            .iter()
            .rev()
            .find(|(_, s)| *s < l.seq)
            .map(|(_, s)| *s);
        if first != mine && first.is_some_and(|f| f < l.seq) {
            not_first += 1;
        }
        if l.get("firm") == &Value::Bool(true) {
            let item = l.str("item_id");
            let admitted = lines[i..]
                .iter()
                .take_while(|x| x.kind != "boundary")
                .any(|x| {
                    x.kind == "stimulus.admitted" && ids(x.get("ids")).iter().any(|s| s == item)
                });
            if !admitted {
                firm_late += 1;
            }
        }
    }
    g.put("fired", fired);
    g.put("missed_during_downtime", missed);
    g.put("duplicates", dupes);
    g.put("not_at_first_boundary", not_first);
    g.put("lateness_wrong", late);
    g.put("firm_not_admitted_at_once", firm_late);
    g.check(fired > 0, "nothing fired");
    g.check(dupes == 0, "an item fired twice");
    g.check(early == 0, "an item fired before it was due");
    g.check(
        not_first == 0,
        "an item fired after the first boundary past due",
    );
    g.check(late == 0, "lateness not recorded as at − due");
    g.check(
        firm_late == 0,
        "a firm item was not admitted at its boundary",
    );
    g
}

/// G-d's downtime half: every item due during a gap fired exactly once on
/// waking (`missed: true`), or was skipped by its policy.
pub fn g_d_downtime(lines: &[Line], expected_missed: &[&str]) -> GateResult {
    let mut g = GateResult::new("G-d/downtime");
    let mut fired: BTreeMap<String, usize> = BTreeMap::new();
    for l in of(lines, "calendar.fired") {
        if l.get("missed") == &Value::Bool(true) {
            *fired.entry(l.str("id").into()).or_default() += 1;
        }
    }
    let skipped: BTreeSet<String> = of(lines, "calendar.skipped")
        .map(|l| l.str("id").to_string())
        .collect();
    let bad: Vec<&str> = expected_missed
        .iter()
        .copied()
        .filter(|id| fired.get(*id).copied().unwrap_or(0) != 1 && !skipped.contains(*id))
        .collect();
    g.put("missed_fired", Value::from(fired.len()));
    g.put("skipped", Value::from(skipped.len()));
    g.put(
        "bad",
        Value::from(bad.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
    );
    g.check(bad.is_empty(), "a missed item did not fire exactly once");
    g
}

// ─── G-e expectations ────────────────────────────────────────────────────────

/// Calibration recomputed from the record alone, in record order.
pub fn calibration_from(lines: &[Line]) -> Value {
    let mut p_now: BTreeMap<String, f64> = BTreeMap::new();
    let mut n = 0u64;
    let mut brier = 0.0f64;
    let mut hits = 0u64;
    let mut bins: Vec<(u64, f64, u64)> = vec![(0, 0.0, 0); 10];
    for l in lines {
        match l.kind.as_str() {
            "expectation.made" | "expectation.revised" => {
                p_now.insert(l.str("id").into(), l.f64("p"));
            }
            "expectation.settled" => {
                let o = match l.str("state") {
                    "met" => 1.0,
                    "missed" => 0.0,
                    _ => continue,
                };
                let p = p_now.get(l.str("id")).copied().unwrap_or(0.5);
                n += 1;
                brier += (p - o) * (p - o);
                hits += o as u64;
                let b = ((p * 10.0).floor() as usize).min(9);
                bins[b].0 += 1;
                bins[b].1 += p;
                bins[b].2 += o as u64;
            }
            _ => {}
        }
    }
    calibration_value(n, brier, hits, &bins)
}

/// The calibration object both the host and [`calibration_from`] report.
pub fn calibration_value(n: u64, brier_sum: f64, hits: u64, bins: &[(u64, f64, u64)]) -> Value {
    if n == 0 {
        return json!({"n": 0});
    }
    let base = hits as f64 / n as f64;
    let mut resolution = 0.0;
    let mut table = Vec::new();
    for (i, (k, psum, h)) in bins.iter().enumerate() {
        if *k == 0 {
            continue;
        }
        let o = *h as f64 / *k as f64;
        resolution += *k as f64 * (o - base) * (o - base);
        table.push(json!({"bin": i, "n": k, "mean_p": crate::canon::fixed(psum / *k as f64), "observed": crate::canon::fixed(o)}));
    }
    json!({
        "n": n,
        "brier": crate::canon::fixed(brier_sum / n as f64),
        "base_rate": crate::canon::fixed(base),
        "resolution": crate::canon::fixed(resolution / n as f64),
        "reliability": table,
    })
}

/// The surprise of an outcome: −log₂ p when met, −log₂(1 − p) when missed.
pub fn surprise(p: f64, met: bool) -> f64 {
    let q = if met { p } else { 1.0 - p };
    crate::canon::fixed(-q.max(1e-9).log2())
}

/// G-e: only the host (or a disjoint judge) settles; surprise and
/// calibration recomputed from the record match the host's exactly; each
/// decidable expectation settled as its predicate says.
pub fn g_e(lines: &[Line]) -> GateResult {
    let mut g = GateResult::new("G-e");
    let mut p_now: BTreeMap<String, f64> = BTreeMap::new();
    let mut made: BTreeMap<String, &Line> = BTreeMap::new();
    let mut facts: Vec<(i64, String, Value)> = Vec::new();
    let mut from: Vec<(i64, String)> = Vec::new();
    let (mut settled, mut met, mut missed) = (0, 0, 0);
    let (mut by_agent, mut surprise_wrong, mut verdict_wrong) = (0, 0, 0);
    let mut last_calibration = Value::Null;
    let mut run = (0u64, 0.0f64, 0u64);
    let mut bins: Vec<(u64, f64, u64)> = vec![(0, 0.0, 0); 10];
    for l in lines {
        match l.kind.as_str() {
            "stimulus.accepted" => {
                let item = l.get("item");
                let at = item["at"].as_i64().unwrap_or(l.at);
                if item["kind"] == "world" {
                    facts.push((
                        at,
                        item["fact"]["key"].as_str().unwrap_or("").into(),
                        item["fact"]["value"].clone(),
                    ));
                }
                from.push((at, item["channel"].as_str().unwrap_or("").into()));
            }
            "expectation.made" => {
                p_now.insert(l.str("id").into(), l.f64("p"));
                made.insert(l.str("id").into(), l);
            }
            "expectation.revised" => {
                p_now.insert(l.str("id").into(), l.f64("p"));
            }
            "expectation.settled" => {
                settled += 1;
                let id = l.str("id");
                let by = l.str("settled_by");
                if !(by == "host" || by.starts_with("judge:")) {
                    by_agent += 1;
                }
                let state = l.str("state");
                let p = p_now.get(id).copied().unwrap_or(0.5);
                match state {
                    "met" => met += 1,
                    "missed" => missed += 1,
                    _ => {}
                }
                if state == "met" || state == "missed" {
                    let s = surprise(p, state == "met");
                    if l.f64("surprise") != s || l.f64("p") != p {
                        surprise_wrong += 1;
                    }
                }
                if let Some(m) = made.get(id) {
                    let check = m.get("check");
                    let made_at = m.at;
                    let due = m.i64("due");
                    let holds = if let Some(w) = check.get("world_fact") {
                        let key = w["key"].as_str().unwrap_or("");
                        facts.iter().any(|(at, k, v)| {
                            *at >= made_at && *at <= due && k == key && v == &w["equals"]
                        })
                    } else if let Some(s) = check.get("stimulus_from") {
                        let ch = s["channel"].as_str().unwrap_or("");
                        from.iter()
                            .any(|(at, c)| *at >= made_at && *at <= due && c == ch)
                    } else {
                        // Judged: the judge's verdict is its own.
                        state == "met"
                    };
                    if (state == "met") != holds && state != "void" {
                        verdict_wrong += 1;
                    }
                }
                last_calibration = l.get("calibration").clone();
                // The same arithmetic as `calibration_from`, kept running.
                if state == "met" || state == "missed" {
                    let o = if state == "met" { 1.0 } else { 0.0 };
                    run.0 += 1;
                    run.1 += (p - o) * (p - o);
                    run.2 += o as u64;
                    let b = ((p * 10.0).floor() as usize).min(9);
                    bins[b].0 += 1;
                    bins[b].1 += p;
                    bins[b].2 += o as u64;
                }
                let offline = calibration_value(run.0, run.1, run.2, &bins);
                if offline != last_calibration {
                    g.check(false, format!("calibration differs at seq {}", l.seq));
                }
            }
            _ => {}
        }
    }
    let agent_settle = of(lines, "tool.call")
        .filter(|l| l.str("name").contains("settle"))
        .count();
    g.put("settled", settled);
    g.put("met", met);
    g.put("missed", missed);
    g.put("settled_by_agent", by_agent + agent_settle);
    g.put("surprise_mismatches", surprise_wrong);
    g.put("verdict_mismatches", verdict_wrong);
    g.put("calibration", last_calibration);
    g.check(met > 0 && missed > 0, "no met and missed outcomes to check");
    g.check(
        by_agent + agent_settle == 0,
        "an expectation was settled by the agent",
    );
    g.check(surprise_wrong == 0, "surprise does not recompute");
    g.check(
        verdict_wrong == 0,
        "a decidable expectation was settled against its predicate",
    );
    g
}

// ─── G-g provider failure ────────────────────────────────────────────────────

/// G-g over a run with injected faults. `exited` is whether the host loop
/// returned before the harness stopped it.
pub fn g_g(lines: &[Line], exited: bool) -> GateResult {
    let mut g = GateResult::new("G-g");
    let mut admitted: BTreeMap<String, u64> = BTreeMap::new();
    let mut requeued: BTreeMap<String, u64> = BTreeMap::new();
    let mut disposed: BTreeMap<String, u64> = BTreeMap::new();
    for l in lines {
        match l.kind.as_str() {
            "stimulus.admitted" => {
                for id in ids(l.get("ids")).into_iter().chain(ids(l.get("digests"))) {
                    *admitted.entry(id).or_default() += 1;
                }
            }
            "stimulus.requeued" => {
                for id in ids(l.get("ids")) {
                    *requeued.entry(id).or_default() += 1;
                }
            }
            "stimulus.disposed" => *disposed.entry(l.str("id").into()).or_default() += 1,
            _ => {}
        }
    }
    let requeue_ids: Vec<&String> = requeued.keys().collect();
    let not_once = requeue_ids
        .iter()
        .filter(|id| {
            admitted.get(**id).copied().unwrap_or(0) != requeued[**id] + 1
                || disposed.get(**id).copied().unwrap_or(0) != 1
        })
        .count();
    // Per failure: what followed it.
    let ts = turns(lines);
    let mut by_class: BTreeMap<String, u64> = BTreeMap::new();
    let (mut retry_violations, mut step_down_missing, mut platform_stepped) = (0, 0, 0);
    let mut reset_violations = 0;
    for (k, (_, e)) in ts.iter().enumerate() {
        let f = e.get("failure");
        if f.is_null() {
            continue;
        }
        let class = f["class"].as_str().unwrap_or("").to_string();
        let origin = f["origin"].as_str().unwrap_or("").to_string();
        *by_class.entry(format!("{origin}:{class}")).or_default() += 1;
        let next = ts.get(k + 1).map(|(s, _)| *s);
        let between: Vec<&Line> = lines
            .iter()
            .filter(|l| l.seq > e.seq && next.is_none_or(|n| l.seq < n.seq))
            .collect();
        if let (Some(ra), Some(n)) = (f["retry_after_ms"].as_i64(), next)
            && n.at < e.at + ra
        {
            retry_violations += 1;
        }
        let switched_down = between
            .iter()
            .any(|l| l.kind == "model.switch" && l.str("direction") == "down");
        let stepping = origin == "provider"
            && matches!(
                class.as_str(),
                "rate_limit" | "overloaded" | "transport" | "timeout"
            );
        let at_bottom = e.get("rung").as_u64().unwrap_or(0) + 1
            >= host_config(lines)["ladder"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(1) as u64;
        if stepping && !switched_down && !at_bottom {
            step_down_missing += 1;
        }
        if origin == "platform" {
            if switched_down {
                platform_stepped += 1;
            }
            if let (Some(reset), Some(n)) = (f["reset_at"].as_i64(), next)
                && n.at < reset
            {
                reset_violations += 1;
            }
        }
    }
    let ups = of(lines, "model.switch")
        .filter(|l| l.str("direction") == "up")
        .count();
    let backoff_over = of(lines, "degraded")
        .filter(|l| l.str("class") == "backoff" && l.i64("until") - l.at > G_G_BACKOFF_CAP_MS)
        .count();
    let degraded = of(lines, "degraded").count();
    let blocked_msgs = of(lines, "outbox.queued")
        .filter(|l| l.str("source") == "host:blocked")
        .count();
    let incidents = of(lines, "degraded")
        .filter(|l| l.str("class") == "blocked" && l.get("incident_start") == &Value::Bool(true))
        .count();
    g.put("exited", exited);
    g.put(
        "failures",
        serde_json::to_value(&by_class).unwrap_or_default(),
    );
    g.put("requeued_items", requeue_ids.len());
    g.put("requeued_not_admitted_exactly_once", not_once);
    g.put("retry_after_violations", retry_violations);
    g.put("provider_failures_without_step_down", step_down_missing);
    g.put("probe_ups", ups);
    g.put("platform_failures_with_step_down", platform_stepped);
    g.put("platform_reset_violations", reset_violations);
    g.put("backoffs_over_cap", backoff_over);
    g.put("degraded_lines", degraded);
    g.put("blocked_incidents", incidents);
    g.put("blocked_owner_messages", blocked_msgs);
    g.check(!exited, "the host exited");
    g.check(!requeue_ids.is_empty(), "nothing was requeued");
    g.check(
        not_once == 0,
        "a requeued item was not admitted exactly once",
    );
    g.check(retry_violations == 0, "Retry-After not honoured");
    g.check(
        step_down_missing == 0,
        "a provider failure did not step down",
    );
    g.check(ups > 0, "never probed back up");
    g.check(platform_stepped == 0, "a platform 429 stepped down");
    g.check(
        reset_violations == 0,
        "a platform 429 did not wait for its reset",
    );
    g.check(backoff_over == 0, "a backoff over the cap");
    g.check(degraded > 0, "status never showed degraded");
    g.check(
        incidents > 0 && blocked_msgs == incidents,
        "not one owner message per blocked incident",
    );
    g
}

// ─── G-h stop ────────────────────────────────────────────────────────────────

/// One measured stop.
#[derive(Debug, Clone, Serialize)]
pub struct StopCase {
    pub case: String,
    /// SIGTERM → process exit, ms.
    pub elapsed_ms: u64,
    /// What was left of the mock call in flight, ms (0 outside a turn).
    pub remaining_call_ms: u64,
    pub exit_code: Option<i32>,
    /// The record's last line is `halted{Stopped}`.
    pub halted_recorded: bool,
}

/// G-h from measured stops and one wedged run: `wedged_detect_ms` is how
/// long after the last watchdog ping the harness (acting as supervisor)
/// saw the interval missed; `watchdog_ms` is `WatchdogSec`.
pub fn g_h(cases: &[StopCase], wedged_detect_ms: Option<u64>, watchdog_ms: u64) -> GateResult {
    let mut g = GateResult::new("G-h");
    for c in cases {
        let limit = if c.case == "mid_turn" {
            c.remaining_call_ms + G_H_TURN_SLACK_MS
        } else {
            G_H_WAIT_MS
        };
        g.check(
            c.elapsed_ms <= limit,
            format!("{}: {} ms > {limit} ms", c.case, c.elapsed_ms),
        );
        g.check(
            c.exit_code == Some(0),
            format!("{}: exit {:?}", c.case, c.exit_code),
        );
        g.check(c.halted_recorded, format!("{}: no halted line", c.case));
    }
    for want in ["mid_turn", "backoff", "paced"] {
        g.check(
            cases.iter().any(|c| c.case == want),
            format!("no {want} case"),
        );
    }
    g.put("cases", serde_json::to_value(cases).unwrap_or_default());
    g.put("wedged_detect_ms", wedged_detect_ms);
    g.put("watchdog_ms", watchdog_ms);
    g.check(
        wedged_detect_ms.is_some_and(|d| d <= watchdog_ms + G_H_TURN_SLACK_MS),
        "the watchdog did not fire on a wedged loop",
    );
    g
}

// ─── G-i restart ─────────────────────────────────────────────────────────────

/// G-i over the record after `kills` random `kill -9`s and a final drain.
/// `replayed` holds, for each `turn.ended` seq, the projection hash
/// recomputed by replaying the record before that line.
pub fn g_i(lines: &[Line], kills: usize, replayed: &BTreeMap<u64, String>) -> GateResult {
    let mut g = GateResult::new("G-i");
    let mut accepted: BTreeSet<String> = BTreeSet::new();
    let mut disposed: BTreeMap<String, u64> = BTreeMap::new();
    for l in lines {
        match l.kind.as_str() {
            "stimulus.accepted" => {
                accepted.insert(l.get("item")["id"].as_str().unwrap_or("").into());
            }
            "stimulus.disposed" => *disposed.entry(l.str("id").into()).or_default() += 1,
            _ => {}
        }
    }
    let not_one = accepted
        .iter()
        .filter(|id| disposed.get(*id).copied().unwrap_or(0) != 1)
        .count();
    let mismatched = of(lines, "turn.ended")
        .filter(|l| {
            replayed
                .get(&l.seq)
                .is_none_or(|h| h != l.str("projection"))
        })
        .count();
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == "host.start")
        .map(|(i, _)| i)
        .collect();
    let (mut no_recovered, mut no_gap_epoch, mut mode_wrong) = (0, 0, 0);
    for w in starts.iter().skip(1) {
        let rest = &lines[*w..];
        let next_start = rest.iter().skip(1).position(|l| l.kind == "host.start");
        let run = match next_start {
            Some(k) => &rest[..=k],
            None => rest,
        };
        // Died while waking, wrote nothing else: the next restart is checked.
        if run.len() == 1 && next_start.is_some() {
            continue;
        }
        let rec = run.iter().find(|l| l.kind == "recovered");
        match rec {
            None => no_recovered += 1,
            Some(r) => {
                // Mode as the kernel lines before this run left it.
                let mut mode = "free".to_string();
                for l in &lines[..*w] {
                    match l.kind.as_str() {
                        "kernel.commit" => mode = format!("committed:{}", l.str("project")),
                        "kernel.release" => mode = "free".into(),
                        _ => {}
                    }
                }
                if r.str("mode") != mode {
                    mode_wrong += 1;
                }
            }
        }
        let epoch_ok = run.iter().any(|l| {
            l.kind == "epoch.rollover"
                && l.str("cause") == "wake"
                && l.get("gap_ms").is_i64()
                && l.str("l1").contains("not running from")
        });
        if !epoch_ok {
            no_gap_epoch += 1;
        }
    }
    g.put("kills", kills);
    g.put("restarts", starts.len().saturating_sub(1));
    g.put("stimuli", accepted.len());
    g.put("stimuli_without_exactly_one_disposition", not_one);
    g.put("turns_ended", of(lines, "turn.ended").count());
    g.put("projection_mismatches", mismatched);
    g.put("restarts_without_recovered", no_recovered);
    g.put("restarts_without_gap_epoch", no_gap_epoch);
    g.put("restarts_with_wrong_mode", mode_wrong);
    g.check(kills >= G_I_KILLS, "fewer than 50 kills");
    g.check(starts.len() > kills, "a kill was not followed by a restart");
    g.check(!accepted.is_empty(), "no stimuli");
    g.check(not_one == 0, "a stimulus without exactly one disposition");
    g.check(mismatched == 0, "a rebuilt projection differs");
    g.check(no_recovered == 0, "a restart without a recovered line");
    g.check(
        no_gap_epoch == 0,
        "a restart without a new epoch holding the gap",
    );
    g.check(mode_wrong == 0, "a restart did not restore the mode");
    g
}

// ─── G-j bounded context ─────────────────────────────────────────────────────

/// G-j over a 10,000-turn run.
pub fn g_j(lines: &[Line]) -> GateResult {
    let mut g = GateResult::new("G-j");
    let budget = host_config(lines)["epoch_budget_tokens"]
        .as_f64()
        .unwrap_or(0.0);
    let started: Vec<&Line> = of(lines, "turn.started").collect();
    let max_start = started
        .iter()
        .map(|l| l.f64("pack_tokens"))
        .fold(0.0, f64::max);
    let max_call = of(lines, "llm.call")
        .map(|l| l.f64("prompt_tokens"))
        .fold(0.0, f64::max);
    let over_ceiling = started
        .iter()
        .filter(|l| l.f64("pack_tokens") > G_J_CEILING * budget)
        .count();
    // Bytes of record per 1,000 turns.
    let mut windows: Vec<f64> = Vec::new();
    let mut bytes = 0usize;
    let mut turns = 0u64;
    for l in lines {
        bytes += l.text().len() + 1;
        if l.kind == "turn.ended" {
            turns += 1;
            if turns.is_multiple_of(1000) {
                windows.push(bytes as f64);
                bytes = 0;
            }
        }
    }
    let tail: Vec<f64> = windows.iter().skip(1).copied().collect();
    let ratio = match (
        tail.iter().copied().fold(f64::MIN, f64::max),
        tail.iter().copied().fold(f64::MAX, f64::min),
    ) {
        (hi, lo) if !tail.is_empty() && lo > 0.0 => hi / lo,
        _ => f64::INFINITY,
    };
    // The copy guard: every trace that copies a recent one is flagged.
    let copies = of(lines, "kernel.trace")
        .filter(|l| l.f64("similarity") > 0.8)
        .count();
    let flagged = of(lines, "copy.guard").count();
    let loops = of(lines, "copy.loop").count();
    let loop_rollovers = of(lines, "epoch.rollover")
        .filter(|l| l.str("cause") == "copy_loop")
        .count();
    g.put("turns", turns);
    g.put("epoch_budget_tokens", budget);
    g.put("max_pack_tokens_at_turn_start", max_start);
    g.put("max_prompt_tokens", max_call);
    g.put("turn_starts_over_85pct", over_ceiling);
    g.put("rollovers", of(lines, "epoch.rollover").count());
    g.put("record_bytes_per_1000_turns", Value::from(windows.clone()));
    g.put(
        "growth_ratio",
        if ratio.is_finite() {
            json!(ratio)
        } else {
            Value::Null
        },
    );
    g.put("copied_traces", copies);
    g.put("copy_guard_lines", flagged);
    g.put("copy_loops", loops);
    g.check(turns >= G_J_TURNS, "fewer than 10,000 turns");
    g.check(
        budget > 0.0 && max_start <= budget && max_call <= budget,
        "the pack exceeded its budget",
    );
    g.check(over_ceiling == 0, "a turn started above 85% of the budget");
    g.check(
        ratio <= G_J_GROWTH_RATIO_MAX,
        "the record does not grow linearly",
    );
    g.check(
        copies > 0 && flagged >= copies,
        "a copied trace was not flagged",
    );
    g.check(
        loops > 0 && loop_rollovers == loops,
        "a copy loop without its rollover",
    );
    g
}

// ─── G-l cache discipline ────────────────────────────────────────────────────

/// One request as the mock provider received it.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Captured {
    pub turn: u64,
    pub call: u32,
    /// The request's session id (the host's epoch id).
    pub session: String,
    pub model: String,
    pub bytes: usize,
    /// The previous request of the same session is a byte-prefix of this one.
    pub extends_prev: Option<bool>,
}

/// G-l over a 10,000-turn run and its captured requests.
pub fn g_l(lines: &[Line], captured: &[Captured]) -> GateResult {
    let mut g = GateResult::new("G-l");
    let in_session = captured.iter().filter(|c| c.extends_prev.is_some()).count();
    let broken: Vec<&Captured> = captured
        .iter()
        .filter(|c| c.extends_prev == Some(false))
        .collect();
    // Hash changes only at their recorded causes.
    let (mut s_bad, mut l_bad) = (0, 0);
    let (mut prev_s, mut prev_l): (Option<String>, Option<String>) = (None, None);
    let (mut swap_since, mut roll_since) = (false, false);
    let mut breaks: BTreeSet<(u64, u64)> = BTreeSet::new();
    let (mut cached, mut expected) = (0.0f64, 0.0f64);
    let mut calls = 0;
    for l in lines {
        match l.kind.as_str() {
            "pack.swap" => swap_since = true,
            "epoch.rollover" => roll_since = true,
            "cache.break" | "cache.cold" => {
                breaks.insert((l.u64("turn"), l.u64("call")));
            }
            "llm.call" => {
                calls += 1;
                let p = l.get("prefix");
                let s = p["s_hash"].as_str().unwrap_or("").to_string();
                let lh = p["l_hash"].as_str().unwrap_or("").to_string();
                if prev_s.as_ref().is_some_and(|x| *x != s) && !swap_since {
                    s_bad += 1;
                }
                if prev_l.as_ref().is_some_and(|x| *x != lh) && !roll_since && !swap_since {
                    l_bad += 1;
                }
                prev_s = Some(s);
                prev_l = Some(lh);
                swap_since = false;
                roll_since = false;
            }
            _ => {}
        }
    }
    for l in of(lines, "llm.call") {
        if breaks.contains(&(l.u64("turn"), l.u64("call"))) {
            continue;
        }
        cached += l.f64("cached_tokens");
        expected += l.get("prefix")["expected_cached_tokens"]
            .as_f64()
            .unwrap_or(0.0);
    }
    let efficiency = if expected > 0.0 {
        cached / expected
    } else {
        0.0
    };
    g.put("calls", calls);
    g.put("captured", captured.len());
    g.put("captured_in_session", in_session);
    g.put("prefix_breaks_in_session", broken.len());
    g.put(
        "first_breaks",
        Value::from(
            broken
                .iter()
                .take(3)
                .map(|c| json!({"turn": c.turn, "call": c.call}))
                .collect::<Vec<_>>(),
        ),
    );
    g.put("s_hash_changes_without_swap", s_bad);
    g.put("l_hash_changes_without_rollover", l_bad);
    g.put("recorded_breaks", breaks.len());
    g.put("cache_efficiency", efficiency);
    g.check(
        captured.len() as u64 >= G_J_TURNS,
        "fewer requests than turns",
    );
    g.check(
        in_session > 0 && broken.is_empty(),
        "a request in an epoch is not a byte-prefix extension of the previous",
    );
    g.check(s_bad == 0, "s_hash changed outside a recorded swap");
    g.check(l_bad == 0, "l_hash changed outside a recorded rollover");
    g.check(
        efficiency >= G_L_EFFICIENCY_MIN,
        "cache efficiency under 0.98",
    );
    g
}

/// G-l's stability half: the same canonical bytes from two processes.
pub fn g_l_stable(first: &str, second: &str) -> GateResult {
    let mut g = GateResult::new("G-l/stable");
    g.put("first", first);
    g.put("second", second);
    g.check(
        !first.is_empty() && first == second,
        "canonical bytes differ across processes",
    );
    g
}

// ─── G-m decisions ───────────────────────────────────────────────────────────

/// G-m's record checks: every decision line carries provenance, and the
/// guards hold whatever the decider said. `ceiling` is the operator ceiling
/// (core included).
pub fn g_m(lines: &[Line], ceiling: &[&str]) -> GateResult {
    let mut g = GateResult::new("G-m");
    let ceiling: BTreeSet<&str> = ceiling.iter().copied().collect();
    let mut by_family: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut unlabelled = 0;
    let mut outside = 0;
    let mut owner: BTreeMap<String, u64> = BTreeMap::new(); // id → waiting since seq
    let mut owner_ids: BTreeSet<String> = BTreeSet::new();
    let mut owner_deferred = 0;
    let mut last_boundary_seq = 0;
    let mut owner_waiting_at_boundary: BTreeSet<String> = BTreeSet::new();
    // Boundaries where the decider stalled, and what each boundary's asks
    // cost together.
    let mut delayed: BTreeSet<u64> = BTreeSet::new();
    let mut ask_cost: BTreeMap<u64, f64> = BTreeMap::new();
    for l in of(lines, "desk.ask") {
        *ask_cost.entry(l.u64("boundary")).or_default() += l.f64("wall_us") / 1000.0;
    }
    for l in lines {
        if l.kind == "boundary" {
            // Owners waiting before this boundary must be admitted at it.
            owner_waiting_at_boundary = owner
                .iter()
                .filter(|(_, s)| **s < l.seq)
                .map(|(id, _)| id.clone())
                .collect();
            last_boundary_seq = l.seq;
        }
        if let Some(family) = l.kind.strip_prefix("decision.") {
            let by = l.get("by");
            let label = if let Some(r) = by.get("rule").and_then(Value::as_str) {
                format!("rule({r})")
            } else if by.get("jev").is_some() {
                "jev".to_string()
            } else {
                unlabelled += 1;
                "?".to_string()
            };
            *by_family
                .entry(family.into())
                .or_default()
                .entry(label)
                .or_default() += 1;
            if family == "tools" {
                for grp in ids(&l.get("choice")["enabled"]) {
                    if !ceiling.contains(grp.as_str()) {
                        outside += 1;
                    }
                }
            }
            if l.get("delayed") == &Value::Bool(true) {
                delayed.insert(l.u64("boundary"));
            }
        }
        match l.kind.as_str() {
            "stimulus.accepted" if l.get("item")["role"] == "owner" => {
                let id = l.get("item")["id"].as_str().unwrap_or("").to_string();
                owner_ids.insert(id.clone());
                owner.insert(id, l.seq);
            }
            "stimulus.requeued" => {
                for id in ids(l.get("ids")) {
                    if owner_ids.contains(&id) {
                        owner.insert(id, l.seq);
                    }
                }
            }
            "stimulus.admitted" => {
                for id in ids(l.get("ids")) {
                    owner.remove(&id);
                    owner_waiting_at_boundary.remove(&id);
                }
            }
            // The boundary admitted what it was going to; an owner item
            // waiting from before it and not admitted was deferred.
            "turn.started"
                if l.seq > last_boundary_seq && !owner_waiting_at_boundary.is_empty() =>
            {
                owner_deferred += owner_waiting_at_boundary.len();
                owner_waiting_at_boundary.clear();
            }
            _ => {}
        }
    }
    let budget = host_config(lines)["epoch_budget_tokens"]
        .as_f64()
        .unwrap_or(0.0);
    let below_floor = of(lines, "epoch.rollover")
        .filter(|l| l.str("cause") == "pack")
        .filter(|l| l.f64("tokens_before") < G_M_SOFT_FLOOR * budget)
        .count();
    let slow: Vec<f64> = delayed
        .iter()
        .map(|b| ask_cost.get(b).copied().unwrap_or(0.0))
        .collect();
    let worst_delayed = slow.iter().copied().fold(0.0, f64::max);
    let missing: Vec<&str> = FAMILIES
        .iter()
        .copied()
        .filter(|f| !by_family.contains_key(*f))
        .collect();
    g.put(
        "by_family",
        serde_json::to_value(&by_family).unwrap_or_default(),
    );
    g.put("decision_lines_without_provenance", unlabelled);
    g.put("tools_outside_ceiling", outside);
    g.put("owner_deferred", owner_deferred);
    g.put("pack_rollovers_below_floor", below_floor);
    g.put("delayed_boundaries", slow.len());
    g.put("worst_delayed_boundary_ms", worst_delayed);
    g.check(
        missing.is_empty(),
        format!("families never decided: {missing:?}"),
    );
    g.check(unlabelled == 0, "a decision line without provenance");
    g.check(outside == 0, "a tool group enabled outside the ceiling");
    g.check(owner_deferred == 0, "an owner item was deferred");
    g.check(below_floor == 0, "a pack rollover below the soft floor");
    g.check(
        worst_delayed <= G_M_BOUNDARY_COST_MS,
        "a delayed decider cost a boundary more than 2 s",
    );
    g
}

// ─── G-k cost ────────────────────────────────────────────────────────────────

/// G-k over every record a slice-1 run wrote: no money moved, no live
/// model, no live Jev.
pub fn g_k(lines: &[Line]) -> GateResult {
    let mut g = GateResult::new("G-k");
    let llm_cost: f64 = of(lines, "llm.call").map(|l| l.f64("cost_usd")).sum();
    let live_llm = of(lines, "llm.call")
        .filter(|l| l.str("provider") != "mock")
        .count();
    let mut jev_cost = 0.0;
    let mut live_jev = 0;
    for l in lines.iter().filter(|l| l.kind.starts_with("decision.")) {
        if let Some(j) = l.get("by").get("jev") {
            jev_cost += j["cost_usd"].as_f64().unwrap_or(0.0);
            let backend = j["backend"].as_str().unwrap_or("");
            if backend != "scripted" && backend != "recorded" {
                live_jev += 1;
            }
        }
    }
    let engines: BTreeSet<String> = of(lines, "host.start")
        .map(|l| l.get("config")["engine"].as_str().unwrap_or("").to_string())
        .collect();
    g.put("llm_cost_usd", llm_cost);
    g.put("jev_cost_usd", jev_cost);
    g.put("live_model_calls", live_llm);
    g.put("live_jev_asks", live_jev);
    g.put(
        "engines",
        Value::from(engines.iter().cloned().collect::<Vec<_>>()),
    );
    g.check(llm_cost == 0.0 && jev_cost == 0.0, "money was spent");
    g.check(live_llm == 0, "a live model was called");
    g.check(live_jev == 0, "a live Jev was asked");
    g.check(
        engines.iter().all(|e| e == "mock"),
        "an engine other than the mock ran",
    );
    g
}

// ═══ Slice 2 ═════════════════════════════════════════════════════════════════
//
// Frozen before the real engine adapter, the ladder's listing filter, ACP
// outward and the startup ladder were first run. Every slice-2 run is
// loopback-only: a scripted provider on 127.0.0.1 answers in the
// OpenAI-compatible wire shape a router documents. No live model, no live
// Jev, no key.

/// G-n: a host tool result at least this long (the CLI history's shortening
/// threshold) must reach a later request of its epoch unchanged.
pub const G_N_LONG_RESULT_CHARS: usize = 4_000;

/// What the scripted loopback provider served for one completion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Served {
    pub model: String,
    pub prompt: u64,
    pub cached: u64,
    pub cache_write: u64,
    pub completion: u64,
    pub cost_usd: f64,
}

/// One HTTP request as the scripted loopback provider received it.
#[derive(Debug, Clone, Serialize)]
pub struct HttpSeen {
    pub method: String,
    pub path: String,
    /// The connection came from a loopback address.
    pub loopback: bool,
    /// It carried an `Authorization` header.
    pub auth: bool,
    /// The request body as JSON (`Null` when it had none).
    pub body: Value,
    pub status: u16,
    /// A completion it served.
    pub served: Option<Served>,
    /// A 429 it sent: `provider` (an upstream's, with the router's provider
    /// metadata) or `platform` (the router's own, with `X-RateLimit-*`).
    pub refusal: Option<String>,
    /// The `X-RateLimit-Reset` it sent, ms since the Unix epoch.
    pub reset_at: Option<i64>,
}

/// The turn a request belongs to: the newest turn header in its messages.
pub fn request_turn(body: &Value) -> Option<u64> {
    let msgs = body["messages"].as_array()?;
    msgs.iter().rev().find_map(|m| {
        if m["role"] != "user" {
            return None;
        }
        let text = match &m["content"] {
            Value::String(t) => t.clone(),
            Value::Array(parts) => parts
                .first()
                .and_then(|p| p["text"].as_str())
                .unwrap_or("")
                .to_string(),
            _ => return None,
        };
        let rest = text.strip_prefix("[turn ")?;
        rest.split(' ').next()?.parse().ok()
    })
}

fn markers(v: &Value) -> usize {
    match v {
        Value::Object(m) => {
            usize::from(m.contains_key("cache_control")) + m.values().map(markers).sum::<usize>()
        }
        Value::Array(a) => a.iter().map(markers).sum(),
        _ => 0,
    }
}

fn last_part_marked(m: &Value) -> bool {
    m["content"]
        .as_array()
        .and_then(|a| a.last())
        .is_some_and(|p| p.get("cache_control").is_some())
}

// ─── G-n the real engine adapter ─────────────────────────────────────────────

/// G-n: the host runs `rung-agent-core`'s engine through the adapter
/// against a loopback provider. The record and the wire agree, and the
/// cache discipline holds on the real request bodies:
///
/// - every request is loopback, and every `llm.call` was served by it;
/// - each request asks for the model of the turn that sent it, with the
///   turn's epoch as `session_id` (L14);
/// - the stable and slow layers' ends carry the only two cache breakpoints
///   (L14, lowered on the OpenAI-compatible wire);
/// - inside a session each request extends the previous one: the same
///   tools, the previous messages as a prefix — nothing is rewritten
///   mid-epoch (L12). A turn's last step (tools withdrawn, one closing
///   instruction appended) extends it too, without its last message;
/// - a long host tool result reaches a later request unchanged;
/// - every `llm.call` carries the usage, cache and cost the provider served
///   (L11, as the adapter reads it);
/// - the model's tool calls ran through the host's tools (a note was
///   written; a disabled tool was refused);
/// - a provider 429 is recorded as the provider's and steps the ladder down,
///   later probing up; a platform 429 is recorded as the platform's, waits
///   for its `X-RateLimit-Reset` and does not step down;
/// - no money moved.
pub fn g_n(lines: &[Line], seen: &[HttpSeen]) -> GateResult {
    let mut g = GateResult::new("G-n");
    let chats: Vec<&HttpSeen> = seen
        .iter()
        .filter(|h| h.method == "POST" && h.path.ends_with("/chat/completions"))
        .collect();
    let not_loopback = seen.iter().filter(|h| !h.loopback).count();
    // turn → (epoch, model)
    let mut turns: BTreeMap<u64, (u64, String)> = BTreeMap::new();
    for l in of(lines, "turn.started") {
        turns.insert(l.u64("turn"), (l.u64("epoch"), l.str("model").to_string()));
    }
    let (mut unplaced, mut wrong_model, mut wrong_session) = (0, 0, 0);
    let (mut bad_markers, mut with_tools) = (0, 0);
    let (mut extends, mut broken, mut last_steps, mut last_broken) = (0, 0, 0, 0);
    let mut prev: BTreeMap<String, (Value, Vec<Value>)> = BTreeMap::new();
    let mut long_sent: Vec<(String, String)> = Vec::new();
    let mut long_verbatim = 0;
    for h in &chats {
        let b = &h.body;
        let Some(turn) = request_turn(b) else {
            unplaced += 1;
            continue;
        };
        let Some((epoch, model)) = turns.get(&turn) else {
            unplaced += 1;
            continue;
        };
        if b["model"].as_str() != Some(model.as_str()) {
            wrong_model += 1;
        }
        let session = b["session_id"].as_str().unwrap_or("").to_string();
        if session != format!("epoch-{epoch}") {
            wrong_session += 1;
        }
        let msgs: Vec<Value> = b["messages"].as_array().cloned().unwrap_or_default();
        let has_tools = b.get("tools").is_some();
        if has_tools {
            with_tools += 1;
            let ok = msgs.len() >= 2
                && msgs[0]["role"] == "system"
                && last_part_marked(&msgs[0])
                && last_part_marked(&msgs[1])
                && markers(b) == 2;
            if !ok {
                bad_markers += 1;
            }
        }
        // Long tool results: sent once, then seen again unchanged.
        for m in &msgs {
            if m["role"] == "tool"
                && let Some(c) = m["content"].as_str()
                && c.chars().count() >= G_N_LONG_RESULT_CHARS
            {
                let key = (session.clone(), c.to_string());
                if long_sent.contains(&key) {
                    long_verbatim += 1;
                } else {
                    long_sent.push(key);
                }
            }
        }
        if let Some((ptools, pmsgs)) = prev.get(&session) {
            if has_tools {
                if *ptools == b["tools"]
                    && msgs.len() >= pmsgs.len()
                    && msgs[..pmsgs.len()] == pmsgs[..]
                {
                    extends += 1;
                } else {
                    broken += 1;
                }
            } else {
                last_steps += 1;
                let body = &msgs[..msgs.len().saturating_sub(1)];
                if !(body.len() >= pmsgs.len() && body[..pmsgs.len()] == pmsgs[..]) {
                    last_broken += 1;
                }
            }
        }
        if has_tools {
            prev.insert(session, (b["tools"].clone(), msgs));
        }
    }
    // The record's calls against what was served, in order.
    let served: Vec<&Served> = chats.iter().filter_map(|h| h.served.as_ref()).collect();
    let calls: Vec<&Line> = of(lines, "llm.call").collect();
    let mut usage_bad = 0;
    for (l, s) in calls.iter().zip(served.iter()) {
        let same = l.str("model_served") == s.model
            && l.u64("prompt_tokens") == s.prompt
            && l.u64("cached_tokens") == s.cached
            && l.u64("cache_write_tokens") == s.cache_write
            && l.u64("completion_tokens") == s.completion
            && (l.f64("cost_usd") - s.cost_usd).abs() < 1e-12;
        if !same {
            usage_bad += 1;
        }
    }
    // Tools ran through the host.
    let notes = of(lines, "note.written").count();
    let tool_ok = of(lines, "tool.call")
        .filter(|l| l.get("ok") == &Value::Bool(true))
        .count();
    let refused_disabled = of(lines, "tool.refused")
        .filter(|l| l.str("why") == "disabled")
        .count();
    // Failures: who refused, what followed.
    let refusals_of = |turn: u64| -> BTreeSet<String> {
        chats
            .iter()
            .filter(|h| request_turn(&h.body) == Some(turn))
            .filter_map(|h| h.refusal.clone())
            .collect()
    };
    let (mut provider_429, mut platform_429) = (0, 0);
    let (mut origin_bad, mut follow_bad) = (0, 0);
    let mut first_down: Option<u64> = None;
    for (i, l) in lines.iter().enumerate() {
        if l.kind != "turn.ended" || l.get("failure")["class"] != "rate_limit" {
            continue;
        }
        let f = l.get("failure");
        let origin = f["origin"].as_str().unwrap_or("");
        let sent = refusals_of(l.u64("turn"));
        if sent.len() != 1 || !sent.contains(origin) {
            origin_bad += 1;
        }
        let after: Vec<&Line> = lines[i + 1..]
            .iter()
            .take_while(|x| x.kind != "turn.started")
            .collect();
        let down = after
            .iter()
            .any(|x| x.kind == "model.switch" && x.str("direction") == "down");
        match origin {
            "provider" => {
                provider_429 += 1;
                if !down {
                    follow_bad += 1;
                }
                first_down.get_or_insert(l.seq);
            }
            "platform" => {
                platform_429 += 1;
                let reset = f["reset_at"].as_i64();
                // The reset of the last refused attempt: the one that
                // stopped the turn.
                let sent_reset = chats
                    .iter()
                    .filter(|h| request_turn(&h.body) == Some(l.u64("turn")))
                    .filter_map(|h| h.reset_at)
                    .next_back();
                let waited = after.iter().any(|x| {
                    x.kind == "degraded"
                        && x.str("class") == "quota"
                        && Some(x.i64("until")) == reset
                });
                if down || reset.is_none() || reset != sent_reset || !waited {
                    follow_bad += 1;
                }
            }
            _ => origin_bad += 1,
        }
    }
    let probed_up = first_down.is_some_and(|seq| {
        of(lines, "model.switch").any(|x| x.seq > seq && x.str("direction") == "up")
    });
    let cost: f64 = calls.iter().map(|l| l.f64("cost_usd")).sum();
    g.put("requests", seen.len());
    g.put("chat_requests", chats.len());
    g.put("not_loopback", not_loopback);
    g.put("llm_calls", calls.len());
    g.put("served", served.len());
    g.put("unplaced", unplaced);
    g.put("wrong_model", wrong_model);
    g.put("wrong_session", wrong_session);
    g.put("with_tools", with_tools);
    g.put("bad_breakpoints", bad_markers);
    g.put("extends", extends);
    g.put("prefix_breaks", broken);
    g.put("last_steps", last_steps);
    g.put("last_step_breaks", last_broken);
    g.put("long_results_verbatim", long_verbatim);
    g.put("usage_mismatch", usage_bad);
    g.put("tool_calls_ok", tool_ok);
    g.put("notes_written", notes);
    g.put("refused_disabled", refused_disabled);
    g.put("provider_429_turns", provider_429);
    g.put("platform_429_turns", platform_429);
    g.put("origin_mismatch", origin_bad);
    g.put("follow_mismatch", follow_bad);
    g.put("probed_up", probed_up);
    g.put("cost_usd", cost);
    g.check(not_loopback == 0, "a request left loopback");
    g.check(!calls.is_empty(), "no model call ran");
    g.check(
        calls.len() == served.len(),
        "the record's calls and the served completions differ in number",
    );
    g.check(unplaced == 0, "a request belongs to no recorded turn");
    g.check(
        wrong_model == 0,
        "a request asked for another model than its turn's",
    );
    g.check(
        wrong_session == 0,
        "a request's session_id is not its turn's epoch",
    );
    g.check(
        with_tools > 0 && bad_markers == 0,
        "the cache breakpoints are not exactly the stable and slow layers' ends",
    );
    g.check(
        extends > 0 && broken == 0,
        "a request in a session does not extend the previous",
    );
    g.check(
        last_steps > 0 && last_broken == 0,
        "no turn reached its last step, or one does not extend the previous request",
    );
    g.check(
        long_verbatim > 0,
        "no long tool result was seen again unchanged",
    );
    g.check(usage_bad == 0, "an llm.call does not carry what was served");
    g.check(
        notes > 0 && tool_ok > 0 && refused_disabled > 0,
        "the model's tool calls did not run through the host's tools",
    );
    g.check(
        provider_429 > 0 && platform_429 > 0,
        "the run did not meet both a provider and a platform 429",
    );
    g.check(origin_bad == 0, "a 429 was recorded with the wrong origin");
    g.check(
        follow_bad == 0,
        "a 429 was not followed by its plan (provider: step down; platform: wait for the reset, no step down)",
    );
    g.check(
        probed_up,
        "the ladder never probed back up after stepping down",
    );
    g.check(cost == 0.0, "money was spent");
    g
}

// ─── G-o the model ladder's listing filter ───────────────────────────────────

/// G-o: the listing is refreshed this often, ms (6 h).
pub const G_O_REFRESH_MS: i64 = 6 * 3_600_000;
/// G-o: a failed listing is tried again within this, ms (15 min).
pub const G_O_RETRY_MS: i64 = 15 * 60_000;
/// G-o: a due listing may wait for the boundary after a running turn, ms
/// (the default turn bound).
pub const G_O_SLACK_MS: i64 = 120_000;

/// Days since 1970-01-01 of a `YYYY-MM-DD` date (proleptic Gregorian).
fn days_of(date: &str) -> Option<i64> {
    let mut it = date.get(..10)?.split('-');
    let (y, m, d): (i64, i64, i64) = (
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    );
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// What the filter must say of `model` at `now`, from the listing and the
/// endpoint answers the scripted router served: `(available, why)`.
///
/// Available when it is listed, its `expiration_date` (if any) has not
/// begun, its prompt and completion prices are zero, it takes `tools`, and
/// one of its endpoints has a status of at least 0.
pub fn listing_oracle(
    listing: &Value,
    endpoints: &BTreeMap<String, Value>,
    model: &str,
    now: i64,
) -> (bool, &'static str) {
    let Some(m) = listing["data"]
        .as_array()
        .and_then(|d| d.iter().find(|m| m["id"] == model))
    else {
        return (false, "not_listed");
    };
    if let Some(day) = m["expiration_date"].as_str().and_then(days_of)
        && now >= day * 86_400_000
    {
        return (false, "expired");
    }
    let zero = |v: &Value| {
        v.as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| v.as_f64())
            == Some(0.0)
    };
    if !(zero(&m["pricing"]["prompt"]) && zero(&m["pricing"]["completion"])) {
        return (false, "not_free");
    }
    let tools = m["supported_parameters"]
        .as_array()
        .is_some_and(|p| p.iter().any(|x| x == "tools"));
    if !tools {
        return (false, "no_tools");
    }
    let up = endpoints.get(model).is_some_and(|e| {
        e["data"]["endpoints"].as_array().is_some_and(|eps| {
            eps.iter()
                .any(|x| x["status"].as_i64().is_some_and(|s| s >= 0))
        })
    });
    if !up {
        return (false, "endpoint_down");
    }
    (true, "ok")
}

/// G-o: the ladder lists the router's models at start and every 6 hours
/// (keyless GETs), keeps only the configured rungs that are available and
/// free, and walks only those:
///
/// - the first `ladder.listed` comes before the first turn;
/// - every successful listing's verdict per rung is the oracle's
///   ([`listing_oracle`]) at its time; a rung's expiry is seen;
/// - a failed listing keeps the previous verdicts and is retried within
///   15 minutes; a successful one is refreshed within 6 hours;
/// - every turn runs on a rung the latest listing called available;
/// - a listing that takes the current rung away is followed by a switch to
///   an available rung before the next turn;
/// - a provider's 429 steps down to the next *available* rung (skipping at
///   least once), a probe goes up to the nearest available one;
/// - every listing GET is keyless and each one is on record; loopback, $0.
pub fn g_o(
    lines: &[Line],
    seen: &[HttpSeen],
    listing: &Value,
    endpoints: &BTreeMap<String, Value>,
    ladder: &[String],
) -> GateResult {
    let mut g = GateResult::new("G-o");
    let listed: Vec<&Line> = of(lines, "ladder.listed").collect();
    let first_turn = of(lines, "turn.started").map(|l| l.seq).next();
    let at_start = match (listed.first(), first_turn) {
        (Some(l), Some(t)) => l.seq < t,
        _ => false,
    };
    let flags = |l: &Line| -> Vec<bool> {
        l.get("rungs")
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|r| r["available"].as_bool().unwrap_or(false))
                    .collect()
            })
            .unwrap_or_default()
    };
    let (mut ok_n, mut failed_n, mut verdict_bad, mut kept_bad) = (0, 0, 0, 0);
    let (mut late_refresh, mut late_retry) = (0, 0);
    let mut expired_seen = false;
    let mut prev: Option<&Line> = None;
    for (i, l) in listed.iter().enumerate() {
        let ok = l.get("ok").as_bool().unwrap_or(false);
        let rungs = l.get("rungs").as_array().cloned().unwrap_or_default();
        if rungs.len() != ladder.len() {
            verdict_bad += 1;
        }
        if ok {
            ok_n += 1;
            for (r, model) in ladder.iter().enumerate() {
                let want = listing_oracle(listing, endpoints, model, l.at);
                let got = rungs.get(r).map(|x| {
                    (
                        x["available"].as_bool().unwrap_or(false),
                        x["why"].as_str().unwrap_or("").to_string(),
                    )
                });
                if got != Some((want.0, want.1.to_string())) {
                    verdict_bad += 1;
                }
                if want.1 == "expired" {
                    expired_seen =
                        expired_seen || prev.is_some_and(|p| flags(p).get(r) == Some(&true));
                }
            }
        } else {
            failed_n += 1;
            if let Some(p) = prev
                && flags(p) != flags(l)
            {
                kept_bad += 1;
            }
        }
        if let Some(next) = listed.get(i + 1) {
            let limit = if ok { G_O_REFRESH_MS } else { G_O_RETRY_MS };
            if next.at - l.at > limit + G_O_SLACK_MS {
                if ok {
                    late_refresh += 1;
                } else {
                    late_retry += 1;
                }
            }
        }
        prev = Some(l);
    }
    // Walk the record: availability, the current rung, switches.
    let mut avail: Vec<bool> = Vec::new();
    let mut current: Option<usize> = None;
    let (mut off_rung, mut stranded) = (0, 0);
    let (mut down_bad, mut up_bad, mut skipped) = (0, 0, 0);
    let mut owe_switch = false;
    for l in lines {
        match l.kind.as_str() {
            "ladder.listed" => {
                avail = flags(l);
                if let Some(c) = current
                    && avail.get(c) == Some(&false)
                    && avail.iter().any(|a| *a)
                {
                    owe_switch = true;
                }
            }
            "model.switch" => {
                let (from, to) = (l.u64("rung_from") as usize, l.u64("rung_to") as usize);
                let why = l.str("why");
                if why.starts_with("provider") {
                    let want = (from + 1..ladder.len()).find(|r| avail.get(*r) != Some(&false));
                    if want != Some(to) {
                        down_bad += 1;
                    }
                    if to > from + 1 {
                        skipped += 1;
                    }
                } else if why.starts_with("probe") {
                    let want = (0..from).rev().find(|r| avail.get(*r) != Some(&false));
                    if want != Some(to) {
                        up_bad += 1;
                    }
                }
                if avail.get(to) != Some(&false) {
                    owe_switch = false;
                }
                current = Some(to);
            }
            "turn.started" => {
                let r = l.u64("rung") as usize;
                if avail.get(r) == Some(&false) && avail.iter().any(|a| *a) {
                    off_rung += 1;
                }
                if owe_switch {
                    stranded += 1;
                    owe_switch = false;
                }
                current = Some(r);
            }
            _ => {}
        }
    }
    let gets: Vec<&HttpSeen> = seen.iter().filter(|h| h.method == "GET").collect();
    let model_gets = gets.iter().filter(|h| h.path.ends_with("/models")).count();
    let keyed_gets = gets.iter().filter(|h| h.auth).count();
    let not_loopback = seen.iter().filter(|h| !h.loopback).count();
    let cost: f64 = of(lines, "llm.call").map(|l| l.f64("cost_usd")).sum();
    g.put("listings", listed.len());
    g.put("ok_listings", ok_n);
    g.put("failed_listings", failed_n);
    g.put("listed_at_start", at_start);
    g.put("verdict_mismatch", verdict_bad);
    g.put("failed_not_kept", kept_bad);
    g.put("expired_seen", expired_seen);
    g.put("late_refresh", late_refresh);
    g.put("late_retry", late_retry);
    g.put("turns_on_unavailable", off_rung);
    g.put("stranded_after_listing", stranded);
    g.put("down_mismatch", down_bad);
    g.put("down_skips", skipped);
    g.put("up_mismatch", up_bad);
    g.put("model_gets", model_gets);
    g.put("keyed_gets", keyed_gets);
    g.put("not_loopback", not_loopback);
    g.put("cost_usd", cost);
    g.check(at_start, "the models were not listed before the first turn");
    g.check(
        ok_n >= 2 && failed_n >= 1,
        "the run did not see both successful and failed listings",
    );
    g.check(
        verdict_bad == 0,
        "a listing's verdict differs from the oracle's",
    );
    g.check(kept_bad == 0, "a failed listing changed the verdicts");
    g.check(expired_seen, "no rung was seen to expire");
    g.check(
        late_refresh == 0,
        "a listing was refreshed later than 6 hours",
    );
    g.check(
        late_retry == 0,
        "a failed listing was retried later than 15 minutes",
    );
    g.check(
        off_rung == 0,
        "a turn ran on a rung the listing called unavailable",
    );
    g.check(
        stranded == 0,
        "a listing took the current rung away and no switch followed",
    );
    g.check(
        down_bad == 0 && skipped > 0,
        "a provider step-down did not land on the next available rung, or none skipped one",
    );
    g.check(
        up_bad == 0,
        "a probe did not go to the nearest available rung above",
    );
    g.check(
        model_gets == listed.len(),
        "the listing GETs and the ladder.listed lines differ in number",
    );
    g.check(keyed_gets == 0, "a listing GET carried a key");
    g.check(not_loopback == 0, "a request left loopback");
    g.check(cost == 0.0, "money was spent");
    g
}

// ─── G-p ACP outward ─────────────────────────────────────────────────────────

/// G-p: the owner's stop ends the process within this, ms.
pub const G_P_STOP_MS: u64 = 5_000;

/// What an ACP test client saw and measured.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AcpRun {
    /// Every JSON-RPC message, in order: (sent by the client, message).
    pub wire: Vec<(bool, Value)>,
    /// `_rung/stimulus` acks: (item id, its `stimulus.accepted` line was on
    /// disk when the ack arrived).
    pub durable_acks: Vec<(String, bool)>,
    /// After the owner's `_rung/stop`: the exit code and ms until exit.
    pub exit: Option<(i32, u64)>,
}

fn text_of_chunk(m: &Value) -> Option<(String, String)> {
    if m["method"] != "session/update" {
        return None;
    }
    let p = &m["params"];
    let u = &p["update"];
    if u["sessionUpdate"] != "agent_message_chunk" {
        return None;
    }
    Some((
        p["sessionId"].as_str()?.to_string(),
        u["content"]["text"].as_str()?.to_string(),
    ))
}

/// G-p: ACP outward. One agent; each `session/new` is a channel to it.
///
/// - every prompt gets exactly one response; an observer's prompt is
///   refused;
/// - an answered prompt's response names the item and the turn that
///   disposed it, as the record does; its streamed text ends with what the
///   agent sent that channel in that turn (or the turn's final text when it
///   sent nothing and its channel is the highest-role channel with a prompt
///   admitted in that turn; any other channel gets neither the final text
///   nor tool calls); its `admitted_with` names no other channel's items;
/// - a cancelled prompt is answered `cancelled` and its item was withdrawn
///   (or was already in a turn); at least one cancel is exercised;
/// - an agent-initiated message reaches the opted-in client as
///   `_rung/outbox`, matching an `outbox.queued` line of a turn with no open
///   prompt from that channel;
/// - `_rung/stimulus` is durable before its ack;
/// - a peer's `_rung/stop`, `_rung/calendar` and `_rung/release` are
///   refused; the owner's calendar entry is added and fires;
/// - `_rung/status` reports the now set; `session/list` lists every channel
///   and `session/load` reopens one;
/// - the owner's `_rung/stop` halts the host, open prompts are answered,
///   and the process exits 0 within [`G_P_STOP_MS`];
/// - the mock engine only; $0.
pub fn g_p(lines: &[Line], run: &AcpRun) -> GateResult {
    let mut g = GateResult::new("G-p");
    // Requests by id, and their responses.
    let mut requests: BTreeMap<String, Value> = BTreeMap::new();
    let mut sent_index: BTreeMap<String, usize> = BTreeMap::new();
    let mut responses: BTreeMap<String, Vec<(usize, Value)>> = BTreeMap::new();
    for (i, (out, m)) in run.wire.iter().enumerate() {
        let id = match &m["id"] {
            Value::Null => continue,
            v => v.to_string(),
        };
        if *out && m.get("method").is_some() {
            sent_index.insert(id.clone(), i);
            requests.insert(id, m.clone());
        } else if !*out && (m.get("result").is_some() || m.get("error").is_some()) {
            responses.entry(id).or_default().push((i, m.clone()));
        }
    }
    // Sessions: id → (role, channel).
    let mut sessions: BTreeMap<String, String> = BTreeMap::new();
    for (id, req) in &requests {
        if req["method"] == "session/new"
            && let Some((_, r)) = responses.get(id).and_then(|v| v.first())
            && let Some(sid) = r["result"]["sessionId"].as_str()
        {
            let role = req["params"]["_meta"]["rung"]["role"]
                .as_str()
                .unwrap_or("owner")
                .to_string();
            sessions.insert(sid.to_string(), role);
        }
    }
    let role_of = |req: &Value| -> String {
        req["params"]["sessionId"]
            .as_str()
            .and_then(|s| sessions.get(s))
            .cloned()
            .unwrap_or_default()
    };
    // Record facts.
    let mut disposed: BTreeMap<String, (String, u64)> = BTreeMap::new();
    let mut admitted_turn: BTreeMap<String, u64> = BTreeMap::new();
    let mut sends: BTreeMap<(u64, String), Vec<String>> = BTreeMap::new();
    let mut finals: BTreeMap<u64, String> = BTreeMap::new();
    let mut tools: BTreeMap<u64, usize> = BTreeMap::new();
    let mut item_channel: BTreeMap<String, String> = BTreeMap::new();
    let mut item_role: BTreeMap<String, String> = BTreeMap::new();
    for l in lines {
        match l.kind.as_str() {
            "stimulus.accepted" => {
                let it = l.get("item");
                if let (Some(id), Some(ch)) = (it["id"].as_str(), it["channel"].as_str()) {
                    item_channel.insert(id.to_string(), ch.to_string());
                    item_role.insert(
                        id.to_string(),
                        it["role"].as_str().unwrap_or("").to_lowercase(),
                    );
                }
            }
            "stimulus.disposed" => {
                disposed.insert(
                    l.str("id").to_string(),
                    (l.str("disposition").to_string(), l.u64("turn")),
                );
            }
            "stimulus.admitted" => {
                for id in crate::inbox::ids(l.get("ids"))
                    .into_iter()
                    .chain(crate::inbox::ids(l.get("digests")))
                {
                    admitted_turn.insert(id, l.u64("turn"));
                }
            }
            "outbox.queued" => sends
                .entry((l.u64("turn"), l.str("channel").to_string()))
                .or_default()
                .push(l.str("text").to_string()),
            "turn.ended" => {
                finals.insert(l.u64("turn"), l.str("final_text").to_string());
            }
            "tool.call" => *tools.entry(l.u64("turn")).or_default() += 1,
            _ => {}
        }
    }
    let rank = |r: &str| match r {
        "owner" => 3,
        "peer" => 2,
        "observer" => 1,
        _ => 0,
    };
    let mut owning: BTreeMap<u64, (i32, String)> = BTreeMap::new();
    for (item, turn) in &admitted_turn {
        let Some(ch) = item_channel.get(item) else {
            continue;
        };
        let r = rank(item_role.get(item).map_or("", String::as_str));
        if r == 0 {
            continue;
        }
        let e = owning.entry(*turn).or_insert((r, ch.clone()));
        if r > e.0 {
            *e = (r, ch.clone());
        }
    }
    let (mut prompts, mut not_one, mut refused_ok, mut observer_prompts) = (0, 0, 0, 0);
    let (mut answered, mut answer_bad, mut tools_bad, mut foreign) = (0, 0, 0, 0);
    let (mut cancelled, mut cancel_bad) = (0, 0);
    let mut answered_channels: BTreeSet<(u64, String)> = BTreeSet::new();
    for (id, req) in &requests {
        if req["method"] != "session/prompt" {
            continue;
        }
        prompts += 1;
        let got = responses.get(id).cloned().unwrap_or_default();
        if got.len() != 1 {
            not_one += 1;
            continue;
        }
        let (at, resp) = &got[0];
        let role = role_of(req);
        if role == "observer" {
            observer_prompts += 1;
            if resp.get("error").is_some() {
                refused_ok += 1;
            }
            continue;
        }
        let sid = req["params"]["sessionId"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let sent_at = sent_index.get(id).copied().unwrap_or(0);
        let meta = &resp["result"]["_meta"]["rung"];
        let item = meta["item"].as_str().unwrap_or("").to_string();
        match resp["result"]["stopReason"].as_str() {
            Some("end_turn") => {
                answered += 1;
                let turn = meta["turn"].as_u64().unwrap_or(0);
                let rec = disposed.get(&item);
                let rec_ok =
                    rec.is_some_and(|(d, t)| *t == turn && (d == "answered" || d == "digested"));
                let channel = item_channel.get(&item).cloned().unwrap_or_default();
                let chunks: Vec<String> = run.wire[sent_at..*at]
                    .iter()
                    .filter(|(out, _)| !*out)
                    .filter_map(|(_, m)| text_of_chunk(m))
                    .filter(|(s, _)| *s == sid)
                    .map(|(_, t)| t)
                    .collect();
                let owns = owning.get(&turn).is_some_and(|(_, c)| *c == channel);
                let want: Vec<String> = match sends.get(&(turn, channel.clone())) {
                    Some(v) if !v.is_empty() => v.clone(),
                    _ if owns => finals
                        .get(&turn)
                        .filter(|t| !t.is_empty())
                        .map(|t| vec![t.clone()])
                        .unwrap_or_default(),
                    _ => Vec::new(),
                };
                let tail_ok =
                    chunks.len() >= want.len() && chunks[chunks.len() - want.len()..] == want[..];
                if !rec_ok || !tail_ok || (owns && want.is_empty()) {
                    answer_bad += 1;
                }
                let sent_here: BTreeSet<&String> = sends
                    .iter()
                    .filter(|((_, c), _)| *c == channel)
                    .flat_map(|(_, v)| v)
                    .collect();
                if !owns && chunks.iter().any(|t| !sent_here.contains(t)) {
                    foreign += 1;
                }
                let mine: BTreeSet<&String> = item_channel
                    .iter()
                    .filter(|(_, c)| **c == channel)
                    .map(|(i, _)| i)
                    .collect();
                if meta["admitted_with"].as_array().is_some_and(|a| {
                    a.iter()
                        .any(|x| x.as_str().is_none_or(|x| !mine.contains(&x.to_string())))
                }) {
                    foreign += 1;
                }
                answered_channels.insert((turn, channel));
                let calls = run.wire[sent_at..*at]
                    .iter()
                    .filter(|(out, m)| {
                        !*out
                            && m["method"] == "session/update"
                            && m["params"]["sessionId"] == sid.as_str()
                            && m["params"]["update"]["sessionUpdate"] == "tool_call"
                    })
                    .count();
                let want_calls = if owns {
                    tools.get(&turn).copied().unwrap_or(0)
                } else {
                    0
                };
                if calls < want_calls || (!owns && calls > 0) {
                    tools_bad += 1;
                }
            }
            Some("cancelled") => {
                cancelled += 1;
                let ok = match disposed.get(&item) {
                    Some((d, _)) if d == "withdrawn" => true,
                    // Already in a turn when it was cancelled, or open at
                    // the halt.
                    _ => {
                        admitted_turn.contains_key(&item) || meta["halted"].as_bool() == Some(true)
                    }
                };
                if !ok {
                    cancel_bad += 1;
                }
            }
            _ => answer_bad += 1,
        }
    }
    let cancels_sent = run
        .wire
        .iter()
        .filter(|(out, m)| *out && m["method"] == "session/cancel")
        .count();
    // Outbox notifications.
    let (mut outbox, mut outbox_bad) = (0, 0);
    for (out, m) in &run.wire {
        if *out || m["method"] != "_rung/outbox" {
            continue;
        }
        outbox += 1;
        let p = &m["params"];
        let (turn, ch, text) = (
            p["turn"].as_u64().unwrap_or(0),
            p["channel"].as_str().unwrap_or("").to_string(),
            p["text"].as_str().unwrap_or("").to_string(),
        );
        let queued = sends
            .get(&(turn, ch.clone()))
            .is_some_and(|v| v.contains(&text));
        if !queued || answered_channels.contains(&(turn, ch)) {
            outbox_bad += 1;
        }
    }
    // Refusals by role.
    let mut forbidden_ok = 0;
    let mut forbidden = 0;
    for (id, req) in &requests {
        let m = req["method"].as_str().unwrap_or("");
        if matches!(m, "_rung/stop" | "_rung/calendar" | "_rung/release") && role_of(req) == "peer"
        {
            forbidden += 1;
            if responses
                .get(id)
                .and_then(|v| v.first())
                .is_some_and(|(_, r)| r.get("error").is_some())
            {
                forbidden_ok += 1;
            }
        }
    }
    // The owner's calendar entry.
    let cal_ids: Vec<String> = requests
        .iter()
        .filter(|(_, r)| r["method"] == "_rung/calendar" && role_of(r) == "owner")
        .filter_map(|(_, r)| r["params"]["id"].as_str().map(str::to_string))
        .collect();
    let cal_ok = !cal_ids.is_empty()
        && cal_ids.iter().all(|id| {
            of(lines, "calendar.added").any(|l| l.get("entry")["id"] == id.as_str())
                && of(lines, "calendar.fired").any(|l| l.str("id") == id)
        });
    // Status, list, load.
    let mut status_ok = false;
    let mut list_ok = false;
    let mut load_ok = false;
    for (id, req) in &requests {
        let r = responses
            .get(id)
            .and_then(|v| v.first())
            .map(|x| x.1.clone());
        let Some(r) = r else { continue };
        match req["method"].as_str() {
            Some("_rung/status") => {
                let s = &r["result"];
                status_ok |= [
                    "turn", "mode", "model", "ladder", "desk", "epoch", "quota", "degraded",
                ]
                .iter()
                .all(|k| s.get(*k).is_some())
                    && s["turn"].as_u64().unwrap_or(0) >= 1;
            }
            Some("session/list") => {
                let listed: BTreeSet<String> = r["result"]["sessions"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x["sessionId"].as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                list_ok |= !sessions.is_empty() && sessions.keys().all(|s| listed.contains(s));
            }
            Some("session/load") => load_ok |= r.get("result").is_some(),
            _ => {}
        }
    }
    let durable = run.durable_acks.iter().filter(|(_, d)| *d).count();
    let halted_by_owner =
        of(lines, "halted").any(|l| l.get("why").to_string().contains("\"owner\""));
    let exit_ok = run
        .exit
        .is_some_and(|(code, ms)| code == 0 && ms <= G_P_STOP_MS);
    let engines: BTreeSet<String> = of(lines, "host.start")
        .map(|l| l.get("config")["engine"].as_str().unwrap_or("").to_string())
        .collect();
    let cost: f64 = of(lines, "llm.call").map(|l| l.f64("cost_usd")).sum();
    g.put("sessions", sessions.len());
    g.put("prompts", prompts);
    g.put("not_one_response", not_one);
    g.put("observer_prompts", observer_prompts);
    g.put("observer_refused", refused_ok);
    g.put("answered", answered);
    g.put("answer_mismatch", answer_bad);
    g.put("tool_updates_missing", tools_bad);
    g.put("foreign_output", foreign);
    g.put("cancels_sent", cancels_sent);
    g.put("cancelled", cancelled);
    g.put("cancel_mismatch", cancel_bad);
    g.put("outbox", outbox);
    g.put("outbox_mismatch", outbox_bad);
    g.put("forbidden", forbidden);
    g.put("forbidden_refused", forbidden_ok);
    g.put("calendar_added_and_fired", cal_ok);
    g.put("status_ok", status_ok);
    g.put("list_ok", list_ok);
    g.put("load_ok", load_ok);
    g.put("durable_acks", durable);
    g.put("acks", run.durable_acks.len());
    g.put("halted_by_owner", halted_by_owner);
    g.put("exit", json!(run.exit));
    g.put("cost_usd", cost);
    g.check(
        prompts > 0 && not_one == 0,
        "a prompt did not get exactly one response",
    );
    g.check(
        observer_prompts > 0 && refused_ok == observer_prompts,
        "an observer's prompt was not refused",
    );
    g.check(
        answered >= 2 && answer_bad == 0,
        "an answered prompt does not match the record",
    );
    g.check(
        tools_bad == 0,
        "an owning channel missed its turn's tool calls, or another channel got them",
    );
    g.check(
        foreign == 0,
        "a channel received output of work it does not own, or another channel's item id",
    );
    g.check(
        cancels_sent > 0 && cancelled > 0 && cancel_bad == 0,
        "a cancel was not exercised, or a cancelled prompt does not match the record",
    );
    g.check(
        outbox > 0 && outbox_bad == 0,
        "no agent-initiated message reached the client as _rung/outbox, or one does not match",
    );
    g.check(
        forbidden >= 3 && forbidden_ok == forbidden,
        "a peer's owner-only request was not refused",
    );
    g.check(cal_ok, "the owner's calendar entry was not added and fired");
    g.check(status_ok, "_rung/status did not report the now set");
    g.check(list_ok && load_ok, "session/list or session/load failed");
    g.check(
        !run.durable_acks.is_empty() && durable == run.durable_acks.len(),
        "a _rung/stimulus was acknowledged before it was on disk",
    );
    g.check(
        halted_by_owner && exit_ok,
        "the owner's stop did not halt the host and exit 0 in time",
    );
    g.check(
        engines.iter().all(|e| e == "mock") && cost == 0.0,
        "not the mock engine, or money was spent",
    );
    g
}

// ─── G-q ACP over Streamable HTTP ────────────────────────────────────────────

/// What a Streamable HTTP ACP test client measured.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HttpAcpRun {
    /// HTTP status by probe name: `no_bearer`, `bad_bearer`, `owner_init`,
    /// `peer_init`, `switched_post`, `switched_get`, `switched_delete`.
    pub status: BTreeMap<String, u16>,
    /// JSON-RPC responses by probe name: `peer_opens_owner`,
    /// `peer_opens_peer`, `owner_opens_owner`, `peer_prompt`, `peer_stop`,
    /// `owner_stop`.
    pub replies: BTreeMap<String, Value>,
    /// After the owner's stop: the exit code and ms until exit.
    pub exit: Option<(i32, u64)>,
    /// Exit codes of the refused starts: `no_tokens`, `unset_token_env`.
    pub refused_starts: BTreeMap<String, i32>,
    /// The token values the run used (they must not reach the record).
    pub tokens: Vec<String>,
    /// The address it bound.
    pub bound: String,
}

/// G-q: ACP over Streamable HTTP with per-role bearer tokens.
///
/// - a request with no token, or an unknown one, is refused (401);
/// - a token's role caps its connection: the peer token cannot open an
///   owner channel, but can open a peer one; the owner token can open an
///   owner channel;
/// - a connection's principal is fixed at initialize: a request on it with
///   another valid token is refused (403) for POST, GET and DELETE;
/// - a peer prompt over HTTP is answered `end_turn`, naming an item the
///   record disposed in the named turn;
/// - the peer's `_rung/stop` is refused; the owner's halts the host and the
///   process exits 0 within [`G_P_STOP_MS`];
/// - no tokens, or a token env var that is not set, refuse to start (exit
///   2); token values never reach the record; the listener bound loopback.
pub fn g_q(lines: &[Line], run: &HttpAcpRun) -> GateResult {
    let mut g = GateResult::new("G-q");
    let st = |k: &str| run.status.get(k).copied().unwrap_or(0);
    let rp = |k: &str| run.replies.get(k).cloned().unwrap_or(Value::Null);
    let is_err = |v: &Value| v.get("error").is_some();
    let is_ok = |v: &Value| v.get("result").is_some();
    let auth_ok = st("no_bearer") == 401 && st("bad_bearer") == 401;
    let init_ok = st("owner_init") == 200 && st("peer_init") == 200;
    let cap_ok = is_err(&rp("peer_opens_owner"))
        && is_ok(&rp("peer_opens_peer"))
        && is_ok(&rp("owner_opens_owner"));
    let bound_ok = st("switched_post") == 403
        && st("switched_get") == 403
        && st("switched_delete") == 403;
    let prompt = rp("peer_prompt");
    let meta = &prompt["result"]["_meta"]["rung"];
    let item = meta["item"].as_str().unwrap_or("");
    let turn = meta["turn"].as_u64();
    let prompt_ok = prompt["result"]["stopReason"] == "end_turn"
        && of(lines, "stimulus.disposed")
            .any(|l| l.str("id") == item && Some(l.u64("turn")) == turn);
    let stop_ok = is_err(&rp("peer_stop"))
        && is_ok(&rp("owner_stop"))
        && run.exit.is_some_and(|(c, ms)| c == 0 && ms <= G_P_STOP_MS)
        && of(lines, "halted").any(|l| l.get("why").to_string().contains("\"owner\""));
    let starts_ok = run.refused_starts.get("no_tokens") == Some(&2)
        && run.refused_starts.get("unset_token_env") == Some(&2);
    let leaked = lines.iter().any(|l| {
        let t = l.text();
        run.tokens.iter().any(|tok| !tok.is_empty() && t.contains(tok.as_str()))
    });
    let loopback = run.bound.starts_with("127.0.0.1:") || run.bound.starts_with("[::1]:");
    g.put("status", json!(run.status));
    g.put("auth_refused", auth_ok);
    g.put("initialized", init_ok);
    g.put("role_capped", cap_ok);
    g.put("principal_bound", bound_ok);
    g.put("prompt_answered", prompt_ok);
    g.put("stop", stop_ok);
    g.put("exit", json!(run.exit));
    g.put("refused_starts", json!(run.refused_starts));
    g.put("token_in_record", leaked);
    g.put("bound", run.bound.clone());
    g.check(auth_ok, "a request with no or an unknown token was not refused 401");
    g.check(init_ok, "a known token could not initialize");
    g.check(cap_ok, "a token's role did not cap its channels");
    g.check(bound_ok, "a connection accepted another token after initialize");
    g.check(prompt_ok, "a prompt over HTTP was not answered as the record says");
    g.check(stop_ok, "the peer's stop was not refused, or the owner's did not halt and exit 0 in time");
    g.check(starts_ok, "a start without usable tokens was not refused");
    g.check(!leaked, "a token reached the record");
    g.check(loopback, "the listener did not bind loopback");
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(seq: u64, at: i64, kind: &str, body: Value) -> Line {
        let Value::Object(m) = body else { panic!() };
        Line {
            seq,
            at,
            kind: kind.into(),
            body: m,
        }
    }

    #[test]
    fn days_of_matches_the_calendar() {
        assert_eq!(days_of("1970-01-01"), Some(0));
        // 2026-10-03T00:00Z, where simulated runs start.
        assert_eq!(
            days_of("2026-10-03").map(|d| d * 86_400_000),
            Some(1_790_985_600_000)
        );
        assert_eq!(days_of("2000-03-01"), Some(11_017));
        assert_eq!(days_of("nope"), None);
    }

    #[test]
    fn quantiles_are_nearest_rank() {
        let xs: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(quantile(&xs, 0.99), 99.0);
        assert_eq!(quantile(&xs, 0.95), 95.0);
        assert_eq!(quantile(&[], 0.5), 0.0);
    }

    #[test]
    fn surprise_is_minus_log2_of_the_outcome_probability() {
        assert_eq!(surprise(0.5, true), 1.0);
        assert_eq!(surprise(0.75, false), 2.0);
    }

    #[test]
    fn calibration_recomputes_from_lines() {
        let ls = vec![
            line(1, 0, "expectation.made", json!({"id": "a", "p": 0.9})),
            line(2, 0, "expectation.made", json!({"id": "b", "p": 0.2})),
            line(3, 0, "expectation.revised", json!({"id": "b", "p": 0.3})),
            line(
                4,
                0,
                "expectation.settled",
                json!({"id": "a", "state": "met"}),
            ),
            line(
                5,
                0,
                "expectation.settled",
                json!({"id": "b", "state": "missed"}),
            ),
        ];
        let c = calibration_from(&ls);
        assert_eq!(c["n"], 2);
        // ((0.9-1)^2 + (0.3-0)^2) / 2 = 0.05
        assert_eq!(c["brier"], json!(0.05));
        assert_eq!(c["base_rate"], json!(0.5));
    }

    #[test]
    fn g_a_fails_a_loop_that_idles() {
        let cfg = json!({"config": {"turn_bound_ms": 120000}});
        let mut ls = vec![line(1, 0, "host.start", cfg)];
        let mut seq = 2;
        let mut at = 0;
        for n in 0..10 {
            ls.push(line(seq, at, "boundary", json!({"n": n})));
            ls.push(line(
                seq + 1,
                at,
                "decision.admit",
                json!({"boundary": n, "by": {"rule": "x"}}),
            ));
            ls.push(line(
                seq + 2,
                at,
                "turn.started",
                json!({"turn": n, "wall_boundary_us": 10}),
            ));
            at += 1_000;
            ls.push(line(seq + 3, at, "turn.ended", json!({"turn": n})));
            // An undeclared minute of rest after each turn.
            at += 60_000 * 3;
            seq += 4;
        }
        let g = g_a(&ls);
        assert!(!g.pass, "{}", g.summary());
        assert!(g.failures.iter().any(|f| f.contains("idle")));
    }

    #[test]
    fn g_l_fails_a_broken_prefix_and_an_unrecorded_hash_change() {
        let ls = vec![
            line(
                1,
                0,
                "llm.call",
                json!({"turn": 1, "call": 1, "cached_tokens": 0, "prefix": {"s_hash": "s", "l_hash": "a", "expected_cached_tokens": 0}}),
            ),
            line(
                2,
                0,
                "llm.call",
                json!({"turn": 2, "call": 1, "cached_tokens": 10, "prefix": {"s_hash": "s", "l_hash": "b", "expected_cached_tokens": 10}}),
            ),
        ];
        let cap = vec![Captured {
            turn: 2,
            call: 1,
            session: "e1".into(),
            model: "m".into(),
            bytes: 10,
            extends_prev: Some(false),
        }];
        let g = g_l(&ls, &cap);
        assert!(!g.pass);
        assert!(g.failures.iter().any(|f| f.contains("byte-prefix")));
        assert!(g.failures.iter().any(|f| f.contains("l_hash")));
    }

    #[test]
    fn g_m_catches_a_deferred_owner_and_a_group_outside_the_ceiling() {
        let ls = vec![
            line(
                1,
                0,
                "stimulus.accepted",
                json!({"item": {"id": "o1", "role": "owner"}}),
            ),
            line(2, 0, "boundary", json!({"n": 1})),
            line(
                3,
                0,
                "decision.tools",
                json!({"boundary": 1, "by": {"jev": {}}, "choice": {"enabled": ["core", "web_read"]}}),
            ),
            line(4, 0, "turn.started", json!({"turn": 1})),
        ];
        let g = g_m(&ls, &["core"]);
        assert!(!g.pass);
        assert_eq!(g.measured["owner_deferred"], 1);
        assert_eq!(g.measured["tools_outside_ceiling"], 1);
    }

    #[test]
    fn g_k_fails_any_spend() {
        let ls = vec![line(
            1,
            0,
            "llm.call",
            json!({"cost_usd": 0.01, "provider": "mock"}),
        )];
        assert!(!g_k(&ls).pass);
    }
}
