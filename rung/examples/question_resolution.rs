//! **The verbs-are-arrows law, proven by running it.**
//!
//! rung is a language for declaring the objects and legal arrows of a category.
//! States are *objects* — inert, no verbs. Transitions are *morphisms* — the
//! only place a verb (compute, judge, call an LLM) may live. That is not a
//! design choice; it is a categorical axiom.
//!
//! We did not read the theory and implement the law. We built a real ladder,
//! tried to fold a live LLM verdict in by *constructing a state from outside*,
//! and the sealed constructor (SPEC.md G2) refused. The type system enforced
//! what a category requires before we had named it. The law arrived from the
//! inside, through the refusal. See `docs/.archive/2026-07-18_HANDOFF_verbs-are-arrows.md`
//! for the full record.
//!
//! This example is that ladder: `Open → Gathered → Evaluated → Synthesized →
//! Folded → {Resolved | Dissolved}`. The `Gathered → Evaluated` transition calls
//! a live LLM (OmniRoute, `localhost:20128`) *inside the transition body* — the
//! only legal place for the call — and narrows the free-text verdict to one bit.
//! The graph remains fully deterministic: the LLM may only pick a *declared,
//! legal* edge. It cannot fabricate a state.
//!
//! A secondary guarantee, also structural: `Folded` is a mandatory rung before
//! any terminal. There is no edge from `Evaluated` to `Resolved`. The day-one
//! bug in `augur/genesis/meta/questions/` (a question filed resolved while the
//! fold was still owed) is a compile error here.
//!
//! ## Running
//!
//! With OmniRoute up:
//! ```bash
//! ROGER_OMNIROUTE_API_KEY=<key> cargo run -p rung --example question_resolution
//! ```
//!
//! Without OmniRoute the `evaluate` transition degrades gracefully to a
//! heuristic (evidence present ⇒ answerable) and records which path it took.

use rung::ladder;

// ── The judge ────────────────────────────────────────────────────────────────
//
// The generative membrane: given a question and its evidence, is the question
// ANSWERABLE or MALFORMED? This is discernment, not computation — exactly where
// an LLM belongs. The judge returns a single bool so the ladder can carry it
// into a declared Outcome. The model cannot fabricate a state; it can only
// nudge which legal edge the graph takes.

mod judge {
    use serde::Deserialize;

    const OMNIROUTE_URL: &str = "http://localhost:20128/v1/chat/completions";
    /// Routes to whatever is currently best/fast. The judge does not care which
    /// model — only that the verdict is legible.
    const MODEL: &str = "auto/best-fast";

    #[derive(Debug, Clone)]
    pub struct Judgment {
        pub answerable: bool,
        /// Kept verbatim for the provenance trace.
        pub raw: String,
    }

    /// Ask the LLM whether `question` is answerable from `evidence`.
    ///
    /// Returns an error rather than a silent default on failure — a fabricated
    /// verdict is exactly the kind of phantom transition rung exists to forbid.
    pub fn evaluate(question: &str, evidence: &[String]) -> Result<Judgment, JudgeError> {
        let api_key = std::env::var("ROGER_OMNIROUTE_API_KEY").map_err(|_| JudgeError::NoKey)?;

        let evidence_block = if evidence.is_empty() {
            "(no evidence found)".to_string()
        } else {
            evidence
                .iter()
                .map(|e| format!("- {e}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let body = serde_json::json!({
            "model": MODEL,
            "stream": false,      // OmniRoute streams by default; we want one JSON object.
            "temperature": 0,
            "max_tokens": 2000,   // best-fast may route to a thinking model; give it headroom.
            "messages": [
                { "role": "system", "content":
                    "You are a research-question triage judge. Decide whether the question \
                     is ANSWERABLE from the given evidence, or MALFORMED (it assumes something \
                     false, is really two questions, or its frame is wrong). \
                     Reply with exactly one word: ANSWERABLE or MALFORMED." },
                { "role": "user", "content":
                    format!("Question: {question}\n\nEvidence gathered:\n{evidence_block}\n\n\
                             Is this question answerable from the evidence, or malformed?") }
            ]
        });

        let resp: ChatResponse = reqwest::blocking::Client::new()
            .post(OMNIROUTE_URL)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .map_err(JudgeError::Http)?
            .error_for_status()
            .map_err(JudgeError::Http)?
            .json()
            .map_err(JudgeError::Http)?;

        let raw = resp
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or(JudgeError::NoContent)?;

        let upper = raw.to_uppercase();
        let answerable = if upper.contains("ANSWERABLE") {
            true
        } else if upper.contains("MALFORMED") {
            false
        } else {
            return Err(JudgeError::Unparseable(raw));
        };

        Ok(Judgment { answerable, raw })
    }

    #[derive(Debug)]
    pub enum JudgeError {
        NoKey,
        Http(reqwest::Error),
        NoContent,
        Unparseable(String),
    }
    impl std::fmt::Display for JudgeError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                JudgeError::NoKey => write!(f, "ROGER_OMNIROUTE_API_KEY not set"),
                JudgeError::Http(e) => write!(f, "OmniRoute request failed: {e}"),
                JudgeError::NoContent => write!(f, "response carried no message content"),
                JudgeError::Unparseable(s) => {
                    write!(f, "verdict was neither ANSWERABLE nor MALFORMED: {s:?}")
                }
            }
        }
    }
    impl std::error::Error for JudgeError {}

    // Just enough of the OpenAI-compatible response shape to read the verdict.
    // BYOT: local-compatible endpoints diverge from the official spec constantly;
    // type only the fields we actually read and leave everything else loose.
    #[derive(Deserialize)]
    pub(super) struct ChatResponse {
        pub choices: Vec<Choice>,
    }
    #[derive(Deserialize)]
    pub(super) struct Choice {
        pub message: Message,
    }
    #[derive(Deserialize)]
    pub(super) struct Message {
        pub content: Option<String>,
    }
}

