//! Delivery as a typed receipt (props section 4).
//!
//! **The durable record is the delivery** (`the-durable-record-is-the-delivery`).
//! [`send`] puts a message in the receiving host's inbox directory and returns
//! a [`Sent`], which is only a promise. [`Sent::confirm`] reads the receiving
//! host's own record and, once the record holds the item, returns a
//! [`Receipt`] that names the record line.
//!
//! Both are `#[must_use]` and the crate denies `unused_must_use`, so a function
//! that sends and drops its receipt fails to compile
//! (`an-unconsumed-receipt-does-not-compile`). A receipt is consumed exactly
//! once: it is neither `Clone` nor `Copy`, and each way of consuming it takes
//! it by value.
//!
//! Delivery is idempotent by **correlation id**
//! (`duplicate-delivery-is-idempotent`): the id is the inbox file's stem, which
//! is the item's id, and the host never accepts the same id twice. Sending the
//! same id again, before or after the host accepted it, adds nothing.

use std::fmt;
use std::path::{Path, PathBuf};

use rung_host::inbox::Role;
use rung_host::read;
use serde_json::json;

/// Why a delivery or its confirmation failed.
#[derive(Debug)]
pub enum DeliveryError {
    /// The correlation id is not a safe file stem.
    BadId(String),
    /// The inbox could not be written.
    Io(std::io::Error),
    /// The receiving host's record could not be read.
    Record(String),
}

impl fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeliveryError::BadId(id) => write!(f, "correlation id {id:?} is not a safe name"),
            DeliveryError::Io(e) => write!(f, "inbox write failed: {e}"),
            DeliveryError::Record(e) => write!(f, "record read failed: {e}"),
        }
    }
}

impl std::error::Error for DeliveryError {}

/// What kind of thing is delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Completion,
    Handoff,
    Escalation,
    /// Direction to a member from its steward: an owner-class item, read at
    /// the target's next boundary (`a-steer-is-an-inbox-item`).
    Steer,
    /// A request to halt a member, through the host's stop authority
    /// (`stop-records-why`): an owner item whose `control` is `stop`, so the host
    /// records the item (its text is the reason, its channel names who asked) and
    /// then halts.
    Stop,
    /// A task given to a member with its brief: an owner item, read at the
    /// target's next boundary.
    Assign,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Completion => "completion",
            Kind::Handoff => "handoff",
            Kind::Escalation => "escalation",
            Kind::Steer => "steer",
            Kind::Stop => "stop",
            Kind::Assign => "assign",
        }
    }
}

/// A correlation id: the name a delivery is deduplicated by. It is joined into
/// a path, so only `[A-Za-z0-9._-]` is allowed, and it may not start with a dot.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CorrelationId(String);

