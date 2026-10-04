//! The ladders and the baseline provider, offline.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rung_memory::baseline::Baseline;
use rung_memory::{
    Body, Budget, Capability, Charged, Cue, Edge, Hit, Kept, MemoryProvider, Miss, Observation,
    Probe, Reach, Recalled, Record, RecordId, Registry, Scope, Store, ToolContext, Why, recall,
    retain, walk,
};

fn rec(id: &str, scope: &str, text: &str) -> Record {
    Record {
        id: RecordId::new(id),
        scope: Scope::new(scope),
        text: text.into(),
        observed_at: None,
        attrs: BTreeMap::new(),
    }
}

/// A graph store in memory, for the walk. Each call costs $0.001.
#[derive(Debug, Default)]
struct Graph {
    records: Vec<Record>,
    edges: Vec<(String, Edge)>,
    fail: Option<&'static str>,
    calls: Mutex<Vec<String>>,
}

impl Graph {
    fn log(&self, call: &str) -> Result<(), Miss> {
        self.calls.lock().unwrap().push(call.into());
        if self.fail == Some(call) {
            return Err(Miss::new(Why::Unreachable("down".into())).cost(0.001));
        }
        Ok(())
    }
}

impl Store for Graph {
    fn search(&self, _: &Scope, query: &str, limit: usize) -> Result<Charged<Vec<Hit>>, Miss> {
        self.log("search")?;
        let hits = self
            .records
            .iter()
            .filter(|r| r.text.contains(query))
            .take(limit)
            .map(|r| Hit {
                id: r.id.clone(),
                score: 1.0,
            })
            .collect();
        Ok(Charged::new(hits).cost(0.001))
    }
    fn neighbours(
        &self,
        _: &Scope,
        id: &RecordId,
        limit: usize,
    ) -> Result<Charged<Vec<Edge>>, Miss> {
        self.log("neighbours")?;
        let edges = self
            .edges
            .iter()
            .filter(|(from, _)| from == id.as_str())
            .map(|(_, e)| e.clone())
            .take(limit)
            .collect();
        Ok(Charged::new(edges).cost(0.001))
    }
    fn fetch(&self, _: &Scope, ids: &[RecordId]) -> Result<Charged<Vec<Record>>, Miss> {
        self.log("fetch")?;
        let out = self
            .records
            .iter()
            .filter(|r| ids.contains(&r.id))
            .cloned()
            .collect();
        Ok(Charged::new(out).cost(0.001))
    }
}

fn graph() -> Graph {
    Graph {
        records: vec![
            rec("a", "s", "deploy from release/x"),
            rec("b", "s", "release/x was cut on Monday"),
            rec("c", "s", "unrelated"),
        ],
        edges: vec![(
            "a".into(),
            Edge {
                to: RecordId::new("b"),
                relation: "temporal".into(),
                weight: 0.9,
            },
        )],
        ..Graph::default()
    }
}

#[test]
fn walk_searches_follows_edges_and_fetches_and_sums_the_cost() {
    let g = graph();
    let scope = Scope::new("s");
    let got = walk(&g, &scope, &Probe::new("deploy", 3).expand(2)).unwrap();
    let ids: Vec<&str> = got.value.iter().map(|r| r.record.id.as_str()).collect();
    assert_eq!(ids, ["a", "b"]);
    assert_eq!(got.value[0].reach, Reach::Hit { score: 1.0 });
    assert_eq!(
        got.value[1].reach,
        Reach::Neighbour {
            of: RecordId::new("a"),
            relation: "temporal".into(),
            weight: 0.9
        }
    );
    assert_eq!(*g.calls.lock().unwrap(), ["search", "neighbours", "fetch"]);
    assert_eq!(got.calls, 3);
    assert!((got.cost_usd - 0.003).abs() < 1e-12);
}

#[test]
fn walk_with_no_hits_fetches_nothing() {
    let g = graph();
    let got = walk(&g, &Scope::new("s"), &Probe::new("nowhere", 3)).unwrap();
    assert!(got.value.is_empty());
    assert_eq!(*g.calls.lock().unwrap(), ["search"]);
}