// ── The ladder ───────────────────────────────────────────────────────────────

/// Evidence gathered toward answering the question.
#[derive(Clone, Debug, PartialEq)]
pub struct Evidence {
    /// The question itself, threaded forward so the judge sees it.
    pub question: String,
    pub sources: Vec<String>,
    pub note: String,
}

/// The verdict of weighing evidence against the resolution condition, carried
/// forward on the spine. The final branch reads it to resolve or dissolve.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub answerable: bool,
    pub reasoning: String,
}

/// The synthesized answer, before it has folded upward into doctrine.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub claim: String,
    /// Where the answering work was recorded.
    pub record: String,
    /// Carried from `Evaluated` so the final branch can honor an early
    /// malformed decision even though the spine is linear.
    pub answerable: bool,
}

/// Proof the fold landed — README law #1 as data.
/// A `Folded` rung cannot exist without naming *what changed and where*.
#[derive(Clone, Debug, PartialEq)]
pub struct FoldReceipt {
    pub answer: Answer,
    pub landed_in: String,
    pub change: String,
}

/// Carried through the terminal `Resolved` verdict.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolution {
    pub landed_in: String,
    pub change: String,
}

/// Carried through the terminal `Dissolved` verdict: why the question was the
/// wrong question. A dissolved question gets a diagnosis, never an answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnosis {
    pub why_malformed: String,
}

