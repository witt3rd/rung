//! Recall quality of the baseline (BM25) provider on the fixture set.
//! Fixtures: `tests/fixtures/recall/` (see its README for the recipe to score
//! your own notes via `RUNG_RECALL_FIXTURES`).

use std::fs;
use std::path::PathBuf;

use rung_memory::baseline::{Baseline, Options};
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
    // Optional second set: inflected / reworded questions (same notes).
    let inflected: Vec<Question> = if dir.join("questions_inflected.jsonl").exists() {
        lines(dir.join("questions_inflected.jsonl"))
    } else {
        Vec::new()
    };

    let tmp = TempDir::new("recall-quality");
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
    fs::write(Baseline::new(tmp.path()).file(&scope), body).unwrap();

    println!("| provider | notes | questions | hit@1 | hit@5 | MRR |");
    println!("|---|---|---|---|---|---|");
    let mut default_row = (0.0, 0.0, 0.0);
    let mut inflected_default = (0.0, 0.0, 0.0);
    for (set, qs) in [("direct", &questions), ("inflected", &inflected)] {
        if qs.is_empty() {
            continue;
        }
        for arg in ["", "stem", "phrase", "stem,phrase"] {
            let b = Baseline::new(tmp.path()).with_options(Options::parse(arg).unwrap());
            let row = score(&b, &scope, qs, arg.is_empty());
            println!(
                "| baseline{} ({set}) | {} | {} | {:.3} | {:.3} | {:.3} |",
                if arg.is_empty() {
                    String::new()
                } else {
                    format!(":{arg}")
                },
                notes.len(),
                qs.len(),
                row.0,
                row.1,
                row.2
            );
            if arg.is_empty() && set == "direct" {
                default_row = row;
            }
            if custom.is_none() && set == "inflected" {
                match arg {
                    "" => inflected_default = row,
                    "stem" => assert!(
                        row.2 > inflected_default.2 && row.1 >= inflected_default.1,
                        "stem should beat the default on inflected questions"
                    ),
                    _ => {}
                }
            }
        }
    }
    let (h1, h5, mrr) = default_row;
    if custom.is_none() {
        assert!(h1 >= 0.8 && h5 >= 0.95 && mrr >= 0.88, "recall regressed");
    }
}

fn score(b: &Baseline, scope: &Scope, questions: &[Question], verbose: bool) -> (f64, f64, f64) {
    let (mut h1, mut h5, mut mrr) = (0.0, 0.0, 0.0);
    for q in questions {
        let hits = b.search(scope, &q.q, 5).unwrap().value;
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
            if verbose {
                println!("miss: {}", q.q);
            }
        }
    }
    let n = questions.len() as f64;
    (h1 / n, h5 / n, mrr / n)
}
