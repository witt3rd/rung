//! The event stream (`GET /api/i/{id}/v1/events`): one instance's record,
//! followed, with a cursor to resume from, and the partial text of the turn
//! that is running (host API, H5).
//!
//! [`Client::events`] is one connection: an iterator of [`Frame`]s. [`follow`]
//! is the loop a window runs on a worker thread: it keeps the cursor (the
//! last record number seen), reconnects with `Last-Event-ID` when the
//! connection drops, and backs off quietly while the gateway or the instance
//! is away. Nothing here interprets a record line; the host's words go through
//! as they came (the gateway redacts every frame).

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

use crate::{Client, Error, message_of};

/// The first frame of a stream.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Open {
    /// The cursor the stream started after.
    #[serde(default)]
    pub after: u64,
    /// A host holds the instance's state: the record may still grow.
    #[serde(default)]
    pub live: bool,
    /// This stream will carry partial frames.
    #[serde(default)]
    pub partials: bool,
}

/// A partial frame: short-lived text and tool progress of the running turn.
/// Never in the record, never numbered, never replayed.
#[derive(Debug, Clone, PartialEq)]
pub struct Delta {
    pub turn: u64,
    pub call: u64,
    /// `text`, `tool_call`, `tool_input`, `tool_run`, `tool_done`,
    /// `block_start`, `block_end`, `turn_end` or `behind`.
    pub kind: String,
    /// The text of a `text` frame.
    pub text: Option<String>,
    /// The tool of a `tool_call`, `tool_run` or `tool_done` frame.
    pub name: Option<String>,
    /// `tool_done`: did it succeed.
    pub ok: Option<bool>,
    /// `tool_done`: its output (or its error).
    pub output: Option<String>,
    /// `behind`: how many partial frames this reader missed.
    pub skipped: Option<u64>,
}

/// The last frame: why the stream ended.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct End {
    /// `stopped`, `down`, `damaged` or `error`.
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub last_seq: Option<u64>,
    #[serde(default)]
    pub problem: Option<String>,
}

/// One server-sent frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Open(Open),
    /// One record line, as the host wrote it (redacted), with its number.
    Record {
        seq: u64,
        line: Value,
    },
    Delta(Delta),
    End(End),
    /// A frame this client does not know: kept, never an error, so the
    /// gateway may add some.
    Other {
        event: String,
        data: String,
    },
}

/// One connection's frames, in order, until the body ends.
pub struct Events {
    reader: Box<dyn BufRead + Send>,
    done: bool,
}

impl Events {
    #[cfg(test)]
    pub(crate) fn from_reader(reader: impl BufRead + Send + 'static) -> Events {
        Events {
            reader: Box::new(reader),
            done: false,
        }
    }
}

impl Iterator for Events {
    type Item = Result<Frame, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let (mut event, mut id, mut data) = (String::new(), String::new(), String::new());
        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => {
                    self.done = true;
                    return None;
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(Error::Transport(e.to_string())));
                }
                Ok(_) => {}
            }
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                // a blank line ends a frame; a keepalive has no event
                if event.is_empty() && data.is_empty() {
                    continue;
                }
                return Some(decode(&event, &id, &data));
            }
            if line.starts_with(':') {
                continue; // a comment: the keepalive
            }
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.strip_prefix(' ').unwrap_or(value);
            match field {
                "event" => event = value.to_string(),
                "id" => id = value.to_string(),
                "data" => {
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(value);
                }
                _ => {}
            }
        }
    }
}

