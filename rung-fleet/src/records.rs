//! The `crew.*` record kinds (`crew-record-kinds-are-fleet-owned`).
//!
//! The host reserves the `crew` group; this module owns what its lines hold.
//! A [`CrewRecord`] encodes to the body of one record line and decodes from
//! one. Decoding is total over every line a record can hold: a line that is
//! not a `crew.*` kind, or a `crew.*` kind this version does not know, decodes
//! to `None` (a reader skips what it does not know); a known kind with a missing
//! or malformed field is an error, never a guess.
//!
//! This module only gives the schema. Appending is the host's act on its own
//! record; the fleet writes no file of the receiver's record.

use std::fmt;

use rung_host::record::Line;
use serde_json::{Map, Value, json};

use crate::brief::{Autonomy, Brief, BriefError};

/// The group every kind here belongs to.
pub const GROUP: &str = "crew";

/// One `crew.*` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrewRecord {
    /// `crew.spawned`: a task exists, with its brief and who is to do it.
    Spawned {
        crew: String,
        brief: Brief,
        assignee: String,
    },
    /// `crew.assigned`: the assignee has taken the task.
    Assigned { crew: String },
    /// `crew.active`: work has begun.
    Active { crew: String },
    /// `crew.held`: waiting for a decision, open until its `key` is resolved.
    Held { crew: String, key: String },
    /// `crew.resolved`: a decision arrived, carrying the key of the hold.
    Resolved { crew: String, key: String },
    /// `crew.stalled`: silent past its bound.
    Stalled { crew: String, stalls: u32 },
    /// `crew.recovered`: a stalled task is moving again.
    Recovered { crew: String },
    /// `crew.done`: judged complete by a validator.
    Done {
        crew: String,
        summary: String,
        validator: String,
    },
    /// `crew.retired`: the task is closed.
    Retired { crew: String },
    /// `crew.receipt`: a delivery was confirmed at a line of the receiver's
    /// record (`seq` is that line's number; the body names it `line`, since
    /// `seq` is the line's own number).
    Confirmed {
        crew: String,
        correlation: String,
        seq: u64,
    },
}

/// Why a `crew.*` line of a known kind was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordError {
    /// A required field is absent or of the wrong type.
    Field { kind: String, field: &'static str },
    /// The brief fields do not make a brief.
    BadBrief(BriefError),
    /// The autonomy word is not one the fleet knows.
    BadAutonomy(String),
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordError::Field { kind, field } => {
                write!(
                    f,
                    "`{kind}` line: field `{field}` is missing or not of its type"
                )
            }
            RecordError::BadBrief(e) => write!(f, "`crew.spawned` line: {e}"),
            RecordError::BadAutonomy(w) => write!(f, "`crew.spawned` line: unknown autonomy {w:?}"),
        }
    }
}

impl std::error::Error for RecordError {}

fn autonomy_word(a: Autonomy) -> &'static str {
    match a {
        Autonomy::Ask => "ask",
        Autonomy::Decide => "decide",
    }
}

