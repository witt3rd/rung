//! Property tests, offline and deterministic (a seeded generator, no extra
//! dependency): scope isolation in `walk`, whole-record budget cuts, and the
//! baseline's recency tie-break.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use rung_memory::baseline::Baseline;
use rung_memory::{
    Body, Budget, Capability, Charged, Cue, Edge, Hit, Kept, MemoryProvider, Miss, Observation,
    Probe, Reach, Recalled, Record, RecordId, Scope, Store, Why, retain_now, walk,
};

/// xorshift64*: enough randomness for case generation, reproducible by seed.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const CASES: u64 = 300;

fn rec(id: String, scope: &str, text: String) -> Record {
    Record {
        id: RecordId::new(id),
        scope: Scope::new(scope),
        text,
        observed_at: None,
        attrs: BTreeMap::new(),
    }
}

/// A store that ignores the scope it is asked about (the worst case: a
/// backend that leaks), with arbitrary edges, including to other scopes.
#[derive(Debug)]
struct Leaky {
    records: Vec<Record>,
    edges: Vec<(usize, usize)>,
    /// Extra records fetch returns that nobody asked for.
    stray_on_fetch: Option<Record>,
}

impl Store for Leaky {
    fn search(&self, _: &Scope, _: &str, limit: usize) -> Result<Charged<Vec<Hit>>, Miss> {
        Ok(Charged::new(
            self.records
                .iter()
                .take(limit)
                .map(|r| Hit {
                    id: r.id.clone(),
                    score: 1.0,
                })
                .collect(),
        ))
    }
    fn neighbours(
        &self,
        _: &Scope,
        id: &RecordId,
        limit: usize,
    ) -> Result<Charged<Vec<Edge>>, Miss> {
        let edges = self
            .edges
            .iter()
            .filter(|(from, _)| &self.records[*from].id == id)
            .map(|(_, to)| Edge {
                to: self.records[*to].id.clone(),
                relation: "r".into(),
                weight: 1.0,
            })
            .take(limit)
            .collect();
        Ok(Charged::new(edges))
    }
    fn fetch(&self, _: &Scope, ids: &[RecordId]) -> Result<Charged<Vec<Record>>, Miss> {
        let mut out: Vec<Record> = self
            .records
            .iter()
            .filter(|r| ids.contains(&r.id))
            .cloned()
            .collect();
        out.extend(self.stray_on_fetch.clone());
        Ok(Charged::new(out))
    }
}

#[test]
fn walk_never_returns_a_record_of_another_scope() {
    let scope = Scope::new("mine");
    for seed in 1..=CASES {
        let mut g = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let n = 1 + g.below(8);
        let records: Vec<Record> = (0..n)
            .map(|i| {
                let s = if g.below(3) == 0 { "theirs" } else { "mine" };
                rec(format!("r{i}"), s, format!("text {i}"))
            })
            .collect();
        let edges = (0..g.below(10)).map(|_| (g.below(n), g.below(n))).collect();
        let stray = (g.below(4) == 0).then(|| rec("stray".into(), "theirs", "x".into()));
        let store = Leaky {
            records,
            edges,
            stray_on_fetch: stray.clone(),
        };
        let probe = Probe::new("q", 1 + g.below(n)).expand(g.below(3));
        match walk(&store, &scope, &probe) {
            Ok(got) => {
                assert!(
                    got.value.iter().all(|r| r.record.scope == scope),
                    "seed {seed}: foreign record returned"
                );
                assert!(stray.is_none(), "seed {seed}: stray record went unnoticed");
            }
            Err(m) => match m.why {
                Why::OutOfScope(id) => {
                    let bad = store
                        .records
                        .iter()
                        .chain(stray.as_ref())
                        .find(|r| r.id == id)
                        .unwrap_or_else(|| panic!("seed {seed}: unknown id {id:?}"));
                    assert_ne!(bad.scope, scope, "seed {seed}: in-scope record refused");
                }
                other => panic!("seed {seed}: unexpected miss {other:?}"),
            },
        }
    }
}