impl CorrelationId {
    pub fn new(id: impl Into<String>) -> Result<Self, DeliveryError> {
        let id = id.into();
        let ok = !id.is_empty()
            && id.len() <= 128
            && !id.starts_with('.')
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if ok {
            Ok(CorrelationId(id))
        } else {
            Err(DeliveryError::BadId(id))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A message to deliver.
#[derive(Clone, Debug)]
pub struct Delivery {
    pub correlation: CorrelationId,
    pub kind: Kind,
    /// The sending member's instance id.
    pub from: String,
    pub text: String,
}

/// Where a delivery goes: the receiving host's inbox directory and the state
/// directory whose record is the delivery.
#[derive(Clone, Debug)]
pub struct Target {
    pub inbox: PathBuf,
    pub state: PathBuf,
}

/// A message handed to the inbox, not yet seen in the receiver's record.
#[must_use = "a Sent is a promise: confirm it against the receiver's record"]
#[derive(Debug)]
pub struct Sent {
    correlation: CorrelationId,
    state: PathBuf,
}

/// Proof that the receiving host's record holds the item; names the line.
#[must_use = "a Receipt must be recorded, handed on, or explicitly discarded"]
#[derive(Debug, PartialEq, Eq)]
pub struct Receipt {
    correlation: CorrelationId,
    seq: u64,
}

/// A receipt that has been recorded by the sender: what is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Acknowledged {
    pub correlation: CorrelationId,
    pub seq: u64,
}

/// Put a delivery in the target's inbox. Idempotent by correlation id: an
/// existing file of that name is left as it is.
pub fn send(target: &Target, d: &Delivery) -> Result<Sent, DeliveryError> {
    std::fs::create_dir_all(&target.inbox).map_err(DeliveryError::Io)?;
    let path = target.inbox.join(format!("{}.msg", d.correlation.as_str()));
    if !path.exists() {
        let owner = matches!(d.kind, Kind::Steer | Kind::Stop | Kind::Assign);
        let mut body = json!({
            "role": if owner { Role::Owner.word() } else { Role::Peer.word() },
            "text": format!(
                "[fleet:{} from={} id={}]\n{}",
                d.kind.word(),
                d.from,
                d.correlation.as_str(),
                d.text
            ),
        });
        if d.kind == Kind::Stop {
            body["control"] = json!("stop");
            body["channel"] = json!(format!("fleet:{}", d.from));
        }
        // Write beside the inbox name and rename, so the host never reads a
        // torn file; the temporary name does not end in `.msg`.
        let tmp = target
            .inbox
            .join(format!(".{}.tmp", d.correlation.as_str()));
        std::fs::write(&tmp, body.to_string()).map_err(DeliveryError::Io)?;
        std::fs::rename(&tmp, &path).map_err(DeliveryError::Io)?;
    }
    Ok(Sent {
        correlation: d.correlation.clone(),
        state: target.state.clone(),
    })
}

impl Sent {
    pub fn correlation(&self) -> &CorrelationId {
        &self.correlation
    }

    /// Read the receiver's record. If it holds the item, the delivery has
    /// happened and the receipt names the line; otherwise the promise is handed
    /// back unchanged, to be confirmed again later.
    pub fn confirm(self) -> Result<Receipt, Pending> {
        let mut found = None;
        let scanned = read::scan(&self.state, |l| {
            if found.is_none()
                && l.kind == "stimulus.accepted"
                && l.get("item")["id"].as_str() == Some(self.correlation.as_str())
            {
                found = Some(l.seq);
            }
        });
        match (found, scanned) {
            (Some(seq), _) => Ok(Receipt {
                correlation: self.correlation,
                seq,
            }),
            // The intact prefix lacks the item, but the record is damaged past
            // it: the item may be beyond the damage, so this is not "waiting".
            (None, Ok(scan)) => match scan.damaged {
                Some(d) => Err(Pending::Unreadable(
                    self,
                    DeliveryError::Record(format!(
                        "record damaged after seq {} ({} line {}): {}",
                        d.after_seq, d.segment, d.line, d.problem
                    )),
                )),
                None => Err(Pending::Waiting(self)),
            },
            (None, Err(e)) => Err(Pending::Unreadable(
                self,
                DeliveryError::Record(e.to_string()),
            )),
        }
    }
}

/// Why a [`Sent`] is not yet a [`Receipt`]. The promise comes back either way.
#[must_use]
#[derive(Debug)]
pub enum Pending {
    /// The record is intact and does not hold the item yet.
    Waiting(Sent),
    /// The record could not be read, or is damaged past its intact prefix.
    Unreadable(Sent, DeliveryError),
}

impl Pending {
    pub fn into_sent(self) -> Sent {
        match self {
            Pending::Waiting(s) | Pending::Unreadable(s, _) => s,
        }
    }
}

impl Receipt {
    pub fn correlation(&self) -> &CorrelationId {
        &self.correlation
    }

    /// The sequence number of the receiver's record line that holds the item.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Consume the receipt by recording it.
    pub fn record(self) -> Acknowledged {
        Acknowledged {
            correlation: self.correlation,
            seq: self.seq,
        }
    }

    /// Consume the receipt by handing it to a function that takes it on.
    pub fn hand_on<T>(self, to: impl FnOnce(Receipt) -> T) -> T {
        to(self)
    }

    /// Consume the receipt saying, explicitly, that the sender does not care.
    pub fn discard(self) {}
}

/// Where a record's accepted lines for a correlation id are (for tests and
/// for a steward that wants to count): how many lines accepted `id`.
pub fn accepted_count(state: &Path, id: &CorrelationId) -> Result<usize, DeliveryError> {
    let mut n = 0;
    read::scan(state, |l| {
        if l.kind == "stimulus.accepted" && l.get("item")["id"].as_str() == Some(id.as_str()) {
            n += 1;
        }
    })
    .map_err(|e| DeliveryError::Record(e.to_string()))?;
    Ok(n)
}
