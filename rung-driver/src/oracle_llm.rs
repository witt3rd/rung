//! An [`Oracle`] backed by a model.
//!
//! The outside, reached over HTTP. `rung_std::llm` puts the call on an arrow;
//! this asks the question and reads the answer back into the two summands Het
//! recognises — a verdict, or a matter raised instead.
//!
//! ## The reading is where the honesty is
//!
//! A model returns prose. Turning prose into a verdict is the one place in this
//! whole arrangement where something could be quietly invented, so the rules
//! are narrow and they all fail the same way:
//!
//! - the reply must be **exactly** one of the forms below. Anything else raises.
//! - a transport failure raises. It is not a `NonConforming` verdict — nothing
//!   was judged, and reporting "does not hold" for a network timeout would put
//!   a fabricated ruling into the record.
//! - a model that says it cannot tell raises. That is what the deferral is for.
//!
//! **Nothing here produces `Conforming` by default.** Every path that is not a
//! model explicitly affirming ends in a raised matter, so the failure mode is
//! always a run that visibly waits rather than one that silently passes.
//!
//! ## What it does not do
//!
//! It does not decide *whether* this model may answer. Capability admitted it
//! to the pool and non-identity is applied per dispatch; by the time `ask` is
//! called those are settled and this only carries the reply.

use crate::config::Backing;
use crate::principal::{Answer, Oracle};
use rung::Raised;
use rung_std::llm::{
    ChatMessage, ContentBlock, DEFAULT_MAX_ATTEMPTS, LlmConfig, LlmRequest, LlmResponse, llmcall,
};

/// How to reach a provider. One config serves a whole population; the model is
/// taken per principal from its [`Backing`].
#[derive(Clone, Debug)]
pub struct Endpoint {
    pub base_url: String,
    pub api_key: String,
    pub timeout_secs: u64,
    pub max_tokens: u32,
    /// Provider-specific reasoning hint, passed through untouched.
    pub reasoning_level: Option<String>,
}

impl Endpoint {
    /// Read an endpoint from the environment.
    ///
    /// Returns `None` when no key is set, and the caller should then wire an
    /// oracle that raises rather than substituting one that agrees.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("RUNG_API_KEY").ok()?;
        Some(Self {
            base_url: std::env::var("RUNG_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com/v1".into()),
            api_key,
            timeout_secs: 120,
            max_tokens: 2048,
            reasoning_level: std::env::var("RUNG_REASONING").ok(),
        })
    }

    fn config_for(&self, model: &str) -> LlmConfig {
        LlmConfig {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            model: model.to_string(),
            timeout_secs: self.timeout_secs,
            max_tokens: self.max_tokens,
            // Judging is not a place for sampling variety.
            temperature: Some(0.0),
            reasoning_level: self.reasoning_level.clone(),
            // The reply is three fixed forms read by `read_reply`, not a
            // schema. Asking the provider to enforce one would move the
            // strictness somewhere this crate cannot check it.
            structured_outputs: false,
            stream_listener: None,
        }
    }
}

/// What a principal is asked, and how its reply is read.
///
/// Separated from the transport so a domain can change the wording without
/// touching the part that decides what counts as an answer.
pub trait Prompt: Send + Sync {
    /// The question put to the model, for this matter.
    fn ask(&self, principal_id: &str, matter: &str) -> String;
}

/// The default prompt: state the matter, and demand one of three replies.
///
/// The third is not politeness. A judge with no way to say *"I cannot settle
/// this"* will say something else instead, and that something else enters the
/// record as a ruling.
pub struct Adjudicate {
    /// What the principal is being asked to judge, in the domain's words.
    pub subject: String,
}

impl Prompt for Adjudicate {
    fn ask(&self, _principal_id: &str, matter: &str) -> String {
        format!(
            "You are judging one claim about the following subject.\n\n\
             SUBJECT\n{}\n\n\
             CLAIM\n{matter}\n\n\
             Reply with exactly one line, and nothing else:\n\
             \x20 HOLDS\n\
             \x20 FAILS <one sentence saying why>\n\
             \x20 CANNOT-SETTLE <one sentence saying what you would need>\n\n\
             Use CANNOT-SETTLE whenever you are not in a position to rule. \
             It is a real answer and is preferred to a guess.",
            self.subject
        )
    }
}

/// An oracle that puts the matter to a model.
pub struct ModelOracle<P: Prompt> {
    endpoint: Endpoint,
    prompt: P,
    /// Reference used when a matter is raised without the model naming one.
    reference: String,
}

impl<P: Prompt> ModelOracle<P> {
    pub fn new(endpoint: Endpoint, prompt: P, reference: impl Into<String>) -> Self {
        Self {
            endpoint,
            prompt,
            reference: reference.into(),
        }
    }

    fn raise(&self, matter: &str, why: impl std::fmt::Display) -> Answer {
        Answer::Raised(Raised::new(
            format!("{}:{why}", self.reference),
            matter.to_string(),
        ))
    }
}

impl<P: Prompt> Oracle for ModelOracle<P> {
    fn ask(&self, id: &str, backing: &Backing, matter: &str) -> Answer {
        let Some(model) = backing.model() else {
            // Declared as reachable out of band. This oracle is not that route.
            return self.raise(matter, "not-reachable-by-model");
        };

        let request = LlmRequest::new(
            self.endpoint.config_for(model),
            vec![ChatMessage::user(self.prompt.ask(id, matter))],
        );

        let mut pending = llmcall::Pending::new(
            request,
            llmcall::Carry {
                call_id: format!("{id}:{matter}"),
            },
        );

        // Drive the ladder, letting its own recover edge handle transient
        // failures. Attempts are bounded by the request, not by this loop.
        let response: LlmResponse = loop {
            match llmcall::step(pending) {
                Ok(llmcall::StepOutcome::Success(s)) => break s.into_payload(),
                Ok(llmcall::StepOutcome::LlmError(e)) => {
                    return self.raise(matter, format!("{:?}", e.into_payload()));
                }
                Err(failed) => {
                    if failed.token.payload.attempts_remaining == 0 {
                        return self.raise(matter, "attempts-exhausted");
                    }
                    pending = llmcall::retry(failed);
                }
            }
        };

        match read_reply(&text_of(&response)) {
            Some(answer) => answer,
            None => self.raise(matter, "unreadable-reply"),
        }
    }
}

/// The model's text, concatenated.
fn text_of(response: &LlmResponse) -> String {
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

/// Read a reply into an answer, or `None` if it is not one of the three forms.
///
/// Strict on purpose. A lenient reader — "it said the word holds somewhere, so
/// call it conforming" — is how a model's hedging becomes a ruling.
pub fn read_reply(text: &str) -> Option<Answer> {
    let line = text.trim().lines().next()?.trim();
    if line == "HOLDS" {
        return Some(Answer::holds());
    }
    if let Some(why) = line.strip_prefix("FAILS") {
        let why = why.trim();
        return Some(Answer::fails(if why.is_empty() {
            "no reason given".to_string()
        } else {
            why.to_string()
        }));
    }
    if let Some(need) = line.strip_prefix("CANNOT-SETTLE") {
        let need = need.trim();
        return Some(Answer::Raised(Raised::new(
            if need.is_empty() {
                "cannot-settle".to_string()
            } else {
                format!("cannot-settle:{need}")
            },
            "judgment",
        )));
    }
    None
}

/// The attempts a request starts with, re-exported so a caller configuring an
/// endpoint does not have to reach into `rung_std`.
pub const MAX_ATTEMPTS: u8 = DEFAULT_MAX_ATTEMPTS;
