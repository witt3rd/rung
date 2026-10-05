//! Every text the host writes into the pack: the stable system text, the
//! slow layer of an epoch, and each turn's header.
//!
//! All of it is a deterministic function of the state: no clocks are read
//! here, and the free-time material is listed in creation order, never
//! ranked (gate G-c pins that random scores leave it unchanged).

use crate::clock::{Millis, iso, span};
use crate::core::HostConfig;
use crate::inbox::Item;
use crate::kernel::{Commitment, KernelState, TurnKind};
use crate::registers::{ExpState, Registers};

/// The free-time rules, in the stable layer.
pub const FREE_TIME_RULES: &str = "\
Free time. When nothing is asked of you and you have no commitment, the turn is yours.
1. No obligation. Nothing is asking you for anything.
2. It has to be your pull. Pick what genuinely draws you, or nothing on the list.
3. Follow it across turns if it keeps going.
4. Leave a trace with `trace`: what pulled you, where it went, what is unresolved. A scribble, not a report.
5. Keep what you found: save real findings to memory.
You may also commit to a project with `commit`; it then continues until you `release` it.
Places to look, as examples only: something you remembered, something you read, the world outside, something to make, a memory search.";

/// The host contract, in the stable layer.
pub const HOST_CONTRACT: &str = "\
How this host works.
- There is no exit and no rest: after every turn the next begins. Waits are the world's (quota, provider trouble) and are stated as such.
- Time is context: each turn opens with a header giving the time and what changed. Older headers are history.
- Stimuli (messages, calendar items, settled expectations) arrive at turn boundaries, never mid-turn.
- `commit` / `progress` / `release` manage one commitment at a time; `trace` ends a free-time session.
- `expect` states what you expect; the host settles it, you cannot. `revise` changes your probability.
- `send` reaches a channel; `note` replaces your carried note, which rides into the next context epoch.
- Every tool is declared; only the groups named in the newest header are enabled. A disabled tool refuses without running; ask with `want_tools`.
- A tool call too long for one turn is refused: break the work into steps, or commit to it as a project.
- You change the world only inside your workspace (the ws_* tools).";

/// The stable system text: identity, contract, rules, pinned memory.
pub fn system(cfg: &HostConfig) -> String {
    let mut s = format!(
        "{}\n\n{HOST_CONTRACT}\n\n{FREE_TIME_RULES}",
        cfg.identity.trim()
    );
    if !cfg.pinned.is_empty() {
        s.push_str("\n\nPinned:\n");
        for p in &cfg.pinned {
            s.push_str(&format!("- {p}\n"));
        }
    }
    s
}

fn clip(s: &str, n: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= n {
        flat
    } else {
        let mut t: String = flat.chars().take(n).collect();
        t.push('…');
        t
    }
}

