//! The queue: what waits for the agent, and the one thing it is answering,
//! computed from the record (S1).
//!
//! A message to the agent is a stimulus. Its life is in the record: accepted
//! (`stimulus.accepted`), admitted to a turn (`stimulus.admitted`), requeued
//! when a turn fails (`stimulus.requeued`), and disposed once
//! (`stimulus.disposed`). [`Fold`] replays those lines, and the desk's latest
//! `decision.admit` for the reason in words a waiting item is held. Nothing
//! here asks a host anything, so a stopped instance lists the same as a running
//! one; nothing here writes.
//!
//! The list the doors give ([`answer`]) is ordered for a page: the item being
//! answered, then what waits (the owner's items first, then every other sender's;
//! within each, in the order they were accepted as `queue.moved` lines have
//! changed it: [`crate::inbox::Order`]), then what is over (the most recently
//! settled first). A page of it is cut by the caller (`offset`,
//! `limit`), never by the host; every text is whole, and the whole answer goes
//! through the shared redactor. Door shapes: `docs/rung-host-api.md` (S1).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use crate::read::{self, Door, DoorError};
use crate::record::Line;
use crate::redact::Redactor;

/// Where an item is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Accepted; no boundary has read it yet.
    Waiting,
    /// In the turn that is answering it.
    Answering,
    /// Disposed by a turn (answered, digested or dealt with).
    Answered,
    /// Withdrawn before any turn read it.
    Withdrawn,
}

impl State {
    pub fn word(self) -> &'static str {
        match self {
            State::Waiting => "waiting",
            State::Answering => "answering",
            State::Answered => "answered",
            State::Withdrawn => "withdrawn",
        }
    }

    fn all() -> [State; 4] {
        [
            State::Waiting,
            State::Answering,
            State::Answered,
            State::Withdrawn,
        ]
    }
}

/// One item as the record leaves it.
#[derive(Debug, Clone)]
struct Entry {
    id: String,
    /// The seq of its `stimulus.accepted` line: its place in the queue.
    order: u64,
    /// The `item` object of that line, as written.
    item: Value,
    state: State,
    /// The turn that admitted it (while answering) or disposed it.
    turn: Option<u64>,
    disposition: Option<String>,
    /// The seq of the line that settled it.
    settled: u64,
    /// The host told the owner at once that the model is unavailable.
    acked: bool,
}

impl Entry {
    fn role(&self) -> &str {
        self.item["role"].as_str().unwrap_or("")
    }

    /// Owner first, then peers, then any other source (the host's own).
    fn rank(&self) -> u8 {
        crate::inbox::group_of(self.role())
    }

    fn channel(&self) -> &str {
        self.item["channel"].as_str().unwrap_or("")
    }

    /// The sender's name: a peer's, or the host's own source's. The owner has
    /// none.
    fn name(&self) -> Option<String> {
        match self.role() {
            "owner" => None,
            "peer" => Some(
                self.channel()
                    .split_once(':')
                    .map_or(self.channel(), |(_, n)| n)
                    .to_string(),
            ),
            _ => Some(self.channel().to_string()),
        }
    }
}

/// The record's queue.
#[derive(Debug, Clone, Default)]
pub struct Fold {
    entries: BTreeMap<String, Entry>,
    /// The forms of the latest `decision.admit`: id → `now` | `at_break` |
    /// `digest`.
    forms: BTreeMap<String, String>,
    /// The order the unsettled items wait in: acceptance, then `queue.moved`.
    order: crate::inbox::Order,
    /// The seq of the last line applied.
    pub seq: u64,
}

