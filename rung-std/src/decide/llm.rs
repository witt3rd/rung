//! The chat-model backend. **A stub.**
//!
//! When it is built (Jev plan, Phase 5) it will render the questions into a
//! prompt, ask a chat model for JSON with structured outputs, and return
//! point masses: a Noul answered 0 or 1, a Choice with all its mass on one
//! option. Such answers carry no calibrated confidence, and the backend will
//! have to say so in what it returns.
//!
//! Until then it answers nothing: every call is [`Undecided::Unavailable`],
//! so a caller that selects it gets "not checked", never a made-up reading.

use crate::llm::LlmConfig;

use super::{Ask, Decided, Decider, Undecided};

pub struct LlmDecider {
    pub config: LlmConfig,
}

impl std::fmt::Debug for LlmDecider {
    // `LlmConfig` holds a key; print only where it points.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmDecider")
            .field("base_url", &self.config.base_url)
            .field("model", &self.config.model)
            .finish()
    }
}

impl Decider for LlmDecider {
    fn decide(&self, _ask: &Ask) -> Result<Decided, Undecided> {
        Err(Undecided::Unavailable(
            "the chat-model decider is not built yet (Jev plan Phase 5)".into(),
        ))
    }
}
