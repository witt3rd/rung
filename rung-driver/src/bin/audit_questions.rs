//! `audit_questions` — a **pure audit** (no rectification) over the whole questions set.
//!
//! ```text
//! cargo run -p rung-driver --bin audit_questions
//! ```
//!
//! Loads the real docket from the `.het/` state sidecar and runs **every
//! decidable sentence** of the questions theory — the five per-question ones
//! (including the well-posedness sort: `answerable_is_declared` for Mode A,
//! `ill_posed_filings_name_their_condition` for Mode B) and the five
//! whole-set ones. It reports every violation and never rectifies anything.
//! Judgmental sentences (`is_well_posed`, `resolution_answers_the_question`)
//! are not settled cold — they are reported as needing a judge, which is the
//! honest boundary of a pure audit.

use rung::Verdict;
use rung_std::questions::{Question, Questions, Scheme, question, questions};

/// rung's own coordinates.
const RUNG: Scheme = Scheme {
    namespace: "rung-questions",
    root: "questions",
    id_prefix: "q",
};

fn docket_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-driver sits in the workspace")
        .join(".het/rung-questions/questions")
}

fn report(q: &Question, settled: &rung::Settled, out: &mut Vec<String>) {
    debug_assert!(
        !settled.consulted_outside(),
        "decidable sentences consult no outside"
    );
    if let Verdict::NonConforming { reason } = settled.verdict() {
        out.push(format!("  {:<4} {:<40} {reason}", q.id, settled.sentence()));
    }
}

fn main() {
    let dir = docket_dir();
    let world = Questions::load(RUNG, &dir);
    println!(
        "── pure audit: {} questions ──────────────────────────────\n",
        world.questions.len()
    );

    // ── per-question decidable sentences ──────────────────────────────
    let mut per_question = Vec::new();
    for q in &world.questions {
        report(
            q,
            &question::id_matches_the_filename::holds(q),
            &mut per_question,
        );
        report(
            q,
            &question::status_is_declared::holds(q),
            &mut per_question,
        );
        report(
            q,
            &question::edge_kinds_are_declared::holds(q),
            &mut per_question,
        );
        // well-posedness sort
        report(
            q,
            &question::answerable_is_declared::holds(q),
            &mut per_question,
        );
        report(
            q,
            &question::ill_posed_filings_name_their_condition::holds(q),
            &mut per_question,
        );
    }
    println!(
        "per-question (decidable × 5): {}",
        if per_question.is_empty() {
            "clean".to_string()
        } else {
            format!("{} violation(s)", per_question.len())
        }
    );
    for v in &per_question {
        println!("{v}");
    }
    println!();

    // ── whole-set decidable sentences ────────────────────────────────
    let settleds = [
        questions::every_dependency_resolves::holds(&world),
        questions::ids_are_unique::holds(&world),
        questions::every_declared_kind_is_lived::holds(&world),
        questions::affects_mirrors_inbound::holds(&world),
        questions::gate_edges_are_acyclic::holds(&world),
    ];
    println!("whole-set (decidable × 5):");
    let mut set_viol = 0;
    for s in settleds {
        match s.verdict() {
            Verdict::Conforming => println!("  {:>40}  ok", s.sentence()),
            Verdict::NonConforming { reason } => {
                set_viol += 1;
                println!("  {:>40}  VIOLATION — {reason}", s.sentence());
            }
        }
    }
    println!();

    // ── judgmental, not settled by a cold audit ──────────────────────
    let mut wp_need = 0;
    let mut wp_ok_mode_a = 0;
    for q in &world.questions {
        if q.declares_resolution() {
            wp_ok_mode_a += 1;
        }
    }
    for q in &world.questions {
        if q.filing.is_well_posed() && !q.declares_resolution() {
            wp_need += 1;
        }
    }
    println!(
        "well-posedness (judgmental, not settled cold): {} file(s) declare a resolution (Mode A, audit-ready); {} file(s) claim Mode A but declare none (cold first cut fails); {} file(s) filed Mode B (escape hatch)",
        wp_ok_mode_a,
        wp_need,
        world.questions.len() - wp_ok_mode_a - wp_need
    );
    println!(
        "  `is_well_posed` (four cuts) and `resolution_answers_the_question` need an outside judge; a pure audit reports them as the screening above, never as settled."
    );

    let total = per_question.len() + set_viol;
    println!("\n── audit complete: {total} decidable violation(s); nothing rectified.");
    std::process::exit(if total > 0 { 1 } else { 0 });
}
