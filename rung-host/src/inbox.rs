//! The inbox: every stimulus, durable on arrival, drained only at a boundary.
//!
//! An [`Item`] arrives from a [`Source`] (the memory queue, a directory of
//! `*.msg` files, the fake world) or from the host itself (a calendar fire,
//! a settled expectation). It is recorded (`stimulus.accepted`) and fsynced
//! before anything else happens. The pending set is a projection of the
//! record ([`InboxState`]); admission is a record line written at a
//! boundary with that boundary's sealed [`crate::presence::Edge`].
//!
//! Each item ends with exactly one `stimulus.disposed` line. A turn that
//! fails, or a process that dies mid-turn, requeues its batch; the items are
//! admitted again later and disposed once.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::clock::Millis;
use crate::read::DoorError;
use crate::record::Line;

/// Who sent an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Peer,
    Observer,
    /// The host itself (calendar, expectations, world checks).
    Host,
}

impl Role {
    /// The group an item waits in: the owner's items (0) stay on top, then
    /// peers' (1), then every other source's (2). A move stays in one group.
    pub fn group(self) -> u8 {
        match self {
            Role::Owner => 0,
            Role::Peer => 1,
            Role::Observer | Role::Host => 2,
        }
    }
}

/// [`Role::group`] for a role as the record spells it (a word that is no role
/// is the host's own: the last group).
pub fn group_of(role: &str) -> u8 {
    use serde::Deserialize;
    use serde::de::IntoDeserializer;
    let de: serde::de::value::StrDeserializer<'_, serde::de::value::Error> =
        role.into_deserializer();
    Role::deserialize(de).map_or(2, Role::group)
}

/// What an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// A message from a channel.
    Peer,
    Calendar,
    /// A settled expectation.
    Expectation,
    /// A world fact.
    World,
    Memory,
    /// An external completion (the delegation extension's entry point).
    Completion,
}

/// A fact the world asserts, by key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    pub key: String,
    pub value: Value,
}

/// One stimulus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub kind: ItemKind,
    pub role: Role,
    /// The channel it came from (`owner`, `peer:<name>`, `calendar`, ...).
    pub channel: String,
    /// When it arrived.
    pub at: Millis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<Millis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urgency: Option<String>,
    /// A firm calendar item is admitted when due, whatever the desk says.
    #[serde(default)]
    pub firm: bool,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact: Option<Fact>,
    /// An owner control (`stop`, `release`): acted on at once, never shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
}

/// At most this many characters of an item reach a decider.
pub const GIST_CHARS: usize = 200;

impl Item {
    /// A message from a channel.
    pub fn message(id: &str, role: Role, channel: &str, at: Millis, text: &str) -> Self {
        Self {
            id: id.into(),
            kind: ItemKind::Peer,
            role,
            channel: channel.into(),
            at,
            due: None,
            urgency: None,
            firm: false,
            text: text.into(),
            fact: None,
            control: None,
        }
    }

    /// The first [`GIST_CHARS`] characters, credentials redacted.
    pub fn gist(&self) -> String {
        gist(&self.text)
    }
}

/// A short, redacted form of `text` for a decider's state.
pub fn gist(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let red = rung_agent_core::mcp::redact(&flat);
    red.chars().take(GIST_CHARS).collect()
}

/// An item waiting in the inbox.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pending {
    pub item: Item,
    /// The seq of its `stimulus.accepted` line (its place in the queue).
    pub order: u64,
    /// When it was last requeued (a failed or interrupted turn).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requeued_at: Option<Millis>,
}

/// The order items wait in: the fold of acceptance order and the moves
/// (`queue.moved`). It holds every item that is not yet disposed (an item
/// being answered keeps its place, so one requeued waits where it did).
///
/// The fold rules, which the host's door and every reader of the record share:
/// an item takes the end of the order when it is accepted; a `queue.moved`
/// line `{id, before}` takes `id` out and puts it in front of `before` (the
/// end of its group when `before` is null), but only when `id` waits, `before`
/// (if any) waits, and both are in one group; a line that does not meet that
/// is read as no move. The later line wins. A move changes order only: when
/// the desk shows an item is the desk's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Order {
    ids: Vec<String>,
}

