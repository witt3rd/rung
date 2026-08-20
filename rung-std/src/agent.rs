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
//! - **G5 (carry immutability).** [`LoopState`], the tool roster, and
//!   optional [`InlinePython`] ride in the immutable carry. The recover
//!   edge threads them forward unchanged. The guest mutates behind a mutex.
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
//!
//! Action is not exclusive. The same loop admits:
//! - native tools on the roster (including [`Sandbox::as_tool`](crate::python::Sandbox::as_tool))
//! - inline Python: no tool schema on the wire, extract code from the
//!   assistant text, strike the guest ([`InlinePython`])
//! - both: tools on the wire, and an EndTurn draft may still be struck
//!
//! Shell waffle is never a strike ([`crate::python::classify_draft`]).

use rung::ladder;

use crate::llm::{
    ChatMessage, ContentBlock, DEFAULT_MAX_ATTEMPTS, LlmConfig, LlmFailure, LlmRequest, StopReason,
    ToolDefinition,
};
use crate::python::{Draft, Sandbox, StrikeReply, classify_draft};
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

/// System prompt for [`InlinePython::only_answer`] / [`InlinePython::only`].
pub const PYTHON_ONLY_SYSTEM: &str = "\
You write Python for a persistent CPython guest.
The harness will exec your code. Print the answer.
Rules:
- Reply with Python only. No prose, no markdown, no bash.
- Use pathlib / os as needed. Names persist across strikes.
- Print the result. Do not explain it.
";

const PYTHON_NUDGE: &str = "That was not Python. Reply with Python only. Print the answer. No markdown, no bash, no explanation.";

/// A successful inline strike is either the answer or another tool-result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterStrike {
    /// First ok strike is `EndTurn` (Anvil smith).
    Finish,
    /// Feed stdout back and iterate (Python is the action channel).
    Continue,
}

/// A draft that is not Python and not a rejected shell dump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterWaffle {
    /// The text is the answer.
    EndTurn,
    /// Nudge the model to write Python and iterate.
    Nudge,
}

/// Inline Python on the agent loop — extract from assistant text, strike.
///
/// Independent of admitting [`Sandbox::collection`](crate::python::Sandbox::collection)
/// as a named tool. Set `exclusive` to omit tool schemas on the wire.
#[derive(Clone, Debug)]
pub struct InlinePython {
    pub sandbox: Sandbox,
    pub after_strike: AfterStrike,
    pub after_waffle: AfterWaffle,
    pub exclusive: bool,
}

impl InlinePython {
    /// No tools on the wire. First ok strike is the answer.
    pub fn only_answer(sandbox: Sandbox) -> Self {
        Self {
            sandbox,
            after_strike: AfterStrike::Finish,
            after_waffle: AfterWaffle::Nudge,
            exclusive: true,
        }
    }

    /// No tools on the wire. Strike results come back; prose ends the loop.
    /// Rejected shell dumps still nudge.
    pub fn only(sandbox: Sandbox) -> Self {
        Self {
            sandbox,
            after_strike: AfterStrike::Continue,
            after_waffle: AfterWaffle::EndTurn,
            exclusive: true,
        }
    }

    /// Tools stay on the wire. An EndTurn draft may still be struck.
    pub fn also(sandbox: Sandbox) -> Self {
        Self {
            sandbox,
            after_strike: AfterStrike::Continue,
            after_waffle: AfterWaffle::EndTurn,
            exclusive: false,
        }
    }
}

/// Tool schemas sent on the request. Exclusive inline Python sends none.
pub fn wire_definitions(python: Option<&InlinePython>, tools: &dyn Toolset) -> Vec<ToolDefinition> {
    if python.is_some_and(|p| p.exclusive) {
        Vec::new()
    } else {
        tools.definitions()
    }
}

/// Result of one inline-Python look at an assistant draft.
#[derive(Debug, Clone)]
pub struct InlineTurn {
    pub assistant: String,
    pub follow_up: Option<String>,
    pub done: Option<String>,
}

/// Strike, nudge, or finish. The verb (guest I/O) lives here so `step` can
/// stay a match on stop-reason.
pub fn inline_turn(py: &InlinePython, assistant_text: &str) -> InlineTurn {
    match classify_draft(assistant_text) {
        Draft::Python(code) => strike_turn(py, assistant_text, &code),
        Draft::Rejected => nudge(assistant_text),
        Draft::Text => match py.after_waffle {
            AfterWaffle::EndTurn => InlineTurn {
                assistant: assistant_text.into(),
                follow_up: None,
                done: Some(assistant_text.to_string()),
            },
            AfterWaffle::Nudge => nudge(assistant_text),
        },
    }
}