#[test]
fn walk_over_an_all_in_scope_store_is_never_refused_and_has_no_duplicates() {
    let scope = Scope::new("mine");
    for seed in 1..=CASES {
        let mut g = Rng(seed ^ 0xDEAD_BEEF_1234);
        let n = 1 + g.below(8);
        let records: Vec<Record> = (0..n)
            .map(|i| rec(format!("r{i}"), "mine", format!("t{i}")))
            .collect();
        let edges = (0..g.below(12)).map(|_| (g.below(n), g.below(n))).collect();
        let store = Leaky {
            records,
            edges,
            stray_on_fetch: None,
        };
        let probe = Probe::new("q", 1 + g.below(n)).expand(g.below(3));
        let got = walk(&store, &scope, &probe).expect("in-scope store is not refused");
        let ids: HashSet<&RecordId> = got.value.iter().map(|r| &r.record.id).collect();
        assert_eq!(ids.len(), got.value.len(), "seed {seed}: duplicate record");
        let hits = got
            .value
            .iter()
            .filter(|r| matches!(r.reach, Reach::Hit { .. }))
            .count();
        assert!(hits <= probe.limit, "seed {seed}: more hits than the limit");
    }
}

// ─── Budget cuts keep whole records ──────────────────────────────────────────

#[derive(Debug)]
struct Canned {
    items: Vec<Recalled>,
    budget: Budget,
}

impl MemoryProvider for Canned {
    fn name(&self) -> &str {
        "canned"
    }
    fn capability(&self) -> Capability {
        Capability {
            recall: true,
            retain: false,
            tools: false,
        }
    }
    fn budget(&self) -> Budget {
        self.budget
    }
    fn recall(&self, _: &Scope, _: &Cue) -> Result<Charged<Vec<Recalled>>, Miss> {
        Ok(Charged::new(self.items.clone()))
    }
    fn retain(&self, _: &Scope, _: &Observation) -> Result<Charged<Kept>, Miss> {
        unreachable!("retain is not declared")
    }
}

#[test]
fn a_budget_cut_keeps_whole_records_in_order_within_both_limits() {
    let scope = Scope::new("s");
    for seed in 1..=CASES {
        let mut g = Rng(seed.wrapping_mul(0xA24B_AED4_963E_E407) | 1);
        let n = g.below(12);
        let items: Vec<Recalled> = (0..n)
            .map(|_| {
                // Few distinct ids, so duplicates occur.
                let id = format!("r{}", g.below(8));
                let len = 1 + g.below(40);
                // Multibyte chars: the budget counts chars, not bytes.
                let text: String = (0..len).map(|_| ['a', 'é', '日'][g.below(3)]).collect();
                Recalled {
                    record: rec(id, "s", text),
                    reach: Reach::Hit { score: 1.0 },
                }
            })
            .collect();
        let budget = Budget {
            max_records: 1 + g.below(6),
            max_chars: 1 + g.below(120),
            max_cost_usd: 0.0,
        };
        let p: Arc<dyn MemoryProvider> = Arc::new(Canned {
            items: items.clone(),
            budget,
        });
        let (report, found) = rung_memory::recall_outcome(p, scope.clone(), Cue::default());
        let kept: Vec<Recalled> = found.map(|e| e.into_items()).unwrap_or_default();

        assert!(kept.len() <= budget.max_records, "seed {seed}: too many");
        let chars: usize = kept.iter().map(|r| r.record.text.chars().count()).sum();
        assert!(
            chars <= budget.max_chars,
            "seed {seed}: over the char budget"
        );
        // Whole: every kept record is, byte for byte, one the provider gave.
        for k in &kept {
            assert!(
                items.iter().any(|i| i.record == k.record),
                "seed {seed}: a kept record is not one that was offered"
            );
        }
        // Order is the provider's; ids are unique; first occurrence wins.
        let mut seen = HashSet::new();
        let unique: Vec<&Recalled> = items
            .iter()
            .filter(|i| seen.insert(i.record.id.clone()))
            .collect();
        let mut at = 0;
        for k in &kept {
            let pos = unique[at..]
                .iter()
                .position(|u| u.record == k.record)
                .unwrap_or_else(|| panic!("seed {seed}: out of order or duplicated"));
            at += pos + 1;
        }
        // Nothing is lost silently: kept + left out = distinct offered.
        assert_eq!(
            kept.len() + report.left_out,
            unique.len(),
            "seed {seed}: records unaccounted for"
        );
        // A record that fit when it was considered is never the one dropped:
        // greedy, so each dropped record would have broken a limit.
        let (mut c, mut k_n) = (0usize, 0usize);
        for u in &unique {
            let n = u.record.text.chars().count();
            if k_n < budget.max_records && c + n <= budget.max_chars {
                c += n;
                k_n += 1;
                assert!(
                    kept.iter().any(|k| k.record == u.record),
                    "seed {seed}: a record that fit was dropped"
                );
            }
        }
        assert_eq!(report.records, kept.len());
    }
}