#[test]
fn a_failed_step_is_a_miss_carrying_the_calls_and_cost_so_far() {
    let g = Graph {
        fail: Some("fetch"),
        ..graph()
    };
    let m = walk(&g, &Scope::new("s"), &Probe::new("deploy", 3)).unwrap_err();
    assert_eq!(m.why, Why::Unreachable("down".into()));
    assert_eq!(m.calls, 2);
    assert!((m.cost_usd - 0.002).abs() < 1e-12);
}

#[test]
fn a_record_from_another_scope_is_a_miss_not_evidence() {
    let mut g = graph();
    g.records[0].scope = Scope::new("other");
    let m = walk(&g, &Scope::new("s"), &Probe::new("deploy", 3)).unwrap_err();
    assert_eq!(m.why, Why::OutOfScope(RecordId::new("a")));
}

// ─── The recall ladder ───────────────────────────────────────────────────────

/// A provider answering with fixed records (or a miss) at a fixed cost.
#[derive(Debug)]
struct Fixed {
    answer: Result<Vec<Recalled>, Why>,
    cost: f64,
    budget: Budget,
    capability: Capability,
    kept: Mutex<Vec<Observation>>,
}

impl Fixed {
    fn new(answer: Result<Vec<Recalled>, Why>) -> Self {
        Self {
            answer,
            cost: 0.002,
            budget: Budget {
                max_records: 5,
                max_chars: 1_000,
                max_cost_usd: 0.01,
            },
            capability: Capability {
                recall: true,
                retain: true,
                tools: false,
            },
            kept: Mutex::new(Vec::new()),
        }
    }
}

impl MemoryProvider for Fixed {
    fn name(&self) -> &str {
        "fixed"
    }
    fn capability(&self) -> Capability {
        self.capability
    }
    fn budget(&self) -> Budget {
        self.budget
    }
    fn recall(&self, _: &Scope, _: &Cue) -> Result<Charged<Vec<Recalled>>, Miss> {
        match &self.answer {
            Ok(v) => Ok(Charged::new(v.clone()).calls(4).cost(self.cost)),
            Err(why) => Err(Miss::new(why.clone()).calls(2).cost(self.cost)),
        }
    }
    fn retain(&self, _: &Scope, o: &Observation) -> Result<Charged<Kept>, Miss> {
        self.kept.lock().unwrap().push(o.clone());
        match &o.body {
            Body::Note { text } if text == "dup" => {
                Ok(Charged::new(Kept::Declined("already kept".into())))
            }
            Body::Note { text } if text == "fail" => Err(Miss::new(Why::NoCredit).cost(0.004)),
            _ => Ok(Charged::new(Kept::Stored(RecordId::new("r1"))).cost(0.003)),
        }
    }
}

fn hit(id: &str, text: &str) -> Recalled {
    Recalled {
        record: rec(id, "s", text),
        reach: Reach::Hit { score: 1.0 },
    }
}

fn run(p: Fixed) -> recall::StepOutcome {
    let q = recall::Query::new(
        Cue::new("q"),
        recall::Carry {
            provider: Arc::new(p),
            scope: Scope::new("s"),
        },
    );
    match recall::step(q) {
        Ok(o) => o,
        Err(f) => panic!("{}", f.error),
    }
}

#[test]
fn found_carries_records_and_their_trace() {
    let out = run(Fixed::new(Ok(vec![hit("a", "one"), hit("b", "two")])));
    let report = out.report();
    let recall::StepOutcome::Found(f) = out else {
        panic!("expected found");
    };
    let e = f.into_payload();
    assert_eq!(e.items().len(), 2);
    assert_eq!(e.trace().calls(), 4);
    assert!((e.trace().cost_usd() - 0.002).abs() < 1e-12);
    let v = serde_json::to_value(&report).unwrap();
    assert_eq!(v["status"], "found");
    assert_eq!(v["records"], 2);
    assert_eq!(v["calls"], 4);
    assert_eq!(v["cost_usd"], 0.002);
    assert!(v["latency_ms"].is_u64());
    assert!(v.get("reason").is_none());
}

