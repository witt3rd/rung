//! A typed, blocking client of the rung-gateway's doors. No GUI, no async
//! runtime: a caller that must not block (a window) runs these on a worker
//! thread.
//!
//! It holds no logic of the host: the state word and the "doing" line are the
//! host's own, read from the summary the gateway lists per instance. The one
//! credential it may hold is a read-only token, sent as a bearer and never
//! printed (`Debug` of [`Client`] shows only whether one is set, and no error
//! text carries it).
//!
//! ```no_run
//! let gw = rung_client::Client::new("http://127.0.0.1:8787")?;
//! assert!(gw.health()?.ok);
//! for i in gw.instances()?.instances {
//!     println!("{} {} {}", i.display_name(), i.state_word(), i.doing());
//! }
//! # Ok::<(), rung_client::Error>(())
//! ```

use std::fmt;
use std::time::Duration;

use serde::Deserialize;

const CONNECT: Duration = Duration::from_secs(5);
const TOTAL: Duration = Duration::from_secs(20);

/// Why a door could not be read. Never carries the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The base address is not an `http://` or `https://` address.
    BadAddress(String),
    /// No answer: refused, unreachable, timed out, or not HTTP.
    Transport(String),
    /// The gateway answered with an error status (`message` is its own words
    /// when it sent a JSON `{"error": {"message": ..}}`).
    Status { code: u16, message: String },
    /// The answer is not the shape this client reads.
    Decode(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadAddress(a) => write!(f, "not an http(s) address: {a}"),
            Error::Transport(e) => write!(f, "the gateway did not answer: {e}"),
            Error::Status { code, message } if message.is_empty() => {
                write!(f, "the gateway answered {code}")
            }
            Error::Status { code, message } => write!(f, "the gateway answered {code}: {message}"),
            Error::Decode(e) => write!(f, "the gateway's answer is not readable: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// `GET /api/health`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Health {
    pub ok: bool,
}

/// `GET /api/instances`: one row per instance, ordered by the gateway.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Instances {
    #[serde(default)]
    pub instances: Vec<Instance>,
    /// Set when the registry of native hosts could not be read.
    #[serde(default)]
    pub registry_error: Option<String>,
}

/// One row of the list. Fields this client does not read are ignored, and
/// every field the two row kinds (registry and configured) do not share is
/// optional.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Instance {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// The registry's word: `running`, `stopped`, `down`, `unreadable`
    /// (registry rows only).
    #[serde(default)]
    pub word: Option<String>,
    /// `registry` or `config`.
    #[serde(default)]
    pub source: Option<String>,
    /// False when the gateway could not read the instance.
    #[serde(default = "yes")]
    pub reachable: bool,
    /// The host's own fold of its record; absent when it cannot be read.
    #[serde(default)]
    pub summary: Option<Summary>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub problem: Option<String>,
}

fn yes() -> bool {
    true
}

/// The part of the host's summary the list screen shows.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Summary {
    /// The host's state word: `Working`, `Free time`, `Stuck`, `Down`, ...
    #[serde(default)]
    pub state: Option<String>,
    /// One plain sentence of what it is doing.
    #[serde(default)]
    pub doing: Option<String>,
    #[serde(default)]
    pub needs_you: bool,
}

impl Instance {
    /// The name to show: its own, else its id.
    pub fn display_name(&self) -> &str {
        self.name
            .as_deref()
            .filter(|n| !n.is_empty())
            .unwrap_or(&self.id)
    }

    /// The state word to show: the host's own, else the registry's word, else
    /// `unreachable` / `unknown`.
    pub fn state_word(&self) -> &str {
        if let Some(s) = self.summary.as_ref().and_then(|s| s.state.as_deref()) {
            return s;
        }
        if let Some(w) = self.word.as_deref() {
            return w;
        }
        if self.reachable {
            "unknown"
        } else {
            "unreachable"
        }
    }

    /// What it is doing, in the host's words; when there is no summary, the
    /// reason there is none (or an empty line).
    pub fn doing(&self) -> &str {
        self.summary
            .as_ref()
            .and_then(|s| s.doing.as_deref())
            .or(self.error.as_deref())
            .or(self.problem.as_deref())
            .unwrap_or("")
    }

    /// The host asks for the owner (`Stuck` or `Down`).
    pub fn needs_you(&self) -> bool {
        self.summary.as_ref().is_some_and(|s| s.needs_you)
    }

    /// The gateway cannot read this instance: it did not answer, or its entry
    /// is unreadable.
    pub fn unreachable(&self) -> bool {
        !self.reachable || self.word.as_deref() == Some("unreadable")
    }

    /// Chief, steward or neither, from the id.
    pub fn role(&self) -> Role {
        Role::of_id(&self.id)
    }
}

/// A member's place in a fleet, read from its instance id (the fleet props,
/// `a-member-is-named-by-its-role`): the first segment of the id, then `-`.
/// There is no registry field for it, and the id says nothing of which chief a
/// steward serves, so neither does this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Chief,
    Steward,
    /// An id that does not begin with a role: not a member of a fleet.
    Other,
}

impl Role {
    /// The role an instance id names.
    pub fn of_id(id: &str) -> Role {
        match id.split_once('-') {
            Some(("chief", rest)) if !rest.is_empty() => Role::Chief,
            Some(("steward", rest)) if !rest.is_empty() => Role::Steward,
            _ => Role::Other,
        }
    }

    /// The plural, as a heading.
    pub fn plural(self) -> &'static str {
        match self {
            Role::Chief => "Chiefs",
            Role::Steward => "Stewards",
            Role::Other => "Other instances",
        }
    }
}

