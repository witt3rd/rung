//! The credential redactor for the host's doors, streams and console sink
//! (design note H4).
//!
//! The redactor itself lives in [`rung_agent_core::redact`], so there is one
//! definition of what a credential looks like for the whole workspace
//! ([`rung_agent_core::mcp::redact`] is the same redactor with its own marker);
//! this module re-exports it and adds the host's canonical JSON line form.
//! See `docs/rung-host-api.md` for the contract.

use std::borrow::Cow;

use serde_json::Value;

use crate::canon;

pub use rung_agent_core::redact::{MARK, Redactor, redact};

/// Redaction of one record or stream line.
pub trait RedactJsonLine {
    /// One JSON line (no newline) with its secrets replaced, as canonical
    /// JSON. A line with nothing to redact comes back byte for byte; a line
    /// that is not JSON is redacted as text.
    fn redact_json_line<'a>(&self, line: &'a str) -> Cow<'a, str>;
}

impl RedactJsonLine for Redactor {
    fn redact_json_line<'a>(&self, line: &'a str) -> Cow<'a, str> {
        match serde_json::from_str::<Value>(line) {
            Ok(v) => {
                let red = self.redact_value(&v);
                if red == v {
                    Cow::Borrowed(line)
                } else {
                    Cow::Owned(canon::string(&red))
                }
            }
            Err(_) => self.redact(line),
        }
    }
}
