//! The turns door (host API, S8): the record folded into turns on the host's
//! side. A turn is what the agent was asked, what it did, what it said and what
//! it cost; the host folds it once, so this only reads it. Texts are whole and
//! already redacted.

use serde::Deserialize;
use serde_json::Value;

use crate::{Client, Error};

/// What a turn cost in model calls.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub calls: u64,
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub cost_usd: f64,
}

/// One turn, as a row of the list.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct TurnRow {
    pub turn: u64,
    /// `free`, `committed`, `responding`, ...
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub ended_at: Option<i64>,
    #[serde(default)]
    pub elapsed_ms: Option<u64>,
    /// `completed`, `bounded`, `failed`, `running` or `interrupted`.
    #[serde(default)]
    pub status: String,
    /// The record's own failure object, when the turn failed.
    #[serde(default)]
    pub failure: Option<Value>,
    /// The reply, whole.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub llm: Usage,
    #[serde(default)]
    pub tool_calls: u64,
    /// How many items the turn was asked.
    #[serde(default)]
    pub asked: u64,
    #[serde(default)]
    pub first_seq: u64,
    #[serde(default)]
    pub last_seq: u64,
}

/// `GET .../v1/turns`: a page, newest first.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct TurnList {
    #[serde(default)]
    pub turns: Vec<TurnRow>,
    /// How many turns the record holds.
    #[serde(default)]
    pub total: u64,
    /// Pass as `before` for the next older page; none on the last.
    #[serde(default)]
    pub next: Option<u64>,
}

/// One item a turn was asked.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Asked {
    #[serde(default)]
    pub id: String,
    /// `owner`, or the item's kind.
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub sent_at: Option<i64>,
    #[serde(default)]
    pub seen_at: Option<i64>,
    /// The turn that was running when it arrived.
    #[serde(default)]
    pub during: Option<u64>,
}

/// One tool call of a turn. `input` and `result` are whole; absent when the
/// record kept only that the call happened.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Call {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub ok: Option<bool>,
    /// Why the call was refused, when it was.
    #[serde(default)]
    pub refused: Option<String>,
    #[serde(default)]
    pub input: Option<Value>,
    #[serde(default)]
    pub result: Option<String>,
}

/// `GET .../v1/turns/{n}`: the turn whole.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct TurnDetail {
    #[serde(flatten)]
    pub row: TurnRow,
    #[serde(default)]
    pub asked_items: Vec<Asked>,
    #[serde(default)]
    pub calls: Vec<Call>,
}

impl Client {
    /// The newest `limit` turns of instance `id` (numbered below `before`
    /// when given).
    pub fn turns(&self, id: &str, limit: u64, before: Option<u64>) -> Result<TurnList, Error> {
        self.check_instance_id(id)?;
        let b = before.map(|n| format!("&before={n}")).unwrap_or_default();
        self.get_streaming(&format!("/api/i/{id}/v1/turns?limit={limit}{b}"))
    }

    /// Turn `n` of instance `id`, whole.
    pub fn turn(&self, id: &str, n: u64) -> Result<TurnDetail, Error> {
        self.check_instance_id(id)?;
        self.get_streaming(&format!("/api/i/{id}/v1/turns/{n}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_and_a_turn_read_as_the_door_sends_them() {
        let l: TurnList = serde_json::from_str(
            r#"{"turns":[{"turn":4,"kind":"free","mode":"free","project":null,"model":"m","epoch":1,
                "started_at":1,"ended_at":2,"elapsed_ms":930,"status":"completed","failure":null,
                "text":"turn 4 done","llm":{"calls":2,"prompt_tokens":4515,"cached_tokens":2216,
                "completion_tokens":80,"cost_usd":0.0},"tool_calls":1,"asked":0,"first_seq":33,"last_seq":40}],
              "total":17,"limit":50,"order":"desc","next":3,"damaged":null,"redaction":{"env_unset":[]}}"#,
        )
        .unwrap();
        assert_eq!((l.total, l.next, l.turns[0].turn), (17, Some(3), 4));
        assert_eq!(l.turns[0].llm.cached_tokens, 2216);
        let d: TurnDetail = serde_json::from_str(
            r#"{"turn":4,"status":"failed","failure":{"class":"provider"},"text":"",
                "asked_items":[{"id":"q-1","from":"owner","intent":"steer","text":"hi","sent_at":1,"seen_at":2,"during":3}],
                "calls":[{"name":"read","group":null,"ok":true,"refused":null,"input":{"path":"a"},"result":"x"},
                         {"name":"write","ok":false,"refused":"not_enabled","result":"off"}]}"#,
        )
        .unwrap();
        assert_eq!(d.row.status, "failed");
        assert_eq!(d.asked_items[0].during, Some(3));
        assert_eq!(d.calls[0].input.as_ref().unwrap()["path"], "a");
        assert_eq!(d.calls[1].refused.as_deref(), Some("not_enabled"));
    }
}
