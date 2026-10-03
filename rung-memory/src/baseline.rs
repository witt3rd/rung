//! `baseline`: the provider rung ships. No model, no network, no cost.
//!
//! - **Store**: one append-only JSON-lines file per scope under the
//!   provider's directory (`<dir>/<scope hash>.jsonl`), one [`Record`] per
//!   line. Nothing is rewritten or deleted. The directory is created by the
//!   first retain; a recall never creates it.
//! - **Recall**: BM25 over the scope's records, with a small recency term so
//!   ties go to the newer record. It walks its own [`Store`] (search, then
//!   fetch). Neighbours are the records kept just before and after.
//! - **Retain**: a finished turn is kept as one record (`User:` / `Assistant:`
//!   lines), a note as its text. An empty text, a text over the recall budget
//!   (it could never be shown whole), or an exact duplicate is declined.
//! - **Tools**: `memory_search` and `memory_retain` ([`crate::tools`]).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rung_std::tools::{ToolRoster, Toolset};

use crate::provider::{
    Body, Budget, Capability, Cue, Kept, MemoryProvider, Observation, ProviderSettings, ToolContext,
};
use crate::store::{
    Charged, Edge, Hit, Miss, Probe, Recalled, Record, RecordId, Scope, Store, Why, walk,
};

/// The name it is chosen by.
pub const NAME: &str = "baseline";

const K1: f64 = 1.2;
const B: f64 = 0.75;
/// Added per position in the file, scaled to (0, RECENCY]: a tie-break only.
const RECENCY: f64 = 0.01;

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "can", "do", "does", "for", "from",
    "has", "have", "how", "i", "if", "in", "is", "it", "its", "me", "my", "no", "not", "of", "ok",
    "okay", "on", "or", "our", "please", "so", "thanks", "thank", "that", "the", "their", "them",
    "then", "there", "these", "this", "to", "us", "was", "we", "were", "what", "when", "where",
    "which", "who", "why", "will", "with", "you", "your", "yes", "continue", "go", "ahead",
];

/// The `baseline` factory for [`crate::Registry`].
pub fn factory(s: &ProviderSettings) -> Result<Arc<dyn MemoryProvider>, String> {
    Ok(Arc::new(Baseline::new(&s.dir)))
}

/// See the module docs.
#[derive(Debug, Clone)]
pub struct Baseline {
    dir: PathBuf,
    budget: Budget,
}

impl Baseline {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
            budget: Budget::default(),
        }
    }

    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    /// The file that holds `scope`'s records.
    pub fn file(&self, scope: &Scope) -> PathBuf {
        self.dir
            .join(format!("{:016x}.jsonl", fnv1a(scope.as_str().as_bytes())))
    }

    /// The scope's records, oldest first. A missing file is no records. A
    /// line that does not parse (a torn append) is skipped.
    fn load(&self, scope: &Scope) -> Result<Vec<Record>, Miss> {
        let path = self.file(scope);
        let body = match fs::read_to_string(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(Miss::new(Why::Unreachable(format!(
                    "{}: {e}",
                    path.display()
                ))));
            }
        };
        Ok(body
            .lines()
            .filter_map(|l| serde_json::from_str::<Record>(l).ok())
            .filter(|r| &r.scope == scope)
            .collect())
    }
}