impl Order {
    /// A newly accepted item goes to the end.
    pub fn push(&mut self, id: &str) {
        if !self.ids.iter().any(|x| x == id) {
            self.ids.push(id.to_string());
        }
    }

    /// An item that is disposed has no place.
    pub fn remove(&mut self, id: &str) {
        if let Some(i) = self.ids.iter().position(|x| x == id) {
            self.ids.remove(i);
        }
    }

    /// Each id's place, for sorting.
    pub fn places(&self) -> BTreeMap<&str, usize> {
        self.ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect()
    }

    /// Fold one `queue.moved` line. `group` says the group of an item that
    /// waits, and `None` for any that does not (unknown, being answered, over).
    /// True when the order changed.
    pub fn apply_move(
        &mut self,
        id: &str,
        before: Option<&str>,
        group: impl Fn(&str) -> Option<u8>,
    ) -> bool {
        let Some(g) = group(id) else {
            return false;
        };
        if let Some(b) = before
            && (b == id || group(b) != Some(g) || !self.ids.iter().any(|x| x == b))
        {
            return false;
        }
        let Some(from) = self.ids.iter().position(|x| x == id) else {
            return false;
        };
        self.ids.remove(from);
        let at = match before {
            Some(b) => self.ids.iter().position(|x| x == b).unwrap_or(from),
            None => self
                .ids
                .iter()
                .rposition(|x| group(x) == Some(g))
                .map_or(from, |i| i + 1),
        };
        let at = at.min(self.ids.len());
        self.ids.insert(at, id.to_string());
        from != at
    }
}

/// The inbox as the record leaves it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct InboxState {
    /// Waiting, by id.
    pub pending: BTreeMap<String, Pending>,
    /// In a running turn: id → (turn, item).
    pub in_flight: BTreeMap<String, (u64, Pending)>,
    /// Every id ever accepted (a directory source deduplicates against it).
    pub seen: BTreeSet<String>,
    /// The last external (non-host) arrival.
    pub last_external_at: Option<Millis>,
    /// Owner items told at once that the model is unavailable (a
    /// `host:ack`), until they are disposed.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub acked: BTreeSet<String>,
    /// The order the unsettled items wait in (acceptance, then moves).
    #[serde(skip)]
    pub order: Order,
    /// The forms of the desk's latest `decision.admit`: id → `now` |
    /// `at_break` | `digest`.
    #[serde(skip)]
    pub forms: BTreeMap<String, String>,
    /// Items withdrawn before any turn read them.
    #[serde(skip)]
    pub withdrawn: BTreeSet<String>,
    /// The key of each `queue.moved` line that has one: key → (id, before).
    #[serde(skip)]
    pub move_keys: BTreeMap<String, (String, Option<String>)>,
}

