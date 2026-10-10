//! Ask a judge typed questions about a typed state; get distributions back.
//!
//! A decision software branches on should not be read out of a generator's
//! prose or out of its stop behaviour. [`Decider`] is the seam instead: an
//! [`Ask`] carries a JSON state and atomic questions, and a backend answers
//! each question with a probability ([`Answer::Noul`]) or a distribution over
//! named options with a confidence ([`Answer::Choice`]). Code composes the
//! answers; the judge is never asked the composite question.
//!
//! A backend that cannot answer says why ([`Undecided`]). It never guesses,
//! and a caller must not turn an `Undecided` into a favourable reading.
//!
//! Three backends:
//!
//! | backend | what it is |
//! |---|---|
//! | [`JevDecider`] | Jev-style decider (default `microsoft/microsoft-decision-1`, legacy `typesafe/jev-1.13`) over the System One API, e.g. through OpenRouter |
//! | [`Recorded`] | replays a recorded exchange from a fixture file; records one when told to |
//! | [`LlmDecider`] | a chat model asked for the same answers. **A stub**: not built yet, always `Undecided` |
//!
//! Only `Noul` and `Choice` are modelled. Jev also has a `Score` primitive;
//! it is left out until a recorded exchange pins its wire shape.

mod jev;
mod llm;
mod recorded;

pub use jev::{DEFAULT_BASE_URL, DEFAULT_MODEL, JevDecider, LEGACY_MODEL};
pub use llm::LlmDecider;
pub use recorded::{Mode, Recorded};

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;
use serde_json::{Value, json};

/// The state a judge reads. Built in code, kept small, and filtered.
pub type State = Value;

/// The two sides of a yes/no question. A high probability means `yes`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoulCriteria {
    #[serde(rename = "true")]
    pub yes: String,
    #[serde(rename = "false")]
    pub no: String,
}

/// One atomic question.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Yes or no; answered with P(yes).
    Noul {
        instructions: String,
        criteria: NoulCriteria,
    },
    /// Pick one option; answered with a probability per option and a confidence.
    Choice {
        instructions: String,
        criteria: BTreeMap<String, String>,
    },
}

impl Question {
    pub fn noul(instructions: &str, yes: &str, no: &str) -> Self {
        Question::Noul {
            instructions: instructions.into(),
            criteria: NoulCriteria {
                yes: yes.into(),
                no: no.into(),
            },
        }
    }

    pub fn choice(instructions: &str, options: &[(&str, &str)]) -> Self {
        Question::Choice {
            instructions: instructions.into(),
            criteria: options
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
    }
}

/// A typed request: one state, every question that might be needed.
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    pub state: State,
    pub questions: BTreeMap<String, Question>,
}

impl Ask {
    /// The wire body for `model`. Holds no credential.
    pub fn body(&self, model: &str) -> Value {
        json!({
            "model": model,
            "state": self.state,
            "questions": self.questions,
        })
    }

    /// Estimated tokens of the body: bytes / 3, a deliberately high estimate.
    pub fn estimated_tokens(&self) -> usize {
        self.body("").to_string().len() / 3
    }
}

/// One answer, typed by the question it answers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Answer {
    Noul {
        p: f64,
    },
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
}

/// What a request cost.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub cost_usd: f64,
}

/// Every question answered.
#[derive(Debug, Clone, PartialEq)]
pub struct Decided {
    /// The versioned model id that actually served the request.
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    pub usage: Usage,
}

impl Decided {
    /// P(yes) for a Noul question.
    pub fn noul(&self, id: &str) -> Option<f64> {
        match self.answers.get(id)? {
            Answer::Noul { p } => Some(*p),
            _ => None,
        }
    }

    /// `(choice, confidence, probabilities)` for a Choice question.
    pub fn choice(&self, id: &str) -> Option<(&str, f64, &BTreeMap<String, f64>)> {
        match self.answers.get(id)? {
            Answer::Choice {
                choice,
                probabilities,
                confidence,
            } => Some((choice.as_str(), *confidence, probabilities)),
            _ => None,
        }
    }
}

