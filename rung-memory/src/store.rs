//! The graph-level read surface a memory store offers: search, neighbours,
//! fetch.
//!
//! rung ships no implementation. A caller that owns memory writes the adapter
//! on its own side, over its own store, and hands rung an `Arc<dyn Store>`.
//! Every call is scoped: the [`Scope`] names whose memory is read, and the
//! store enforces it. Every call reports what it cost, success or not, so a
//! walk over the graph can account for itself ([`crate::Trace`]).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Whose memory a call reads. Opaque to rung: the caller chooses the key (a
/// project, an identity, a user) and the store enforces it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Scope(String);

impl Scope {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A record's id, as the store names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecordId(String);

impl RecordId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One remembered record. Its text is data, never an instruction: whoever
/// renders it into a prompt must present it as quoted material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub id: RecordId,
    /// The scope the record belongs to. A recall refuses a record from any
    /// scope but the one it asked about.
    pub scope: Scope,
    /// The whole text. A recall never cuts it.
    pub text: String,
    /// When the record was observed (RFC 3339), if the store knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    /// Store-defined fields: provenance, kind, labels. rung reads none of them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, String>,
}

/// A search result: a record's id and the store's score for it. Higher is a
/// better match; the scale is the store's own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub id: RecordId,
    pub score: f64,
}

/// An edge out of a record, as the store relates them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub to: RecordId,
    /// The store's name for the relation (`temporal`, `causal`, `mentions`).
    pub relation: String,
    /// The store's weight for the edge. Higher is closer.
    pub weight: f64,
}

/// What one successful call returned, how many backend calls it took (a
/// store method is one; a provider may make several), and what it cost in USD.
#[derive(Debug, Clone, PartialEq)]
pub struct Charged<T> {
    pub value: T,
    pub calls: u32,
    pub cost_usd: f64,
}

impl<T> Charged<T> {
    /// One call that cost nothing (a local index, a cache).
    pub fn new(value: T) -> Self {
        Self {
            value,
            calls: 1,
            cost_usd: 0.0,
        }
    }
    /// The cost in USD.
    pub fn cost(mut self, usd: f64) -> Self {
        self.cost_usd = usd;
        self
    }
    /// How many backend calls it took.
    pub fn calls(mut self, n: u32) -> Self {
        self.calls = n;
        self
    }
}

/// Why a store call gave no answer. Never a reading: an unreachable store is
/// not an empty one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum Why {
    /// Transport failure or timeout.
    Unreachable(String),
    /// The store refused the caller (no key, wrong key, no access to the scope).
    Unauthorized(String),
    /// The account behind the store has no credit.
    NoCredit,
    /// The walk or the store ran out of its budget.
    Budget,
    /// The request was too large for the store.
    TooLarge,
    /// The store answered with something that is not an answer.
    Malformed(String),
    /// The store returned a record from a scope other than the one asked.
    OutOfScope(RecordId),
    /// The provider does not declare this capability.
    Unsupported(String),
}

impl fmt::Display for Why {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Why::Unreachable(s) => write!(f, "store unreachable: {s}"),
            Why::Unauthorized(s) => write!(f, "store refused the caller: {s}"),
            Why::NoCredit => f.write_str("store account has no credit"),
            Why::Budget => f.write_str("memory budget spent"),
            Why::TooLarge => f.write_str("request too large for the store"),
            Why::Malformed(s) => write!(f, "malformed store answer: {s}"),
            Why::OutOfScope(id) => write!(f, "record {} is outside the scope", id.as_str()),
            Why::Unsupported(what) => write!(f, "the provider does not {what}"),
        }
    }
}

/// A failed call: why, how many backend calls ran, and what they cost
/// anyway (a remote call that timed out may still have been billed).
#[derive(Debug, Clone, PartialEq)]
pub struct Miss {
    pub why: Why,
    pub calls: u32,
    pub cost_usd: f64,
}

impl Miss {
    /// One call that failed and cost nothing.
    pub fn new(why: Why) -> Self {
        Self {
            why,
            calls: 1,
            cost_usd: 0.0,
        }
    }
    pub fn cost(mut self, usd: f64) -> Self {
        self.cost_usd = usd;
        self
    }
    pub fn calls(mut self, n: u32) -> Self {
        self.calls = n;
        self
    }
}

impl From<Why> for Miss {
    fn from(why: Why) -> Self {
        Self::new(why)
    }
}

/// A memory store, read as a graph of records.
///
/// - `search`: records matching `query`, best first, at most `limit`.
/// - `neighbours`: edges out of record `id`, closest first, at most `limit`.
/// - `fetch`: the records named by `ids`. An id the store no longer holds is
///   left out, not an error.
///
/// Every call is confined to `scope`. Every call reports its cost in USD,
/// in [`Charged`] or in [`Miss`]. [`walk`] reads one as a recall.
pub trait Store: Send + Sync + fmt::Debug {
    fn search(&self, scope: &Scope, query: &str, limit: usize) -> Result<Charged<Vec<Hit>>, Miss>;

