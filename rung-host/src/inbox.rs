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
                self.seen.insert(item.id.clone());
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
            }
            _ => {}
        }
    }

    /// Waiting items, oldest first.
    pub fn waiting(&self) -> Vec<&Pending> {
        let mut v: Vec<&Pending> = self.pending.values().collect();
        v.sort_by_key(|p| p.order);
        v
    }

    /// In-flight ids (a turn that never ended leaves some).
    pub fn in_flight_ids(&self) -> Vec<String> {
        self.in_flight.keys().cloned().collect()
    }
}

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