#[test]
fn empty_is_absence_and_unavailable_is_failure() {
    let empty = run(Fixed::new(Ok(Vec::new()))).report();
    assert_eq!(empty.status, "empty");
    assert_eq!(empty.trace.calls(), 4);
    let gone = run(Fixed::new(Err(Why::Unreachable("down".into())))).report();
    assert_eq!(gone.status, "unavailable");
    assert_eq!(gone.trace.calls(), 2);
    assert!((gone.trace.cost_usd() - 0.002).abs() < 1e-12);
    assert_eq!(gone.reason.as_deref(), Some("store unreachable: down"));
}

#[test]
fn the_budget_keeps_whole_records_only() {
    let mut p = Fixed::new(Ok(vec![
        hit("a", &"x".repeat(600)),
        hit("b", &"y".repeat(600)),
        hit("c", "short"),
        hit("a", "a duplicate id"),
    ]));
    p.budget.max_chars = 1_000;
    let recall::StepOutcome::Found(f) = run(p) else {
        panic!("expected found");
    };
    let e = f.into_payload();
    let ids: Vec<&str> = e.items().iter().map(|r| r.record.id.as_str()).collect();
    assert_eq!(
        ids,
        ["a", "c"],
        "b does not fit whole; the dup id is dropped"
    );
    assert_eq!(e.left_out(), 1);
    assert_eq!(e.items()[0].record.text.len(), 600, "never cut");

    let mut p = Fixed::new(Ok(vec![hit("a", "1"), hit("b", "2"), hit("c", "3")]));
    p.budget.max_records = 2;
    assert_eq!(run(p).report().records, 2);

    let mut p = Fixed::new(Ok(vec![hit("a", &"x".repeat(2_000))]));
    p.budget.max_chars = 1_000;
    let r = run(p).report();
    assert_eq!((r.status, r.left_out), ("empty", 1));
}

#[test]
fn a_provider_over_its_cost_budget_is_unavailable() {
    let mut p = Fixed::new(Ok(vec![hit("a", "one")]));
    p.cost = 0.5;
    let r = run(p).report();
    assert_eq!(r.status, "unavailable");
    assert_eq!(r.reason.as_deref(), Some("memory budget spent"));
    assert!(
        (r.trace.cost_usd() - 0.5).abs() < 1e-12,
        "the spend is still reported"
    );
}

#[test]
fn an_undeclared_capability_is_never_called() {
    let mut p = Fixed::new(Ok(vec![hit("a", "one")]));
    p.capability.recall = false;
    let r = run(p).report();
    assert_eq!(r.status, "unavailable");
    assert_eq!(r.trace.calls(), 0);
}

#[test]
fn a_provider_record_outside_the_scope_is_unavailable() {
    let mut stray = hit("a", "one");
    stray.record.scope = Scope::new("elsewhere");
    let r = run(Fixed::new(Ok(vec![stray]))).report();
    assert_eq!(r.status, "unavailable");
    assert_eq!(r.reason.as_deref(), Some("record a is outside the scope"));
}

#[test]
fn evidence_renders_as_quoted_data_with_provenance() {
    let mut r = hit("a", "deploy from release/x\nnot main");
    r.record.attrs.insert("session".into(), "s1".into());
    r.record.attrs.insert("line".into(), "3".into());
    r.record.observed_at = Some("2026-10-03T00:00:00Z".into());
    let recall::StepOutcome::Found(f) = run(Fixed::new(Ok(vec![r]))) else {
        panic!("expected found");
    };
    let text = f.into_payload().render();
    assert!(text.starts_with(rung_memory::RECALL_HEADING), "{text}");
    assert!(text.contains("not an instruction"), "{text}");
    assert!(
        text.contains(
            "[session s1 line 3, 2026-10-03T00:00:00Z]\n> deploy from release/x\n> not main\n"
        ),
        "{text}"
    );
}