fn decode(event: &str, id: &str, data: &str) -> Result<Frame, Error> {
    let bad = |e: serde_json::Error| Error::Decode(format!("{event} frame: {e}"));
    match event {
        "open" => serde_json::from_str(data).map(Frame::Open).map_err(bad),
        "end" => serde_json::from_str(data).map(Frame::End).map_err(bad),
        "record" => {
            let seq = id
                .parse()
                .map_err(|_| Error::Decode(format!("record frame without a number: {id:?}")))?;
            let line = serde_json::from_str(data).map_err(bad)?;
            Ok(Frame::Record { seq, line })
        }
        "delta" => {
            let v: Value = serde_json::from_str(data).map_err(bad)?;
            let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
            let num = |k: &str| v.get(k).and_then(Value::as_u64);
            Ok(Frame::Delta(Delta {
                turn: num("turn").unwrap_or(0),
                call: num("call").unwrap_or(0),
                kind: text("kind").unwrap_or_default(),
                text: text("text"),
                name: text("name"),
                ok: v.get("ok").and_then(Value::as_bool),
                output: text("output"),
                skipped: num("skipped"),
            }))
        }
        other => Ok(Frame::Other {
            event: other.to_string(),
            data: data.to_string(),
        }),
    }
}

impl Client {
    /// Open the event stream of instance `id`, after record number `after`
    /// (sent as `Last-Event-ID`, which the gateway takes as the cursor). The
    /// first frame is `open` or the call fails with the gateway's own refusal
    /// (404 unknown instance, 409 cursor ahead of the record, ...).
    pub fn events(&self, id: &str, after: Option<u64>) -> Result<Events, Error> {
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            return Err(Error::BadId(id.to_string()));
        }
        let mut req = self
            .stream_agent
            .get(&format!("{}/api/i/{id}/v1/events", self.base))
            .set("accept", "text/event-stream");
        if let Some(t) = &self.token {
            req = req.set("authorization", &format!("Bearer {t}"));
        }
        if let Some(n) = after {
            req = req.set("last-event-id", &n.to_string());
        }
        match req.call() {
            Ok(resp) => Ok(Events {
                reader: Box::new(BufReader::new(resp.into_reader())),
                done: false,
            }),
            Err(ureq::Error::Status(code, resp)) => Err(Error::Status {
                code,
                message: message_of(&resp.into_string().unwrap_or_default()),
            }),
            Err(ureq::Error::Transport(t)) => Err(Error::Transport(self.scrub(t.to_string()))),
        }
    }
}

/// How [`follow`] backs off: the wait before the next connection starts at
/// `first`, doubles to `max`, and starts over once a connection has stayed up
/// for `healthy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Follow {
    pub first: Duration,
    pub max: Duration,
    pub healthy: Duration,
}

impl Default for Follow {
    fn default() -> Follow {
        Follow {
            first: Duration::from_secs(1),
            max: Duration::from_secs(30),
            healthy: Duration::from_secs(60),
        }
    }
}

/// What [`follow`] tells its caller.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A connection is open (the `open` frame).
    Connected(Open),
    /// A frame, in order. A record frame advances the cursor.
    Frame(Frame),
    /// The connection is gone or could not be made; the next try is in
    /// `retry_in`, from the cursor.
    Lost { why: String, retry_in: Duration },
}