impl InboxState {
    pub fn apply(&mut self, l: &Line) {
        match l.kind.as_str() {
            "stimulus.accepted" => {
                let Ok(item) = serde_json::from_value::<Item>(l.get("item").clone()) else {
                    return;
                };
                if item.role != Role::Host {
                    self.last_external_at = Some(item.at.max(self.last_external_at.unwrap_or(0)));
                }
                if self.seen.insert(item.id.clone()) {
                    self.order.push(&item.id);
                }
                self.pending.insert(
                    item.id.clone(),
                    Pending {
                        item,
                        order: l.seq,
                        requeued_at: None,
                    },
                );
            }
            "stimulus.admitted" => {
                let turn = l.u64("turn");
                for id in ids(l.get("ids")).into_iter().chain(ids(l.get("digests"))) {
                    if let Some(p) = self.pending.remove(&id) {
                        self.in_flight.insert(id, (turn, p));
                    }
                }
            }
            "stimulus.requeued" => {
                for id in ids(l.get("ids")) {
                    if let Some((_, mut p)) = self.in_flight.remove(&id) {
                        p.requeued_at = Some(l.at);
                        self.pending.insert(id, p);
                    }
                }
            }
            "stimulus.disposed" => {
                let id = l.str("id");
                self.in_flight.remove(id);
                self.pending.remove(id);
                self.acked.remove(id);
                self.order.remove(id);
                if l.str("disposition") == "withdrawn" {
                    self.withdrawn.insert(id.to_string());
                }
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
            "queue.moved" => {
                let (id, before) = (l.str("id"), l.get("before").as_str());
                let pending = &self.pending;
                self.order.apply_move(id, before, |x| {
                    pending
                        .get(x)
                        .filter(|p| p.item.control.is_none())
                        .map(|p| p.item.role.group())
                });
                if let Some(k) = l.get("key").as_str() {
                    self.move_keys
                        .entry(k.to_string())
                        .or_insert_with(|| (id.to_string(), before.map(str::to_string)));
                }
            }
            "outbox.queued" if l.str("source") == ACK_SOURCE => {
                self.acked.insert(l.str("item").to_string());
            }
            _ => {}
        }
    }

    /// Waiting items in the order they wait in: the owner's first, then peers',
    /// then any other source's; within a group, the order of acceptance as the
    /// moves have changed it.
    pub fn waiting(&self) -> Vec<&Pending> {
        let places = self.order.places();
        let mut v: Vec<&Pending> = self.pending.values().collect();
        v.sort_by_key(|p| {
            (
                p.item.role.group(),
                places
                    .get(p.item.id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                p.order,
            )
        });
        v
    }

    /// Where an item stands, for the doors that move or cancel it. An owner
    /// control (`stop`, `release`) is acted on and never listed: to a door it is
    /// no item.
    pub fn standing(&self, id: &str) -> Standing {
        if let Some(p) = self.pending.get(id) {
            return if p.item.control.is_some() {
                Standing::Unknown
            } else {
                Standing::Waiting
            };
        }
        if let Some((turn, _)) = self.in_flight.get(id) {
            return Standing::Answering(*turn);
        }
        if self.withdrawn.contains(id) {
            Standing::Withdrawn
        } else if self.seen.contains(id) {
            Standing::Answered
        } else {
            Standing::Unknown
        }
    }

    /// What a waiting item waits for: the boundary the desk last gave it. The
    /// owner's items are always shown at the next boundary; any other item is
    /// shown then, at the agent's next break, or as a digest line, as the
    /// desk's latest `decision.admit` said.
    fn boundary(&self, p: &Pending) -> &'static str {
        if p.item.role == Role::Owner {
            return "next";
        }
        match self.forms.get(&p.item.id).map(String::as_str) {
            Some("at_break") => "break",
            Some("digest") => "digest",
            _ => "next",
        }
    }

    /// Judge a move before it is written: refused in words (the status and code
    /// the door answers), a move to the item's own place a no-op, else a move.
    /// Nothing here writes; the host asks and writes under one lock.
    ///
    /// The rules (`docs/rung-host-api.md`, S3): `id` and `before` must wait; they
    /// must be in one group (the owner's items stay above every other sender's,
    /// so a peer's item is never put above an owner's nor an owner's below a
    /// peer's); and every item the move jumps over must wait for the same
    /// boundary as `id`. `before` null is the end of `id`'s group.
    pub fn plan_move(&self, id: &str, before: Option<&str>) -> Result<MovePlan, DoorError> {
        let waits = |x: &str, role: &str| -> Result<(), DoorError> {
            match self.standing(x) {
                Standing::Waiting => Ok(()),
                Standing::Unknown => Err(DoorError::new(
                    404,
                    "not_found",
                    format!("no item has the id {x}: nothing was written"),
                )),
                other => Err(DoorError::new(
                    409,
                    "not_waiting",
                    format!("{x} {}, so {role}: nothing was written", other.words()),
                )),
            }
        };
        waits(id, "it cannot be moved")?;
        if let Some(b) = before {
            if b == id {
                return Ok(MovePlan::Nothing);
            }
            waits(b, "nothing can be put in front of it")?;
        }
        let me = &self.pending[id];
        let group = me.item.role.group();
        if let Some(b) = before {
            let other = &self.pending[b];
            let og = other.item.role.group();
            if og != group {
                let (mine, theirs) = (group_name(group), group_name(og));
                let why = if group > og {
                    format!(
                        "{theirs} items stay above {mine} ones, so {id} cannot be put above one"
                    )
                } else {
                    format!(
                        "{mine} items stay above {theirs} ones, so {id} cannot be put below one"
                    )
                };
                return Err(DoorError::new(
                    409,
                    "group",
                    format!(
                        "{id} is {mine} item and {b} is {theirs} item: {why}; nothing was written"
                    ),
                ));
            }
        }
        let line: Vec<&Pending> = self
            .waiting()
            .into_iter()
            .filter(|p| p.item.role.group() == group)
            .collect();
        let i = line.iter().position(|p| p.item.id == id).unwrap_or(0);
        let jumped: &[&Pending] = match before {
            None => &line[(i + 1).min(line.len())..],
            Some(b) => {
                let j = line.iter().position(|p| p.item.id == b).unwrap_or(i);
                if j < i {
                    &line[j..i]
                } else if j > i + 1 {
                    &line[i + 1..j]
                } else {
                    &[]
                }
            }
        };
        if jumped.is_empty() {
            return Ok(MovePlan::Nothing);
        }
        let mine = self.boundary(me);
        if let Some(p) = jumped.iter().find(|p| self.boundary(p) != mine) {
            return Err(DoorError::new(
                409,
                "boundary",
                format!(
                    "{id} waits for {} and {} for {}: a move only reorders items that wait for the same boundary, and the desk decides when each is shown; nothing was written",
                    boundary_words(mine),
                    p.item.id,
                    boundary_words(self.boundary(p)),
                ),
            ));
        }
        Ok(MovePlan::Move)
    }

    /// In-flight ids (a turn that never ended leaves some).
    pub fn in_flight_ids(&self) -> Vec<String> {
        self.in_flight.keys().cloned().collect()
    }
}

/// Where an item stands (see [`InboxState::standing`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Waiting,
    /// In the turn that is answering it.
    Answering(u64),
    Withdrawn,
    /// Disposed by a turn.
    Answered,
    Unknown,
}