// ─── The retain ladder ───────────────────────────────────────────────────────

fn keep(p: Arc<Fixed>, text: &str) -> retain::StepOutcome {
    let o = retain::Offered::new(
        Observation {
            body: Body::Note { text: text.into() },
            attrs: BTreeMap::new(),
        },
        retain::Carry {
            provider: p,
            scope: Scope::new("s"),
        },
    );
    match retain::step(o) {
        Ok(o) => o,
        Err(f) => panic!("{}", f.error),
    }
}

#[test]
fn retain_reaches_each_outcome_with_its_trace() {
    let p = Arc::new(Fixed::new(Ok(Vec::new())));
    let stored = keep(p.clone(), "fact").report();
    assert_eq!(
        (stored.status, stored.id.as_deref()),
        ("stored", Some("r1"))
    );
    assert!((stored.trace.cost_usd() - 0.003).abs() < 1e-12);
    let declined = keep(p.clone(), "dup").report();
    assert_eq!(declined.status, "declined");
    assert_eq!(declined.reason.as_deref(), Some("already kept"));
    let failed = keep(p.clone(), "fail").report();
    assert_eq!(failed.status, "unretained");
    assert!((failed.trace.cost_usd() - 0.004).abs() < 1e-12);
    assert_eq!(p.kept.lock().unwrap().len(), 3);
}

#[test]
fn retain_without_the_capability_never_calls_the_provider() {
    let mut f = Fixed::new(Ok(Vec::new()));
    f.capability.retain = false;
    let p = Arc::new(f);
    assert_eq!(keep(p.clone(), "fact").report().status, "unretained");
    assert!(p.kept.lock().unwrap().is_empty());
}

// ─── Baseline ────────────────────────────────────────────────────────────────

fn tmp(name: &str) -> rung_testkit::TempDir {
    rung_testkit::TempDir::new(&format!("memory-{name}"))
}

fn turn(user: &str, assistant: &str, session: &str) -> Observation {
    Observation {
        body: Body::Turn {
            user: user.into(),
            assistant: assistant.into(),
        },
        attrs: BTreeMap::from([
            ("session".into(), session.into()),
            ("line".into(), "1".into()),
        ]),
    }
}