impl Fold {
    pub fn apply(&mut self, l: &Line) {
        self.seq = l.seq;
        match l.kind.as_str() {
            "stimulus.accepted" => {
                let item = l.get("item");
                let Some(id) = item["id"].as_str() else {
                    return;
                };
                // an owner control (stop, release) is acted on and never shown
                if item.get("control").is_some_and(|c| !c.is_null()) {
                    return;
                }
                if !self.entries.contains_key(id) {
                    self.order.push(id);
                }
                self.entries.entry(id.to_string()).or_insert_with(|| Entry {
                    id: id.to_string(),
                    order: l.seq,
                    item: item.clone(),
                    state: State::Waiting,
                    turn: None,
                    disposition: None,
                    settled: 0,
                    acked: false,
                });
            }
            "stimulus.admitted" => {
                let turn = l.u64("turn");
                for id in crate::inbox::ids(l.get("ids"))
                    .into_iter()
                    .chain(crate::inbox::ids(l.get("digests")))
                {
                    if let Some(e) = self.entries.get_mut(&id)
                        && e.state == State::Waiting
                    {
                        e.state = State::Answering;
                        e.turn = Some(turn);
                    }
                }
            }
            "stimulus.requeued" => {
                for id in crate::inbox::ids(l.get("ids")) {
                    if let Some(e) = self.entries.get_mut(&id)
                        && e.state == State::Answering
                    {
                        e.state = State::Waiting;
                        e.turn = None;
                    }
                }
            }
            "stimulus.disposed" => {
                if let Some(e) = self.entries.get_mut(l.str("id")) {
                    let d = l.str("disposition");
                    e.state = if d == "withdrawn" {
                        State::Withdrawn
                    } else {
                        State::Answered
                    };
                    e.disposition = Some(d.to_string());
                    e.turn = l.get("turn").as_u64().or(e.turn);
                    e.settled = l.seq;
                    let id = e.id.clone();
                    self.order.remove(&id);
                }
            }
            "queue.moved" => {
                let entries = &self.entries;
                self.order
                    .apply_move(l.str("id"), l.get("before").as_str(), |x| {
                        entries
                            .get(x)
                            .filter(|e| e.state == State::Waiting)
                            .map(Entry::rank)
                    });
            }
            "decision.admit" => {
                self.forms = l.get("choice")["forms"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
            }
            "outbox.queued" if l.str("source") == crate::inbox::ACK_SOURCE => {
                if let Some(e) = self.entries.get_mut(l.str("item")) {
                    e.acked = true;
                }
            }
            _ => {}
        }
    }

    /// Every item in list order, with the 1-based place of those that wait.
    fn sorted(&self) -> Vec<(&Entry, Option<usize>)> {
        let mut answering: Vec<&Entry> = self.in_state(State::Answering);
        answering.sort_by_key(|e| e.order);
        let places = self.order.places();
        let mut waiting: Vec<&Entry> = self.in_state(State::Waiting);
        waiting.sort_by_key(|e| {
            (
                e.rank(),
                places.get(e.id.as_str()).copied().unwrap_or(usize::MAX),
                e.order,
            )
        });
        let mut over: Vec<&Entry> = self
            .entries
            .values()
            .filter(|e| matches!(e.state, State::Answered | State::Withdrawn))
            .collect();
        over.sort_by_key(|e| std::cmp::Reverse(e.settled));
        answering
            .into_iter()
            .map(|e| (e, None))
            .chain(
                waiting
                    .into_iter()
                    .enumerate()
                    .map(|(i, e)| (e, Some(i + 1))),
            )
            .chain(over.into_iter().map(|e| (e, None)))
            .collect()
    }

    fn in_state(&self, s: State) -> Vec<&Entry> {
        self.entries.values().filter(|e| e.state == s).collect()
    }

    /// Why the host is holding a waiting item, in words; `None` when nothing
    /// holds it (it is simply next in line).
    fn held(&self, e: &Entry) -> Option<&'static str> {
        if e.state != State::Waiting {
            return None;
        }
        match self.forms.get(&e.id).map(String::as_str) {
            Some("at_break") => {
                return Some("held by the decision desk until the agent's next break");
            }
            Some("digest") => {
                return Some(
                    "held by the decision desk: it will be one line in the next turn header, not shown in full",
                );
            }
            _ => {}
        }
        e.acked.then_some(
            "the model is not available right now: the owner was told, and it is read when the model is back",
        )
    }

    fn item_json(&self, e: &Entry, position: Option<usize>) -> Value {
        let item = &e.item;
        json!({
            "id": e.id,
            "text": item["text"],
            "role": e.role(),
            "name": e.name(),
            "channel": e.channel(),
            "kind": item["kind"],
            "intent": item.get("intent").filter(|v| v.is_string()).cloned().unwrap_or_else(|| json!("next")),
            "order": e.order,
            "position": position,
            "accepted_at": item.get("at").filter(|v| v.is_i64() || v.is_u64()).cloned().unwrap_or(Value::Null),
            "state": e.state.word(),
            "turn": e.turn,
            "disposition": e.disposition,
            "held": self.held(e),
        })
    }

