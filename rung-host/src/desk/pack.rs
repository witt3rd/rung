//! Pack: when to roll the context over, and which segments of the old
//! epoch stay verbatim. Asked only when the pack's mechanical gate opens
//! (≥ 40% of the epoch budget, or a natural break at ≥ 25%, or a copy loop).

use std::collections::BTreeMap;

use rung_std::decide::{Decided, Question};
use serde::Serialize;

use super::{HostQuestion, Knobs, choice, noul, qid};

/// A contiguous run of the epoch's turns.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Segment {
    pub id: String,
    pub first_turn: u64,
    pub last_turn: u64,
    pub tokens: usize,
    pub gist: String,
    pub referenced_by_open_commitment: bool,
    pub referenced_by_open_expectation: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackInput {
    /// The pack now.
    pub epoch_tokens: usize,
    /// What the next turn's header will add (held back for the ceiling).
    pub header_reserve: usize,
    pub budget: usize,
    pub turns_in_epoch: u64,
    pub at_break: bool,
    pub mode: String,
    pub cache_read_ratio_last10: f64,
    pub copy_flag: bool,
    /// The segments the decider is asked about, each with its whole text.
    pub segments: Vec<Segment>,
    /// The segments this ask leaves out because the decider's window cannot
    /// hold them (see [`PackInput::split_for_window`]); never sent. They are
    /// decided as the rule decides them: kept when an open commitment or
    /// expectation refers to them or they are the epoch's last turns.
    #[serde(skip)]
    pub unasked: Vec<Segment>,
    /// The epoch's last turn (the rule keeps the last two).
    pub last_turn: u64,
}

impl PackInput {
    /// Every segment of the epoch, asked or not.
    pub fn all_segments(&self) -> impl Iterator<Item = &Segment> {
        self.segments.iter().chain(self.unasked.iter())
    }

    /// Whether the rule alone keeps `s`: an open commitment or expectation
    /// refers to it, or it holds the epoch's last two turns.
    fn rule_keeps(&self, s: &Segment) -> bool {
        referenced(s) || s.last_turn >= self.last_turn.saturating_sub(1)
    }