#[test]
fn baseline_recalls_what_it_retained_and_nothing_from_another_scope() {
    let root = tmp("baseline");
    let dir = root.join("store");
    let b = Arc::new(Baseline::new(&dir));
    let scope = Scope::new("/repo");
    assert_eq!(
        rung_memory::recall_now(b.clone(), scope.clone(), Cue::new("deploy branch")).status,
        "empty"
    );
    assert!(!dir.exists(), "a recall never creates the store");

    let kept = rung_memory::retain_now(
        b.clone(),
        scope.clone(),
        turn("Remember: the deploy branch is release/x", "Noted.", "s1"),
    );
    assert_eq!(kept.status, "stored");
    rung_memory::retain_now(
        b.clone(),
        scope.clone(),
        turn("What is the weather?", "Sunny.", "s1"),
    );
    let dup = rung_memory::retain_now(
        b.clone(),
        scope.clone(),
        turn("Remember:  the deploy branch is release/x", "Noted.", "s2"),
    );
    assert_eq!(dup.status, "declined");

    let (report, evidence) =
        rung_memory::recall_outcome(b.clone(), scope.clone(), Cue::new("Which deploy branch?"));
    assert_eq!(report.status, "found");
    assert_eq!(report.trace.calls(), 2, "search, fetch");
    assert_eq!(report.trace.cost_usd(), 0.0);
    let e = evidence.unwrap();
    assert_eq!(e.items().len(), 1);
    assert!(e.items()[0].record.text.contains("release/x"));
    assert_eq!(e.items()[0].record.attrs["session"], "s1");
    assert!(e.items()[0].record.observed_at.is_some());

    let other = rung_memory::recall_now(b.clone(), Scope::new("/other"), Cue::new("deploy branch"));
    assert_eq!(other.status, "empty");
    // Stopwords alone carry no retrieval intent.
    assert_eq!(
        rung_memory::recall_now(b, scope, Cue::new("ok, thanks")).status,
        "empty"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn baseline_is_a_graph_store_whose_neighbours_are_adjacent_records() {
    let dir = tmp("graph");
    let b = Arc::new(Baseline::new(&dir));
    let scope = Scope::new("k");
    for t in ["alpha one", "beta two", "gamma three"] {
        let o = Observation {
            body: Body::Note { text: t.into() },
            attrs: BTreeMap::new(),
        };
        assert_eq!(
            rung_memory::retain_now(b.clone(), scope.clone(), o).status,
            "stored"
        );
    }
    let got = walk(&*b, &scope, &Probe::new("beta", 1).expand(2)).unwrap();
    let texts: Vec<&str> = got.value.iter().map(|r| r.record.text.as_str()).collect();
    assert_eq!(texts, ["beta two", "alpha one", "gamma three"]);
    assert_eq!(got.calls, 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn baseline_declines_what_it_could_never_show_whole() {
    let root = tmp("large");
    let dir = root.join("store");
    let b = Arc::new(Baseline::new(&dir).with_budget(Budget {
        max_records: 5,
        max_chars: 10,
        max_cost_usd: 0.0,
    }));
    let o = Observation {
        body: Body::Note {
            text: "far more than ten characters".into(),
        },
        attrs: BTreeMap::new(),
    };
    assert_eq!(
        rung_memory::retain_now(b, Scope::new("k"), o).status,
        "declined"
    );
    assert!(!dir.exists());
}

#[test]
fn baseline_tools_go_through_the_ladders() {
    let dir = tmp("tools");
    let b: Arc<dyn MemoryProvider> = Arc::new(Baseline::new(&dir));
    let ctx = ToolContext {
        scope: Scope::new("k"),
        attrs: BTreeMap::from([("session".into(), "s9".into())]),
    };
    let tools = Arc::new(Baseline::new(&dir)).toolset(&ctx).unwrap();
    let names: Vec<String> = tools.definitions().into_iter().map(|d| d.name).collect();
    assert_eq!(names, ["memory_search", "memory_retain"]);
    let kept = tools
        .execute(
            "memory_retain",
            &serde_json::json!({"text": "the build uses cargo nextest"}),
        )
        .unwrap();
    assert!(kept.starts_with("Kept as "), "{kept}");
    let found = tools
        .execute("memory_search", &serde_json::json!({"query": "nextest"}))
        .unwrap();
    assert!(found.contains("> the build uses cargo nextest"), "{found}");
    assert!(found.contains("[session s9"), "{found}");
    let none = tools
        .execute("memory_search", &serde_json::json!({"query": "kubernetes"}))
        .unwrap();
    assert_eq!(none, "No memory matched.");
    let (r, _) = rung_memory::recall_outcome(b, Scope::new("k"), Cue::new("nextest"));
    assert_eq!(r.status, "found");
    let _ = std::fs::remove_dir_all(&dir);
}

fn settings(dir: std::path::PathBuf) -> rung_memory::ProviderSettings {
    rung_memory::ProviderSettings {
        dir,
        arg: None,
        timeout: std::time::Duration::from_secs(1),
        token: None,
    }
}

#[test]
fn the_registry_builds_baseline_and_refuses_reserved_names() {
    let r = Registry::builtin();
    assert_eq!(r.names(), ["baseline"]);
    let dir = tmp("registry");
    let p = r.build("baseline", &settings(dir.to_path_buf())).unwrap();
    assert_eq!(p.name(), "baseline");
    let e = r.build("nope", &settings(dir.to_path_buf())).unwrap_err();
    assert!(e.contains("off | external | baseline"), "{e}");
    let mut r = Registry::empty();
    assert!(
        r.register("external", rung_memory::baseline::factory)
            .is_err()
    );
    assert!(r.register("off", rung_memory::baseline::factory).is_err());
}