impl CrewRecord {
    /// The record kind of this line.
    pub fn kind(&self) -> &'static str {
        match self {
            CrewRecord::Spawned { .. } => "crew.spawned",
            CrewRecord::Assigned { .. } => "crew.assigned",
            CrewRecord::Active { .. } => "crew.active",
            CrewRecord::Held { .. } => "crew.held",
            CrewRecord::Resolved { .. } => "crew.resolved",
            CrewRecord::Stalled { .. } => "crew.stalled",
            CrewRecord::Recovered { .. } => "crew.recovered",
            CrewRecord::Done { .. } => "crew.done",
            CrewRecord::Retired { .. } => "crew.retired",
            CrewRecord::Confirmed { .. } => "crew.receipt",
        }
    }

    /// Every kind this version writes.
    pub const KINDS: [&'static str; 10] = [
        "crew.spawned",
        "crew.assigned",
        "crew.active",
        "crew.held",
        "crew.resolved",
        "crew.stalled",
        "crew.recovered",
        "crew.done",
        "crew.retired",
        "crew.receipt",
    ];

    /// The task this line is about.
    pub fn crew(&self) -> &str {
        match self {
            CrewRecord::Spawned { crew, .. }
            | CrewRecord::Assigned { crew }
            | CrewRecord::Active { crew }
            | CrewRecord::Held { crew, .. }
            | CrewRecord::Resolved { crew, .. }
            | CrewRecord::Stalled { crew, .. }
            | CrewRecord::Recovered { crew }
            | CrewRecord::Done { crew, .. }
            | CrewRecord::Retired { crew }
            | CrewRecord::Confirmed { crew, .. } => crew,
        }
    }

    /// The fields of the line other than `seq`, `at` and `kind`.
    pub fn body(&self) -> Map<String, Value> {
        let v = match self {
            CrewRecord::Spawned {
                crew,
                brief,
                assignee,
            } => json!({
                "crew": crew,
                "assignee": assignee,
                "objective": brief.objective(),
                "done_when": brief.done_when(),
                "delivery": brief.delivery(),
                "autonomy": autonomy_word(brief.autonomy()),
            }),
            CrewRecord::Assigned { crew }
            | CrewRecord::Active { crew }
            | CrewRecord::Recovered { crew }
            | CrewRecord::Retired { crew } => json!({ "crew": crew }),
            CrewRecord::Held { crew, key } | CrewRecord::Resolved { crew, key } => {
                json!({ "crew": crew, "key": key })
            }
            CrewRecord::Stalled { crew, stalls } => json!({ "crew": crew, "stalls": stalls }),
            CrewRecord::Done {
                crew,
                summary,
                validator,
            } => json!({ "crew": crew, "summary": summary, "validator": validator }),
            CrewRecord::Confirmed {
                crew,
                correlation,
                seq,
            } => json!({ "crew": crew, "correlation": correlation, "line": seq }),
        };
        match v {
            Value::Object(m) => m,
            _ => unreachable!("every body is an object"),
        }
    }

    /// Read a record line. `Ok(None)` for a line that is not a `crew.*` kind
    /// this version knows.
    pub fn from_line(line: &Line) -> Result<Option<CrewRecord>, RecordError> {
        let kind = line.kind.as_str();
        if !Self::KINDS.contains(&kind) {
            return Ok(None);
        }
        let text = |field: &'static str| -> Result<String, RecordError> {
            line.get(field)
                .as_str()
                .map(str::to_string)
                .ok_or(RecordError::Field {
                    kind: kind.to_string(),
                    field,
                })
        };
        let crew = text("crew")?;
        Ok(Some(match kind {
            "crew.spawned" => {
                let autonomy = match text("autonomy")?.as_str() {
                    "ask" => Autonomy::Ask,
                    "decide" => Autonomy::Decide,
                    other => return Err(RecordError::BadAutonomy(other.to_string())),
                };
                let brief = Brief::new(
                    text("objective")?,
                    text("done_when")?,
                    text("delivery")?,
                    autonomy,
                )
                .map_err(RecordError::BadBrief)?;
                CrewRecord::Spawned {
                    crew,
                    brief,
                    assignee: text("assignee")?,
                }
            }
            "crew.assigned" => CrewRecord::Assigned { crew },
            "crew.active" => CrewRecord::Active { crew },
            "crew.held" => CrewRecord::Held {
                crew,
                key: text("key")?,
            },
            "crew.resolved" => CrewRecord::Resolved {
                crew,
                key: text("key")?,
            },
            "crew.stalled" => CrewRecord::Stalled {
                crew,
                stalls: line
                    .get("stalls")
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or(RecordError::Field {
                        kind: kind.to_string(),
                        field: "stalls",
                    })?,
            },
            "crew.recovered" => CrewRecord::Recovered { crew },
            "crew.done" => CrewRecord::Done {
                crew,
                summary: text("summary")?,
                validator: text("validator")?,
            },
            "crew.retired" => CrewRecord::Retired { crew },
            "crew.receipt" => CrewRecord::Confirmed {
                crew,
                correlation: text("correlation")?,
                seq: line.get("line").as_u64().ok_or(RecordError::Field {
                    kind: kind.to_string(),
                    field: "line",
                })?,
            },
            _ => unreachable!("checked against KINDS"),
        }))
    }

    /// The line this record would be, at a given sequence number and time.
    pub fn to_line(&self, seq: u64, at: i64) -> Line {
        Line {
            seq,
            at,
            kind: self.kind().to_string(),
            body: self.body(),
        }
    }
}
