//! `enact_mode_b` — re-file q18/q19 to Mode B *through the driver*: load the
//! real docket, apply the typed `Refile` edit (the remedy the real judgments
//! license), and `persist` — the driver writes the files itself.

use rung_het::Applies;
use rung_std::questions::{Filing, QuestionEdit, Questions, Scheme};
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
const COND: &str = "As posed this is a design decision and work item, not a determinate question: \
 it specifies what to build or decide rather than naming a fact the structure determines. The real \
 judge refused it repeatedly on the authentic (and, here, the unique) cut, and authorial repair \
 attempts were likewise refused. Filed Mode B: tracked as a decision/work item, not claimed as a \
 well-posed question.";

fn main() {
    let mut world = Questions::load(RUNG, &root().join(".het/rung-questions/questions"));
    for id in ["q18", "q19"] {
        let before = world.by_id(id).unwrap().filing;
        world
            .apply(
                id,
                &QuestionEdit::Refile {
                    to: Filing::IllPosed,
                    condition: Some(COND.into()),
                },
            )
            .expect("the edit applies");
        let path = world
            .persist(id)
            .expect("the driver persists to the carrier");
        let on_disk = std::fs::read_to_string(&path).unwrap();
        let reaffirmed = on_disk.contains("filing: ill-posed") && on_disk.contains("ill_posed: |");
        println!(
            "{id}: {before:?} → written by the driver → filing: ill-posed on disk = {reaffirmed}; \
             body intact = {}",
            on_disk.contains("# Q1")
        );
        println!("  wrote {}", path.display());
    }
}
