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
//!
//! Loop hardening (OpenCode / grok-build): last-step text-only nudge and
//! empty tool schemas; tool results capped; identical name+input streaks
//! warned then stopped; LLM failures keep a [`FailureKind`] (overflow is
//! not a content filter); usage is accumulated on the terminal payload.
//!
//! Nested work is [`NestedLoop`] admitted as the `task` tool: one child
//! AgentLoop, default depth 1, child roster without `task`.

use rung::ladder;

use crate::llm::{
    ChatMessage, ContentBlock, DEFAULT_MAX_ATTEMPTS, LlmConfig, LlmFailure, LlmRequest, StopReason,
    ToolDefinition, Usage,
};
use crate::python::{Draft, Sandbox, StrikeReply, classify_draft};
use crate::tools::{MAX_DEPTH, Spawn, Task, TaskRequest, TaskResult, Toolset, WithoutTask};
use serde_json::Value;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// ─── Carry data ────────────────────────────────────────────────────────────────

/// Characters of a tool result kept in history. OpenCode prunes at 2k for
/// compaction; live turns stay larger. 0 means no cap.
pub const DEFAULT_TOOL_OUTPUT_LIMIT: usize = 8_192;
/// Identical name+input in a row before we stop executing (OpenCode uses 3).
pub const DOOM_STREAK: u32 = 3;

const LAST_STEP_NUDGE: &str = "\
CRITICAL: this is the last step. Tools are disabled. Reply with text only: \
what was done, what remains, and what to do next. Do not call tools.";

/// Loop-control counters threaded through every rung as immutable carry (G5).
#[derive(Clone, Debug)]
pub struct LoopState {
    pub api_call_count: u32,
    pub max_iterations: u32,
    pub budget_remaining: u32,
    /// One-shot: allow a single extra call when budget is zero.
    pub grace_call: bool,
    pub usage: Usage,
    pub watch: ToolWatch,
    pub tool_output_limit: usize,
    /// Shared cancel. Checked before an LLM call and around each tool.
    pub cancel: Option<Arc<AtomicBool>>,
}

impl LoopState {
    pub fn new(max_iterations: u32, budget: u32) -> Self {
        Self {
            api_call_count: 0,
            max_iterations,
            budget_remaining: budget,
            grace_call: true,
            usage: Usage::default(),
            watch: ToolWatch::default(),
            tool_output_limit: DEFAULT_TOOL_OUTPUT_LIMIT,
            cancel: None,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|f| f.load(Ordering::SeqCst))
    }
}

