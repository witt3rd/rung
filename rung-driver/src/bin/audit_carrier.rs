//! `audit_carrier` — audit a carrier through the generic driver, from config.
//!
//! The driver reads an instance `config.yaml` (Q18), builds the [`Carrier`]
//! it declares, enumerates its subjects extensionally, and audits them with the
//! governing theory. This is the shape that replaces hand-rolled fragments: the
//! *walking* is the generic carrier, the *audit* is the theory's sentences, and
//! the driver never knows either's content.
//!
//! ```text
//! cargo run -p rung-driver --bin audit_carrier -- --config instance.yaml
//! ```
//!
//! The theory is selected by the config's `theory:` field; a theory crate would
//! instantiate this driver with its own audit logic. For demonstration, the
//! `rung-question` theory audits each subject (a question file) with its four
//! per-question decidable sentences.

use rung_driver::Instance;
use rung_het::Verdict;
use rung_std::questions::{Question, Scheme, question};
use std::path::Path;

/// rung's own questions coordinates — a Scheme is static strings, so the
/// theory crate knows its own; the driver holds it alongside the instance.
const RUNG: Scheme = Scheme {
    namespace: "rung-questions",
    root: "questions",
    id_prefix: "q",
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let config = args
        .iter()
        .position(|a| a == "--config")
        .and_then(|i| args.get(i + 1))
        .expect("usage: audit_carrier --config <instance.yaml>");
    let text = std::fs::read_to_string(config).expect("config readable");
    let inst = Instance::from_yaml(&text).expect("config parses");

    // a config's relative carrier path is relative to the config itself
    let base = Path::new(&config)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match inst.theory.as_str() {
        "rung-question" => run_question_audit(&inst, base),
        other => {
            eprintln!(
                "audit_carrier: unsupported theory `{other}` — a theory crate would supply one"
            );
            std::process::exit(2);
        }
    }
}

fn run_question_audit(inst: &Instance, base: &Path) {
    let carrier = inst
        .build_carrier_at(base)
        .expect("carrier builds from config");
    println!(
        "── audit through the carrier ───────────────────────────────
"
    );
    println!("  theory  : {}", inst.theory);
    println!("  carrier : {}", carrier.id());

    let mut audited = 0usize;
    let mut violations = 0usize;
    for subject in carrier.iter() {
        let id = match subject {
            Ok(id) => id,
            Err(e) => {
                eprintln!("  carrier fault: {e}");
                violations += 1;
                continue;
            }
        };
        let text = match carrier.read(&id) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  {}: unreadable: {e}", id);
                violations += 1;
                continue;
            }
        };
        // derive the question's coordinates from its carrier id (a file path)
        let path = Path::new(id.as_str());
        let dir = path
            .parent()
            .and_then(|d| d.file_name())
            .and_then(|d| d.to_str())
            .unwrap_or("");
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let Some(q) = Question::parse(RUNG, &text, dir, stem) else {
            eprintln!("  {id}: not a parseable question");
            violations += 1;
            continue;
        };
        audited += 1;
        for (name, settled) in [
            (
                "id_matches_the_filename",
                question::id_matches_the_filename::holds(&q),
            ),
            (
                "status_is_declared",
                question::status_is_declared::holds(&q),
            ),
            (
                "status_agrees_with_the_directory",
                question::status_agrees_with_the_directory::holds(&q),
            ),
            (
                "edge_kinds_are_declared",
                question::edge_kinds_are_declared::holds(&q),
            ),
        ] {
            if let Verdict::NonConforming { reason } = settled.verdict() {
                println!("  {:<42} violates {name}: {reason}", q.id);
                violations += 1;
            }
        }
    }

    println!();
    println!("  audited {audited} subject(s) through the carrier; {violations} violation(s).");
    std::process::exit(if violations > 0 { 1 } else { 0 });
}