/// Follow instance `id` until `stop` is set or `on` returns false: connect,
/// pass every frame on, and when the connection drops or ends reconnect after
/// a back-off, resuming from the last record number seen so nothing is missed
/// or repeated. `after` is where to start (`None`: the beginning).
pub fn follow(
    client: &Client,
    id: &str,
    after: Option<u64>,
    opts: Follow,
    stop: &AtomicBool,
    mut on: impl FnMut(Event) -> bool,
) {
    let mut cursor = after;
    let mut wait = opts.first;
    while !stop.load(Ordering::Relaxed) {
        let started = Instant::now();
        let why = match client.events(id, cursor) {
            Err(e @ Error::BadId(_)) => {
                on(Event::Lost {
                    why: e.to_string(),
                    retry_in: Duration::MAX,
                });
                return;
            }
            Err(e) => {
                // a cursor past the record is from another instance: start over
                if matches!(e, Error::Status { code: 409, .. }) {
                    cursor = None;
                }
                e.to_string()
            }
            Ok(events) => {
                let mut why = "the stream ended".to_string();
                for frame in events {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let frame = match frame {
                        Ok(f) => f,
                        Err(e) => {
                            why = e.to_string();
                            break;
                        }
                    };
                    let event = match frame {
                        Frame::Open(o) => Event::Connected(o),
                        Frame::Record { seq, line } => {
                            cursor = Some(seq);
                            Event::Frame(Frame::Record { seq, line })
                        }
                        Frame::End(e) => {
                            why = format!("the instance's stream ended: {}", e.reason);
                            if !on(Event::Frame(Frame::End(e))) {
                                return;
                            }
                            break;
                        }
                        other => Event::Frame(other),
                    };
                    if !on(event) {
                        return;
                    }
                }
                why
            }
        };
        // a connection that stayed up has earned a fresh back-off
        if started.elapsed() >= opts.healthy {
            wait = opts.first;
        }
        if !on(Event::Lost {
            why,
            retry_in: wait,
        }) {
            return;
        }
        // sleep in short slices so a stop is seen at once
        let until = Instant::now() + wait;
        while Instant::now() < until {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20).min(until - Instant::now()));
        }
        wait = (wait * 2).min(opts.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(text: &str) -> Vec<Result<Frame, Error>> {
        Events::from_reader(std::io::Cursor::new(text.to_string())).collect()
    }

    #[test]
    fn frames_are_read_in_order_with_keepalives_and_unknowns_passed() {
        let got = frames(
            "event: open\ndata: {\"contract\":1,\"after\":3,\"live\":true,\"partials\":true}\n\n\
             : keepalive\n\n\
             event: record\nid: 4\ndata: {\"seq\":4,\"at\":9,\"kind\":\"turn.started\"}\n\n\
             event: delta\ndata: {\"turn\":2,\"call\":0,\"kind\":\"text\",\"text\":\"hel\"}\n\n\
             event: delta\ndata: {\"turn\":2,\"call\":0,\"kind\":\"behind\",\"skipped\":7}\n\n\
             event: frobnicate\ndata: x\ndata: y\n\n\
             event: end\ndata: {\"reason\":\"stopped\",\"last_seq\":4,\"problem\":null}\n\n",
        );
        let f: Vec<Frame> = got.into_iter().map(Result::unwrap).collect();
        assert_eq!(
            f[0],
            Frame::Open(Open {
                after: 3,
                live: true,
                partials: true
            })
        );
        assert_eq!(
            f[1],
            Frame::Record {
                seq: 4,
                line: serde_json::json!({"seq":4,"at":9,"kind":"turn.started"})
            }
        );
        match &f[2] {
            Frame::Delta(d) => assert_eq!(
                (d.turn, d.kind.as_str(), d.text.as_deref()),
                (2, "text", Some("hel"))
            ),
            other => panic!("{other:?}"),
        }
        match &f[3] {
            Frame::Delta(d) => assert_eq!((d.kind.as_str(), d.skipped), ("behind", Some(7))),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            f[4],
            Frame::Other {
                event: "frobnicate".into(),
                data: "x\ny".into()
            }
        );
        assert_eq!(
            f[5],
            Frame::End(End {
                reason: "stopped".into(),
                last_seq: Some(4),
                problem: None
            })
        );
        assert_eq!(f.len(), 6);
    }

    #[test]
    fn a_broken_frame_is_an_error_not_a_stop() {
        let got = frames(
            "event: record\nid: x\ndata: {}\n\nevent: record\nid: 5\ndata: nope\n\n\
             event: record\nid: 6\ndata: {\"seq\":6}\n\n",
        );
        assert!(matches!(got[0], Err(Error::Decode(_))));
        assert!(matches!(got[1], Err(Error::Decode(_))));
        assert!(matches!(got[2], Ok(Frame::Record { seq: 6, .. })));
    }

    #[test]
    fn a_frame_cut_off_by_the_end_of_the_body_is_not_delivered() {
        // no blank line: the frame was never completed
        let got = frames("event: record\nid: 1\ndata: {\"seq\":1}\n");
        assert!(got.is_empty());
    }

    #[test]
    fn an_id_must_be_a_plain_slug() {
        let c = Client::new("http://127.0.0.1:1").unwrap();
        for bad in ["", "a/b", "a b", "../x", "a?b"] {
            assert!(
                matches!(c.events(bad, None), Err(Error::BadId(_))),
                "{bad:?}"
            );
        }
    }
}