    /// One item in the list's shape (by id).
    pub fn item(&self, id: &str) -> Option<Value> {
        self.sorted()
            .into_iter()
            .find(|(e, _)| e.id == id)
            .map(|(e, p)| self.item_json(e, p))
    }

    /// The text of one item as the record holds it (not redacted).
    pub fn raw(&self, id: &str) -> Option<(&str, &str)> {
        let e = self.entries.get(id)?;
        Some((e.item["text"].as_str().unwrap_or(""), e.role()))
    }

    fn counts(&self) -> Value {
        let mut m = serde_json::Map::new();
        for s in State::all() {
            m.insert(
                s.word().into(),
                self.entries
                    .values()
                    .filter(|e| e.state == s)
                    .count()
                    .into(),
            );
        }
        Value::Object(m)
    }
}

/// A query on the queue door: which states, and which page. No maximum on
/// `limit`.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub states: Vec<State>,
    pub offset: u64,
    pub limit: u64,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            states: vec![State::Waiting, State::Answering],
            offset: 0,
            limit: read::DEFAULT_LIMIT,
        }
    }
}

impl Query {
    /// Parse the query's decoded pairs. `state` is a comma list of `waiting`,
    /// `answering`, `answered`, `withdrawn`, `open` (waiting and answering: the
    /// default) or `all`. Anything else, a repeated name, or a number that is not
    /// whole (or a `limit` of 0) is 400.
    pub fn parse(pairs: &[(String, String)]) -> Door<Self> {
        let bad = |m: String| DoorError::new(400, "bad_request", m);
        let mut q = Self::default();
        let mut seen: Vec<&str> = Vec::new();
        for (k, v) in pairs {
            if seen.contains(&k.as_str()) {
                return Err(bad(format!("{k} is given twice")));
            }
            seen.push(k);
            let number = || -> Door<u64> {
                if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(bad(format!("{k} must be a whole number, not {v:?}")));
                }
                Ok(v.parse().unwrap_or(u64::MAX))
            };
            match k.as_str() {
                "offset" => q.offset = number()?,
                "limit" => {
                    q.limit = number()?;
                    if q.limit == 0 {
                        return Err(bad("limit must be at least 1".into()));
                    }
                }
                "state" => {
                    let mut states = Vec::new();
                    for w in v.split(',') {
                        let add: &[State] = match w {
                            "waiting" => &[State::Waiting],
                            "answering" => &[State::Answering],
                            "answered" => &[State::Answered],
                            "withdrawn" => &[State::Withdrawn],
                            "open" => &[State::Waiting, State::Answering],
                            "all" => &State::all(),
                            other => {
                                return Err(bad(format!(
                                    "no such state {other:?} (waiting, answering, answered, withdrawn, open, all)"
                                )));
                            }
                        };
                        for s in add {
                            if !states.contains(s) {
                                states.push(*s);
                            }
                        }
                    }
                    q.states = states;
                }
                other => {
                    return Err(bad(format!(
                        "no such parameter {other:?} (state, offset, limit)"
                    )));
                }
            }
        }
        Ok(q)
    }
}

/// A path segment, percent-decoded; `None` when it is not UTF-8 or is empty.
pub fn decode_segment(seg: &str) -> Option<String> {
    let b = seg.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok().filter(|s| !s.is_empty())
}

/// Replay the record in `state_dir`. A damaged record is read as far as it is
/// intact; the damage is returned.
pub fn replay(state_dir: &Path) -> Door<(Fold, Option<read::Damaged>)> {
    let mut f = Fold::default();
    let scanned = read::scan(state_dir, |l| f.apply(l))?;
    Ok((f, scanned.damaged))
}

