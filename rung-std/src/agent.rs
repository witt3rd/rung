//! Canonical **agent** ladder — the fourth building block in rung-std.
//!
//! ## What this is
//!
//! A one-turn agent loop modeled as a [`ladder!`](rung::ladder) over the
//! single-call [`llm`](crate::llm) block. Drive the LLM, dispatch tools,
//! accumulate conversation history, and iterate until the model finishes,
//! the budget is exhausted, or the iteration limit is reached.
//!
//! ## How this ladder uses rung's guarantees
//!
//! - **G2 (sealed construction).** Only the entry rung `Idle::new` is
//!   public. `EndTurn` and other verdicts are built by `step` inside the
//!   module — no caller can fabricate a terminal outcome
//!   (`the-law`).
//!
//! - **G5 (carry immutability).** [`LoopState`] and the tool roster ride
//!   in the immutable carry. The recover edge threads them forward
//!   unchanged.
//!
//! - **G4 (no silent drop).** Every token — `Idle`, `Calling`, `EndTurn`,
//!   and all continue-arm verdicts — is `#[must_use]`.
//!
//! - **G7/G9 (error-path recover — unguarded).** `api_retry: Failed(Calling) => Calling`
//!   re-enters with the unconsumed token. It is deliberately unguarded:
//!   a retry after a transient config or tool failure may legitimately
//!   re-use the identical token.
//!
//! - **G11 (terminal payloads).** `EndTurn(AgentResult)` carries the
//!   structured result out through the verdict.
//!
//! ## Membership criteria (rung-std)
//!
//! This ladder is rung-std because:
//! 1. It recurs across independent domain projects (inner-loop, het-rs, …)
//! 2. Its canonical shape (iteration, budget, grace, tool dispatch) is
//!    domain-generic — no caller-specific knowledge is embedded.
//! 3. It sits directly on the [`llm`](crate::llm) block: the single-call
//!    ladder drives the model; the agent ladder drives the single-call
//!    ladder repeatedly.

use rung::ladder;

use crate::llm::{
    ChatMessage, ContentBlock, DEFAULT_MAX_ATTEMPTS, LlmConfig, LlmFailure, LlmRequest, StopReason,
};
use crate::tools::Toolset;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// ─── Carry data ────────────────────────────────────────────────────────────────

/// Loop-control counters threaded through every rung as immutable carry (G5).
#[derive(Clone, Debug)]
pub struct LoopState {
    pub api_call_count: u32,
    pub max_iterations: u32,
    pub budget_remaining: u32,
    /// One-shot: allow a single extra call when budget is zero.
    pub grace_call: bool,
}

impl LoopState {
    pub fn new(max_iterations: u32, budget: u32) -> Self {
        Self {
            api_call_count: 0,
            max_iterations,
            budget_remaining: budget,
            grace_call: true,
        }
    }
}

/// Produce the loop state for the next iteration — after a successful LLM call
/// that elected to continue. Increments the call counter, optionally decrements
/// budget (unless this was a grace call), and consumes the grace flag only when
/// the grace call was actually taken.
fn next_state(prev: &LoopState, is_grace: bool) -> LoopState {
    let mut next = prev.clone();
    next.api_call_count += 1;
    if !is_grace {
        next.budget_remaining = next.budget_remaining.saturating_sub(1);
    } else {
        next.grace_call = false; // grace call consumed, one-shot exhausted
    }
    next
}

// ─── Payloads ──────────────────────────────────────────────────────────────────

/// The conversation context threaded from iteration to iteration.
///
/// Each `Calling` rung holds one of these. When the step body returns a continue
/// arm (`Iterate -> Calling` / `GraceIterate -> Calling`), it builds a new
/// `Thread` with the accumulated conversation history so the next LLM call sees
/// all prior messages.
#[derive(Clone, Debug)]
pub struct Thread {
    pub system_prompt: String,
    /// Accumulated conversation — system, user, assistant, and tool-result
    /// messages.
    pub messages: Vec<ChatMessage>,
}

/// Carried out through the `EndTurn` terminal verdict.
#[derive(Debug)]
pub struct AgentResult {
    pub final_response: String,
    pub api_calls_made: u32,
}

/// Iteration limit reached before the model finished.
#[derive(Debug)]
pub struct LimitHit {
    pub api_calls_made: u32,
}

/// Budget exhausted (and no grace call available).
#[derive(Debug)]
pub struct BudgetHit {
    pub api_calls_made: u32,
}

/// Reserved for future signal handling.
#[derive(Debug)]
pub struct Interrupt {
    pub at_api_call: u32,
}

/// The model refused or an unrecoverable error occurred.
#[derive(Debug)]
pub struct Filtered {
    pub reason: String,
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

/// Generate a per-call tracing id.
fn fresh_call_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("call-{n:04}")
}

