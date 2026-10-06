//! Retain quality: what per-turn noise costs recall, and what distillation
//! recovers. Generic fixtures; the table is printed with `--nocapture`.
//!
//! Recipe for another corpus: replace `turns()` and `questions()` with your
//! own `(id, user, assistant)` turns and `(question, id)` pairs; the scorer
//! is `score()`.

use std::collections::BTreeMap;
use std::sync::Arc;

use rung_memory::baseline::Baseline;
use rung_memory::distill::distill;
use rung_memory::{Body, Cue, Kept, MemoryProvider, Observation, Scope};

const CODE: &str = "```rust\nfn handler(req: Request) -> Response {\n    let cache = Cache::new();\n    let plan = schedule(req.plan, req.region, req.budget, req.database);\n    Response::ok(plan)\n}\n```";

fn turns() -> Vec<(&'static str, String, String)> {
    let t = |id, u: &str, a: String| (id, u.to_string(), a);
    vec![
        t("t1", "Which database do we use for the billing service?",
          format!("Let me check the repo.\nRunning grep -r database billing/\n{CODE}\nWe use Postgres 15 for billing, decided in March.")),
        t("t2", "Remind me what the standup time is.",
          "Sure, let me look.\nThe standup is at 09:30 on weekdays, moved from 10:00.".into()),
        t("t3", "I prefer tabs over spaces in the go projects.",
          "Okay, noted.\nYou prefer tabs for indentation in Go code.".into()),
        t("t4", "What is the deploy command for staging?",
          format!("Running kubectl config current-context\n$ make deploy ENV=staging\n{CODE}\nDeploy staging with `make deploy ENV=staging`; it needs the vpn up.")),
        t("t5", "Correction: the release freeze starts Friday, not Thursday.",
          "Got it.\nThe release freeze starts on Friday; the earlier Thursday date was wrong.".into()),
        t("t6", "Where is the invoice export job configured?",
          format!("Reading config/jobs.yaml\nSearching for export\n{CODE}\nThe invoice export job lives in config/jobs.yaml under the nightly section, running at 02:00.")),
        t("t7", "Which region hosts the analytics warehouse?",
          "Calling tool: describe_warehouse\nOutput: ok\nThe analytics warehouse is in the eu-west region.".into()),
        t("t8", "How long do we keep audit logs?",
          format!("Let me search.\n{CODE}\nAudit logs are retained for 400 days, then archived to cold storage.")),
        t("t9", "thanks", "You're welcome.".into()),
        t("t10", "ok", "Anything else?".into()),
        t("t11", "Who approves vendor contracts above the limit?",
          "I'll check the policy doc.\nContracts above the 20k limit need finance director approval.".into()),
        t("t12", "What is the on-call rotation length?",
          format!("Running pagerctl list\n{CODE}\nOn-call rotations last one week, handover on Monday morning.")),
    ]
}

fn questions() -> Vec<(&'static str, &'static str)> {
    vec![
        ("which database does billing use", "t1"),
        ("when is the standup", "t2"),
        ("tabs or spaces for go", "t3"),
        ("how do I deploy to staging", "t4"),
        ("when does the release freeze start", "t5"),
        ("where is the invoice export job configured", "t6"),
        ("which region is the analytics warehouse in", "t7"),
        ("how long are audit logs retained", "t8"),
        ("who approves large vendor contracts", "t11"),
        ("how long is an on-call rotation", "t12"),
    ]
}

struct Score {
    hit1: f64,
    hit5: f64,
    mrr: f64,
    stored: usize,
    chars: usize,
}

fn score(distilled: bool) -> Score {
    let dir = rung_testkit::TempDir::new(&format!("retain-quality-{distilled}"));
    let provider = Arc::new(Baseline::new(dir.path()));
    let scope = Scope::new("quality");
    let mut by_record = BTreeMap::new();
    let (mut stored, mut chars) = (0, 0);
    for (id, user, assistant) in turns() {
        let mut o = Observation {
            body: Body::Turn { user, assistant },
            attrs: BTreeMap::new(),
        };
        if distilled {
            match distill(&o) {
                Some(d) => o = d,
                None => continue,
            }
        }
        if let Body::Turn { user, assistant } = &o.body {
            chars += user.len() + assistant.len();
        }
        if let Ok(c) = provider.retain(&scope, &o)
            && let Kept::Stored(rid) = c.value
        {
            by_record.insert(rid.as_str().to_string(), id);
            stored += 1;
        }
    }
    let qs = questions();
    let (mut h1, mut h5, mut mrr) = (0.0, 0.0, 0.0);
    for (q, want) in &qs {
        let got = provider.recall(&scope, &Cue::new(*q)).unwrap().value;
        let rank = got
            .iter()
            .position(|r| by_record.get(r.record.id.as_str()) == Some(want));
        if let Some(r) = rank {
            if r == 0 {
                h1 += 1.0;
            }
            if r < 5 {
                h5 += 1.0;
            }
            mrr += 1.0 / (r as f64 + 1.0);
        }
    }
    let n = qs.len() as f64;
    Score {
        hit1: h1 / n,
        hit5: h5 / n,
        mrr: mrr / n,
        stored,
        chars,
    }
}

#[test]
fn distilling_cuts_noise_without_losing_recall() {
    let (raw, d) = (score(false), score(true));
    println!(
        "{:<10} {:>6} {:>6} {:>6} {:>7} {:>6}",
        "mode", "hit@1", "hit@5", "mrr", "stored", "chars"
    );
    for (m, s) in [("raw", &raw), ("distilled", &d)] {
        println!(
            "{:<10} {:>6.2} {:>6.2} {:>6.2} {:>7} {:>6}",
            m, s.hit1, s.hit5, s.mrr, s.stored, s.chars
        );
    }
    assert!(d.mrr >= raw.mrr, "distilling must not lower MRR");
    assert!(d.hit5 >= 0.9, "regression floor");
    assert!(d.stored < raw.stored, "no-content turns are skipped");
    assert!(d.chars * 2 < raw.chars, "retained text at least halves");
}

#[test]
fn notes_and_substantive_short_turns_survive() {
    let note = Observation {
        body: Body::Note { text: "x".into() },
        attrs: BTreeMap::new(),
    };
    assert_eq!(distill(&note), Some(note));
    let o = Observation {
        body: Body::Turn {
            user: "Standup is at 9?".into(),
            assistant: "Yes, 09:30.".into(),
        },
        attrs: BTreeMap::new(),
    };
    assert!(distill(&o).is_some());
}
