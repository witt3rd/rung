//! `baseline`: the provider rung ships. No model, no network, no cost.
//!
//! - **Store**: one JSON-lines file per scope, appended to, under the
//!   provider's directory (`<dir>/<scope hash>.jsonl`), one [`Record`] per
//!   line. Only an explicit inspection command ([`Baseline::delete_record`],
//!   [`Baseline::delete_scope`]) removes anything. The directory is created by the
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
///
/// The optional `arg` (`baseline:stem`) turns on ranking options; see
/// [`Options`]. With no `arg` the ranking is unchanged.
pub fn factory(s: &ProviderSettings) -> Result<Arc<dyn MemoryProvider>, String> {
    let options = match &s.arg {
        Some(a) => Options::parse(a)?,
        None => Options::default(),
    };
    Ok(Arc::new(Baseline::new(&s.dir).with_options(options)))
}

/// Opt-in ranking changes. Default: all off (plain BM25 plus recency).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Fold plural and verb endings (`meetings`/`meeting`, `walked`/`walk`)
    /// on both sides of the match. Silent-e verbs do not fold (`moved` is
    /// `mov`, `move` stays `move`).
    pub stem: bool,
}

impl Options {
    /// `stem`; anything else is an error.
    pub fn parse(arg: &str) -> Result<Self, String> {
        let mut o = Self::default();
        for w in arg.split(',').map(str::trim).filter(|w| !w.is_empty()) {
            match w {
                "stem" => o.stem = true,
                other => {
                    return Err(format!("baseline: unknown option '{other}' (stem)"));
                }
            }
        }
        Ok(o)
    }
}

/// A crude English suffix fold: enough to join inflections, no dictionary.
fn stem(w: &str) -> String {
    let n = w.chars().count();
    let cut = |k: usize| w.chars().take(n - k).collect::<String>();
    if n > 4 && w.ends_with("ies") {
        return cut(3) + "y";
    }
    if n > 5 && w.ends_with("ing") {
        return cut(3);
    }
    if n > 4 && w.ends_with("ed") {
        return cut(2);
    }
    if n > 3 && w.ends_with("es") && !w.ends_with("ses") {
        return cut(2);
    }
    if n > 3 && w.ends_with('s') && !w.ends_with("ss") {
        return cut(1);
    }
    w.to_string()
}

/// See the module docs.
#[derive(Debug, Clone)]
pub struct Baseline {
    dir: PathBuf,
    budget: Budget,
    options: Options,
}

impl Baseline {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
            budget: Budget::default(),
            options: Options::default(),
        }
    }

    pub fn with_options(mut self, options: Options) -> Self {
        self.options = options;
        self
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

    /// The scope's records, oldest first (for inspection; same as recall sees).
    pub fn records(&self, scope: &Scope) -> Result<Vec<Record>, Miss> {
        self.load(scope)
    }

    /// Delete one record by id; true if it was there. Rewrites the scope file
    /// (via a temp file and rename) with the remaining lines untouched.
    pub fn delete_record(&self, scope: &Scope, id: &RecordId) -> Result<bool, String> {
        let path = self.file(scope);
        let body = match fs::read_to_string(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let mut found = false;
        let mut kept = String::new();
        for l in body.lines() {
            let hit = serde_json::from_str::<Record>(l)
                .map(|r| &r.scope == scope && &r.id == id)
                .unwrap_or(false);
            if hit {
                found = true;
            } else {
                kept.push_str(l);
                kept.push('\n');
            }
        }
        if found {
            let tmp = path.with_extension("jsonl.tmp");
            fs::write(&tmp, kept).map_err(|e| format!("{}: {e}", tmp.display()))?;
            fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        Ok(found)
    }

    /// Delete the scope's file; true if it existed.
    pub fn delete_scope(&self, scope: &Scope) -> Result<bool, String> {
        let path = self.file(scope);
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
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

fn terms_with(text: &str, o: Options) -> Vec<String> {
    let t = terms(text);
    if o.stem {
        t.iter().map(|w| stem(w)).collect()
    } else {
        t
    }
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

impl Store for Baseline {
    fn search(&self, scope: &Scope, query: &str, limit: usize) -> Result<Charged<Vec<Hit>>, Miss> {
        let records = self.load(scope)?;
        let q: HashSet<String> = terms_with(query, self.options).into_iter().collect();
        if q.is_empty() || records.is_empty() {
            return Ok(Charged::new(Vec::new()));
        }
        let docs: Vec<Vec<String>> = records
            .iter()
            .map(|r| terms_with(&r.text, self.options))
            .collect();
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
            observed_at: Some(rung_std::time::utc_now()),
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
    fn options_parse_and_stem() {
        assert_eq!(Options::parse("").unwrap(), Options::default());
        let o = Options::parse("stem").unwrap();
        assert!(o.stem);
        assert!(Options::parse("fuzzy").is_err());
        assert_eq!(stem("meetings"), "meeting");
        assert_eq!(stem("moved"), "mov");
        assert_eq!(stem("move"), "move");
        assert_eq!(stem("policies"), "policy");
        assert_eq!(stem("pass"), "pass");
        assert_eq!(terms_with("Moved meetings", o), ["mov", "meeting"]);
    }

    #[test]
    fn rfc3339_shape() {
        let t = rung_std::time::utc_now();
        assert_eq!(t.len(), 20, "{t}");
        assert!(t.ends_with('Z') && t.as_bytes()[10] == b'T', "{t}");
    }
}
