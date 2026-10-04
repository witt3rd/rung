//! The host is the memory host. The engine keeps no memory of its own; the
//! host holds one `rung_memory` provider (the `baseline` one by default),
//! recalls at most one block per turn at the boundary (Inject decides when
//! and on what cue), and retains only the agent's own text and the host's
//! mechanical observations (Consolidate decides which). A provider failure
//! is an outcome, never a failed turn.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use rung_memory::{
    Body, Cue, MemoryProvider, Observation, RecallReport, RetainReport, Scope, baseline::Baseline,
};

pub struct MemoryHost {
    provider: Arc<dyn MemoryProvider>,
    scope: Scope,
}

impl std::fmt::Debug for MemoryHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryHost")
            .field("provider", &self.provider.name())
            .field("scope", &self.scope)
            .finish()
    }
}

impl MemoryHost {
    /// The `baseline` provider, kept under `dir`.
    pub fn baseline(dir: &Path, scope: &str) -> Self {
        Self::new(Arc::new(Baseline::new(dir)), scope)
    }

    pub fn new(provider: Arc<dyn MemoryProvider>, scope: &str) -> Self {
        Self {
            provider,
            scope: Scope::new(scope),
        }
    }

    pub fn name(&self) -> &str {
        self.provider.name()
    }

    /// One recall: the report and, when something was found, the block.
    pub fn recall(&self, prompt: &str, context: Vec<String>) -> (RecallReport, Option<String>) {
        let cue = Cue {
            prompt: prompt.chars().take(2_000).collect(),
            context,
        };
        let (report, evidence) =
            rung_memory::recall_outcome(self.provider.clone(), self.scope.clone(), cue);
        (report, evidence.map(|e| e.render()))
    }

    /// Offer one text to keep, with its provenance.
    pub fn retain(&self, text: &str, attrs: BTreeMap<String, String>) -> RetainReport {
        let observation = Observation {
            body: Body::Note { text: text.into() },
            attrs,
        };
        rung_memory::retain_now(self.provider.clone(), self.scope.clone(), observation)
    }
}
