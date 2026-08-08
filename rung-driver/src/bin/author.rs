//! `author` — the REAL author drafts a repaired `answerable:` for a question
//! the judge ruled not well-posed. Free-text generation (not a verdict): the
//! model writes the single, determinate, well-posed resolution condition.

use rung_driver::{Roster, SystemConfig, resolve};
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
            call_id: "author".to_string(),
        },
    );
    let response: LlmResponse = loop {
        match llmcall::step(pending) {
            Ok(llmcall::StepOutcome::Success(s)) => break s.into_payload(),
            Ok(llmcall::StepOutcome::LlmError(_)) => return "«llm-error»".to_string(),
            Err(failed) => {
                if failed.token.payload.attempts_remaining == 0 {
                    return "«attempts-exhausted»".to_string();
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

fn main() {
    let pop =
        Roster::from_yaml(&std::fs::read_to_string(root().join(".het/population.yaml")).unwrap())
            .unwrap();
    let system = SystemConfig::load();
    let author_id = "ds-curator";
    let backing = pop.by_id(author_id).unwrap().backing.clone();
    let config = resolve(&pop, &backing, &system).expect("author resolves via ~/.rung");

    for id in ["q18", "q19"] {
        let text = qtext(id);
        let prompt = format!(
            "You are the author of a question registry. A judge ruled this question NOT well-posed \
             (it reads as a decision or work item, not a determinate question). Repair it: write a \
             SINGLE, determinate, well-posed resolution condition — one reachable, unique, stable, \
             authentic answer the structure can produce. \
             Output ONLY the new `answerable:` text — one or two sentences, no bullets, no preamble.\n\n\
             QUESTION:\n{text}"
        );
        println!("── authoring a repair for {id} …");
        let repair = ask(&config, &prompt);
        println!("  → repaired answerable:\n    {}\n", repair.trim());
    }
    println!("(draft only — nothing was written yet)");
}