/// Consecutive identical tool calls. A streak of [`DOOM_STREAK`] warns;
/// one more of the same stops the loop.
#[derive(Clone, Debug, Default)]
pub struct ToolWatch {
    last: Option<(String, String)>,
    streak: u32,
    warned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watch {
    Execute,
    Warn,
    Stop,
}

impl ToolWatch {
    pub fn observe(&self, name: &str, input: &Value) -> (Self, Watch) {
        let fp = (
            name.to_string(),
            serde_json::to_string(input).unwrap_or_default(),
        );
        let same = self.last.as_ref() == Some(&fp);
        let streak = if same {
            self.streak.saturating_add(1)
        } else {
            1
        };
        let warned = if same { self.warned } else { false };
        let action = if streak > DOOM_STREAK && warned {
            Watch::Stop
        } else if streak >= DOOM_STREAK {
            Watch::Warn
        } else {
            Watch::Execute
        };
        (
            Self {
                last: Some(fp),
                streak,
                warned: matches!(action, Watch::Warn) || warned,
            },
            action,
        )
    }
}

/// Produce the loop state for the next iteration — after a successful LLM call
/// that elected to continue. Increments the call counter, optionally decrements
/// budget (unless this was a grace call), and consumes the grace flag only when
/// the grace call was actually taken.
fn next_state(prev: &LoopState, is_grace: bool, usage: &Usage, watch: ToolWatch) -> LoopState {
    let mut next = prev.clone();
    next.api_call_count += 1;
    next.usage = next.usage.saturating_add(usage);
    next.watch = watch;
    if !is_grace {
        next.budget_remaining = next.budget_remaining.saturating_sub(1);
    } else {
        next.grace_call = false; // grace call consumed, one-shot exhausted
    }
    next
}

/// Last allowed provider turn: iteration cap, or the one-shot grace call.
pub fn is_last_call(state: &LoopState) -> bool {
    state.api_call_count + 1 >= state.max_iterations
        || (state.budget_remaining == 0 && state.grace_call)
}

pub fn cap_output(text: &str, limit: usize) -> String {
    if limit == 0 || text.len() <= limit {
        return text.to_string();
    }
    let mut end = limit;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[truncated {} chars]", &text[..end], text.len() - end)
}

fn ensure_system(thread: &Thread) -> Vec<ChatMessage> {
    let mut messages = thread.messages.clone();
    if thread.system_prompt.is_empty() {
        return messages;
    }
    let has_system = messages.iter().any(|m| m.role == "system");
    if !has_system {
        messages.insert(0, ChatMessage::system(thread.system_prompt.clone()));
    }
    messages
}

fn map_llm_failure(failure: LlmFailure) -> Filtered {
    match failure {
        LlmFailure::Auth(msg) => Filtered {
            kind: FailureKind::Auth,
            reason: format!("auth: {msg}"),
        },
        LlmFailure::Forbidden(msg) => Filtered {
            kind: FailureKind::Forbidden,
            reason: format!("forbidden: {msg}"),
        },
        LlmFailure::IdleTimeout { elapsed_secs } => Filtered {
            kind: FailureKind::Provider,
            reason: format!("idle-timeout after {elapsed_secs}s"),
        },
        LlmFailure::Config(msg) => Filtered {
            kind: FailureKind::Config,
            reason: format!("config: {msg}"),
        },
        LlmFailure::MaxRetries { last_error } => Filtered {
            kind: FailureKind::Provider,
            reason: format!("max retries exhausted: {last_error}"),
        },
        LlmFailure::ContentPolicy(msg) => Filtered {
            kind: FailureKind::ContentPolicy,
            reason: format!("content-policy: {msg}"),
        },
        LlmFailure::QuotaExceeded(msg) => Filtered {
            kind: FailureKind::Quota,
            reason: format!("quota: {msg}"),
        },
        LlmFailure::InvalidRequest {
            message,
            classification,
        } => {
            let overflow = classification
                .as_deref()
                .is_some_and(|c| c == "context-overflow");
            Filtered {
                kind: if overflow {
                    FailureKind::Overflow
                } else {
                    FailureKind::Provider
                },
                reason: match classification {
                    Some(c) => format!("invalid-request ({c}): {message}"),
                    None => format!("invalid-request: {message}"),
                },
            }
        }
    }
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
    pub usage: Usage,
}

/// Iteration limit reached before the model finished.
#[derive(Debug)]
pub struct LimitHit {
    pub api_calls_made: u32,
    pub usage: Usage,
}

/// Budget exhausted (and no grace call available).
#[derive(Debug)]
pub struct BudgetHit {
    pub api_calls_made: u32,
    pub usage: Usage,
}

/// [`LoopState::cancel`] was set (ACP `session/cancel`, `session/close`).
#[derive(Debug)]
pub struct Interrupt {
    pub at_api_call: u32,
}

/// Why the loop stopped without an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    ContentPolicy,
    Refusal,
    Auth,
    Forbidden,
    Overflow,
    Quota,
    Config,
    Provider,
    DoomLoop,
    Interrupted,
}

/// The model refused or an unrecoverable error occurred.
#[derive(Debug)]
pub struct Filtered {
    pub reason: String,
    pub kind: FailureKind,
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
        if state.is_cancelled() {
            return Ok(StepOutcome::Interrupted(Interrupted::new(Interrupt {
                at_api_call: state.api_call_count,
            })));
        }

        if state.api_call_count >= state.max_iterations {
            return Ok(StepOutcome::MaxIterations(MaxIterations::new(
                LimitHit {
                    api_calls_made: state.api_call_count,
                    usage: state.usage.clone(),
                },
            )));
        }

