//! `repair_judge` — put the author's repaired `answerable:` back before the
//! real judge; only a well-posed repair would be enacted.

use rung::Verdict;
use rung_driver::{ModelOracle, Oracle, Roster, WellPosedAdjudicate};
use std::path::Path;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn qpath(id: &str) -> std::path::PathBuf {
    std::fs::read_dir(root().join(".het/rung-questions/questions"))
        .unwrap()
        .filter_map(|e| e.ok().map(|d| d.path()))
        .filter(|p| p.is_file())
        .find(|p| {
            p.file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .starts_with(&format!("{id}-"))
        })
        .unwrap()
}
/// Replace the `answerable:` block (up to the closing `---`) with the repair.
fn repaired(id: &str, new_answerable: &str) -> String {
    let t = std::fs::read_to_string(qpath(id)).unwrap();
    let start = t.find("answerable:").unwrap();
    let end = t.find("\n---").unwrap();
    let head = &t[..start];
    let tail = &t[end..]; // from '\n---' to EOF
    format!("{head}answerable: |{new_answerable}{tail}")
}

fn main() {
    let pop = Roster::from_yaml(
        &std::fs::read_to_string(root().join(".het/rung-questions/population.yaml")).unwrap(),
    )
    .unwrap();
    let judge_id = "gpt-interrogator";
    let backing = pop.by_id(judge_id).unwrap().backing.clone();

    let repairs = [
        (
            "q18",
            "  The state sidecar convention is: each carrier instance keeps its loop state in a single declared home directory (`.het/<instance>/`) containing `config.yaml` (theory, scheme, carrier location, population scope) and `state/`; shared `population.yaml` and `commissions.yaml` may be shared one level up or bespoke, and the instance's `config.yaml` declares which applies.",
        ),
        (
            "q19",
            "  The generic driver is the single theory-blind engine, instantiated by a theory crate through the Q18 config, that owns the audit-rectify loop, judgment/authoring discipline, question resolution, bookkeeping, suspend/resume, principal dispatch, and carrier walking.",
        ),
    ];
    for (id, repair) in repairs {
        let q = repaired(id, repair);
        let oracle = ModelOracle::new(
            pop.clone(),
            WellPosedAdjudicate { subject: q },
            "repair-judge",
        );
        println!("── judge of the repaired {id} …");
        match oracle.ask(
            judge_id,
            &backing,
            "Does this repaired question meet the well-posedness standard above?",
        ) {
            rung_driver::Answer::Verdict(Verdict::Conforming) => {
                println!("  ✅ WELL-POSED — this repair is enactable.\n")
            }
            rung_driver::Answer::Verdict(Verdict::NonConforming { reason }) => {
                println!("  ❌ still not well-posed: {reason}\n")
            }
            rung_driver::Answer::Raised(r) => {
                println!("  ⏸ deferred: {} ({})\n", r.reference(), r.matter())
            }
        }
    }
}
