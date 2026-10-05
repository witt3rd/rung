//! The host is the memory host. The engine keeps no memory of its own; the
//! host holds one `rung_memory` provider (the `baseline` one by default),
//! recalls at most one block per turn at the boundary (Inject decides when
//! and on what cue), and retains only the agent's own text and the host's
//! mechanical observations (Consolidate decides which). A provider failure
//! is an outcome, never a failed turn.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rung_memory::{
    Body, Cue, MemoryProvider, Observation, RecallReport, RetainReport, Scope, baseline::Baseline,
};

/// How long an identical recall is answered from the last one.
const RECALL_TTL: Duration = Duration::from_secs(60);
/// Per-entry clip of the recent-context cue, in chars.
const CONTEXT_CLIP: usize = 500;
/// Most recent-context entries a cue carries.
const CONTEXT_MAX: usize = 5;

type Last = (Cue, Instant, RecallReport, Option<String>);

pub struct MemoryHost {
    provider: Arc<dyn MemoryProvider>,
    scope: Scope,
    /// The last recall. Identical cue within the TTL and with nothing
    /// retained since is answered from here (no provider round trip).
    last: Mutex<Option<Last>>,
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
            last: Mutex::new(None),
        }
    }

    pub fn name(&self) -> &str {
        self.provider.name()
    }

    /// One recall: the report, the block when something was found, and
    /// whether it came from the last identical recall.
    pub fn recall(
        &self,
        prompt: &str,
        context: Vec<String>,
    ) -> (RecallReport, Option<String>, bool) {
        let skip = context.len().saturating_sub(CONTEXT_MAX);
        let cue = Cue {
            prompt: prompt.chars().take(2_000).collect(),
            context: context
                .into_iter()
                .skip(skip)
                .map(|c| c.chars().take(CONTEXT_CLIP).collect())
                .collect(),
        };
        if let Some((c, at, r, b)) = &*self.last.lock().unwrap()
            && *c == cue
            && at.elapsed() < RECALL_TTL
        {
            return (r.clone(), b.clone(), true);
        }
        let (report, evidence) =
            rung_memory::recall_outcome(self.provider.clone(), self.scope.clone(), cue.clone());
        let block = evidence.map(|e| e.render());
        // A failed recall is not worth repeating from memory.
        if report.status != "unavailable" {
            *self.last.lock().unwrap() = Some((cue, Instant::now(), report.clone(), block.clone()));
        }
        (report, block, false)
    }

    /// Offer one text to keep, with its provenance.
    pub fn retain(&self, text: &str, attrs: BTreeMap<String, String>) -> RetainReport {
        *self.last.lock().unwrap() = None;
        let observation = Observation {
            body: Body::Note { text: text.into() },
            attrs,
        };
        rung_memory::retain_now(self.provider.clone(), self.scope.clone(), observation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rung_memory::{Budget, Capability, Charged, Kept, Miss, Reach, Recalled, Record, RecordId};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default, Debug)]
    struct Counting {
        recalls: AtomicUsize,
        cues: Mutex<Vec<Cue>>,
    }

    impl MemoryProvider for Counting {
        fn name(&self) -> &str {
            "counting"
        }
        fn capability(&self) -> Capability {
            Capability {
                recall: true,
                retain: true,
                tools: false,
            }
        }
        fn budget(&self) -> Budget {
            Budget::default()
        }
        fn recall(&self, scope: &Scope, cue: &Cue) -> Result<Charged<Vec<Recalled>>, Miss> {
            self.recalls.fetch_add(1, Ordering::SeqCst);
            self.cues.lock().unwrap().push(cue.clone());
            Ok(Charged::new(vec![Recalled {
                record: Record {
                    id: RecordId::new("r1"),
                    scope: scope.clone(),
                    text: "the deploy window is Tuesday".into(),
                    observed_at: None,
                    attrs: BTreeMap::new(),
                },
                reach: Reach::Hit { score: 1.0 },
            }]))
        }
        fn retain(&self, _: &Scope, _: &Observation) -> Result<Charged<Kept>, Miss> {
            Ok(Charged::new(Kept::Stored(RecordId::new("r2"))))
        }
    }

    fn host() -> (Arc<Counting>, MemoryHost) {
        let p = Arc::new(Counting::default());
        (p.clone(), MemoryHost::new(p, "s"))
    }

    #[test]
    fn identical_recall_is_answered_once() {
        let (p, m) = host();
        let (_, a, c1) = m.recall("when is the deploy", vec!["x".into()]);
        let (_, b, c2) = m.recall("when is the deploy", vec!["x".into()]);
        assert!(!c1 && c2);
        assert_eq!(a, b);
        assert!(a.is_some());
        assert_eq!(p.recalls.load(Ordering::SeqCst), 1);
        // A different cue goes to the provider.
        m.recall("something else", vec![]);
        assert_eq!(p.recalls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn retain_invalidates_the_cached_recall() {
        let (p, m) = host();
        m.recall("q", vec![]);
        m.retain("a fact", BTreeMap::new());
        let (_, _, cached) = m.recall("q", vec![]);
        assert!(!cached);
        assert_eq!(p.recalls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn context_is_bounded() {
        let (p, m) = host();
        let ctx: Vec<String> = (0..9)
            .map(|i| format!("{i}{}", "z".repeat(5_000)))
            .collect();
        m.recall("q", ctx);
        let cue = p.cues.lock().unwrap()[0].clone();
        assert_eq!(cue.context.len(), CONTEXT_MAX);
        assert!(
            cue.context
                .iter()
                .all(|c| c.chars().count() <= CONTEXT_CLIP)
        );
        // The newest entries survive.
        assert!(cue.context.last().unwrap().starts_with('8'));
    }
}