/// The agent's own material, in creation order. Scores the agent gave
/// (priorities) are never used to order it.
pub fn material(reg: &Registers, kernel: &KernelState, now: Millis) -> String {
    let mut out = String::from("your material (unordered, by creation date):\n");
    let mut todo: Vec<(&String, &crate::registers::Todo)> =
        reg.todo.iter().filter(|(_, t)| !t.done).collect();
    todo.sort_by(|a, b| (a.1.created_at, a.0).cmp(&(b.1.created_at, b.0)));
    out.push_str(&format!("  todo: {} open", todo.len()));
    if let Some((_, t)) = todo.first() {
        out.push_str(&format!(" (oldest {})", span(now - t.created_at)));
    }
    for (id, t) in todo.iter().take(5) {
        out.push_str(&format!("\n    {id}: {}", clip(&t.text, 80)));
    }
    let mut projects: Vec<(&String, &crate::registers::Project)> = reg
        .projects
        .iter()
        .filter(|(_, p)| p.status != "done" && p.status != "abandoned")
        .collect();
    projects.sort_by(|a, b| (a.1.created_at, a.0).cmp(&(b.1.created_at, b.0)));
    out.push_str("\n  projects:");
    if projects.is_empty() {
        out.push_str(" none");
    }
    for (id, p) in projects.iter().take(8) {
        out.push_str(&format!(
            "\n    {id}: {} ({}, {} since last step)",
            clip(&p.title, 60),
            p.status,
            span(now - p.last_step_at)
        ));
    }
    let mut qs: Vec<(&String, &crate::registers::Question)> =
        reg.questions.iter().filter(|(_, q)| q.open).collect();
    qs.sort_by(|a, b| (a.1.created_at, a.0).cmp(&(b.1.created_at, b.0)));
    out.push_str(&format!("\n  research: {} open questions", qs.len()));
    for (id, q) in qs.iter().take(3) {
        out.push_str(&format!("\n    {id}: {}", clip(&q.text, 80)));
    }
    let open: Vec<(&String, &crate::registers::Expectation)> = reg
        .expectations
        .iter()
        .filter(|(_, e)| e.state == ExpState::Open)
        .collect();
    out.push_str(&format!("\n  expectations: {} open", open.len()));
    if kernel.copy_streak > 0 {
        out.push_str(&format!(
            "\n  integrity: repetition guard: your last {} turns repeated earlier text",
            kernel.copy_streak
        ));
    }
    if !kernel.recent.is_empty() {
        out.push_str("\n  recent traces and answers:");
        for (turn, t) in &kernel.recent {
            out.push_str(&format!("\n    turn {turn}: \"{}\"", clip(t, 60)));
        }
    }
    out
}

/// What a turn header is built from.
pub struct HeaderCtx<'a> {
    pub turn: u64,
    pub kind: TurnKind,
    pub now: Millis,
    pub since_external: Option<Millis>,
    /// (left, per day).
    pub quota: Option<(u64, u64)>,
    pub model: &'a str,
    pub rung: usize,
    pub enabled: &'a [String],
    pub admitted: Vec<&'a Item>,
    pub digests: Vec<&'a Item>,
    pub commitment: Option<&'a Commitment>,
    pub turns_since_progress: u64,
    pub resumed: bool,
    pub material: Option<String>,
    pub recall: Option<String>,
    pub expectations: Option<String>,
    pub calendar: Option<String>,
    pub note_line: bool,
    pub notices: Vec<String>,
}

/// One turn header. The only new bytes at a boundary.
pub fn header(h: &HeaderCtx) -> String {
    let label = match h.kind {
        TurnKind::Free => "free time",
        TurnKind::Committed => "committed",
        TurnKind::Responding => "responding",
    };
    let mut first = format!("[turn {} · {label} · {}", h.turn, iso(h.now));
    match h.since_external {
        Some(t) => first.push_str(&format!(" · {} since anything external", span(h.now - t))),
        None => first.push_str(" · nothing external yet"),
    }
    if let Some((left, per)) = h.quota {
        first.push_str(&format!(" · quota {left}/{per} left"));
    }
    first.push_str(&format!(" · model {} (rung {})]", h.model, h.rung));
    let mut out = vec![first, format!("tools on: {}", h.enabled.join(", "))];
    for n in &h.notices {
        out.push(format!("note: {n}"));
    }
    if !h.admitted.is_empty() {
        out.push("admitted:".into());
        for i in &h.admitted {
            let late = match i.due {
                Some(d) if h.now > d => format!(", due {}, late by {}", iso(d), span(h.now - d)),
                _ => String::new(),
            };
            out.push(format!(
                "  [{}/{} @{}{late}] {}",
                i.channel,
                i.id,
                iso(i.at),
                clip(&i.text, 1_000)
            ));
        }
    }
    for i in &h.digests {
        out.push(format!(
            "digest: [{}/{}] {}",
            i.channel,
            i.id,
            clip(&i.text, 160)
        ));
    }
    if let Some(c) = h.commitment {
        let resumed = if h.resumed && h.kind == TurnKind::Committed {
            " (back to it after an interruption)"
        } else {
            ""
        };
        out.push(format!(
            "commitment {}: {}{resumed} · since turn {} · done when: {}",
            c.project,
            clip(&c.title, 80),
            c.since_turn,
            clip(&c.done_when, 120)
        ));
        out.push(format!(
            "  next step: {} · {} turns since progress{}",
            c.next_step
                .as_deref()
                .map(|s| clip(s, 160))
                .unwrap_or_else(|| "(none yet)".into()),
            h.turns_since_progress,
            match c.until {
                Some(u) if h.now > u => format!(" · `until` passed {} ago", span(h.now - u)),
                Some(u) => format!(" · until {}", iso(u)),
                None => String::new(),
            }
        ));
    }
    if let Some(m) = &h.material {
        out.push(m.clone());
    }
    if let Some(e) = &h.expectations {
        out.push(e.clone());
    }
    if let Some(c) = &h.calendar {
        out.push(c.clone());
    }
    if h.note_line {
        out.push(
            "Context will roll over soon; update your note if you want anything carried.".into(),
        );
    }
    if let Some(r) = &h.recall {
        out.push(r.clone());
    }
    out.join("\n")
}

