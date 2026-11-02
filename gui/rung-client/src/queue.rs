//! The queue and the owner's controls (host API: S1 queue, S2 intent, S3 move
//! and cancel, S7 stop and release).
//!
//! Reading the queue is open to the read-only role. Every other call here
//! writes: the gateway answers it only for the owner (a client with **no**
//! token; the read-only token is refused 403 `read_only` in the gateway's own
//! words), and only for an instance it holds a write key for. Each call that
//! may be retried takes a `key`, so a retry changes nothing twice. Nothing is
//! sent on its own: a caller sends on a click.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{Client, Error, message_of};

/// What the owner asks of a message (S2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// A task: read at the next turn boundary.
    Next,
    /// A message about what the agent is doing now.
    Steer,
    /// Wait for free time.
    Free,
}

impl Intent {
    pub fn as_str(self) -> &'static str {
        match self {
            Intent::Next => "next",
            Intent::Steer => "steer",
            Intent::Free => "free",
        }
    }
}

/// One item of an instance's queue.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct QueueItem {
    pub id: String,
    #[serde(default)]
    pub text: String,
    /// Who sent it: `owner`, `peer`, `host`.
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub channel: Option<String>,
    /// `next`, `steer` or `free`.
    #[serde(default)]
    pub intent: Option<String>,
    /// Its place among the waiting, from 1.
    #[serde(default)]
    pub position: Option<u64>,
    #[serde(default)]
    pub accepted_at: Option<i64>,
    /// `waiting`, `answering`, `answered` or `withdrawn`.
    #[serde(default)]
    pub state: String,
    /// The turn answering it.
    #[serde(default)]
    pub turn: Option<u64>,
}

/// How many items are in each state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Counts {
    #[serde(default)]
    pub waiting: u64,
    #[serde(default)]
    pub answering: u64,
    #[serde(default)]
    pub answered: u64,
    #[serde(default)]
    pub withdrawn: u64,
}

/// `GET /api/i/{id}/v1/queue`: the item being answered, then what waits
/// (the owner's first), in the host's order.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct QueueList {
    #[serde(default)]
    pub items: Vec<QueueItem>,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub counts: Counts,
}

/// `POST .../queue`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Added {
    pub id: String,
    /// The key was already accepted: nothing was written again.
    #[serde(default)]
    pub repeat: bool,
}

/// `POST .../queue/{id}/move`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Moved {
    #[serde(default)]
    pub moved: bool,
    #[serde(default)]
    pub repeat: bool,
    /// Why nothing moved, when it was already in place.
    #[serde(default)]
    pub message: Option<String>,
}

/// `DELETE .../queue/{id}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Cancelled {
    #[serde(default)]
    pub withdrawn: bool,
    #[serde(default)]
    pub repeat: bool,
}

/// `POST .../stop`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Stopped {
    #[serde(default)]
    pub stopping: bool,
    #[serde(default)]
    pub repeat: bool,
}

/// `POST .../release`: let go of whatever commitment is held right now.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Released {
    #[serde(default)]
    pub released: bool,
    #[serde(default)]
    pub repeat: bool,
}

/// An id in a path: letters, digits and `-._~` as they are, the rest as
/// `%XX` (a peer's item id may hold a space).
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn check_id(id: &str) -> Result<(), Error> {
    let plain = !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    if plain {
        Ok(())
    } else {
        Err(Error::BadId(id.to_string()))
    }
}

impl Client {
    /// The instance's queue: what is being answered and what waits (`state`
    /// is the door's own list: `waiting`, `answering`, `answered`,
    /// `withdrawn`, `open` (the default) or `all`).
    pub fn queue(&self, id: &str, state: Option<&str>) -> Result<QueueList, Error> {
        check_id(id)?;
        let q = state
            .map(|s| format!("?state={}", encode(s)))
            .unwrap_or_default();
        self.get(&format!("/api/i/{id}/v1/queue{q}"))
    }

    /// Add a message to the queue. `key` becomes the item's id, so sending
    /// the same key again adds once.
    pub fn queue_add(
        &self,
        id: &str,
        text: &str,
        intent: Intent,
        key: &str,
    ) -> Result<Added, Error> {
        check_id(id)?;
        self.write(
            "POST",
            &format!("/api/i/{id}/v1/queue"),
            Some(json!({"text": text, "key": key, "intent": intent.as_str()})),
        )
    }

    /// Put `item` in front of `before` (`None`: at the end of its group).
    pub fn queue_move(
        &self,
        id: &str,
        item: &str,
        before: Option<&str>,
        key: &str,
    ) -> Result<Moved, Error> {
        check_id(id)?;
        self.write(
            "POST",
            &format!("/api/i/{id}/v1/queue/{}/move", encode(item)),
            Some(json!({"before": before, "key": key})),
        )
    }

    /// Withdraw a waiting item.
    pub fn queue_cancel(&self, id: &str, item: &str) -> Result<Cancelled, Error> {
        check_id(id)?;
        self.write(
            "DELETE",
            &format!("/api/i/{id}/v1/queue/{}", encode(item)),
            None,
        )
    }

    /// Ask the host to stop. It halts at its next check.
    pub fn stop(&self, id: &str) -> Result<Stopped, Error> {
        check_id(id)?;
        self.write("POST", &format!("/api/i/{id}/v1/stop"), None)
    }

    /// Release whatever commitment is held at the moment this arrives.
    pub fn release(&self, id: &str, reason: Option<&str>) -> Result<Released, Error> {
        check_id(id)?;
        let body = reason.map(|r| json!({"reason": r}));
        self.write("POST", &format!("/api/i/{id}/v1/release"), body)
    }

    fn write<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<T, Error> {
        let mut req = self.agent.request(method, &format!("{}{path}", self.base));
        if let Some(t) = &self.token {
            req = req.set("authorization", &format!("Bearer {t}"));
        }
        let sent = match body {
            Some(b) => req
                .set("content-type", "application/json")
                .send_string(&b.to_string()),
            None => req.call(),
        };
        match sent {
            Ok(resp) => {
                let text = resp
                    .into_string()
                    .map_err(|e| Error::Transport(self.scrub(e.to_string())))?;
                serde_json::from_str(&text).map_err(|e| Error::Decode(e.to_string()))
            }
            Err(ureq::Error::Status(code, resp)) => Err(Error::Status {
                code,
                message: message_of(&resp.into_string().unwrap_or_default()),
            }),
            Err(ureq::Error::Transport(t)) => Err(Error::Transport(self.scrub(t.to_string()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_ids_are_percent_encoded() {
        assert_eq!(encode("q-77"), "q-77");
        assert_eq!(encode("a b/c?"), "a%20b%2Fc%3F");
        assert_eq!(encode("é"), "%C3%A9");
    }

    #[test]
    fn an_item_reads_as_the_door_sends_it() {
        let q: QueueList = serde_json::from_str(
            r#"{"items":[{"id":"k-1","text":"hello","role":"owner","name":null,"channel":"owner","kind":"peer",
               "intent":"steer","order":50,"position":1,"accepted_at":5,"state":"waiting","turn":null,
               "disposition":null,"held":null}],"total":1,"offset":0,"limit":500,"next":null,
               "counts":{"waiting":1,"answering":0,"answered":9,"withdrawn":0},"record_lines":50}"#,
        )
        .unwrap();
        assert_eq!(q.items[0].intent.as_deref(), Some("steer"));
        assert_eq!((q.items[0].position, q.counts.answered), (Some(1), 9));
    }
}
