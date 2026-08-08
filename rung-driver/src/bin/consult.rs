//! `consult` — dispatch a **real** judge through the `ModelOracle` (the
//! machinery that has never actually run). No test double: it reaches the real
//! model through the system provider catalog (`~/.rung/`), via OpenRouter, and
//! renders whatever the outside says.
//!
//! ```text
//! cargo run -p rung-driver --bin consult
//! ```

use rung_driver::{ModelOracle, Oracle, Roster, WellPosedAdjudicate};

fn docket() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".het/rung-questions/questions")
}

fn question_text(id: &str) -> String {
    std::fs::read_dir(docket())
        .unwrap()
        .find_map(|e| {
            let p = e.unwrap().path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with(&format!("{id}-")).then_some(p)
        })
        .and_then(|p| std::fs::read_to_string(p).ok())
        .expect("question file")
}

fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let pop =
        Roster::from_yaml(&std::fs::read_to_string(root.join(".het/population.yaml")).unwrap())
            .expect("population parses");

    let judge_id = "gpt-interrogator";
    let backing = pop.by_id(judge_id).expect("declared").backing.clone();

    println!("── dispatching a real judge ──────────────────────────────────────");
    println!("  principal : {judge_id}");
    println!("  backing   : {backing:?}   (provider omitted -> DEFAULT = OpenRouter)");

    let claim = "Does this question meet the well-posedness standard above?";
    for q in ["q18", "q19"] {
        let oracle = ModelOracle::new(
            pop.clone(),
            WellPosedAdjudicate {
                subject: question_text(q),
            },
            "consult",
        );
        println!("\n  — {q}: asking the real model… (this calls OpenRouter)");
        match oracle.ask(judge_id, &backing, claim) {
            rung_driver::Answer::Verdict(v) => println!("  {q} → {v:?}"),
            rung_driver::Answer::Raised(r) => {
                println!("  {q} → raised: {} ({})", r.reference(), r.matter())
            }
        }
    }
}