/// Why no decision came back. None of these is a reading.
#[derive(Debug, Clone, PartialEq)]
pub enum Undecided {
    /// Transport failure, timeout, or a 5xx that outlasted the retries.
    Unreachable(String),
    RateLimited,
    Overloaded,
    /// 401 / 403, or no key at all.
    Unauthorized(String),
    /// 402: the account has no credit.
    NoCredit,
    /// 400: the request is wrong. A bug on our side.
    Invalid(String),
    /// The request is over the context, measured before sending or reported (413).
    TooLarge,
    /// The response does not answer what was asked.
    Malformed(String),
    /// This backend cannot decide at all.
    Unavailable(String),
}

impl fmt::Display for Undecided {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Undecided::Unreachable(s) => write!(f, "unreachable: {s}"),
            Undecided::RateLimited => write!(f, "rate limited"),
            Undecided::Overloaded => write!(f, "overloaded"),
            Undecided::Unauthorized(s) => write!(f, "unauthorized: {s}"),
            Undecided::NoCredit => write!(f, "no credit"),
            Undecided::Invalid(s) => write!(f, "invalid request: {s}"),
            Undecided::TooLarge => write!(f, "request too large"),
            Undecided::Malformed(s) => write!(f, "malformed response: {s}"),
            Undecided::Unavailable(s) => write!(f, "unavailable: {s}"),
        }
    }
}

/// A judge that answers typed questions.
///
/// `Debug` is required so a decider can sit in a ladder's carry; an
/// implementation must not print a credential there.
pub trait Decider: Send + Sync + fmt::Debug {
    fn decide(&self, ask: &Ask) -> Result<Decided, Undecided>;
}

/// Probabilities are reported to two decimals, so five options can miss 1 by
/// a little. Anything further off is not a distribution.
const SUM_TOLERANCE: f64 = 0.02;

fn unit(x: f64) -> bool {
    x.is_finite() && (0.0..=1.0).contains(&x)
}