        let is_grace = state.budget_remaining == 0 && state.grace_call;
        if state.budget_remaining == 0 && !state.grace_call {
            return Ok(StepOutcome::BudgetExhausted(BudgetExhausted::new(
                BudgetHit {
                    api_calls_made: state.api_call_count,
                    usage: state.usage.clone(),
                },
            )));
        }

        let last_call = is_last_call(&state);

        // ── LLM call ─────────────────────────────────────────────────────
        let call_id = fresh_call_id();
        eprintln!(
            "[rung-std] {call_id}: starting LLM call (attempt {}/{})",
            state.api_call_count + 1,
            state.max_iterations
        );

        let mut messages = ensure_system(&thread);
        if last_call {
            messages.push(ChatMessage::user(LAST_STEP_NUDGE));
        }
        let wire_tools = if last_call {
            Vec::new()
        } else {
            wire_definitions(python.as_ref(), tools.as_ref())
        };
        let request = LlmRequest {
            config: config.clone(),
            messages,
            tools: wire_tools,
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
                    return Ok(StepOutcome::ContentFiltered(ContentFiltered::new(
                        map_llm_failure(e.into_payload()),
                    )));
                }
                Err(f) => {
                    eprintln!("[rung-std] {call_id}: retryable LLM error — {}", f.error);
                    llm_pending = crate::llm::llmcall::retry(f);
                }
            }
        };

        // ── classify the response ────────────────────────────────────────
        let next = next_state(&state, is_grace, &response.usage, state.watch.clone());
        if next.is_cancelled() {
            return Ok(StepOutcome::Interrupted(Interrupted::new(Interrupt {
                at_api_call: next.api_call_count,
            })));
        }

        let has_tool_blocks = response.content.iter().any(|b| {
            matches!(
                b,
                ContentBlock::ToolUse { .. } | ContentBlock::InvalidToolUse { .. }
            )
        });
        let stop_reason = if has_tool_blocks {
            StopReason::ToolUse
        } else {
            response.stop_reason
        };

        match stop_reason {
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
                            usage: next.usage.clone(),
                        })));
                    }
                    eprintln!(
                        "[rung-std] {call_id}: inline python — iterating"
                    );
                    let mut updated_messages = thread.messages.clone();
                    updated_messages.push(ChatMessage::assistant(turn.assistant));
                    if let Some(follow) = turn.follow_up {
                        updated_messages.push(ChatMessage::user(cap_output(
                            &follow,
                            next.tool_output_limit,
                        )));
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
                    usage: next.usage.clone(),
                })))
            }

            StopReason::ToolUse => {
                let mut assistant_blocks: Vec<crate::llm::MessageContentBlock> = Vec::new();
                let mut tool_count = 0u32;
                let mut tool_result_blocks: Vec<crate::llm::MessageContentBlock> = Vec::new();
                let mut invalid_tool_diagnostics: Vec<(String, String, crate::llm::ToolDiagnostic)> =
                    Vec::new();
                let mut executed_tools: Vec<String> = Vec::new();
                let mut next = next;

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
                            if next.is_cancelled() {
                                return Ok(StepOutcome::Interrupted(Interrupted::new(
                                    Interrupt {
                                        at_api_call: next.api_call_count,
                                    },
                                )));
                            }
                            let (watch, action) = next.watch.observe(name, input);
                            next.watch = watch;
                            let (content, is_error) = match action {
                                Watch::Stop => {
                                    return Ok(StepOutcome::ContentFiltered(
                                        ContentFiltered::new(Filtered {
                                            kind: FailureKind::DoomLoop,
                                            reason: format!(
                                                "repeated {name} with the same input"
                                            ),
                                        }),
                                    ));
                                }
                                Watch::Warn => (
                                    format!(
                                        "Repeated {name} with the same input {DOOM_STREAK} times. Use the previous result or change the arguments."
                                    ),
                                    true,
                                ),
                                Watch::Execute => match tools.execute(name, input) {
                                    Ok(s) => (cap_output(&s, next.tool_output_limit), false),
                                    Err(e) => (format!("error: {e}"), true),
                                },
                            };
                            if next.is_cancelled() {
                                return Ok(StepOutcome::Interrupted(Interrupted::new(
                                    Interrupt {
                                        at_api_call: next.api_call_count,
                                    },
                                )));
                            }
                            if is_error {
                                eprintln!("[rung-std] {call_id}:   -> {content}");
                            } else {
                                eprintln!("[rung-std] {call_id}:   -> {:.120}", content);
                            }
                            tool_result_blocks.push(
                                crate::llm::MessageContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content,
                                    is_error,
                                    cache: None,
                                },
                            );
                            executed_tools.push(name.clone());
                            tool_count += 1;
                        }
                        ContentBlock::InvalidToolUse { id, name, diagnostic } => {
                            eprintln!(
                                "[rung-std] {call_id}: invalid tool call '{name}' (id: '{id}'): {diagnostic}"
                            );
                            invalid_tool_diagnostics
                                .push((id.clone(), name.clone(), diagnostic.clone()));
                        }
                    }
                }

                if tool_count == 0 && invalid_tool_diagnostics.is_empty() {
                    let text = response_text(&response.content)
                        .unwrap_or_else(|| "(no text in response)".into());
                    return Ok(StepOutcome::EndTurn(EndTurn::new(AgentResult {
                        final_response: text,
                        api_calls_made: next.api_call_count,
                        usage: next.usage.clone(),
                    })));
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

                if !invalid_tool_diagnostics.is_empty() {
                    let mut feedback = Vec::new();
                    for (id, name, diag) in &invalid_tool_diagnostics {
                        let id_str = if id.is_empty() {
                            String::new()
                        } else {
                            format!(" (id: '{id}')")
                        };
                        feedback.push(format!(
                            "Tool call '{name}'{id_str} could not be executed because its arguments were malformed or incomplete:\n{diag}"
                        ));
                    }
                    if !executed_tools.is_empty() {
                        feedback.push(format!(
                            "The following sibling tool calls already executed successfully: {}.\nDo NOT repeat or re-call these completed tools. Proceed with their results or fix the failed tool call.",
                            executed_tools.join(", ")
                        ));
                    } else {
                        feedback.push(
                            "Please provide a valid, complete JSON tool call or answer with text."
                                .to_string(),
                        );
                    }
                    updated_messages.push(ChatMessage::user(feedback.join("\n\n")));
                }

                if invalid_tool_diagnostics.is_empty() {
                    eprintln!(
                        "[rung-std] {call_id}: tool_use — {tool_count} tool(s), iterating"
                    );
                } else {
                    eprintln!(
                        "[rung-std] {call_id}: tool_use — {tool_count} valid tool(s), {} invalid, iterating",
                        invalid_tool_diagnostics.len()
                    );
                }

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
                        kind: FailureKind::Refusal,
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

/// Drive [`AgentLoop`] from Idle to a terminal verdict.
pub fn run(thread: Thread, carry: agentloop::Carry) -> Result<AgentResult, Filtered> {
    let mut calling = agentloop::calling(agentloop::Idle::new(thread, carry));
    loop {
        match agentloop::step(calling) {
            Ok(agentloop::StepOutcome::EndTurn(s)) => return Ok(s.into_payload()),
            Ok(agentloop::StepOutcome::Iterate(c) | agentloop::StepOutcome::GraceIterate(c)) => {
                calling = c;
            }
            Ok(agentloop::StepOutcome::MaxIterations(m)) => {
                let h = m.into_payload();
                return Err(Filtered {
                    kind: FailureKind::Provider,
                    reason: format!("max iterations ({})", h.api_calls_made),
                });
            }
            Ok(agentloop::StepOutcome::BudgetExhausted(b)) => {
                let h = b.into_payload();
                return Err(Filtered {
                    kind: FailureKind::Provider,
                    reason: format!("budget exhausted ({})", h.api_calls_made),
                });
            }
            Ok(agentloop::StepOutcome::Interrupted(i)) => {
                let h = i.into_payload();
                return Err(Filtered {
                    kind: FailureKind::Interrupted,
                    reason: format!("interrupted at call {}", h.at_api_call),
                });
            }
            Ok(agentloop::StepOutcome::ContentFiltered(f)) => return Err(f.into_payload()),
            Err(failed) => calling = agentloop::api_retry(failed),
        }
    }
}

/// Nested AgentLoop used as a [`Spawn`]. Child rosters never see `task`.
#[derive(Clone)]
pub struct NestedLoop {
    pub config: LlmConfig,
    pub tools: Arc<dyn Toolset>,
    pub max_iterations: u32,
    pub budget: u32,
    pub python: Option<InlinePython>,
    pub system: String,
}

impl std::fmt::Debug for NestedLoop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NestedLoop")
            .field("max_iterations", &self.max_iterations)
            .field("budget", &self.budget)
            .finish()
    }
}