/// The queue door's answer: one page of the list, every text whole, the whole
/// answer through `red`.
///
/// ```json
/// {"items": [...], "total": 3, "offset": 0, "limit": 500, "next": null,
///  "counts": {"waiting": 2, "answering": 1, "answered": 9, "withdrawn": 0},
///  "record_lines": 412, "damaged": null,
///  "redaction": {"env_unset": []}}
/// ```
///
/// `total` is the number of items in the states asked for, exact; `next` is
/// the next offset, or null on the last page.
pub fn answer(state_dir: &Path, q: &Query, red: &Redactor, env_unset: &[String]) -> Door {
    let (fold, damaged) = replay(state_dir)?;
    let all = fold.sorted();
    let chosen: Vec<&(&Entry, Option<usize>)> = all
        .iter()
        .filter(|(e, _)| q.states.contains(&e.state))
        .collect();
    let total = chosen.len() as u64;
    let from = q.offset.min(total) as usize;
    let to = (from as u64).saturating_add(q.limit).min(total) as usize;
    let items: Vec<Value> = chosen[from..to]
        .iter()
        .map(|(e, p)| fold.item_json(e, *p))
        .collect();
    let next = (to as u64) < total;
    let out = json!({
        "items": items,
        "total": total,
        "offset": q.offset,
        "limit": q.limit,
        "next": if next { json!(to) } else { Value::Null },
        "counts": fold.counts(),
        "record_lines": fold.seq,
        "damaged": damaged.map(|d| json!({"after_seq": d.after_seq, "segment": d.segment,
                                          "line": d.line, "problem": d.problem})),
        "redaction": {"env_unset": env_unset},
    });
    Ok(red.redact_value(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(seq: u64, kind: &str, body: Value) -> Line {
        let Value::Object(body) = body else { panic!() };
        Line {
            seq,
            at: seq as i64,
            kind: kind.into(),
            body,
        }
    }

    fn accept(f: &mut Fold, seq: u64, id: &str, role: &str, channel: &str) {
        f.apply(&line(
            seq,
            "stimulus.accepted",
            json!({"item": {"id": id, "kind": "peer", "role": role, "channel": channel,
                            "at": 1000 + seq, "text": format!("text of {id}")}}),
        ));
    }

    #[test]
    fn an_owner_control_is_never_listed() {
        let mut f = Fold::default();
        f.apply(&line(
            1,
            "stimulus.accepted",
            json!({"item": {"id": "s", "kind": "peer", "role": "owner", "channel": "owner",
                            "at": 1, "text": "stop", "control": "stop"}}),
        ));
        assert!(f.item("s").is_none());
    }

    #[test]
    fn a_requeued_item_waits_again_in_its_old_place() {
        let mut f = Fold::default();
        accept(&mut f, 1, "a", "owner", "owner");
        accept(&mut f, 2, "b", "owner", "owner");
        f.apply(&line(
            3,
            "stimulus.admitted",
            json!({"turn": 1, "ids": ["a"]}),
        ));
        assert_eq!(f.item("a").unwrap()["state"], "answering");
        f.apply(&line(4, "stimulus.requeued", json!({"ids": ["a"]})));
        let a = f.item("a").unwrap();
        assert_eq!(a["state"], "waiting");
        assert_eq!(a["position"], 1);
        assert_eq!(f.item("b").unwrap()["position"], 2);
    }

    fn moved(f: &mut Fold, seq: u64, id: &str, before: Option<&str>) {
        f.apply(&line(
            seq,
            "queue.moved",
            json!({"id": id, "before": before}),
        ));
    }

    fn waiting(f: &Fold) -> Vec<String> {
        f.sorted()
            .into_iter()
            .filter(|(_, p)| p.is_some())
            .map(|(e, _)| e.id.clone())
            .collect()
    }

    #[test]
    fn the_list_follows_the_moves_and_keeps_the_owner_on_top() {
        let mut f = Fold::default();
        accept(&mut f, 1, "p1", "peer", "peer:atlas");
        accept(&mut f, 2, "o1", "owner", "owner");
        accept(&mut f, 3, "o2", "owner", "owner");
        accept(&mut f, 4, "p2", "peer", "peer:bolt");
        assert_eq!(waiting(&f), ["o1", "o2", "p1", "p2"]);
        moved(&mut f, 5, "o2", Some("o1"));
        moved(&mut f, 6, "p2", Some("p1"));
        assert_eq!(waiting(&f), ["o2", "o1", "p2", "p1"]);
        // never across groups, whatever a line says
        moved(&mut f, 7, "p2", Some("o1"));
        moved(&mut f, 8, "o2", Some("p2"));
        assert_eq!(waiting(&f), ["o2", "o1", "p2", "p1"]);
        assert_eq!(f.item("o2").unwrap()["position"], 1);
        assert_eq!(f.item("p1").unwrap()["position"], 4);
        // a move changes order, not the item's `order` (its acceptance)
        assert_eq!(f.item("o2").unwrap()["order"], 3);
        // the end of the group
        moved(&mut f, 9, "o2", None);
        assert_eq!(waiting(&f), ["o1", "o2", "p2", "p1"]);
        // a withdrawn item is out of the order; a move of it is no move
        f.apply(&line(
            10,
            "stimulus.disposed",
            json!({"id": "o1", "disposition": "withdrawn", "turn": 1}),
        ));
        moved(&mut f, 11, "o1", Some("o2"));
        assert_eq!(waiting(&f), ["o2", "p2", "p1"]);
        assert_eq!(f.item("o1").unwrap()["state"], "withdrawn");
    }

    #[test]
    fn an_item_disposed_once_stays_settled() {
        let mut f = Fold::default();
        accept(&mut f, 1, "a", "owner", "owner");
        f.apply(&line(
            2,
            "stimulus.disposed",
            json!({"id": "a", "disposition": "digested", "turn": 4}),
        ));
        let a = f.item("a").unwrap();
        assert_eq!(a["state"], "answered");
        assert_eq!(a["disposition"], "digested");
        assert_eq!(a["turn"], 4);
    }

    #[test]
    fn a_held_item_stops_being_held_once_admitted() {
        let mut f = Fold::default();
        accept(&mut f, 1, "p", "peer", "peer:atlas");
        f.apply(&line(
            2,
            "decision.admit",
            json!({"choice": {"forms": {"p": "at_break"}}}),
        ));
        assert!(f.item("p").unwrap()["held"].is_string());
        f.apply(&line(
            3,
            "stimulus.admitted",
            json!({"turn": 2, "ids": ["p"]}),
        ));
        assert!(f.item("p").unwrap()["held"].is_null());
    }

    fn state_with(n: u64) -> rung_testkit::TempDir {
        use std::io::Write;
        let dir = rung_testkit::TempDir::new("queue-page");
        std::fs::create_dir_all(dir.path().join("record")).unwrap();
        let mut f = std::fs::File::create(dir.path().join("record/seg-00000001.ndjson")).unwrap();
        for i in 1..=n {
            let l = json!({"seq": i, "at": 1000 + i, "kind": "stimulus.accepted",
                           "item": {"id": format!("m{i}"), "kind": "peer", "role": "owner",
                                    "channel": "owner", "at": 1000 + i, "text": format!("t{i}")}});
            writeln!(f, "{l}").unwrap();
        }
        dir
    }

    #[test]
    fn the_page_is_cut_by_the_caller_and_the_total_is_exact() {
        let dir = state_with(5);
        let red = Redactor::new();
        let page = |offset: u64, limit: u64| {
            let q = Query {
                offset,
                limit,
                ..Query::default()
            };
            answer(dir.path(), &q, &red, &[]).unwrap()
        };
        let p = page(0, 2);
        assert_eq!(p["total"], 5);
        assert_eq!(p["items"].as_array().unwrap().len(), 2);
        assert_eq!(p["next"], 2);
        let p = page(4, 2);
        assert_eq!(p["items"].as_array().unwrap().len(), 1);
        assert!(p["next"].is_null());
        // past the end is an empty last page; a huge limit is no maximum
        let p = page(99, 2);
        assert_eq!(p["items"], json!([]));
        assert!(p["next"].is_null());
        assert_eq!(page(0, u64::MAX)["items"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn a_query_names_its_states_and_refuses_what_it_does_not_know() {
        let pair = |k: &str, v: &str| vec![(k.to_string(), v.to_string())];
        assert_eq!(Query::parse(&pair("limit", "0")).unwrap_err().status, 400);
        assert_eq!(Query::parse(&pair("limit", "x")).unwrap_err().status, 400);
        assert_eq!(
            Query::parse(&pair("state", "sleeping")).unwrap_err().status,
            400
        );
        assert_eq!(
            Query::parse(&pair("colour", "red")).unwrap_err().status,
            400
        );
        assert_eq!(Query::parse(&pair("state", "all")).unwrap().states.len(), 4);
        assert_eq!(
            Query::parse(&pair("state", "answered,withdrawn,answered"))
                .unwrap()
                .states,
            [State::Answered, State::Withdrawn]
        );
        let twice = vec![("limit".into(), "1".into()), ("limit".into(), "2".into())];
        assert_eq!(Query::parse(&twice).unwrap_err().status, 400);
    }
}