impl Standing {
    /// What the doors say of an item that is not waiting.
    pub fn words(self) -> String {
        match self {
            Standing::Waiting => "is waiting".into(),
            Standing::Answering(turn) => {
                format!("is being answered right now (turn {turn})")
            }
            Standing::Withdrawn => "was withdrawn".into(),
            Standing::Answered => "was answered".into(),
            Standing::Unknown => "is not known".into(),
        }
    }
}

/// What judging a move found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovePlan {
    /// The item is already where the move puts it.
    Nothing,
    Move,
}

fn group_name(group: u8) -> &'static str {
    match group {
        0 => "the owner's",
        1 => "a peer's",
        _ => "the host's own",
    }
}

fn boundary_words(b: &str) -> &'static str {
    match b {
        "break" => "the agent's next break",
        "digest" => "a digest line in a later turn header",
        _ => "the next boundary",
    }
}

/// The source of the host's own acknowledgement to an owner while the model
/// cannot be called.
pub const ACK_SOURCE: &str = "host:ack";

pub(crate) fn ids(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// ─── Sources ─────────────────────────────────────────────────────────────────

/// Where items come from.
pub trait Source: Send {
    /// Items that have arrived by `now`. `seen` holds every id already
    /// accepted, so a source that may offer an item twice (a crash between
    /// recording it and removing its file) can skip it.
    fn poll(&mut self, now: Millis, seen: &BTreeSet<String>) -> Vec<Item>;

    /// Called after `item` is recorded and fsynced.
    fn accepted(&mut self, item: &Item) {
        let _ = item;
    }

    /// The time of the next item, when the source knows it (a scheduled
    /// world does; a directory does not).
    fn next_at(&self) -> Option<Millis> {
        None
    }
}

/// An in-memory queue: items pushed by the host's own code or a test.
#[derive(Debug, Default)]
pub struct MemorySource {
    queue: Vec<Item>,
}

impl MemorySource {
    pub fn push(&mut self, item: Item) {
        self.queue.push(item);
    }
}

impl Source for MemorySource {
    fn poll(&mut self, now: Millis, seen: &BTreeSet<String>) -> Vec<Item> {
        let (due, later): (Vec<Item>, Vec<Item>) = self.queue.drain(..).partition(|i| i.at <= now);
        self.queue = later;
        due.into_iter().filter(|i| !seen.contains(&i.id)).collect()
    }

    fn next_at(&self) -> Option<Millis> {
        self.queue.iter().map(|i| i.at).min()
    }
}

/// A `*.msg` file: one JSON object.
#[derive(Debug, Deserialize)]
struct MsgFile {
    #[serde(default)]
    role: Option<Role>,
    #[serde(default)]
    channel: Option<String>,
    text: String,
    #[serde(default)]
    urgency: Option<String>,
    /// Owner only: `stop` or `release`.
    #[serde(default)]
    control: Option<String>,
}

/// A directory of `*.msg` files. Each file is one message:
/// `{"role": "owner"|"peer"|"observer", "channel": "...", "text": "...",
/// "urgency": "..."}`; the file stem is the item id. A file is removed only
/// after its item is recorded; a malformed one moves to `rejected/`.
#[derive(Debug)]
pub struct DirSource {
    dir: PathBuf,
    /// Malformed files seen this poll: (name, why).
    pub rejected: Vec<(String, String)>,
}

impl DirSource {
    pub fn new(dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            rejected: Vec::new(),
        })
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.msg"))
    }
}