/// Read a System One response against the ask it answers.
///
/// Every asked id must be answered with the asked primitive. A Noul `p` and
/// every Choice probability and confidence must lie in [0, 1]; a Choice's
/// probabilities must cover exactly the options and sum to 1; the chosen
/// option must be one of them. Anything else is [`Undecided::Malformed`]: a
/// response that fails validation is never read as an answer.
pub fn read_answers(ask: &Ask, response: &Value) -> Result<Decided, Undecided> {
    let bad = |s: String| Err(Undecided::Malformed(s));
    let Some(given) = response.get("answers").and_then(Value::as_object) else {
        return bad("no `answers` object".into());
    };
    let mut answers = BTreeMap::new();
    for (id, q) in &ask.questions {
        let Some(a) = given.get(id) else {
            return bad(format!("`{id}` not answered"));
        };
        let kind = a.get("type").and_then(Value::as_str).unwrap_or("");
        match q {
            Question::Noul { .. } => {
                if kind != "noul" {
                    return bad(format!("`{id}` answered as `{kind}`, asked as noul"));
                }
                let Some(p) = a.get("noul").and_then(Value::as_f64) else {
                    return bad(format!("`{id}` has no numeric `noul`"));
                };
                if !unit(p) {
                    return bad(format!("`{id}` p={p} outside [0,1]"));
                }
                answers.insert(id.clone(), Answer::Noul { p });
            }
            Question::Choice { criteria, .. } => {
                if kind != "choice" {
                    return bad(format!("`{id}` answered as `{kind}`, asked as choice"));
                }
                let Some(choice) = a.get("choice").and_then(Value::as_str) else {
                    return bad(format!("`{id}` has no `choice`"));
                };
                if !criteria.contains_key(choice) {
                    return bad(format!("`{id}` chose `{choice}`, not an option"));
                }
                let Some(conf) = a.get("confidence").and_then(Value::as_f64) else {
                    return bad(format!("`{id}` has no numeric `confidence`"));
                };
                if !unit(conf) {
                    return bad(format!("`{id}` confidence={conf} outside [0,1]"));
                }
                let Some(probs) = a.get("probabilities").and_then(Value::as_object) else {
                    return bad(format!("`{id}` has no `probabilities`"));
                };
                let mut probabilities = BTreeMap::new();
                for opt in criteria.keys() {
                    match probs.get(opt).and_then(Value::as_f64) {
                        Some(p) if unit(p) => {
                            probabilities.insert(opt.clone(), p);
                        }
                        Some(p) => return bad(format!("`{id}.{opt}` p={p} outside [0,1]")),
                        None => return bad(format!("`{id}` has no probability for `{opt}`")),
                    }
                }
                if probs.len() != criteria.len() {
                    return bad(format!("`{id}` gives probabilities for unasked options"));
                }
                let sum: f64 = probabilities.values().sum();
                if (sum - 1.0).abs() > SUM_TOLERANCE {
                    return bad(format!("`{id}` probabilities sum to {sum}"));
                }
                answers.insert(
                    id.clone(),
                    Answer::Choice {
                        choice: choice.to_string(),
                        probabilities,
                        confidence: conf,
                    },
                );
            }
        }
    }
    let model = response
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let usage = response.get("usage");
    let usage = Usage {
        input_tokens: usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cost_usd: usage
            .and_then(|u| u.get("cost"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
    };
    Ok(Decided {
        model,
        answers,
        usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask() -> Ask {
        let mut questions = BTreeMap::new();
        questions.insert("q".into(), Question::noul("Is it?", "It is.", "It is not."));
        questions.insert(
            "c".into(),
            Question::choice("Which?", &[("a", "A."), ("b", "B.")]),
        );
        Ask {
            state: json!({"x": 1}),
            questions,
        }
    }

    /// The shape of a real Jev response (Phase 0 probe, `calls.jsonl`).
    fn response() -> Value {
        json!({
            "model": "typesafe/jev-1.13-20260917",
            "answers": {
                "q": {"type": "noul", "noul": 0.97},
                "c": {"type": "choice", "choice": "a",
                      "probabilities": {"a": 0.99, "b": 0.01}, "confidence": 0.99}
            },
            "usage": {"input_tokens": 806, "output_tokens": 138, "cost": 3.3852e-05}
        })
    }

    #[test]
    fn a_well_formed_response_reads() {
        let d = read_answers(&ask(), &response()).unwrap();
        assert_eq!(d.model, "typesafe/jev-1.13-20260917");
        assert_eq!(d.noul("q"), Some(0.97));
        let (choice, conf, _) = d.choice("c").unwrap();
        assert_eq!((choice, conf), ("a", 0.99));
        assert_eq!(d.usage.input_tokens, 806);
    }

    #[test]
    fn the_body_holds_questions_in_wire_shape() {
        let b = ask().body("m");
        assert_eq!(b["model"], "m");
        assert_eq!(b["questions"]["q"]["type"], "noul");
        assert_eq!(b["questions"]["q"]["criteria"]["true"], "It is.");
        assert_eq!(b["questions"]["c"]["criteria"]["b"], "B.");
    }

    fn malformed(edit: impl Fn(&mut Value)) -> Undecided {
        let mut r = response();
        edit(&mut r);
        read_answers(&ask(), &r).unwrap_err()
    }

    #[test]
    fn every_malformation_is_undecided() {
        type Edit = Box<dyn Fn(&mut Value)>;
        let cases: Vec<Edit> = vec![
            Box::new(|r| {
                r["answers"].as_object_mut().unwrap().remove("q");
            }),
            Box::new(|r| r["answers"]["q"]["noul"] = json!(1.4)),
            Box::new(|r| r["answers"]["q"]["noul"] = json!("high")),
            Box::new(|r| r["answers"]["q"]["type"] = json!("choice")),
            Box::new(|r| r["answers"]["c"]["probabilities"]["a"] = json!(0.5)),
            Box::new(|r| r["answers"]["c"]["choice"] = json!("z")),
            Box::new(|r| r["answers"]["c"]["confidence"] = json!(-0.1)),
            Box::new(|r| r["answers"]["c"]["probabilities"]["z"] = json!(0.0)),
            Box::new(|r| {
                r["answers"]["c"]["probabilities"]
                    .as_object_mut()
                    .unwrap()
                    .remove("b");
            }),
            Box::new(|r| r["answers"] = json!(null)),
        ];
        for edit in cases {
            assert!(matches!(malformed(edit), Undecided::Malformed(_)));
        }
    }
}