/// A gateway to ask. Cheap to clone and share: it holds an address, an
/// optional read-only token and a connection pool.
#[derive(Clone)]
pub struct Client {
    base: String,
    token: Option<String>,
    agent: ureq::Agent,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base)
            .field("token", &self.token.as_ref().map(|_| "set"))
            .finish()
    }
}

impl Client {
    /// A client of the gateway at `base` (`http://host:port`, `https://name`),
    /// with no token: the owner role, where the gateway allows it.
    pub fn new(base: &str) -> Result<Client, Error> {
        let trimmed = base.trim().trim_end_matches('/');
        let rest = trimmed
            .strip_prefix("http://")
            .or_else(|| trimmed.strip_prefix("https://"))
            .ok_or_else(|| Error::BadAddress(base.to_string()))?;
        if rest.is_empty() || rest.contains(char::is_whitespace) {
            return Err(Error::BadAddress(base.to_string()));
        }
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(CONNECT)
            .timeout(TOTAL)
            .build();
        Ok(Client {
            base: trimmed.to_string(),
            token: None,
            agent,
        })
    }

    /// Send `token` as a bearer on every request (the read-only role).
    pub fn with_token(mut self, token: impl Into<String>) -> Client {
        self.token = Some(token.into()).filter(|t| !t.is_empty());
        self
    }

    /// The address this client asks (no token).
    pub fn base(&self) -> &str {
        &self.base
    }

    /// `GET /api/health`.
    pub fn health(&self) -> Result<Health, Error> {
        self.get("/api/health")
    }

    /// `GET /api/instances`.
    pub fn instances(&self) -> Result<Instances, Error> {
        self.get("/api/instances")
    }

    fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, Error> {
        let mut req = self.agent.get(&format!("{}{path}", self.base));
        if let Some(t) = &self.token {
            req = req.set("authorization", &format!("Bearer {t}"));
        }
        match req.call() {
            Ok(resp) => {
                let body = resp
                    .into_string()
                    .map_err(|e| Error::Transport(self.scrub(e.to_string())))?;
                serde_json::from_str(&body).map_err(|e| Error::Decode(e.to_string()))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                Err(Error::Status {
                    code,
                    message: message_of(&body),
                })
            }
            Err(ureq::Error::Transport(t)) => Err(Error::Transport(self.scrub(t.to_string()))),
        }
    }

    /// No error text carries the token, whatever the transport wrote.
    fn scrub(&self, text: String) -> String {
        match &self.token {
            Some(t) => text.replace(t.as_str(), "[token]"),
            None => text,
        }
    }
}

/// The gateway's own words from `{"error": {"message": ".."}}`, else nothing.
fn message_of(body: &str) -> String {
    #[derive(Deserialize)]
    struct Wrap {
        error: Inner,
    }
    #[derive(Deserialize)]
    struct Inner {
        #[serde(default)]
        message: String,
    }
    serde_json::from_str::<Wrap>(body)
        .map(|w| w.error.message)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_must_be_http() {
        assert!(Client::new("http://127.0.0.1:1").is_ok());
        assert!(Client::new("https://gw.example/").is_ok());
        assert_eq!(
            Client::new("https://gw.example/").unwrap().base(),
            "https://gw.example"
        );
        for bad in ["", "gw:80", "ftp://x", "http://", "http://a b"] {
            assert!(
                matches!(Client::new(bad), Err(Error::BadAddress(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_role_is_the_first_segment_of_the_id() {
        for (id, role) in [
            ("chief-main", Role::Chief),
            ("steward-alpha", Role::Steward),
            ("steward-a-b", Role::Steward),
            ("chief", Role::Other),
            ("chief-", Role::Other),
            ("chiefs-x", Role::Other),
            ("my-chief-x", Role::Other),
            ("crew-1", Role::Other),
            ("", Role::Other),
        ] {
            assert_eq!(Role::of_id(id), role, "{id:?}");
        }
    }

    #[test]
    fn unreachable_is_the_gateway_not_reading_it() {
        let row = |j: &str| serde_json::from_str::<Instance>(j).unwrap();
        assert!(row(r#"{"id":"a","reachable":false}"#).unreachable());
        assert!(row(r#"{"id":"a","word":"unreadable"}"#).unreachable());
        assert!(!row(r#"{"id":"a","word":"down"}"#).unreachable());
        assert!(!row(r#"{"id":"a"}"#).unreachable());
    }

    #[test]
    fn debug_never_shows_the_token() {
        let c = Client::new("http://127.0.0.1:1")
            .unwrap()
            .with_token("s3cret-t0ken");
        let shown = format!("{c:?}");
        assert!(!shown.contains("s3cret"), "{shown}");
        assert!(shown.contains("set"));
    }

    #[test]
    fn rows_read_as_the_list_screen_needs() {
        let reg: Instance = serde_json::from_str(
            r#"{"id":"chief-main","name":"chief-main","word":"running","source":"registry",
                "reachable":true,"extra":1,
                "summary":{"state":"Free time","doing":"Free time","needs_you":false}}"#,
        )
        .unwrap();
        assert_eq!(reg.state_word(), "Free time");
        assert_eq!(reg.doing(), "Free time");
        assert!(!reg.needs_you());

        let down: Instance = serde_json::from_str(
            r#"{"id":"steward-x","word":"down","reachable":true,"summary":null,"error":"nope"}"#,
        )
        .unwrap();
        assert_eq!(down.display_name(), "steward-x");
        assert_eq!(down.state_word(), "down");
        assert_eq!(down.doing(), "nope");

        let cfg: Instance = serde_json::from_str(
            r#"{"id":"a","name":"A","source":"config","reachable":false,"summary":null,"error":"x"}"#,
        )
        .unwrap();
        assert_eq!(cfg.state_word(), "unreachable");
    }
}
