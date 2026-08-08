//! `audit_rectify_each` — drive an **audit-rectify pass over every question in
//! the collection** (one cycle per element — a bounded sweep, NOT iterate-until-
//! convergence). For each of the 19 questions it:
//!
//!   1. **audits** the per-question well-posedness screen (Mode A must declare a
//!      resolution; Mode B must name its condition),
//!   2. if defective, the author **proposes** the licensed remedy
//!      (`remedies_for(IllPosed)`),
//!   3. a judge **disposes**, the author **enacts**, the observer **verifies**,
//!   4. reports the per-question outcome.
//!
//! Runs **in-memory** (source not set, so nothing is written to the real
//! docket); a real collection is never touched by this sweep.

use rung_het::{Applies, Verify};
use rung_std::questions::{Filing, JudgmentClass, QuestionEdit, Questions, Scheme, question};
use std::path::Path;

const RUNG: Scheme = Scheme {
    namespace: "rung-questions",
    root: "questions",
    id_prefix: "q",
};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn main() {
    let docket = root().join(".het/rung-questions/questions");
    let mut world = Questions::load(RUNG, &docket);
    // snapshot ids so we can iterate the collection while mutating the model
    let ids: Vec<String> = world.questions.iter().map(|q| q.id.clone()).collect();

    println!(
        "── audit-rectify EACH of {} questions (in-memory sweep) ──────────\n",
        ids.len()
    );
    let mut audited = 0usize;
    let mut defective = 0usize;
    let mut enacted = 0usize;

    for id in &ids {
        audited += 1;
        let q = world.by_id(id).unwrap();
        // 1 · AUDIT — the per-question well-posedness screen
        let ok = match q.filing {
            Filing::WellPosed => question::answerable_is_declared::holds(q)
                .verdict()
                .is_conforming(),
            Filing::IllPosed => question::ill_posed_filings_name_their_condition::holds(q)
                .verdict()
                .is_conforming(),
        };
        if ok {
            println!("  {id:<3}  clean");
            continue;
        }
        defective += 1;

        // 2 · the author receives the (implicit) ill-posed judgment and proposes
        let remedies = world.remedies_for(&JudgmentClass::IllPosed);
        let Some(edit) = remedies.first().cloned() else {
            println!("  {id:<3}  defective, no licensed remedy");
            continue;
        };
        let label = match &edit {
            QuestionEdit::Rewrite { .. } => "REPAIR (Rewrite → Mode A)",
            QuestionEdit::Refile { .. } => "RE-FILE (Refile → Mode B)",
            _ => "other",
        };

        // 3 · dispose (deterministic accept) → 4 · enact → 5 · verify
        let verdict = world.apply(id, &edit);
        let verified = world.confirms(&edit, id);
        match verdict {
            Ok(()) if verified => {
                enacted += 1;
                println!("  {id:<3}  DEFECTIVE → {label}  (enacted, verified)");
            }
            Ok(()) => println!("  {id:<3}  DEFECTIVE → {label}  (enacted, verify FAILED)"),
            Err(e) => println!("  {id:<3}  DEFECTIVE → {label}  (refused: {e:?})"),
        }
    }

    println!(
        "\n  audited {audited} / {total}; {defective} defective; {enacted} enacted (in-memory; real docket untouched).",
        total = ids.len()
    );
}