impl Source for DirSource {
    fn poll(&mut self, now: Millis, seen: &BTreeSet<String>) -> Vec<Item> {
        let mut names: Vec<String> = match fs::read_dir(&self.dir) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(".msg"))
                .collect(),
            Err(_) => return Vec::new(),
        };
        names.sort();
        let mut out = Vec::new();
        for name in names {
            let id = name.trim_end_matches(".msg").to_string();
            let path = self.dir.join(&name);
            if seen.contains(&id) {
                // Recorded before a crash cut the removal short.
                let _ = fs::remove_file(&path);
                continue;
            }
            let Ok(body) = fs::read_to_string(&path) else {
                continue;
            };
            match serde_json::from_str::<MsgFile>(&body) {
                Ok(m) => {
                    let role = m.role.filter(|r| *r != Role::Host).unwrap_or(Role::Peer);
                    // Only an owner file may name its channel; anyone else
                    // is pinned to `peer:<id>`.
                    let channel = match (role, m.channel) {
                        (Role::Owner, c) => c.unwrap_or_else(|| "owner".into()),
                        _ => format!("peer:{id}"),
                    };
                    let mut item = Item::message(&id, role, &channel, now, &m.text);
                    item.urgency = m.urgency;
                    item.control = m.control.filter(|_| role == Role::Owner);
                    out.push(item);
                }
                Err(e) => {
                    let rejected = self.dir.join("rejected");
                    let _ = fs::create_dir_all(&rejected);
                    let _ = fs::rename(&path, rejected.join(&name));
                    self.rejected.push((name, e.to_string()));
                }
            }
        }
        out
    }

    fn accepted(&mut self, item: &Item) {
        let _ = fs::remove_file(self.path(&item.id));
    }
}

