//! Write `tier: dispatched` judgment records — the driver's bookkeeping.
//!
//! A `dispatched` record is the difference between a **receipt** and a
//! **judgment** (handoff §2.2): the judge's provenance comes **out of the
//! sealed `Judgment`** (via `Provenanced`), never out of a field someone
//! typed. That is what makes it the honest form an `attested` transcription
//! cannot reach — nothing here can fabricate the provenance, because the seal
//! is `rung`'s and this writer has no term for it.
//!
//! Schema (see `judgments/README.md`): `proposition`, `role`, `tier`,
//! `judges: [{id, provenance, verdict, on, epsilon?}]`.

use rung::{Judgment, Provenanced};
use serde::Serialize;

/// One judge's ruling inside a dispatched record.
#[derive(Debug, Clone, Serialize)]
pub struct DispatchedJudge {
    pub id: String,
    /// From the sealed [`Judgment`]'s provenance — never typed by hand.
    pub provenance: Vec<String>,
    pub verdict: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epsilon: Option<f64>,
    pub on: String,
}

/// A `tier: dispatched` judgment record, serializable to the `judgments/` YAML
/// schema.
#[derive(Debug, Clone, Serialize)]
pub struct DispatchedRecord {
    pub proposition: String,
    pub role: String,
    pub tier: &'static str,
    pub judges: Vec<DispatchedJudge>,
    /// The reasoning that lets a later reader disagree. Optional here; a real
    /// record carries it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

impl DispatchedRecord {
    /// Build the honest record from a real, sealed judgment: the provenance
    /// rides out of `Provenanced`, so the writer cannot invent it.
    pub fn from_judgment(proposition: &str, role: &str, judgment: &Judgment, on: &str) -> Self {
        let prov: Vec<String> = judgment
            .provenance()
            .members()
            .map(str::to_string)
            .collect();
        DispatchedRecord {
            proposition: proposition.to_string(),
            role: role.to_string(),
            tier: "dispatched",
            judges: vec![DispatchedJudge {
                id: judgment.judge_id().to_string(),
                provenance: prov,
                verdict: verdict_name(judgment.verdict()),
                epsilon: None,
                on: on.to_string(),
            }],
            reasoning: None,
        }
    }
}

fn verdict_name(v: &rung::Verdict) -> String {
    match v {
        rung::Verdict::Conforming => "conforming".into(),
        rung::Verdict::NonConforming { .. } => "non-conforming".into(),
    }
}