/// Lowercased alphanumeric words of two or more chars, stopwords dropped.
fn terms(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.chars().count() >= 2 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Collapse whitespace, so a duplicate differing only in spacing is one.
fn normal(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Now, as RFC 3339 UTC to the second.
fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

impl Store for Baseline {
    fn search(&self, scope: &Scope, query: &str, limit: usize) -> Result<Charged<Vec<Hit>>, Miss> {
        let records = self.load(scope)?;
        let q: HashSet<String> = terms(query).into_iter().collect();
        if q.is_empty() || records.is_empty() {
            return Ok(Charged::new(Vec::new()));
        }
        let docs: Vec<Vec<String>> = records.iter().map(|r| terms(&r.text)).collect();
        let n = docs.len() as f64;
        let avg = docs.iter().map(Vec::len).sum::<usize>() as f64 / n;
        let mut df: HashMap<&str, usize> = HashMap::new();
        for d in &docs {
            for t in d.iter().collect::<HashSet<_>>() {
                *df.entry(t.as_str()).or_default() += 1;
            }
        }
        let mut hits: Vec<(f64, usize)> = Vec::new();
        for (i, d) in docs.iter().enumerate() {
            let len = d.len() as f64;
            let mut score = 0.0;
            for t in &q {
                let tf = d.iter().filter(|w| *w == t).count() as f64;
                if tf == 0.0 {
                    continue;
                }
                let df = *df.get(t.as_str()).unwrap_or(&0) as f64;
                let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                score += idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * len / avg.max(1.0)));
            }
            if score > 0.0 {
                hits.push((score + RECENCY * (i + 1) as f64 / n, i));
            }
        }
        hits.sort_by(|a, b| b.0.total_cmp(&a.0));
        Ok(Charged::new(
            hits.into_iter()
                .take(limit)
                .map(|(score, i)| Hit {
                    id: records[i].id.clone(),
                    score,
                })
                .collect(),
        ))
    }

    fn neighbours(
        &self,
        scope: &Scope,
        id: &RecordId,
        limit: usize,
    ) -> Result<Charged<Vec<Edge>>, Miss> {
        let records = self.load(scope)?;
        let mut edges = Vec::new();
        if let Some(i) = records.iter().position(|r| &r.id == id) {
            if i > 0 {
                edges.push(Edge {
                    to: records[i - 1].id.clone(),
                    relation: "before".into(),
                    weight: 1.0,
                });
            }
            if let Some(next) = records.get(i + 1) {
                edges.push(Edge {
                    to: next.id.clone(),
                    relation: "after".into(),
                    weight: 1.0,
                });
            }
        }
        edges.truncate(limit);
        Ok(Charged::new(edges))
    }

    fn fetch(&self, scope: &Scope, ids: &[RecordId]) -> Result<Charged<Vec<Record>>, Miss> {
        let want: HashSet<&RecordId> = ids.iter().collect();
        Ok(Charged::new(
            self.load(scope)?
                .into_iter()
                .filter(|r| want.contains(&r.id))
                .collect(),
        ))
    }
}

impl MemoryProvider for Baseline {
    fn name(&self) -> &str {
        NAME
    }

    fn capability(&self) -> Capability {
        Capability {
            recall: true,
            retain: true,
            tools: true,
        }
    }

    fn budget(&self) -> Budget {
        self.budget
    }

    fn recall(&self, scope: &Scope, cue: &Cue) -> Result<Charged<Vec<Recalled>>, Miss> {
        walk(
            self,
            scope,
            &Probe::new(&cue.prompt, self.budget.max_records),
        )
    }

    fn retain(&self, scope: &Scope, o: &Observation) -> Result<Charged<Kept>, Miss> {
        let text = match &o.body {
            Body::Turn { user, assistant } => {
                format!("User: {}\nAssistant: {}", user.trim(), assistant.trim())
            }
            Body::Note { text } => text.trim().to_string(),
        };
        let declined = |why: &str| Ok(Charged::new(Kept::Declined(why.into())));
        if normal(&text).is_empty() {
            return declined("empty");
        }
        if text.chars().count() > self.budget.max_chars {
            return declined("larger than the recall budget; it could never be shown whole");
        }
        let records = self.load(scope)?;
        let key = normal(&text);
        if records.iter().any(|r| normal(&r.text) == key) {
            return declined("already kept");
        }
        let id = RecordId::new(format!("{:016x}", fnv1a(key.as_bytes())));
        let record = Record {
            id: id.clone(),
            scope: scope.clone(),
            text,
            observed_at: Some(now_rfc3339()),
            attrs: o.attrs.clone().into_iter().collect::<BTreeMap<_, _>>(),
        };
        let io =
            |e: std::io::Error| Miss::new(Why::Unreachable(format!("{}: {e}", self.dir.display())));
        fs::create_dir_all(&self.dir).map_err(io)?;
        let mut line =
            serde_json::to_string(&record).map_err(|e| Miss::new(Why::Malformed(e.to_string())))?;
        line.push('\n');
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.file(scope))
            .and_then(|mut f| f.write_all(line.as_bytes()))
            .map_err(io)?;
        Ok(Charged::new(Kept::Stored(id)))
    }

    fn toolset(self: Arc<Self>, ctx: &ToolContext) -> Option<Arc<dyn Toolset>> {
        let mut roster = ToolRoster::new();
        roster.add(crate::tools::collection(self, ctx));
        Some(Arc::new(roster))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_drop_stopwords_and_short_words() {
        assert_eq!(
            terms("Which branch do we deploy from? x"),
            ["branch", "deploy"]
        );
        assert!(terms("ok, thanks!").is_empty());
    }

    #[test]
    fn rfc3339_shape() {
        let t = now_rfc3339();
        assert_eq!(t.len(), 20, "{t}");
        assert!(t.ends_with('Z') && t.as_bytes()[10] == b'T', "{t}");
    }
}