/// The epoch header: the slow layer's first line.
pub fn epoch_line(epoch: u64, now: Millis, model: &str, rung: usize) -> String {
    format!(
        "[epoch {epoch} · started {} · model {model} (rung {rung})]",
        iso(now)
    )
}

/// The recovered line, when an epoch starts after a gap.
pub fn recovered_line(from: Millis, to: Millis, last_turn: u64, requeued: usize) -> String {
    format!(
        "[recovered: not running from {} to {} (gap {}); last turn {last_turn}; {requeued} interrupted items requeued]",
        iso(from),
        iso(to),
        span(to - from)
    )
}

/// The owner's "what are you doing" report, from state alone (no model
/// call): the current commitment, its last progress, and the next calendar
/// item due.
pub fn status_report(st: &crate::state::State, now: crate::clock::Millis) -> String {
    let mut s = String::new();
    match st.kernel.commitment() {
        Some(c) => {
            s.push_str(&format!(
                "Committed to {}: {} (done when: {}).\n",
                c.project,
                clip(&c.title, 80),
                clip(&c.done_when, 80)
            ));
            let ago = st.turn.saturating_sub(c.last_progress_turn);
            s.push_str(&format!(
                "Last progress: turn {} ({ago} turns ago); next step: {}.\n",
                c.last_progress_turn,
                c.next_step
                    .as_deref()
                    .map_or("not yet stated".into(), |n| clip(n, 80))
            ));
        }
        None => s.push_str("Free time: no commitment.\n"),
    }
    match st.calendar.next_entry() {
        Some((e, due)) => {
            let d = due - now;
            let when = if d >= 0 {
                format!("in {}", crate::clock::span(d))
            } else {
                format!("{} overdue", crate::clock::span(-d))
            };
            s.push_str(&format!(
                "Next due: {} ({}, {when}).\n",
                clip(&e.text, 80),
                crate::clock::iso(due)
            ));
        }
        None => s.push_str("Next due: nothing on the calendar.\n"),
    }
    s
}

