//! `real_repair_all` — repair the whole defective collection with the REAL
//! author and the REAL judge. No placeholder answerable, no accept-double:
//! for each Mode-A-unanchored question, a real model (ds-curator) drafts the
//! actual repaired `answerable:`, and a real model (gpt-interrogator) rules on
//! whether the repaired question is now well-posed (the four cuts, via
//! WellPosedAdjudicate). In-memory — nothing is persisted.

use rung::Verdict;
use rung_driver::{ModelOracle, Oracle, Roster, SystemConfig, WellPosedAdjudicate, resolve};
use rung_std::llm::{ChatMessage, ContentBlock, LlmRequest, LlmResponse, llmcall};
use std::path::Path;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
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

fn ask(config: &rung_std::llm::LlmConfig, prompt: &str) -> String {
    let request = LlmRequest::new(config.clone(), vec![ChatMessage::user(prompt.to_string())]);
    let mut pending = llmcall::Pending::new(
        request,
        llmcall::Carry {
            call_id: "a".into(),
        },
    );
    let response: LlmResponse = loop {
        match llmcall::step(pending) {
            Ok(llmcall::StepOutcome::Success(s)) => break s.into_payload(),
            Ok(llmcall::StepOutcome::LlmError(_)) => return "«err»".into(),
            Err(failed) => {
                if failed.token.payload.attempts_remaining == 0 {
                    return "«exhausted»".into();
                }
                pending = llmcall::retry(failed);
            }
        }
    };
    response
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn with_answerable(original: &str, draft: &str) -> String {
    let lines: Vec<&str> = original.split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let mut inserted = false;
    for l in lines {
        if l.trim() == "---" && !inserted && out.iter().any(|x| x.starts_with("id:")) {
            out.push("answerable: |".into());
            for piece in draft.split('\n') {
                out.push(format!("  {piece}"));
            }
            inserted = true;
        }
        out.push(l.to_string());
    }
    if !inserted {
        out.push(format!("answerable: |\n  {draft}"));
    }
    out.join("\n")
}

fn main() {
    let pop = Roster::from_yaml(
        &std::fs::read_to_string(root().join(".het/rung-questions/population.yaml")).unwrap(),
    )
    .unwrap();
    let system = SystemConfig::load();
    let author_id = "ds-curator";
    let a_back = pop.by_id(author_id).unwrap().backing.clone();
    let a_cfg = resolve(&pop, &a_back, &system).expect("author resolves");
    let judge_id = "gpt-interrogator";
    let j_back = pop.by_id(judge_id).unwrap().backing.clone();

    let ids = [
        "q1", "q10", "q11", "q12", "q13", "q14", "q16", "q17", "q2", "q3", "q4", "q5", "q6", "q7",
        "q8", "q9",
    ];
    let mut passed = 0;
    println!(
        "── REAL repair over all {} defective questions ──────────────────\n",
        ids.len()
    );
    for id in ids {
        let original = qtext(id);
        let prompt = format!(
            "You are the author of a question registry. A judge ruled this question not well-posed: \
             it is a Mode A question that declares no resolution, or reads as a decision/work item. \
             Repair it: write the SINGLE, determinate, well-posed resolution condition — one reachable, \
             unique, stable, authentic answer the structure can produce. Output ONLY the new `answerable:` \
             text, one or two sentences, no preamble.\n\nQUESTION:\n{original}"
        );
        let draft = ask(&a_cfg, &prompt).trim().to_string();
        let repaired = with_answerable(&original, &draft);
        let oracle = ModelOracle::new(
            pop.clone(),
            WellPosedAdjudicate { subject: repaired },
            "repair",
        );
        let verdict = oracle.ask(
            judge_id,
            &j_back,
            "Does this repaired question meet the well-posedness standard above?",
        );
        match verdict {
            rung_driver::Answer::Verdict(Verdict::Conforming) => {
                passed += 1;
                println!("  {id:<3} PASS  author: {}", short(&draft));
            }
            rung_driver::Answer::Verdict(Verdict::NonConforming { reason }) => println!(
                "  {id:<3} FAIL  ({}) author: {}",
                short(&reason),
                short(&draft)
            ),
            rung_driver::Answer::Raised(r) => println!(
                "  {id:<3} DEFER ({}) author: {}",
                r.reference(),
                short(&draft)
            ),
        }
    }
    println!(
        "\n  {passed}/{} repairs passed the real judge (in-memory; nothing persisted).",
        ids.len()
    );
}
fn short(s: &str) -> String {
    let s = s.trim();
    if s.len() > 60 {
        s[..60].to_string() + "…"
    } else {
        s.to_string()
    }
}