fn nudge(assistant: &str) -> InlineTurn {
    InlineTurn {
        assistant: assistant.into(),
        follow_up: Some(PYTHON_NUDGE.into()),
        done: None,
    }
}

fn strike_turn(py: &InlinePython, assistant: &str, code: &str) -> InlineTurn {
    match py.sandbox.strike(code) {
        Ok(reply) if reply.ok => match py.after_strike {
            AfterStrike::Finish => InlineTurn {
                assistant: assistant.into(),
                follow_up: None,
                done: Some(ok_display(&reply)),
            },
            AfterStrike::Continue => InlineTurn {
                assistant: assistant.into(),
                follow_up: Some(format_ok(&reply)),
                done: None,
            },
        },
        Ok(reply) => InlineTurn {
            assistant: assistant.into(),
            follow_up: Some(format_fail(
                reply.error.as_deref().unwrap_or("strike failed"),
            )),
            done: None,
        },
        Err(e) => InlineTurn {
            assistant: assistant.into(),
            follow_up: Some(format_fail(&e.to_string())),
            done: None,
        },
    }
}

fn ok_display(reply: &StrikeReply) -> String {
    let shown = reply.display();
    if shown.is_empty() {
        "(ok)".into()
    } else {
        shown
    }
}

fn format_ok(reply: &StrikeReply) -> String {
    let shown = reply.display();
    if shown.is_empty() {
        "Python returned no output.".into()
    } else {
        format!("Python result:\n{shown}")
    }
}