/// Extract concatenated text from all [`ContentBlock::Text`] blocks.
fn response_text(blocks: &[ContentBlock]) -> Option<String> {
    let parts: Vec<&str> = blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

// ─── Ladder ────────────────────────────────────────────────────────────────────

ladder!(AgentLoop {
    carry {
        state: LoopState,
        tools: Arc<dyn Toolset>,
        config: LlmConfig,
    }

    Idle(Thread)
      => Calling(Thread)
      => {
          EndTurn(AgentResult)
          | MaxIterations(LimitHit)
          | BudgetExhausted(BudgetHit)
          | Interrupted(Interrupt)
          | ContentFiltered(Filtered)
          | Iterate -> Calling
          | GraceIterate -> Calling
      }

    // Error-path recovery (G9): unguarded — a retry after an infrastructure
    // failure may legitimately re-use the same token.
    recover {
        api_retry: Failed(Calling) => Calling
    }
} impl {
    // Idle → Calling: thread the Thread payload and carry forward.
    calling = |idle| {
        let carry = idle.carry().clone();
        Calling::new(idle.payload, carry)
    },

    // Calling → verdict. THE verb on the arrow: the LLM API call lives here,
    // inside the transition body (the-law).
    step = |calling| {
        let state = calling.carry().state.clone();
        let thread = calling.payload.clone();  // will be consumed below
        let config = calling.carry().config.clone();
        let tools = calling.carry().tools.clone();  // Arc<dyn Toolset>

        // ── termination gates (before the LLM call) ──────────────────────
        if state.api_call_count >= state.max_iterations {
            return Ok(StepOutcome::MaxIterations(MaxIterations::new(
                LimitHit { api_calls_made: state.api_call_count },
            )));
        }

        let is_grace = state.budget_remaining == 0 && state.grace_call;
        if state.budget_remaining == 0 && !state.grace_call {
            return Ok(StepOutcome::BudgetExhausted(BudgetExhausted::new(
                BudgetHit { api_calls_made: state.api_call_count },
            )));
        }

        // ── LLM call ─────────────────────────────────────────────────────
        let call_id = fresh_call_id();
        eprintln!(
            "[rung-std] {call_id}: starting LLM call (attempt {}/{})",
            state.api_call_count + 1,
            state.max_iterations
        );

        let request = LlmRequest {
            config: config.clone(),
            messages: thread.messages.clone(),
            tools: tools.definitions(),
            attempts_remaining: DEFAULT_MAX_ATTEMPTS,
        };

        // Drive the LlmCall ladder to completion, handling its retries internally.
        let mut llm_pending = crate::llm::llmcall::Pending::new(
            request,
            crate::llm::llmcall::Carry { call_id: call_id.clone() },
        );

        let response: crate::llm::LlmResponse = loop {
            match crate::llm::llmcall::step(llm_pending) {
                Ok(crate::llm::llmcall::StepOutcome::Success(s)) => {
                    break s.into_payload();
                }
                Ok(crate::llm::llmcall::StepOutcome::LlmError(e)) => {
                    let failure = e.into_payload();
                    let reason = match failure {
                        LlmFailure::Auth(msg) => format!("auth: {msg}"),
                        LlmFailure::Config(msg) => format!("config: {msg}"),
                        LlmFailure::MaxRetries { last_error } => {
                            format!("max retries exhausted: {last_error}")
                        }
                    };
                    return Ok(StepOutcome::ContentFiltered(ContentFiltered::new(
                        Filtered { reason },
                    )));
                }
                Err(f) => {
                    eprintln!("[rung-std] {call_id}: retryable LLM error — {}", f.error);
                    llm_pending = crate::llm::llmcall::retry(f);
                }
            }
        };

        // ── classify the response ────────────────────────────────────────
        let next = next_state(&state, is_grace);

        match response.stop_reason {
            StopReason::EndTurn | StopReason::MaxTokens | StopReason::StopSequence => {
                let text = response_text(&response.content)
                    .unwrap_or_else(|| "(no text in response)".into());
                eprintln!("[rung-std] {call_id}: end_turn — {text:.120}");
                Ok(StepOutcome::EndTurn(EndTurn::new(AgentResult {
                    final_response: text,
                    api_calls_made: next.api_call_count,
                })))
            }

            StopReason::ToolUse => {
                let mut assistant_blocks: Vec<crate::llm::MessageContentBlock> = Vec::new();
                let mut tool_count = 0u32;
                let mut tool_result_blocks: Vec<crate::llm::MessageContentBlock> = Vec::new();

                for block in &response.content {
                    match block {
                        ContentBlock::Text { text } => {
                            assistant_blocks.push(crate::llm::MessageContentBlock::Text {
                                text: text.clone(),
                            });
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            eprintln!("[rung-std] {call_id}: executing tool '{name}'");
                            assistant_blocks.push(
                                crate::llm::MessageContentBlock::ToolUse {
                                    id: id.clone(),
                                    name: name.clone(),
                                    input: input.clone(),
                                },
                            );
                            let result = tools
                                .execute(name, input)
                                .unwrap_or_else(|e| format!("error: {e}"));
                            eprintln!("[rung-std] {call_id}:   -> {:.120}", result);
                            tool_result_blocks.push(
                                crate::llm::MessageContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: result,
                                },
                            );
                            tool_count += 1;
                        }
                        ContentBlock::Thinking { .. } => {
                            // Thinking is internal — not re-submitted.
                        }
                    }
                }

                let mut updated_messages = thread.messages.clone();
                if !assistant_blocks.is_empty() {
                    updated_messages
                        .push(ChatMessage::assistant_with_blocks(assistant_blocks));
                }
                if !tool_result_blocks.is_empty() {
                    updated_messages
                        .push(ChatMessage::user_with_blocks(tool_result_blocks));
                }
                eprintln!(
                    "[rung-std] {call_id}: tool_use — {tool_count} tool(s), iterating"
                );

                let updated_thread = Thread {
                    system_prompt: thread.system_prompt.clone(),
                    messages: updated_messages,
                };

                if is_grace {
                    Ok(StepOutcome::GraceIterate(Calling::new(
                        updated_thread,
                        Carry { state: next, tools, config },
                    )))
                } else {
                    Ok(StepOutcome::Iterate(Calling::new(
                        updated_thread,
                        Carry { state: next, tools, config },
                    )))
                }
            }

            StopReason::Refusal => {
                Ok(StepOutcome::ContentFiltered(ContentFiltered::new(
                    Filtered {
                        reason: "model refused the request".into(),
                    },
                )))
            }
        }
    },

    // Error-path recovery (G9): unguarded. Hand the token back unchanged — the
    // driver may decide to retry or give up.
    api_retry = |f| {
        f.token
    },
});
