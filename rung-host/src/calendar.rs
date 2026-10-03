//! The calendar: time as context, owned by the host.
//!
//! Every entry has an origin:
//!
//! - **owner**: standing items, from the operator's seed (the agent cannot
//!   remove them);
//! - **routine**: host mechanics only;
//! - **agent**: dated register entries — an expectation's due time, a
//!   commitment's `until`. The agent edits them by editing the entry; there
//!   is no free-standing "wake me" tool.
//!
//! At each boundary every due entry fires into the inbox as a calendar
//! item, with its lateness. An entry missed while the host was down fires
//! once on waking (`missed: once_late`) or is skipped (`missed: skip`). The
//! calendar is a projection of `calendar.*` lines.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::clock::Millis;
use crate::record::Line;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Owner,
    Routine,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Missed {
    /// Fire once, late, on waking.
    OnceLate,
    /// Skip what was missed.
    Skip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum When {
    At(Millis),
    /// Every `period` ms from `start`.
    Every {
        start: Millis,
        period: Millis,
    },
}

/// One calendar entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub when: When,
    pub origin: Origin,
    pub text: String,
    #[serde(default)]
    pub firm: bool,
    pub missed: Missed,
}

/// An entry with its next due time.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scheduled {
    pub entry: Entry,
    /// `None` once a one-shot entry has fired.
    pub next_due: Option<Millis>,
    pub fired: u64,
}

/// What firing an entry produces.
#[derive(Debug, Clone, PartialEq)]
pub struct Fire {
    pub id: String,
    pub due: Millis,
    pub late_by_ms: Millis,
    pub missed: bool,
    pub firm: bool,
    pub text: String,
    /// Fired (`true`) or skipped by its missed policy.
    pub fire: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CalendarState {
    pub entries: BTreeMap<String, Scheduled>,
}

impl CalendarState {
    pub fn apply(&mut self, l: &Line) {
        match l.kind.as_str() {
            "calendar.added" => {
                if let Ok(entry) = serde_json::from_value::<Entry>(l.get("entry").clone()) {
                    let next_due = Some(match entry.when {
                        When::At(t) => t,
                        When::Every { start, .. } => start,
                    });
                    self.entries.insert(
                        entry.id.clone(),
                        Scheduled {
                            entry,
                            next_due,
                            fired: 0,
                        },
                    );
                }
            }
            "calendar.fired" | "calendar.skipped" => {
                let id = l.str("id");
                let due = l.i64("due");
                if let Some(s) = self.entries.get_mut(id) {
                    if l.kind == "calendar.fired" {
                        s.fired += 1;
                    }
                    s.next_due = match s.entry.when {
                        When::At(_) => None,
                        // The first slot after the boundary that fired it.
                        When::Every { start, period } => {
                            let k = (l.at - start).div_euclid(period) + 1;
                            Some((start + k * period).max(due + period))
                        }
                    };
                }
            }
            "calendar.removed" => {
                self.entries.remove(l.str("id"));
            }
            _ => {}
        }
    }

    /// What is due at `now`. `down_since` marks the first boundary after a
    /// gap: entries due then were missed and follow their missed policy.
    pub fn due(&self, now: Millis, down_since: Option<Millis>) -> Vec<Fire> {
        let mut out = Vec::new();
        for s in self.entries.values() {
            let Some(due) = s.next_due else { continue };
            if due > now {
                continue;
            }
            // At the first boundary after a gap, anything due was missed.
            let missed = down_since.is_some();
            let fire = !(missed && s.entry.missed == Missed::Skip);
            out.push(Fire {
                id: s.entry.id.clone(),
                due,
                late_by_ms: now - due,
                missed,
                firm: s.entry.firm,
                text: s.entry.text.clone(),
                fire,
            });
        }
        out.sort_by(|a, b| (a.due, &a.id).cmp(&(b.due, &b.id)));
        out
    }

    /// The next due time of any entry.
    pub fn next_due(&self) -> Option<Millis> {
        self.entries.values().filter_map(|s| s.next_due).min()
    }

    /// Entries due within `ms` of `now`.
    pub fn within(&self, now: Millis, ms: Millis) -> usize {
        self.entries
            .values()
            .filter(|s| s.next_due.is_some_and(|d| d >= now && d <= now + ms))
            .count()
    }
}

pub(crate) fn added_body(e: &Entry) -> Value {
    json!({ "entry": e })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(seq: u64, at: Millis, kind: &str, body: Value) -> Line {
        let Value::Object(m) = body else { panic!() };
        Line {
            seq,
            at,
            kind: kind.into(),
            body: m,
        }
    }

    #[test]
    fn a_repeating_entry_fires_and_reschedules_after_the_boundary() {
        let mut c = CalendarState::default();
        let e = Entry {
            id: "r".into(),
            when: When::Every {
                start: 100,
                period: 50,
            },
            origin: Origin::Owner,
            text: "tick".into(),
            firm: false,
            missed: Missed::OnceLate,
        };
        c.apply(&line(1, 0, "calendar.added", added_body(&e)));
        assert!(c.due(99, None).is_empty());
        let f = c.due(120, None);
        assert_eq!((f[0].due, f[0].late_by_ms, f[0].missed), (100, 20, false));
        c.apply(&line(
            2,
            120,
            "calendar.fired",
            json!({"id": "r", "due": 100}),
        ));
        assert_eq!(c.next_due(), Some(150));
        // Down from 130 to 400: several slots missed, fired once on waking.
        let f = c.due(400, Some(130));
        assert_eq!(f.len(), 1);
        assert!(f[0].missed && f[0].fire);
        c.apply(&line(
            3,
            400,
            "calendar.fired",
            json!({"id": "r", "due": 150}),
        ));
        assert_eq!(c.next_due(), Some(450));
    }

    #[test]
    fn a_skip_entry_missed_in_a_gap_is_skipped() {
        let mut c = CalendarState::default();
        let e = Entry {
            id: "s".into(),
            when: When::At(200),
            origin: Origin::Owner,
            text: "x".into(),
            firm: true,
            missed: Missed::Skip,
        };
        c.apply(&line(1, 0, "calendar.added", added_body(&e)));
        let f = c.due(500, Some(100));
        assert!(!f[0].fire && f[0].missed);
    }
}