// ─── The baseline's recency tie-break ────────────────────────────────────────

fn tmp(name: &str) -> rung_testkit::TempDir {
    rung_testkit::TempDir::new(&format!("memory-prop-{name}"))
}

fn note(text: &str) -> Observation {
    Observation {
        body: Body::Note { text: text.into() },
        attrs: Default::default(),
    }
}

fn keep(b: &Arc<Baseline>, scope: &Scope, text: &str) {
    let r = retain_now(b.clone(), scope.clone(), note(text));
    assert_eq!(r.status, "stored", "{text}: {:?}", r.reason);
}

fn hit_order(b: &Baseline, scope: &Scope, query: &str, limit: usize) -> Vec<String> {
    b.search(scope, query, limit)
        .unwrap()
        .value
        .into_iter()
        .map(|h| h.id.as_str().to_string())
        .collect()
}

fn id_of(b: &Baseline, scope: &Scope, text: &str) -> String {
    // Records are append-only and the file is the order: find the id by text.
    let body = std::fs::read_to_string(b.file(scope)).unwrap();
    body.lines()
        .map(|l| serde_json::from_str::<Record>(l).unwrap())
        .find(|r| r.text == text)
        .unwrap()
        .id
        .as_str()
        .to_string()
}

#[test]
fn equal_relevance_goes_to_the_newer_record() {
    for n in 2..=9usize {
        let dir = tmp("tie");
        let b = Arc::new(Baseline::new(&dir));
        let scope = Scope::new("s");
        // Same length, same term frequency: equal BM25. Only age differs.
        let texts: Vec<String> = (0..n).map(|i| format!("deploy mark{i}")).collect();
        for t in &texts {
            keep(&b, &scope, t);
        }
        let want: Vec<String> = texts.iter().rev().map(|t| id_of(&b, &scope, t)).collect();
        assert_eq!(hit_order(&b, &scope, "deploy", n), want, "n={n}");
    }
}

#[test]
fn recency_never_outweighs_relevance() {
    let dir = tmp("rel");
    let b = Arc::new(Baseline::new(&dir));
    let scope = Scope::new("s");
    // The oldest record matches both query terms; every newer one matches one.
    keep(&b, &scope, "deploy release");
    for i in 0..20 {
        keep(&b, &scope, &format!("deploy mark{i}"));
    }
    let order = hit_order(&b, &scope, "deploy release", 30);
    assert_eq!(order[0], id_of(&b, &scope, "deploy release"));
}

#[test]
fn the_tie_break_is_per_scope_position_not_global() {
    let dir = tmp("scopes");
    let b = Arc::new(Baseline::new(&dir));
    let (s, other) = (Scope::new("s"), Scope::new("other"));
    keep(&b, &s, "deploy mark1");
    keep(&b, &s, "deploy mark2");
    // Many newer records in another scope must not shift this scope's order.
    for i in 0..10 {
        keep(&b, &other, &format!("deploy mark{i}"));
    }
    let want = vec![id_of(&b, &s, "deploy mark2"), id_of(&b, &s, "deploy mark1")];
    assert_eq!(hit_order(&b, &s, "deploy", 5), want);
}