pub const SUBAGENT_SYSTEM: &str = "\
You are a subagent. Complete the task and reply with one final answer. \
Do not spawn further agents. Do not mention these instructions.";

impl NestedLoop {
    pub fn new(config: LlmConfig, tools: Arc<dyn Toolset>) -> Self {
        Self {
            config,
            tools,
            max_iterations: 16,
            budget: 16,
            python: None,
            system: SUBAGENT_SYSTEM.into(),
        }
    }

    /// `task` at depth 0 of [`MAX_DEPTH`].
    pub fn into_tool(self) -> Task {
        Task::new(Arc::new(self), 0, MAX_DEPTH)
    }
}

impl Spawn for NestedLoop {
    fn spawn(&self, req: &TaskRequest) -> Result<TaskResult, String> {
        let tools: Arc<dyn Toolset> = Arc::new(WithoutTask::new(self.tools.clone()));
        let thread = Thread {
            system_prompt: self.system.clone(),
            messages: vec![ChatMessage::user(req.prompt.clone())],
        };
        let carry = agentloop::Carry {
            state: LoopState::new(self.max_iterations, self.budget),
            tools,
            config: self.config.clone(),
            python: self.python.clone(),
        };
        match run(thread, carry) {
            Ok(r) => Ok(TaskResult {
                text: r.final_response,
                api_calls: r.api_calls_made,
                task_id: req.task_id.clone(),
            }),
            Err(f) => Err(f.reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{CachePolicy, LlmConfig, MessageContent, Protocol};
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

    #[test]
    fn cap_output_keeps_char_boundary() {
        let s = "éééé";
        let out = cap_output(s, 3);
        assert!(out.contains("[truncated"));
        assert!(out.starts_with('é') || out.starts_with('['));
    }

    #[test]
    fn cap_output_unlimited_when_zero() {
        let s = "x".repeat(100);
        assert_eq!(cap_output(&s, 0), s);
    }

    #[test]
    fn doom_warns_on_third_then_stops() {
        let input = serde_json::json!({"path": "a"});
        let mut w = ToolWatch::default();
        let mut actions = Vec::new();
        for _ in 0..4 {
            let (next, a) = w.observe("read_file", &input);
            w = next;
            actions.push(a);
        }
        assert_eq!(
            actions,
            vec![Watch::Execute, Watch::Execute, Watch::Warn, Watch::Stop]
        );
        let (_, a) = w.observe("read_file", &serde_json::json!({"path": "b"}));
        assert_eq!(a, Watch::Execute);
    }

    fn dummy_llm() -> LlmConfig {
        LlmConfig {
            base_url: "http://127.0.0.1:9/v1".into(),
            api_key: "test".into(),
            model: "dummy".into(),
            timeout_secs: 1,
            idle_timeout_secs: None,
            max_tokens: 16,
            temperature: None,
            top_p: None,
            top_k: None,
            seed: None,
            stop: Vec::new(),
            reasoning_level: None,
            structured_outputs: false,
            protocol: Protocol::OpenAiChat,
            cache: CachePolicy::None,
            stream_listener: None,
        }
    }

    #[test]
    fn last_call_on_iteration_cap_and_grace() {
        let mut s = LoopState::new(3, 10);
        s.api_call_count = 2;
        assert!(is_last_call(&s));
        let mut g = LoopState::new(10, 0);
        g.grace_call = true;
        assert!(is_last_call(&g));
        let mut n = LoopState::new(10, 2);
        n.api_call_count = 0;
        assert!(!is_last_call(&n));
    }

    #[test]
    fn overflow_is_not_a_content_filter() {
        let f = map_llm_failure(LlmFailure::InvalidRequest {
            message: "too long".into(),
            classification: Some("context-overflow".into()),
        });
        assert_eq!(f.kind, FailureKind::Overflow);
        let auth = map_llm_failure(LlmFailure::Auth("bad key".into()));
        assert_eq!(auth.kind, FailureKind::Auth);
    }

    #[test]
    fn ensure_system_prepends_once() {
        let thread = Thread {
            system_prompt: "be brief".into(),
            messages: vec![ChatMessage::user("hi")],
        };
        let msgs = ensure_system(&thread);
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs.len(), 2);
        let again = Thread {
            system_prompt: "be brief".into(),
            messages: msgs,
        };
        assert_eq!(ensure_system(&again).len(), 2);
    }

    #[test]
    fn cancel_before_llm_is_interrupted() {
        let state = LoopState {
            cancel: Some(Arc::new(AtomicBool::new(true))),
            ..LoopState::new(3, 3)
        };
        assert!(state.is_cancelled());
        let carry = agentloop::Carry {
            state,
            tools: Arc::new(ToolRoster::new()),
            config: dummy_llm(),
            python: None,
        };
        let thread = Thread {
            system_prompt: String::new(),
            messages: vec![ChatMessage::user("hi")],
        };
        let err = run(thread, carry).expect_err("cancelled");
        assert_eq!(err.kind, FailureKind::Interrupted);
    }

    use std::sync::atomic::AtomicUsize;

    #[derive(Debug)]
    struct CountingTool {
        name: &'static str,
        exec_count: Arc<AtomicUsize>,
        error: Option<String>,
    }

    impl Tool for CountingTool {
        fn name(&self) -> &'static str {
            self.name
        }
        fn description(&self) -> &'static str {
            "counting tool"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        fn execute(&self, _input: &serde_json::Value) -> Result<String, String> {
            self.exec_count.fetch_add(1, Ordering::SeqCst);
            if let Some(err) = &self.error {
                Err(err.clone())
            } else {
                Ok("S".repeat(200))
            }
        }
    }

    fn serve_json(body: &str) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let body_owned = body.to_string();
        let handle = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body_owned.len(),
                body_owned
            );
            let _ = socket.write_all(resp.as_bytes());
        });
        (format!("http://127.0.0.1:{port}"), handle)
    }

    #[test]
    fn mixed_valid_invalid_siblings_execute_valid_once_and_preserves_results() {
        let response_body = r#"{
            "id": "cmpl-mixed",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "tool_calls": [
                        {
                            "id": "call_valid_1",
                            "function": {"name": "valid_tool", "arguments": "{\"path\":\"a.txt\"}"}
                        },
                        {
                            "id": "call_invalid_2",
                            "function": {"name": "invalid_tool", "arguments": "{\"path\":"}
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }]
        }"#;

        let (url, handle) = serve_json(response_body);
        let mut cfg = dummy_llm();
        cfg.base_url = url;

        let valid_count = Arc::new(AtomicUsize::new(0));
        let invalid_count = Arc::new(AtomicUsize::new(0));

        let mut c = ToolCollection::new("test");
        c.admit(CountingTool {
            name: "valid_tool",
            exec_count: valid_count.clone(),
            error: None,
        });
        c.admit(CountingTool {
            name: "invalid_tool",
            exec_count: invalid_count.clone(),
            error: None,
        });
        let mut roster = ToolRoster::new();
        roster.add(c);

        let carry = agentloop::Carry {
            state: LoopState::new(5, 5),
            tools: Arc::new(roster),
            config: cfg,
            python: None,
        };
        let thread = Thread {
            system_prompt: "system prompt".into(),
            messages: vec![ChatMessage::user("run tools")],
        };
        let calling = agentloop::calling(agentloop::Idle::new(thread, carry));
        let outcome = match agentloop::step(calling) {
            Ok(o) => o,
            Err(_) => panic!("step failed"),
        };
        handle.join().unwrap();

        // 1. Valid sibling executed ONCE
        assert_eq!(valid_count.load(Ordering::SeqCst), 1);
        // 2. Invalid sibling NEVER dispatched
        assert_eq!(invalid_count.load(Ordering::SeqCst), 0);

        // 3. Loop iterates
        let next_calling = match outcome {
            agentloop::StepOutcome::Iterate(c) => c,
            _ => panic!("expected Iterate"),
        };

        // 4. Thread messages preserve valid sibling history/results and include corrective feedback
        let msgs = &next_calling.payload.messages;
        assert_eq!(msgs.len(), 4); // [initial user, assistant (valid tool), user (tool result), user (corrective)]

        // Assistant message has valid ToolUse
        match &msgs[1].content {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    crate::llm::MessageContentBlock::ToolUse { id, name, .. } => {
                        assert_eq!(id, "call_valid_1");
                        assert_eq!(name, "valid_tool");
                    }
                    other => panic!("expected ToolUse block, got {other:?}"),
                }
            }
            other => panic!("expected Blocks, got {other:?}"),
        }

        // ToolResult block
        match &msgs[2].content {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    crate::llm::MessageContentBlock::ToolResult {
                        tool_use_id,
                        is_error,
                        ..
                    } => {
                        assert_eq!(tool_use_id, "call_valid_1");
                        assert!(!is_error);
                    }
                    other => panic!("expected ToolResult block, got {other:?}"),
                }
            }
            other => panic!("expected Blocks, got {other:?}"),
        }

        // Corrective user message
        match &msgs[3].content {
            MessageContent::Text(text) => {
                assert!(text.contains("invalid_tool"));
                assert!(text.contains("valid_tool"));
                assert!(text.contains("Do NOT repeat or re-call these completed tools"));
            }
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn tool_errors_unclipped_in_agent_loop() {
        let response_body = r#"{
            "id": "cmpl-err",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "tool_calls": [
                        {
                            "id": "call_err",
                            "function": {"name": "failing_tool", "arguments": "{}"}
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }]
        }"#;

        let (url, handle) = serve_json(response_body);
        let mut cfg = dummy_llm();
        cfg.base_url = url;

        let long_err_message =
            "CRITICAL_DATABASE_ERROR: connection refused by host at cluster-node-99".repeat(10);
        let mut c = ToolCollection::new("test");
        c.admit(CountingTool {
            name: "failing_tool",
            exec_count: Arc::new(AtomicUsize::new(0)),
            error: Some(long_err_message.clone()),
        });
        let mut roster = ToolRoster::new();
        roster.add(c);

        let mut state = LoopState::new(5, 5);
        state.tool_output_limit = 20; // Very small limit!

        let carry = agentloop::Carry {
            state,
            tools: Arc::new(roster),
            config: cfg,
            python: None,
        };
        let thread = Thread {
            system_prompt: String::new(),
            messages: vec![ChatMessage::user("run tool")],
        };
        let calling = agentloop::calling(agentloop::Idle::new(thread, carry));
        let outcome = match agentloop::step(calling) {
            Ok(o) => o,
            Err(_) => panic!("step failed"),
        };
        handle.join().unwrap();

        let next_calling = match outcome {
            agentloop::StepOutcome::Iterate(c) => c,
            _ => panic!("expected Iterate"),
        };

        // ToolResult block MUST NOT be clipped on error!
        let msgs = &next_calling.payload.messages;
        match &msgs[2].content {
            MessageContent::Blocks(blocks) => match &blocks[0] {
                crate::llm::MessageContentBlock::ToolResult {
                    content,
                    is_error,
                    ..
                } => {
                    assert!(is_error);
                    // The entire error message must be present without truncation!
                    assert!(!content.contains("[truncated"));
                    assert!(content.contains(&long_err_message));
                }
                other => panic!("expected ToolResult, got {other:?}"),
            },
            other => panic!("expected Blocks, got {other:?}"),
        }
    }

    #[test]
    fn invalid_tool_forces_tool_use_even_on_max_tokens() {
        let response_body = r#"{
            "id": "cmpl-len",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "content": "I am calling the tool",
                    "tool_calls": [
                        {
                            "id": "call_cut",
                            "function": {"name": "broken_tool", "arguments": "{\"incomplete\": true"}
                        }
                    ]
                },
                "finish_reason": "length"
            }]
        }"#;

        let (url, handle) = serve_json(response_body);
        let mut cfg = dummy_llm();
        cfg.base_url = url;

        let mut c = ToolCollection::new("test");
        c.admit(CountingTool {
            name: "broken_tool",
            exec_count: Arc::new(AtomicUsize::new(0)),
            error: None,
        });
        let mut roster = ToolRoster::new();
        roster.add(c);

        let carry = agentloop::Carry {
            state: LoopState::new(5, 5),
            tools: Arc::new(roster),
            config: cfg,
            python: None,
        };
        let thread = Thread {
            system_prompt: String::new(),
            messages: vec![ChatMessage::user("run tool")],
        };
        let calling = agentloop::calling(agentloop::Idle::new(thread, carry));
        let outcome = match agentloop::step(calling) {
            Ok(o) => o,
            Err(_) => panic!("step failed"),
        };
        handle.join().unwrap();

        // Must NOT be EndTurn despite finish_reason: length / MaxTokens
        match outcome {
            agentloop::StepOutcome::Iterate(c) => {
                let msgs = &c.payload.messages;
                let last_msg = msgs.last().unwrap();
                assert!(last_msg.content.as_text().unwrap().contains("broken_tool"));
            }
            _ => panic!("expected Iterate due to forced ToolUse"),
        }
    }

    #[test]
    fn invalid_tool_without_assistant_text_appends_corrective_user_message_without_empty_assistant()
    {
        // Model emitted only an invalid tool call and NO text content
        let response_body = r#"{
            "id": "cmpl-notext",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "tool_calls": [
                        {
                            "id": "call_bad",
                            "function": {"name": "broken_tool", "arguments": "invalid-json"}
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }]
        }"#;

        let (url, handle) = serve_json(response_body);
        let mut cfg = dummy_llm();
        cfg.base_url = url;

        let mut c = ToolCollection::new("test");
        c.admit(CountingTool {
            name: "broken_tool",
            exec_count: Arc::new(AtomicUsize::new(0)),
            error: None,
        });
        let mut roster = ToolRoster::new();
        roster.add(c);

        let carry = agentloop::Carry {
            state: LoopState::new(5, 5),
            tools: Arc::new(roster),
            config: cfg,
            python: None,
        };
        let thread = Thread {
            system_prompt: String::new(),
            messages: vec![ChatMessage::user("run tool")],
        };
        let calling = agentloop::calling(agentloop::Idle::new(thread, carry));
        let outcome = match agentloop::step(calling) {
            Ok(o) => o,
            Err(_) => panic!("step failed"),
        };
        handle.join().unwrap();

        match outcome {
            agentloop::StepOutcome::Iterate(c) => {
                let msgs = &c.payload.messages;
                // Verify: No empty assistant message was inserted!
                // Initial user message, then corrective user message.
                for m in msgs {
                    if m.role == "assistant" {
                        match &m.content {
                            MessageContent::Text(t) => assert!(!t.is_empty()),
                            MessageContent::Blocks(b) => assert!(!b.is_empty()),
                        }
                    }
                }
                let last_msg = msgs.last().unwrap();
                assert_eq!(last_msg.role, "user");
                assert!(last_msg.content.as_text().unwrap().contains("broken_tool"));
            }
            _ => panic!("expected Iterate"),
        }
    }
}