ladder!(QuestionResolution {
    carry { question_id: String }

    Open(String)             // the question, one precise sentence
      => Gathered(Evidence)  // evidence collected
      => Evaluated(Outcome)  // answerable, or malformed? (decided here, carried)
      => Synthesized(Answer) // an answer drafted (null draft if malformed)
      => Folded(FoldReceipt) // MANDATORY: the fold happened. No path skips this.
      => {
          Resolved(Resolution)  // terminal: fold landed, law #1 satisfied
          | Dissolved(Diagnosis) // terminal: question was malformed
      }
} impl {
    // Open → Gathered: collect evidence.
    // Threads the question forward — the judge needs it.
    gathered = |open| {
        let carry = open.carry().clone();
        let question = open.payload.clone();
        Gathered::new(
            Evidence {
                question,
                sources: vec![
                    "Fritz, Advances in Mathematics 370 (2020)".into(),
                    "Capucci et al., arXiv:2105.06332 (2021)".into(),
                ],
                note: "Both candidate groundings found at theorem strength.".into(),
            },
            carry,
        )
    },

    // Gathered → Evaluated: THE VERBS-ARE-ARROWS SEAM.
    //
    // The LLM call lives HERE — inside the transition body, on the arrow —
    // because that is the only place a verb may live. States are objects;
    // objects have no verbs. The sealed constructor (G2) makes "call the LLM
    // then build the state from outside" a compile error; the call must flow
    // THROUGH the transition. The model returns free text; we narrow it to one
    // bool; the graph carries that bool into a declared Outcome. Generative
    // reasoning inside a deterministic guardrail.
    //
    // Degrades gracefully if the endpoint is unreachable — records which path.
    evaluated = |gathered| {
        let carry = gathered.carry().clone();
        let (answerable, reasoning) =
            match judge::evaluate(&gathered.payload.question, &gathered.payload.sources) {
                Ok(j) => (j.answerable, format!("LLM verdict (OmniRoute): {}", j.raw.trim())),
                Err(e) => {
                    let a = !gathered.payload.sources.is_empty();
                    (a, format!("LLM unreachable ({e}); fell back to heuristic: \
                                 evidence present ⇒ answerable"))
                }
            };
        Evaluated::new(Outcome { answerable, reasoning }, carry)
    },

    // Evaluated → Synthesized: draft the answer (or null if malformed).
    synthesized = |evaluated| {
        let carry = evaluated.carry().clone();
        let answerable = evaluated.payload.answerable;
        Synthesized::new(
            Answer {
                claim: if answerable {
                    "Markov categories (Fritz) and categorical cybernetics \
                     (Capucci/Hedges) are real, refereed, and have the claimed \
                     properties.".into()
                } else {
                    String::new()
                },
                record: "meta/2026-07-18_the-grounding-checked.md".into(),
                answerable,
            },
            carry,
        )
    },

    // Synthesized → Folded: record where the answer landed in doctrine.
    // For a malformed question this is a no-op fold; the final branch dissolves it.
    folded = |synth| {
        let carry = synth.carry().clone();
        let a = synth.payload.clone();
        Folded::new(
            FoldReceipt {
                landed_in: if a.answerable {
                    "meta/2026-07-17_toward-a-categorical-substrate.md \
                     §'Candidate formal grounding'".into()
                } else {
                    String::new()
                },
                change: if a.answerable {
                    "Struck 'recalled from training, not verified'; \
                     replaced with 'Grounding: verified'.".into()
                } else {
                    String::new()
                },
                answer: a,
            },
            carry,
        )
    },

    // Folded → verdict: resolve or dissolve.
    // This is the only terminal gate. `step` consumes a `Folded` by value —
    // you cannot reach it without walking the spine through every prior rung.
    step = |folded| {
        let r = folded.payload;
        if r.answer.answerable {
            Ok(StepOutcome::Resolved(Resolved::new(Resolution {
                landed_in: r.landed_in,
                change: r.change,
            })))
        } else {
            Ok(StepOutcome::Dissolved(Dissolved::new(Diagnosis {
                why_malformed: "The question assumed a frame that did not survive scrutiny."
                    .into(),
            })))
        }
    },
});

// ── Driver ───────────────────────────────────────────────────────────────────

fn main() {
    println!("══════════════════════════════════════════════════════════");
    println!("  question_resolution — verbs-are-arrows, running");
    println!("══════════════════════════════════════════════════════════\n");

    let question = "Are Markov categories and categorical cybernetics real, \
                    and do they do what the seed claimed?";

    let carry = questionresolution::Carry {
        question_id: "markov-optics-grounding".into(),
    };

    // Open → Gathered
    let open = questionresolution::Open::new(question.into(), carry);
    println!("  [Open]      {question}");

    let gathered = questionresolution::gathered(open);
    println!("  [Gathered]  {} source(s)", gathered.payload.sources.len());

    // Gathered → Evaluated: the LLM call fires INSIDE this transition.
    print!("  [Evaluated] asking OmniRoute inside the transition body … ");
    use std::io::Write;
    std::io::stdout().flush().ok();

    let evaluated = questionresolution::evaluated(gathered);
    println!(
        "{}",
        if evaluated.payload.answerable {
            "ANSWERABLE"
        } else {
            "MALFORMED"
        }
    );
    println!("              {}", evaluated.payload.reasoning);

    // Evaluated → Synthesized → Folded → verdict
    let synthesized = questionresolution::synthesized(evaluated);
    println!("  [Synthesized] answer drafted");

    let folded = questionresolution::folded(synthesized);
    if !folded.payload.landed_in.is_empty() {
        println!("  [Folded]    → {}", folded.payload.landed_in);
    } else {
        println!("  [Folded]    (no-op: malformed question has nowhere to fold)");
    }

    match questionresolution::step(folded) {
        Ok(questionresolution::StepOutcome::Resolved(res)) => {
            let r = res.payload();
            println!("\n  ✓ RESOLVED — the fold landed.");
            println!("      where: {}", r.landed_in);
            println!("      what:  {}", r.change);
        }
        Ok(questionresolution::StepOutcome::Dissolved(d)) => {
            println!("\n  ⊘ DISSOLVED — the question was malformed.");
            println!("      diagnosis: {}", d.payload().why_malformed);
        }
        Err(f) => println!("\n  ⚠ error path: {}", f.error),
    }

    println!("\n  The graph was rung's. The judgment was the LLM's.");
    println!("  The LLM could only pick a legal edge. ⚒️\n");
}