    /// Ask about as many segments as `fits(asked)` allows, whole and never
    /// clipped. Segments an open commitment or expectation refers to are kept
    /// whatever the decider says, so they are not asked; of the rest the
    /// newest are asked first. What does
    /// not fit is left in [`PackInput::unasked`] and decided by the rule: the
    /// pack still completes at any epoch size (the budget is far above the
    /// decider's window). `fits` is called with the input holding the
    /// candidate asked set.
    pub fn split_for_window(&mut self, mut fits: impl FnMut(&PackInput) -> bool) {
        let all: Vec<Segment> = std::mem::take(&mut self.segments);
        let (mut askable, rest): (Vec<Segment>, Vec<Segment>) =
            all.into_iter().partition(|s| !referenced(s));
        askable.sort_by_key(|s| std::cmp::Reverse(s.last_turn));
        let mut with = |n: usize, me: &mut PackInput| {
            let mut asked: Vec<Segment> = askable[..n].to_vec();
            asked.sort_by_key(|s| s.first_turn);
            me.segments = asked;
            fits(me)
        };
        // Zero is allowed here: the ask still carries the context action.
        let (mut lo, mut hi) = (0, askable.len());
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if with(mid, self) {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        with(lo, self);
        let kept: Vec<String> = self.segments.iter().map(|s| s.id.clone()).collect();
        let mut unasked: Vec<Segment> = askable
            .into_iter()
            .filter(|s| !kept.contains(&s.id))
            .chain(rest)
            .collect();
        unasked.sort_by_key(|s| s.first_turn);
        self.unasked = unasked;
    }

    /// The pack's share of the budget now.
    pub fn fraction(&self) -> f64 {
        self.epoch_tokens as f64 / self.budget.max(1) as f64
    }

    /// The share the next turn would start with.
    pub fn next_fraction(&self) -> f64 {
        (self.epoch_tokens + self.header_reserve) as f64 / self.budget.max(1) as f64
    }

    /// The pack's mechanical gate: is it worth asking at all?
    pub fn gate_open(&self, k: &Knobs) -> bool {
        let f = self.fraction();
        self.copy_flag
            || f >= k.pack_floor
            || self.next_fraction() >= k.pack_ceiling
            || (self.at_break && f >= k.pack_break_floor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Append,
    Rollover,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PackChoice {
    pub action: Action,
    /// Segment ids kept verbatim.
    pub keep: Vec<String>,
    /// `pack`, `ceiling` or `copy_loop`.
    pub cause: String,
}

pub struct Pack;

fn keep_capped(input: &PackInput, mut ranked: Vec<(f64, &Segment)>, k: &Knobs) -> Vec<String> {
    let cap = (k.keep_budget * input.budget as f64) as usize;
    // Referenced segments first, then by p, then newest.
    ranked.sort_by(|a, b| {
        let ra = a.1.referenced_by_open_commitment || a.1.referenced_by_open_expectation;
        let rb = b.1.referenced_by_open_commitment || b.1.referenced_by_open_expectation;
        rb.cmp(&ra)
            .then(b.0.total_cmp(&a.0))
            .then(b.1.last_turn.cmp(&a.1.last_turn))
    });
    let mut used = 0;
    let mut out = Vec::new();
    for (_, s) in ranked {
        if used + s.tokens <= cap {
            used += s.tokens;
            out.push(s.id.clone());
        }
    }
    out.sort();
    out
}

fn referenced(s: &Segment) -> bool {
    s.referenced_by_open_commitment || s.referenced_by_open_expectation
}

impl HostQuestion for Pack {
    const ID: &'static str = "pack";
    type Input = PackInput;
    type Ctx = ();
    type Choice = PackChoice;

    fn questions(input: &PackInput) -> BTreeMap<String, Question> {
        let mut q = BTreeMap::new();
        q.insert(
            "context_action".into(),
            Question::choice(
                "What should happen to the agent's context at this boundary?",
                &[
                    (
                        "append",
                        "Keep appending: the context has room and no break is due.",
                    ),
                    ("rollover_now", "Start a new context epoch now."),
                    (
                        "rollover_at_break",
                        "Start a new epoch at the agent's next natural break.",
                    ),
                ],
            ),
        );
        for s in &input.segments {
            q.insert(
                qid("keep", &s.id),
                Question::noul(
                    &format!(
                        "Will the agent likely need segment {}'s exact text in the next epoch, rather than its gist?",
                        s.id
                    ),
                    "Yes: the exact words will matter.",
                    "No: the gist is enough.",
                ),
            );
        }
        q
    }

    fn compose(input: &PackInput, d: &Decided, k: &Knobs, _ctx: &()) -> Option<PackChoice> {
        let action = match choice(d, "context_action")? {
            "rollover_now" => Action::Rollover,
            "rollover_at_break" if input.at_break => Action::Rollover,
            _ => Action::Append,
        };
        let mut ranked = Vec::new();
        for s in &input.segments {
            let p = noul(d, &qid("keep", &s.id))?;
            if p >= k.keep_p || referenced(s) {
                ranked.push((p, s));
            }
        }
        for s in input.unasked.iter().filter(|s| input.rule_keeps(s)) {
            ranked.push((1.0, s));
        }
        Some(PackChoice {
            action,
            keep: keep_capped(input, ranked, k),
            cause: "pack".into(),
        })
    }

    fn rule(input: &PackInput, k: &Knobs, _ctx: &()) -> PackChoice {
        let f = input.fraction();
        let action = if input.next_fraction() >= k.pack_ceiling
            || input.copy_flag
            || (input.at_break && f >= k.pack_rule_break)
        {
            Action::Rollover
        } else {
            Action::Append
        };
        let ranked = input
            .all_segments()
            .filter(|s| input.rule_keeps(s))
            .map(|s| (1.0, s))
            .collect();
        PackChoice {
            action,
            keep: keep_capped(input, ranked, k),
            cause: "pack".into(),
        }
    }

    fn guard(input: &PackInput, mut c: PackChoice, k: &Knobs, _ctx: &()) -> PackChoice {
        if input.copy_flag {
            c.action = Action::Rollover;
            c.cause = "copy_loop".into();
        } else if input.next_fraction() >= k.pack_ceiling {
            c.action = Action::Rollover;
            c.cause = "ceiling".into();
        } else if !input.gate_open(k) {
            // Below every floor that applies (break floor only at a break).
            c.action = Action::Append;
            c.cause = "pack".into();
        } else {
            c.cause = "pack".into();
        }
        // Only real segments, within the keep budget.
        let ranked = input
            .all_segments()
            .filter(|s| c.keep.contains(&s.id))
            .map(|s| (1.0, s))
            .collect();
        c.keep = keep_capped(input, ranked, k);
        if c.action == Action::Append {
            c.keep.clear();
        }
        c
    }
}
