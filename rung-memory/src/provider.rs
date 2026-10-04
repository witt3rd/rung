//! The memory provider model: what a provider declares, what it is asked,
//! and how one is chosen by name.
//!
//! A provider answers three things, each optional by its [`Capability`]:
//!
//! - **recall**: records that bear on a [`Cue`] (the prompt and recent
//!   context), read before a turn and shown to the model as data;
//! - **retain**: an [`Observation`] after a turn the product has verified;
//! - **tools**: agent-facing tools the provider contributes (for example
//!   `memory_search`, `memory_retain`).
//!
//! It also declares a [`Budget`] the product holds it to. The ladders in this
//! crate ([`crate::recall`], [`crate::retain`]) are the only callers of
//! `recall` and `retain`, and each ends in a typed outcome with a
//! [`crate::Trace`]: a provider failure is an outcome, never a failed turn.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rung_std::tools::Toolset;

use crate::store::{Charged, Miss, Recalled, RecordId, Scope};

/// What a provider does. The product calls only what is declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    /// Answers [`MemoryProvider::recall`]: the product recalls before a turn.
    pub recall: bool,
    /// Answers [`MemoryProvider::retain`]: the product retains after a turn.
    pub retain: bool,
    /// Contributes agent tools (`MemoryProvider::tools`).
    pub tools: bool,
}

/// What a provider may spend and show.
///
/// - `max_records`, `max_chars`: a recall keeps whole records, best first,
///   while both hold. A record is never cut: clipping can drop a condition
///   or reverse a meaning. A record that does not fit is left out.
/// - `max_cost_usd`: what one recall may cost. A provider that reports more
///   broke its own declaration, and the recall is unavailable
///   ([`crate::Why::Budget`]), not evidence. A retain's cost is reported in
///   its trace; it is not refused after the fact, since the commit is made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    pub max_records: usize,
    pub max_chars: usize,
    pub max_cost_usd: f64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_records: 5,
            max_chars: 4_000,
            max_cost_usd: 0.0,
        }
    }
}

/// What a recall is about: the prompt, and recent context (earlier turns'
/// text, most recent last). A provider chooses what to read from it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cue {
    pub prompt: String,
    pub context: Vec<String>,
}

impl Cue {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            context: Vec::new(),
        }
    }
}

/// What a retain is offered.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    /// A finished turn: the request and the final answer.
    Turn { user: String, assistant: String },
    /// A note the agent asked to keep.
    Note { text: String },
}

/// One offer to retain, with provenance (`session`, `line`, `source`: the
/// product's names; a provider stores them and reads none).
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub body: Body,
    pub attrs: BTreeMap<String, String>,
}

/// What a provider did with an observation.
#[derive(Debug, Clone, PartialEq)]
pub enum Kept {
    /// Committed, under this id.
    Stored(RecordId),
    /// Judged not worth keeping (a duplicate, too large, empty).
    Declined(String),
}

/// What the product tells a provider's tools about the run they serve.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolContext {
    pub scope: Scope,
    /// Provenance for anything the tools retain.
    pub attrs: BTreeMap<String, String>,
}

/// A memory provider. See the module docs.
pub trait MemoryProvider: Send + Sync + fmt::Debug {
    /// The name it is chosen by (`RUNG_MEMORY`, `memory.provider`, `--memory`).
    fn name(&self) -> &str;

    fn capability(&self) -> Capability;

    fn budget(&self) -> Budget;

    /// Records that bear on `cue`, best first. The ladder applies the budget.
    fn recall(&self, scope: &Scope, cue: &Cue) -> Result<Charged<Vec<Recalled>>, Miss>;

    /// Keep `observation`, or decline it.
    fn retain(&self, scope: &Scope, observation: &Observation) -> Result<Charged<Kept>, Miss>;

    /// The agent tools this provider contributes, if any. Tools that read or
    /// write memory go through [`crate::recall`] and [`crate::retain`] too.
    fn toolset(self: Arc<Self>, ctx: &ToolContext) -> Option<Arc<dyn Toolset>> {
        let _ = ctx;
        None
    }
}

/// Where a provider may keep what it stores. The product resolves it; a
/// provider that stores nothing ignores it.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderSettings {
    /// Where to keep a local store.
    pub dir: PathBuf,
    /// The `arg` of `name:arg`, as written.
    pub arg: Option<String>,
    /// The longest one provider call may take; the provider enforces it on
    /// its own transport.
    pub timeout: Duration,
    /// An optional bearer for a provider reached over HTTP. A provider on
    /// another transport ignores it.
    pub token: Option<Token>,
}

/// A secret that never prints: `Debug` shows `Token(..)`, and there is no
/// `Display`. Read it with [`Token::expose`] only to put it on the wire.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// A bearer token: non-empty visible ASCII, no whitespace. The error
    /// never carries the value.
    pub fn new(value: &str) -> Result<Self, String> {
        let v = value.trim();
        if v.is_empty() {
            return Err("is empty".into());
        }
        if !v.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("must be visible ASCII without whitespace".into());
        }
        Ok(Self(v.to_string()))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(..)")
    }
}

/// Builds a provider from its settings.
pub type Factory = fn(&ProviderSettings) -> Result<Arc<dyn MemoryProvider>, String>;

/// Providers by name. `off` and `external` are settings, not providers, and
/// cannot be registered.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    factories: BTreeMap<String, Factory>,
}

impl Registry {
    /// No providers.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The providers rung ships: `baseline` ([`crate::baseline`]).
    pub fn builtin() -> Self {
        let mut r = Self::empty();
        r.register(crate::baseline::NAME, crate::baseline::factory)
            .expect("builtin names are valid");
        r
    }

    /// Add a provider under `name`. A reserved or malformed name is refused.
    pub fn register(&mut self, name: &str, factory: Factory) -> Result<(), String> {
        crate::authority::check_provider_name(name)?;
        self.factories.insert(name.to_string(), factory);
        Ok(())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.factories.contains_key(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.factories.keys().map(String::as_str).collect()
    }

    /// Build the provider called `name`.
    pub fn build(
        &self,
        name: &str,
        settings: &ProviderSettings,
    ) -> Result<Arc<dyn MemoryProvider>, String> {
        match self.factories.get(name) {
            Some(f) => f(settings),
            None => Err(format!(
                "memory provider '{name}' is not registered (off | external | {})",
                self.names().join(" | ")
            )),
        }
    }
}