/// The slow layer of a new epoch.
pub fn slow(
    l1: &str,
    note: Option<&str>,
    reg: &Registers,
    kernel: &KernelState,
    outline: &[String],
    kept: &str,
) -> String {
    let mut s = String::from(l1);
    s.push_str("\n## Your note (as of this epoch's start)\n");
    s.push_str(note.unwrap_or("(none yet)"));
    s.push_str("\n## Registers\n");
    match kernel.commitment() {
        Some(c) => s.push_str(&format!(
            "commitment: {} ({})\n",
            c.project,
            clip(&c.title, 80)
        )),
        None => s.push_str("commitment: none (free time)\n"),
    }
    let active: Vec<String> = reg
        .projects
        .iter()
        .filter(|(_, p)| p.status != "done" && p.status != "abandoned")
        .map(|(id, p)| format!("{id} {}", clip(&p.title, 40)))
        .take(8)
        .collect();
    s.push_str(&format!(
        "projects: {}\n",
        if active.is_empty() {
            "none".into()
        } else {
            active.join("; ")
        }
    ));
    let open = reg
        .expectations
        .values()
        .filter(|e| e.state == ExpState::Open)
        .count();
    let todo = reg.todo.values().filter(|t| !t.done).count();
    let qs = reg.questions.values().filter(|q| q.open).count();
    s.push_str(&format!(
        "open expectations: {open} · todo open: {todo} · open questions: {qs}\n"
    ));
    let cal = reg.calibration.value();
    if cal["n"].as_u64().unwrap_or(0) > 0 {
        s.push_str(&format!(
            "calibration: n={} brier={} resolution={}\n",
            cal["n"], cal["brier"], cal["resolution"]
        ));
    }
    s.push_str("## Previous epoch (outline)\n");
    if outline.is_empty() {
        s.push_str("(none)\n");
    }
    for o in outline {
        s.push_str(&format!("- {o}\n"));
    }
    if !kept.is_empty() {
        s.push_str("## Kept verbatim from the previous epoch\n");
        s.push_str(kept);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ln(seq: u64, at: i64, kind: &str, body: serde_json::Value) -> crate::record::Line {
        let serde_json::Value::Object(body) = body else {
            panic!()
        };
        crate::record::Line {
            seq,
            at,
            kind: kind.into(),
            body,
        }
    }

    #[test]
    fn the_status_report_answers_from_state() {
        let mut st = crate::state::State::default();
        let r = status_report(&st, 0);
        assert!(r.contains("Free time: no commitment."));
        assert!(r.contains("nothing on the calendar"));

        st.apply(&ln(
            1,
            0,
            "turn.started",
            serde_json::json!({"turn": 3, "turn_kind": "free"}),
        ));
        st.apply(&ln(
            2,
            0,
            "kernel.commit",
            serde_json::json!({"project": "garden", "title": "Plant beds", "done_when": "beds planted", "turn": 3}),
        ));
        st.apply(&ln(
            3,
            0,
            "kernel.progress",
            serde_json::json!({"turn": 4, "next_step": "water seedlings"}),
        ));
        st.apply(&ln(
            4,
            0,
            "turn.started",
            serde_json::json!({"turn": 6, "turn_kind": "committed"}),
        ));
        let later = crate::calendar::Entry {
            id: "later".into(),
            when: crate::calendar::When::At(7_200_000),
            origin: crate::calendar::Origin::Owner,
            text: "call the plumber".into(),
            firm: false,
            missed: crate::calendar::Missed::OnceLate,
        };
        let mut soon = later.clone();
        soon.id = "soon".into();
        soon.when = crate::calendar::When::At(3_600_000);
        soon.text = "check the oven".into();
        for e in [&later, &soon] {
            st.apply(&ln(5, 0, "calendar.added", crate::calendar::added_body(e)));
        }
        let r = status_report(&st, 0);
        assert!(
            r.contains("Committed to garden: Plant beds (done when: beds planted)."),
            "{r}"
        );
        assert!(
            r.contains("Last progress: turn 4 (2 turns ago); next step: water seedlings."),
            "{r}"
        );
        assert!(r.contains("Next due: check the oven"), "{r}");
        assert!(!r.contains("plumber"), "{r}");
    }

    #[test]
    fn a_header_states_the_facts() {
        let h = HeaderCtx {
            turn: 7,
            kind: TurnKind::Free,
            now: 1_790_000_000_000,
            since_external: Some(1_790_000_000_000 - 11_520_000),
            quota: Some((612, 1000)),
            model: "m",
            rung: 0,
            enabled: &["core".into(), "read".into()],
            admitted: vec![],
            digests: vec![],
            commitment: None,
            turns_since_progress: 0,
            resumed: false,
            material: None,
            recall: None,
            expectations: None,
            calendar: None,
            note_line: false,
            notices: vec![],
        };
        let s = header(&h);
        assert!(s.starts_with("[turn 7 · free time · 2026-09-21T14:13:20Z · 3h12m since anything external · quota 612/1000 left"));
        assert!(s.contains("tools on: core, read"));
    }
}
