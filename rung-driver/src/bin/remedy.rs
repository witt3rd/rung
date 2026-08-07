//! `remedy` — judge, then show what the author decides post-judgment
//! (`remedy-presupposes-the-judgment`).
//!
//! For q18/q19: a **real** judge (OpenRouter, via `~/.rung/`) rules on
//! well-posedness; the author *receives* that judgment; `remedies_for` is the
//! licensed set; the author re-proposes `Refile → Mode B` and it is enacted
//! in-memory (the real files are untouched).

use rung_driver::{ModelOracle, Oracle, Roster, WellPosedAdjudicate};
use rung_het::Applies;
use rung_std::questions::{JudgmentClass, QuestionEdit, Questions, Scheme};

const RUNG: Scheme = Scheme {
    namespace: "rung-questions",
    root: "questions",
    id_prefix: "q",
};

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn qtext(id: &str) -> String {
    std::fs::read_dir(root().join(".het/rung-questions/questions"))
        .unwrap()
        .filter_map(|e| e.ok().map(|d| d.path()))
        .filter(|p| p.is_file())
        .find_map(|p| {
            let n = p.file_name().and_then(|x| x.to_str()).unwrap_or("");
            n.starts_with(&format!("{id}-"))
                .then_some(std::fs::read_to_string(&p).unwrap())
        })
        .expect("question file")
}

fn main() {
    let pop = Roster::from_yaml(
        &std::fs::read_to_string(root().join(".het/rung-questions/population.yaml")).unwrap(),
    )
    .unwrap();
    let mut world = Questions::load(RUNG, &root().join(".het/rung-questions/questions"));
    let judge_id = "gpt-interrogator";
    let backing = pop.by_id(judge_id).unwrap().backing.clone();

    println!("── judge, then the author decides ──────────────────────────────\n");
    for id in ["q18", "q19"] {
        let oracle = ModelOracle::new(
            pop.clone(),
            WellPosedAdjudicate { subject: qtext(id) },
            "remedy",
        );
        println!("– {id}");
        match oracle.ask(
            judge_id,
            &backing,
            "Does this question meet the well-posedness standard above?",
        ) {
            rung_driver::Answer::Verdict(rung::Verdict::Conforming) => {
                println!("  judge: WELL-POSED — no remedy is licensed.\n")
            }
            rung_driver::Answer::Verdict(rung::Verdict::NonConforming { reason }) => {
                println!("  judge: NOT well-posed — {reason}");
                // the author receives the judgment
                let class = JudgmentClass::IllPosed;
                let remedies = world.remedies_for(&class);
                println!("  author  : remedies_for(IllPosed) =");
                for r_ in &remedies {
                    println!("             • {r_:?}");
                }
                // a real choice: repair to conform (primary) or demote to Mode B
                let Some(QuestionEdit::Rewrite { answerable }) = remedies.first() else {
                    continue;
                };
                println!("  -> author chooses REPAIR (Rewrite {id}; stays Mode A)");
                world
                    .apply(
                        id,
                        &QuestionEdit::Rewrite {
                            answerable: answerable.clone(),
                        },
                    )
                    .expect("enact applies");
                let q = world.questions.iter().find(|q| q.id == id).unwrap();
                println!(
                    "  enacted: filing={:?}, answerable=`{:.40}…`, ill_posed={:?}",
                    q.filing,
                    q.answerable.as_deref().unwrap_or(""),
                    q.ill_posed
                );
                println!();
            }
            rung_driver::Answer::Raised(r) => {
                println!("  judge deferred: {} ({})\n", r.reference(), r.matter())
            }
        }
    }
    println!("(in-memory; the real question files were not touched)");
}
