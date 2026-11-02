//! What was sent to the model on one call of one turn (host API, S4:
//! `GET /api/i/{id}/v1/turns/{n}/context?call=k`): the request as bands, in
//! the order the model sees them, each with the host's estimate of its size
//! and whether the provider had it cached. The answer is the host's own; this
//! only reads it. Texts come back whole and already redacted.

use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::{Client, Error, message_of};

/// The provider's own counts for the call (the truth for the hit rate).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct ProviderCounts {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub expected_cached_tokens: Option<u64>,
}

/// A cache breakpoint the engine sets.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Boundary {
    /// The band it comes after (`system`, `epoch`).
    #[serde(default)]
    pub after: String,
    #[serde(default)]
    pub est_tokens: u64,
    /// At the provider's scale; absent without a provider count.
    #[serde(default)]
    pub tokens: Option<u64>,
}

/// A recorded cache event for this call (`cache.break` or `cache.cold`).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct CacheEvent {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub cause: Option<String>,
}

/// One tool definition.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct ToolDef {
    #[serde(default)]
    pub name: String,
}

/// One band of the request. `band` is `tools`, `system`, `epoch`, `history`,
/// `message`, `recall` or `this_turn`; the other fields are the ones that band
/// has.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Band {
    #[serde(default)]
    pub band: String,
    #[serde(default)]
    pub est_tokens: u64,
    /// `cached`, `partly`, `new` or `unknown`.
    #[serde(default)]
    pub cache: Option<String>,
    /// The whole text (`system`, `epoch`, `message`, `recall`).
    #[serde(default)]
    pub text: Option<String>,
    /// `tools`: how many, and which.
    #[serde(default)]
    pub count: Option<u64>,
    #[serde(default)]
    pub items: Vec<ToolDef>,
    /// `history`: the earlier turns of the epoch it holds.
    #[serde(default)]
    pub turns: Option<u64>,
    /// `this_turn`: the model calls so far, and how many messages the turn had.
    #[serde(default)]
    pub calls_so_far: Option<u64>,
    #[serde(default, deserialize_with = "count_of")]
    pub messages: u64,
    /// `recall`: false when its text does not match its recorded hash.
    #[serde(default)]
    pub intact: Option<bool>,
}

/// The number of elements of an array, without keeping them (a turn's messages
/// can be megabytes and the page shows only how many).
fn count_of<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let v: Vec<IgnoredAny> = Vec::deserialize(d)?;
    Ok(v.len() as u64)
}

/// The answer.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct ContextView {
    pub turn: u64,
    /// This call of the turn, from 1, and how many the record holds.
    #[serde(default)]
    pub call: u64,
    #[serde(default)]
    pub calls: u64,
    /// The turn has not ended: this is call 1 as assembled.
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub provider: Option<ProviderCounts>,
    #[serde(default)]
    pub cache_event: Option<CacheEvent>,
    /// Every layer of this call is in the record.
    #[serde(default)]
    pub recorded: bool,
    /// In words, what the record cannot give back whole.
    #[serde(default)]
    pub problems: Vec<String>,
    /// For a turn that predates context recording.
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub estimate_tokens: u64,
    #[serde(default)]
    pub boundaries: Vec<Boundary>,
    #[serde(default)]
    pub bands: Vec<Band>,
}

impl Client {
    /// What the model was sent on `call` (from 1; the first when `None`) of
    /// turn `turn` of instance `id`. A turn that has not been logged yet is
    /// the host's 409 in words; an unknown turn or call a 404.
    pub fn context(&self, id: &str, turn: u64, call: Option<u64>) -> Result<ContextView, Error> {
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            return Err(Error::BadId(id.to_string()));
        }
        let q = call.map(|c| format!("?call={c}")).unwrap_or_default();
        let mut req = self.stream_agent.get(&format!(
            "{}/api/i/{id}/v1/turns/{turn}/context{q}",
            self.base
        ));
        if let Some(t) = &self.token {
            req = req.set("authorization", &format!("Bearer {t}"));
        }
        match req.call() {
            Ok(resp) => serde_json::from_reader(resp.into_reader())
                .map_err(|e| Error::Decode(format!("the context: {e}"))),
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
    fn the_answer_reads_as_the_door_sends_it() {
        let v: ContextView = serde_json::from_str(
            r#"{"turn":3,"call":1,"calls":2,"epoch":1,"turn_kind":"free",
            "model":{"requested":"m","served":"m","provider":"p"},
            "provider":{"prompt_tokens":2662,"cached_tokens":2469,"cache_write_tokens":0,
                        "completion_tokens":40,"expected_cached_tokens":2469},
            "cache_event":null,"rewritten":false,"live":false,"recorded":true,"note":null,
            "missing":[],"problems":[],"estimate_tokens":2665,"scale":0.99,
            "boundaries":[{"after":"system","why":"w","est_tokens":2055,"tokens":2053}],
            "bands":[{"count":2,"band":"tools","est_tokens":1620,"cache":"cached",
                      "items":[{"name":"read","text":"{}"},{"name":"write","text":"{}"}]},
                     {"band":"system","est_tokens":435,"cache":"cached","text":"You are"},
                     {"band":"history","cache":"partly","est_tokens":362,"turns":2,
                      "rows":[{"turn":1,"messages":[1,2,3]},{"turn":2}]},
                     {"calls_so_far":1,"band":"this_turn","est_tokens":9,"cache":"new",
                      "messages":[{"role":"assistant"},{"role":"tool"}]}],
            "damaged":null,"redaction":{"env_unset":[]}}"#,
        )
        .unwrap();
        assert_eq!((v.turn, v.call, v.calls, v.live), (3, 1, 2, false));
        assert_eq!(v.provider.as_ref().unwrap().cached_tokens, 2469);
        assert_eq!(v.boundaries[0].tokens, Some(2053));
        let names: Vec<_> = v.bands.iter().map(|b| b.band.as_str()).collect();
        assert_eq!(names, ["tools", "system", "history", "this_turn"]);
        assert_eq!(v.bands[0].items[1].name, "write");
        assert_eq!(v.bands[1].text.as_deref(), Some("You are"));
        assert_eq!(v.bands[2].turns, Some(2));
        assert_eq!(v.bands[3].messages, 2, "messages are counted, not kept");
    }
}