    fn neighbours(
        &self,
        scope: &Scope,
        id: &RecordId,
        limit: usize,
    ) -> Result<Charged<Vec<Edge>>, Miss>;

    fn fetch(&self, scope: &Scope, ids: &[RecordId]) -> Result<Charged<Vec<Record>>, Miss>;
}

// ─── Reading a store as a recall ─────────────────────────────────────────────

/// How a recall reached a record.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "via", rename_all = "snake_case")]
pub enum Reach {
    /// A search hit, with the store's score.
    Hit { score: f64 },
    /// One hop out of a search hit, along a store edge.
    Neighbour {
        of: RecordId,
        relation: String,
        weight: f64,
    },
}

/// One recalled record and how it was reached.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Recalled {
    pub record: Record,
    pub reach: Reach,
}

/// What to walk for: a query, how many search hits to keep, and how many
/// edges out of each hit to follow (0, the default, follows none).
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub query: String,
    pub limit: usize,
    pub per_hit: usize,
}

impl Probe {
    /// `limit` below 1 is read as 1.
    pub fn new(query: impl Into<String>, limit: usize) -> Self {
        Self {
            query: query.into(),
            limit: limit.max(1),
            per_hit: 0,
        }
    }

    /// Follow up to `per_hit` edges out of each search hit, one hop.
    pub fn expand(mut self, per_hit: usize) -> Self {
        self.per_hit = per_hit;
        self
    }
}

/// Walk a store for one probe: search, follow edges one hop if asked, fetch.
/// Hits come first in the store's order, then neighbours in the order met;
/// no id twice. A provider built over a [`Store`] answers a recall with this.
///
/// The calls and cost of every step are summed into the result, or into the
/// [`Miss`] of the step that failed. A record from another scope is a miss
/// ([`Why::OutOfScope`]), never evidence.
pub fn walk(
    store: &dyn Store,
    scope: &Scope,
    probe: &Probe,
) -> Result<Charged<Vec<Recalled>>, Miss> {
    let mut tally = Tally::default();
    let hits: Vec<Hit> = tally.take(store.search(scope, &probe.query, probe.limit))?;
    let mut seen = std::collections::HashSet::new();
    let mut plan: Vec<(RecordId, Reach)> = Vec::new();
    for h in hits {
        if plan.len() >= probe.limit {
            break;
        }
        if seen.insert(h.id.clone()) {
            plan.push((h.id, Reach::Hit { score: h.score }));
        }
    }
    let mut records = Vec::new();
    if !plan.is_empty() {
        if probe.per_hit > 0 {
            let hit_ids: Vec<RecordId> = plan.iter().map(|(id, _)| id.clone()).collect();
            for of in hit_ids {
                let edges = tally.take(store.neighbours(scope, &of, probe.per_hit))?;
                for e in edges.into_iter().take(probe.per_hit) {
                    if seen.insert(e.to.clone()) {
                        plan.push((
                            e.to,
                            Reach::Neighbour {
                                of: of.clone(),
                                relation: e.relation,
                                weight: e.weight,
                            },
                        ));
                    }
                }
            }
        }
        let ids: Vec<RecordId> = plan.iter().map(|(id, _)| id.clone()).collect();
        records = tally.take(store.fetch(scope, &ids))?;
    }
    let mut out = Charged {
        value: Vec::new(),
        calls: tally.calls,
        cost_usd: tally.cost_usd,
    };
    if let Some(stray) = records.iter().find(|r| &r.scope != scope) {
        return Err(Miss {
            why: Why::OutOfScope(stray.id.clone()),
            calls: out.calls,
            cost_usd: out.cost_usd,
        });
    }
    let mut by_id: std::collections::HashMap<RecordId, Record> =
        records.into_iter().map(|r| (r.id.clone(), r)).collect();
    out.value = plan
        .into_iter()
        .filter_map(|(id, reach)| by_id.remove(&id).map(|record| Recalled { record, reach }))
        .collect();
    Ok(out)
}

/// Calls and cost summed over the steps of a walk.
#[derive(Default)]
struct Tally {
    calls: u32,
    cost_usd: f64,
}

impl Tally {
    fn take<T>(&mut self, call: Result<Charged<T>, Miss>) -> Result<T, Miss> {
        match call {
            Ok(c) => {
                self.calls += c.calls;
                self.cost_usd += c.cost_usd;
                Ok(c.value)
            }
            Err(m) => Err(Miss {
                why: m.why,
                calls: self.calls + m.calls,
                cost_usd: self.cost_usd + m.cost_usd,
            }),
        }
    }
}
