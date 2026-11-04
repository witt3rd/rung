//! The fleet's observing verbs, as a library (`fleet-verbs`): `list` and
//! `read`. Both are reads: they take no lock, write no file and send no
//! message (`read-and-list-never-write`), so they are fit for a read-only role.
//!
//! A member is a host with an entry in the instance index; it is found there
//! and read through the host's own record reader. The command in
//! `src/main.rs` holds no behaviour this module lacks.
//!
//! Why the fleet has its own `list` and `read` beside `rung-host ls` and the
//! gateway: they are the same reads over a library surface a steward can call
//! (no process, no HTTP), and `read` adds the crew view; the index and record
//! readers, and the list's JSON shape, are the host's and are reused, not copied.

use std::fmt;
use std::path::Path;

use rung_host::read;
use rung_host::record::Line;
use rung_host::registry::{self, Row};

use crate::records::{CrewRecord, RecordError};

/// Why a verb could not answer.
#[derive(Debug)]
pub enum VerbError {
    /// The instance index could not be read.
    Index(String),
    /// No member has this id.
    NoSuchMember(String),
    /// The member has no readable entry (its entry file is damaged).
    NoEntry(String),
    /// The member's record could not be read.
    Record(String),
    /// A crew line of a known kind was malformed.
    Crew(RecordError),
}

impl fmt::Display for VerbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerbError::Index(e) => write!(f, "instance index: {e}"),
            VerbError::NoSuchMember(id) => write!(f, "no member `{id}` in the instance index"),
            VerbError::NoEntry(id) => write!(f, "member `{id}` has no readable entry"),
            VerbError::Record(e) => write!(f, "record: {e}"),
            VerbError::Crew(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for VerbError {}

/// Every member in the instance index `dir`, by id. A stopped or down member
/// stays in the list, and a damaged entry is a row with a problem.
pub fn list(dir: &Path) -> Result<Vec<Row>, VerbError> {
    registry::list_in(dir).map_err(VerbError::Index)
}

/// What `read` returns: the lines asked for, and whether the record is damaged
/// past them.
#[derive(Debug)]
pub struct Read {
    pub lines: Vec<Line>,
    /// The record's total intact lines.
    pub total: u64,
    /// A description of damage after the intact prefix, if any.
    pub damaged: Option<String>,
}

fn state_of(dir: &Path, id: &str) -> Result<std::path::PathBuf, VerbError> {
    match registry::row_in(dir, id).map_err(VerbError::Index)? {
        None => Err(VerbError::NoSuchMember(id.to_string())),
        Some(row) => row
            .entry
            .map(|e| e.state_dir)
            .ok_or_else(|| VerbError::NoEntry(id.to_string())),
    }
}

/// Read a member's record: up to `limit` lines with `seq` greater than
/// `after`, in order. Works on a stopped member.
pub fn read(dir: &Path, id: &str, after: u64, limit: usize) -> Result<Read, VerbError> {
    let state = state_of(dir, id)?;
    let mut lines = Vec::new();
    let scanned = read::scan(&state, |l| {
        if l.seq > after && lines.len() < limit {
            lines.push(l.clone());
        }
    })
    .map_err(|e| VerbError::Record(e.message))?;
    Ok(Read {
        lines,
        total: scanned.total,
        damaged: scanned.damaged.map(|d| {
            format!(
                "damaged after seq {} ({} line {}): {}",
                d.after_seq, d.segment, d.line, d.problem
            )
        }),
    })
}

/// The crew lines of a member's record, in order. Other kinds, and crew kinds
/// this version does not know, are skipped.
pub fn crew(dir: &Path, id: &str) -> Result<Vec<(u64, CrewRecord)>, VerbError> {
    let state = state_of(dir, id)?;
    let mut out = Vec::new();
    let mut bad = None;
    read::scan(&state, |l| match CrewRecord::from_line(l) {
        Ok(Some(r)) => out.push((l.seq, r)),
        Ok(None) => {}
        Err(e) => bad = bad.take().or(Some(e)),
    })
    .map_err(|e| VerbError::Record(e.message))?;
    match bad {
        Some(e) => Err(VerbError::Crew(e)),
        None => Ok(out),
    }
}