fn format_fail(err: &str) -> String {
    format!("That Python failed:\n{err}\nWrite fixed Python only. Print the answer.")
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
        python: Option<InlinePython>,
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
        let python = calling.carry().python.clone();

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
            tools: wire_definitions(python.as_ref(), tools.as_ref()),
            attempts_remaining: DEFAULT_MAX_ATTEMPTS,
            next_delay_ms: None,
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
                        LlmFailure::Forbidden(msg) => format!("forbidden: {msg}"),
                        LlmFailure::IdleTimeout { elapsed_secs } => {
                            format!("idle-timeout after {elapsed_secs}s")
                        }
                        LlmFailure::Config(msg) => format!("config: {msg}"),
                        LlmFailure::MaxRetries { last_error } => {
                            format!("max retries exhausted: {last_error}")
                        }
                        LlmFailure::ContentPolicy(msg) => format!("content-policy: {msg}"),
                        LlmFailure::QuotaExceeded(msg) => format!("quota: {msg}"),
                        LlmFailure::InvalidRequest {
                            message,
                            classification,
                        } => match classification {
                            Some(c) => format!("invalid-request ({c}): {message}"),
                            None => format!("invalid-request: {message}"),
                        },
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
                if let Some(py) = python.as_ref() {
                    let turn = inline_turn(py, &text);
                    if let Some(done) = turn.done {
                        eprintln!("[rung-std] {call_id}: end_turn — {done:.120}");
                        return Ok(StepOutcome::EndTurn(EndTurn::new(AgentResult {
                            final_response: done,
                            api_calls_made: next.api_call_count,
                        })));
                    }
                    eprintln!(
                        "[rung-std] {call_id}: inline python — iterating"
                    );
                    let mut updated_messages = thread.messages.clone();
                    updated_messages.push(ChatMessage::assistant(turn.assistant));
                    if let Some(follow) = turn.follow_up {
                        updated_messages.push(ChatMessage::user(follow));
                    }
                    let updated_thread = Thread {
                        system_prompt: thread.system_prompt.clone(),
                        messages: updated_messages,
                    };
                    let carry = Carry {
                        state: next,
                        tools,
                        config,
                        python,
                    };
                    return if is_grace {
                        Ok(StepOutcome::GraceIterate(Calling::new(
                            updated_thread,
                            carry,
                        )))
                    } else {
                        Ok(StepOutcome::Iterate(Calling::new(
                            updated_thread,
                            carry,
                        )))
                    };
                }
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
                                cache: None,
                            });
                        }
                        ContentBlock::Thinking { thinking, signature } => {
                            assistant_blocks.push(crate::llm::MessageContentBlock::Thinking {
                                thinking: thinking.clone(),
                                signature: signature.clone(),
                                cache: None,
                            });
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            eprintln!("[rung-std] {call_id}: executing tool '{name}'");
                            assistant_blocks.push(
                                crate::llm::MessageContentBlock::ToolUse {
                                    id: id.clone(),
                                    name: name.clone(),
                                    input: input.clone(),
                                    cache: None,
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
                                    cache: None,
                                },
                            );
                            tool_count += 1;
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
                        Carry { state: next, tools, config, python },
                    )))
                } else {
                    Ok(StepOutcome::Iterate(Calling::new(
                        updated_thread,
                        Carry { state: next, tools, config, python },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::python::{Jail, SandboxConfig};
    use crate::tools::{Tool, ToolCollection, ToolRoster};
    use std::path::PathBuf;
    use std::time::Duration;

    fn tmp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "rung-agent-py-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn sandbox() -> (PathBuf, Sandbox) {
        let dir = tmp();
        let mut c = SandboxConfig::in_dir(&dir);
        c.jail = Jail::Off;
        c.strike_timeout = Duration::from_secs(8);
        let sb = Sandbox::open(c).unwrap();
        (dir, sb)
    }

    fn cleanup(dir: &PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[derive(Debug)]
    struct Named;

    impl Tool for Named {
        fn name(&self) -> &'static str {
            "named"
        }
        fn description(&self) -> &'static str {
            "a named tool"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        fn execute(&self, _input: &serde_json::Value) -> Result<String, String> {
            Ok("ok".into())
        }
    }

    fn roster_with_named() -> ToolRoster {
        let mut c = ToolCollection::new("test");
        c.admit(Named);
        let mut r = ToolRoster::new();
        r.add(c);
        r
    }

    #[test]
    fn exclusive_sends_no_tool_schemas() {
        let (dir, sb) = sandbox();
        let roster = roster_with_named();
        let py = InlinePython::only_answer(sb);
        assert!(wire_definitions(Some(&py), &roster).is_empty());
        assert_eq!(wire_definitions(None, &roster).len(), 1);
        cleanup(&dir);
    }

    #[test]
    fn also_keeps_tool_schemas() {
        let (dir, sb) = sandbox();
        let roster = roster_with_named();
        let py = InlinePython::also(sb);
        assert_eq!(wire_definitions(Some(&py), &roster).len(), 1);
        cleanup(&dir);
    }

    #[test]
    fn waffle_nudges_even_when_also() {
        let (dir, sb) = sandbox();
        let py = InlinePython::also(sb);
        let turn = inline_turn(
            &py,
            "Here's the one-liner:\n```bash\nfind . -type l\n```\n### What this does\n",
        );
        assert!(turn.done.is_none());
        assert!(
            turn.follow_up
                .as_deref()
                .unwrap_or("")
                .contains("not Python")
        );
        cleanup(&dir);
    }

    #[test]
    fn prose_ends_when_after_waffle_is_end_turn() {
        let (dir, sb) = sandbox();
        let py = InlinePython::only(sb);
        let turn = inline_turn(&py, "The answer is 4.");
        assert_eq!(turn.done.as_deref(), Some("The answer is 4."));
        cleanup(&dir);
    }

    #[test]
    fn only_answer_finishes_on_ok_strike() {
        let (dir, sb) = sandbox();
        let py = InlinePython::only_answer(sb);
        let turn = inline_turn(&py, "```python\nprint(2+2)\n```");
        assert_eq!(turn.done.as_deref(), Some("4"));
        assert!(turn.follow_up.is_none());
        cleanup(&dir);
    }

    #[test]
    fn only_continues_with_the_result() {
        let (dir, sb) = sandbox();
        let py = InlinePython::only(sb);
        let turn = inline_turn(&py, "print(6*7)");
        assert!(turn.done.is_none());
        assert!(turn.follow_up.as_deref().unwrap_or("").contains("42"));
        cleanup(&dir);
    }

    #[test]
    fn failed_strike_comes_back_as_follow_up() {
        let (dir, sb) = sandbox();
        let py = InlinePython::only_answer(sb);
        let turn = inline_turn(&py, "```python\n1/0\n```");
        assert!(turn.done.is_none());
        assert!(
            turn.follow_up
                .as_deref()
                .unwrap_or("")
                .contains("ZeroDivisionError")
        );
        cleanup(&dir);
    }
}