/// The record body of an accepted item.
pub(crate) fn accepted_body(item: &Item) -> Value {
    json!({ "item": item })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(seq: u64, kind: &str, body: Value) -> Line {
        let Value::Object(m) = body else { panic!() };
        Line {
            seq,
            at: seq as i64,
            kind: kind.into(),
            body: m,
        }
    }

    #[test]
    fn the_inbox_follows_the_record() {
        let mut s = InboxState::default();
        let a = Item::message("a", Role::Owner, "owner", 1, "hi");
        let b = Item::message("b", Role::Peer, "peer:x", 2, "yo");
        s.apply(&line(1, "stimulus.accepted", accepted_body(&a)));
        s.apply(&line(2, "stimulus.accepted", accepted_body(&b)));
        assert_eq!(s.waiting().len(), 2);
        s.apply(&line(
            3,
            "stimulus.admitted",
            json!({"turn": 1, "ids": ["a"], "digests": ["b"]}),
        ));
        assert!(s.pending.is_empty());
        s.apply(&line(4, "stimulus.requeued", json!({"ids": ["a"]})));
        assert_eq!(s.waiting()[0].item.id, "a");
        s.apply(&line(5, "stimulus.disposed", json!({"id": "b"})));
        assert_eq!(s.in_flight.len(), 0);
        assert_eq!(s.seen.len(), 2);
    }

    fn accept(s: &mut InboxState, seq: u64, id: &str, role: Role, channel: &str) {
        let it = Item::message(id, role, channel, seq as i64, "text");
        s.apply(&line(seq, "stimulus.accepted", accepted_body(&it)));
    }

    fn moved(s: &mut InboxState, seq: u64, id: &str, before: Option<&str>) {
        s.apply(&line(
            seq,
            "queue.moved",
            json!({"id": id, "before": before}),
        ));
    }

    fn order(s: &InboxState) -> Vec<&str> {
        s.waiting().iter().map(|p| p.item.id.as_str()).collect()
    }

    #[test]
    fn a_move_is_folded_in_front_of_the_item_or_at_the_end_of_its_group() {
        let mut s = InboxState::default();
        for (i, id) in ["a", "b", "c", "d"].iter().enumerate() {
            accept(&mut s, i as u64 + 1, id, Role::Owner, "owner");
        }
        assert_eq!(order(&s), ["a", "b", "c", "d"]);
        moved(&mut s, 5, "d", Some("a"));
        assert_eq!(order(&s), ["d", "a", "b", "c"]);
        moved(&mut s, 6, "d", None);
        assert_eq!(order(&s), ["a", "b", "c", "d"]);
        moved(&mut s, 7, "b", Some("d"));
        assert_eq!(order(&s), ["a", "c", "b", "d"]);
        // the later line wins
        moved(&mut s, 8, "a", Some("b"));
        moved(&mut s, 9, "b", Some("a"));
        assert_eq!(order(&s), ["c", "b", "a", "d"]);
    }

    #[test]
    fn a_line_that_does_not_meet_the_rules_is_read_as_no_move() {
        let mut s = InboxState::default();
        accept(&mut s, 1, "o1", Role::Owner, "owner");
        accept(&mut s, 2, "p1", Role::Peer, "peer:p1");
        accept(&mut s, 3, "p2", Role::Peer, "peer:p2");
        accept(&mut s, 4, "o2", Role::Owner, "owner");
        assert_eq!(order(&s), ["o1", "o2", "p1", "p2"]);
        // across groups, either way; an unknown item; itself; an item in front
        // of an unknown one
        moved(&mut s, 5, "p2", Some("o1"));
        moved(&mut s, 6, "o1", Some("p2"));
        moved(&mut s, 7, "ghost", Some("o1"));
        moved(&mut s, 8, "p1", Some("p1"));
        moved(&mut s, 9, "p1", Some("ghost"));
        assert_eq!(order(&s), ["o1", "o2", "p1", "p2"]);
        // an item that is being answered, or is over, takes no move
        s.apply(&line(
            10,
            "stimulus.admitted",
            json!({"turn": 1, "ids": ["p2"]}),
        ));
        moved(&mut s, 11, "p2", Some("p1"));
        moved(&mut s, 12, "p1", Some("p2"));
        assert_eq!(order(&s), ["o1", "o2", "p1"]);
        // a requeued item waits again in its own place
        s.apply(&line(13, "stimulus.requeued", json!({"ids": ["p2"]})));
        assert_eq!(order(&s), ["o1", "o2", "p1", "p2"]);
        s.apply(&line(
            14,
            "stimulus.disposed",
            json!({"id": "p1", "disposition": "withdrawn", "turn": 1}),
        ));
        moved(&mut s, 15, "o1", Some("p1"));
        assert_eq!(order(&s), ["o1", "o2", "p2"]);
        assert_eq!(s.standing("p1"), Standing::Withdrawn);
    }

    #[test]
    fn a_move_never_changes_which_group_an_item_is_in() {
        let mut s = InboxState::default();
        accept(&mut s, 1, "p1", Role::Peer, "peer:p1");
        accept(&mut s, 2, "o1", Role::Owner, "owner");
        accept(&mut s, 3, "h1", Role::Host, "calendar");
        moved(&mut s, 4, "o1", None);
        moved(&mut s, 5, "p1", None);
        assert_eq!(order(&s), ["o1", "p1", "h1"]);
    }

    #[test]
    fn a_move_judged_before_it_is_written() {
        let mut s = InboxState::default();
        accept(&mut s, 1, "o1", Role::Owner, "owner");
        accept(&mut s, 2, "o2", Role::Owner, "owner");
        accept(&mut s, 3, "p1", Role::Peer, "peer:p1");
        accept(&mut s, 4, "p2", Role::Peer, "peer:p2");
        accept(&mut s, 5, "p3", Role::Peer, "peer:p3");
        let plan = |s: &InboxState, id: &str, b: Option<&str>| s.plan_move(id, b);
        assert_eq!(plan(&s, "o2", Some("o1")).unwrap(), MovePlan::Move);
        assert_eq!(plan(&s, "o1", Some("o2")).unwrap(), MovePlan::Nothing);
        assert_eq!(plan(&s, "o2", None).unwrap(), MovePlan::Nothing);
        assert_eq!(plan(&s, "o1", None).unwrap(), MovePlan::Move);
        assert_eq!(plan(&s, "o1", Some("o1")).unwrap(), MovePlan::Nothing);
        let e = plan(&s, "p1", Some("o1")).unwrap_err();
        assert_eq!((e.status, e.code), (409, "group"));
        let e = plan(&s, "o2", Some("p1")).unwrap_err();
        assert_eq!((e.status, e.code), (409, "group"));
        assert_eq!(plan(&s, "ghost", None).unwrap_err().status, 404);
        assert_eq!(plan(&s, "o1", Some("ghost")).unwrap_err().status, 404);
        // the desk gave p1 a break and p2 and p3 the next boundary: a move that
        // jumps over an item waiting for another boundary is refused, one that
        // jumps over none is not
        s.apply(&line(
            6,
            "decision.admit",
            json!({"choice": {"forms": {"p1": "at_break", "p2": "now", "p3": "now"}}}),
        ));
        assert_eq!(plan(&s, "p3", Some("p2")).unwrap(), MovePlan::Move);
        let e = plan(&s, "p3", Some("p1")).unwrap_err();
        assert_eq!((e.status, e.code), (409, "boundary"));
        assert!(e.message.contains("break"), "{}", e.message);
        let e = plan(&s, "p1", None).unwrap_err();
        assert_eq!(e.code, "boundary");
        // the end of the group, jumping nothing: no move
        assert_eq!(plan(&s, "p3", None).unwrap(), MovePlan::Nothing);
        // owner items never wait for the desk's boundary, whatever it said
        s.apply(&line(
            7,
            "decision.admit",
            json!({"choice": {"forms": {"o1": "at_break", "p1": "at_break"}}}),
        ));
        assert_eq!(plan(&s, "o2", Some("o1")).unwrap(), MovePlan::Move);
    }

    #[test]
    fn a_key_is_remembered_from_the_line() {
        let mut s = InboxState::default();
        accept(&mut s, 1, "a", Role::Owner, "owner");
        accept(&mut s, 2, "b", Role::Owner, "owner");
        s.apply(&line(
            3,
            "queue.moved",
            json!({"id": "b", "before": "a", "key": "mv-1"}),
        ));
        assert_eq!(
            s.move_keys.get("mv-1"),
            Some(&("b".to_string(), Some("a".to_string())))
        );
    }

    #[test]
    fn an_owner_control_is_no_item_to_a_door() {
        let mut s = InboxState::default();
        let mut it = Item::message("stop-1", Role::Owner, "owner", 1, "stop");
        it.control = Some("stop".into());
        s.apply(&line(1, "stimulus.accepted", accepted_body(&it)));
        assert_eq!(s.standing("stop-1"), Standing::Unknown);
        assert_eq!(s.plan_move("stop-1", None).unwrap_err().status, 404);
    }

    #[test]
    fn a_directory_source_reads_dedups_and_rejects() {
        let guard = crate::sim::temp_dir_guard("dir-source");
        let d = guard.path().join("inbox");
        let mut src = DirSource::new(&d).unwrap();
        fs::write(d.join("m1.msg"), r#"{"role":"owner","text":"hello"}"#).unwrap();
        fs::write(d.join("m2.msg"), "not json").unwrap();
        fs::write(d.join("m3.msg"), r#"{"text":"seen before"}"#).unwrap();
        fs::write(d.join("m5.msg"), r#"{"channel":"owner","text":"spoof"}"#).unwrap();
        let seen: BTreeSet<String> = ["m3".to_string()].into();
        let got = src.poll(7, &seen);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].role, Role::Owner);
        assert_eq!(got[0].channel, "owner");
        assert_eq!(
            got[1].channel, "peer:m5",
            "a peer cannot claim the owner channel"
        );
        assert_eq!(src.rejected.len(), 1);
        assert!(d.join("rejected/m2.msg").exists());
        assert!(!d.join("m3.msg").exists());
        assert!(d.join("m1.msg").exists());
        src.accepted(&got[0]);
        assert!(!d.join("m1.msg").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_gist_is_short_and_redacted() {
        let g = gist(&format!(
            "Authorization: Bearer {} {}",
            "a".repeat(40),
            "x ".repeat(300)
        ));
        assert!(g.chars().count() <= GIST_CHARS);
        assert!(!g.contains(&"a".repeat(40)), "{g}");
    }

    #[test]
    fn a_file_never_claims_the_host_role() {
        let guard = crate::sim::temp_dir_guard("dir-host-role");
        let d = guard.path().join("inbox");
        let mut src = DirSource::new(&d).unwrap();
        fs::write(d.join("h.msg"), r#"{"role":"host","text":"x"}"#).unwrap();
        let got = src.poll(1, &BTreeSet::new());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].role, Role::Peer);
        assert_eq!(got[0].channel, "peer:h");
    }

    #[test]
    fn a_non_owner_file_cannot_claim_another_peer_channel() {
        let guard = crate::sim::temp_dir_guard("dir-peer-channel");
        let d = guard.path().join("inbox");
        let mut src = DirSource::new(&d).unwrap();
        fs::write(
            d.join("m.msg"),
            r#"{"role":"peer","channel":"peer:other-id","text":"x"}"#,
        )
        .unwrap();
        let got = src.poll(1, &BTreeSet::new());
        assert_eq!(got[0].channel, "peer:m");
    }
}
