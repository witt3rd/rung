//! `Scripted`: a test decider that answers by script.
//!
//! Each ask takes the next [`Step`] (the last one repeats once the script
//! runs out), or a policy closure picks one per ask. A step answers every
//! question, answers a chosen subset, says `Undecided`, or stalls first. It
//! drives every desk path offline: answers, every `Undecided` variant,
//! delays past the timeout, and spend.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rung_std::decide::{Answer, Ask, Decided, Decider, Question, Undecided, Usage};

/// What one ask gets.
#[derive(Debug, Clone)]
pub enum Step {
    /// Answer every question: a Noul with `p`, a Choice with `pick` when it
    /// is an option (else the first option).
    Uniform {
        p: f64,
        pick: String,
    },
    /// Answer from a map of question id → answer; unlisted questions are
    /// left unanswered (an incomplete reply).
    Answers(BTreeMap<String, Answer>),
    /// A pseudo-random valid answer to every question, from `seed` and the
    /// ask itself (deterministic).
    Seeded(u64),
    Undecided(Undecided),
    /// Stall for `ms`, then do the inner step.
    Delay(u64, Box<Step>),
    /// Do the inner step and report this cost.
    Cost(f64, Box<Step>),
}

type Policy = dyn Fn(&Ask, u64) -> Step + Send + Sync;

pub struct Scripted {
    steps: Mutex<VecDeque<Step>>,
    last: Mutex<Option<Step>>,
    policy: Option<Arc<Policy>>,
    asked: Mutex<u64>,
    /// Every ask seen, for tests.
    pub log: Mutex<Vec<Ask>>,
    pub model: String,
}

impl std::fmt::Debug for Scripted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scripted")
            .field("model", &self.model)
            .finish()
    }
}

impl Scripted {
    pub fn sequence(steps: Vec<Step>) -> Self {
        Self {
            steps: Mutex::new(steps.into()),
            last: Mutex::new(None),
            policy: None,
            asked: Mutex::new(0),
            log: Mutex::new(Vec::new()),
            model: "scripted".into(),
        }
    }

    pub fn always(step: Step) -> Self {
        Self::sequence(vec![step])
    }

    /// A step per ask from `f(ask, n)`.
    pub fn policy(f: impl Fn(&Ask, u64) -> Step + Send + Sync + 'static) -> Self {
        let mut s = Self::sequence(Vec::new());
        s.policy = Some(Arc::new(f));
        s
    }

    pub fn asks(&self) -> u64 {
        *self.asked.lock().expect("scripted")
    }

    fn next(&self, ask: &Ask) -> Step {
        let n = {
            let mut a = self.asked.lock().expect("scripted");
            *a += 1;
            *a
        };
        if let Some(p) = &self.policy {
            return p(ask, n);
        }
        let mut steps = self.steps.lock().expect("scripted");
        let mut last = self.last.lock().expect("scripted");
        match steps.pop_front() {
            Some(s) => {
                *last = Some(s.clone());
                s
            }
            None => last
                .clone()
                .unwrap_or(Step::Undecided(Undecided::Unavailable(
                    "script ran out".into(),
                ))),
        }
    }

    fn run(&self, step: Step, ask: &Ask, cost: f64) -> Result<Decided, Undecided> {
        let answers = match step {
            Step::Uniform { p, pick } => ask
                .questions
                .iter()
                .map(|(id, q)| (id.clone(), uniform(q, p, &pick)))
                .collect(),
            Step::Answers(a) => a
                .into_iter()
                .filter(|(id, _)| ask.questions.contains_key(id))
                .collect(),
            Step::Seeded(seed) => ask
                .questions
                .iter()
                .map(|(id, q)| (id.clone(), seeded(q, seed, id, &ask.state)))
                .collect(),
            Step::Undecided(u) => return Err(u),
            Step::Delay(ms, inner) => {
                std::thread::sleep(Duration::from_millis(ms));
                return self.run(*inner, ask, cost);
            }
            Step::Cost(c, inner) => return self.run(*inner, ask, c),
        };
        Ok(Decided {
            model: self.model.clone(),
            answers,
            usage: Usage {
                input_tokens: ask.estimated_tokens() as u64,
                cost_usd: cost,
            },
        })
    }
}

fn options(q: &Question) -> Vec<String> {
    match q {
        Question::Choice { criteria, .. } => criteria.keys().cloned().collect(),
        Question::Noul { .. } => Vec::new(),
    }
}

fn choice(opts: &[String], pick: &str) -> Answer {
    let chosen = if opts.iter().any(|o| o == pick) {
        pick.to_string()
    } else {
        opts.first().cloned().unwrap_or_default()
    };
    let probabilities = opts
        .iter()
        .map(|o| (o.clone(), if *o == chosen { 1.0 } else { 0.0 }))
        .collect();
    Answer::Choice {
        choice: chosen,
        probabilities,
        confidence: 0.9,
    }
}

fn uniform(q: &Question, p: f64, pick: &str) -> Answer {
    match q {
        Question::Noul { .. } => Answer::Noul { p },
        Question::Choice { .. } => choice(&options(q), pick),
    }
}

fn mix(mut h: u64, s: &str) -> u64 {
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn seeded(q: &Question, seed: u64, id: &str, state: &serde_json::Value) -> Answer {
    let h = mix(mix(seed ^ 0xcbf2_9ce4_8422_2325, id), &state.to_string());
    let p = (h % 1001) as f64 / 1000.0;
    match q {
        Question::Noul { .. } => Answer::Noul { p },
        Question::Choice { .. } => {
            let opts = options(q);
            let pick = opts
                .get((h >> 16) as usize % opts.len().max(1))
                .cloned()
                .unwrap_or_default();
            choice(&opts, &pick)
        }
    }
}

impl Decider for Scripted {
    fn decide(&self, ask: &Ask) -> Result<Decided, Undecided> {
        self.log.lock().expect("scripted").push(ask.clone());
        let step = self.next(ask);
        self.run(step, ask, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ask() -> Ask {
        let mut questions = BTreeMap::new();
        questions.insert("n".into(), Question::noul("?", "y", "n"));
        questions.insert("c".into(), Question::choice("?", &[("a", "A"), ("b", "B")]));
        Ask {
            state: json!({}),
            questions,
        }
    }

    #[test]
    fn steps_run_in_order_and_the_last_repeats() {
        let s = Scripted::sequence(vec![
            Step::Undecided(Undecided::RateLimited),
            Step::Uniform {
                p: 0.7,
                pick: "b".into(),
            },
        ]);
        assert_eq!(s.decide(&ask()), Err(Undecided::RateLimited));
        let d = s.decide(&ask()).unwrap();
        assert_eq!(d.noul("n"), Some(0.7));
        assert_eq!(d.choice("c").unwrap().0, "b");
        assert!(s.decide(&ask()).is_ok());
        assert_eq!(s.asks(), 3);
    }

    #[test]
    fn seeded_answers_are_valid_and_repeatable() {
        let a = Scripted::always(Step::Seeded(7)).decide(&ask()).unwrap();
        let b = Scripted::always(Step::Seeded(7)).decide(&ask()).unwrap();
        assert_eq!(a.answers, b.answers);
        let p = a.noul("n").unwrap();
        assert!((0.0..=1.0).contains(&p));
    }
}
