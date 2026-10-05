//! Recall quality of the baseline (BM25) provider on the fixture set.
//! Fixtures: `tests/fixtures/recall/` (see its README for the recipe to score
//! your own notes via `RUNG_RECALL_FIXTURES`).

use std::fs;
use std::path::PathBuf;

use rung_memory::baseline::Baseline;
use rung_memory::{Record, Scope, Store};
use rung_testkit::TempDir;

#[derive(serde::Deserialize)]
struct Note {
    id: String,
    text: String,
}
#[derive(serde::Deserialize)]
struct Question {
    q: String,
    expect: Vec<String>,
}

fn lines<T: serde::de::DeserializeOwned>(path: PathBuf) -> Vec<T> {
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn baseline_recall_quality() {
    let custom = std::env::var("RUNG_RECALL_FIXTURES")
        .ok()
        .map(PathBuf::from);
    let dir = custom
        .clone()
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recall"));
    let notes: Vec<Note> = lines(dir.join("notes.jsonl"));
    let questions: Vec<Question> = lines(dir.join("questions.jsonl"));

    let tmp = TempDir::new("recall-quality");
    let b = Baseline::new(tmp.path());
    let scope = Scope::new("fixture");
    let body: String = notes
        .iter()
        .map(|n| {
            let r = Record {
                id: rung_memory::RecordId::new(&n.id),
                scope: scope.clone(),
                text: n.text.clone(),
                observed_at: None,
                attrs: Default::default(),
            };
            serde_json::to_string(&r).unwrap() + "\n"
        })
        .collect();
    fs::write(b.file(&scope), body).unwrap();

    let (mut h1, mut h5, mut mrr) = (0.0, 0.0, 0.0);
    for q in &questions {
        let hits = b.search(&scope, &q.q, 5).unwrap().value;
        let rank = hits
            .iter()
            .position(|h| q.expect.iter().any(|e| e == h.id.as_str()));
        if let Some(r) = rank {
            if r == 0 {
                h1 += 1.0;
            }
            h5 += 1.0;
            mrr += 1.0 / (r as f64 + 1.0);
        } else {
            println!("miss: {}", q.q);
        }
    }
    let n = questions.len() as f64;
    let (h1, h5, mrr) = (h1 / n, h5 / n, mrr / n);
    println!("| provider | notes | questions | hit@1 | hit@5 | MRR |");
    println!("|---|---|---|---|---|---|");
    println!(
        "| baseline | {} | {} | {h1:.3} | {h5:.3} | {mrr:.3} |",
        notes.len(),
        questions.len()
    );
    if custom.is_none() {
        assert!(h1 >= 0.8 && h5 >= 0.95 && mrr >= 0.88, "recall regressed");
    }
}
