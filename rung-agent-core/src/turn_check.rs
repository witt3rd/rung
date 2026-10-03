//! TurnCheck: "completed" is a rung reached only through a judge's check.
//!
//! The agent loop ends a turn when the model stops calling tools. That is the
//! model's own report that it is done, and a model that only *writes* "I kept
//! it as a note" ends its turn exactly like one that called the tool (#128).
//! This rung reads what the turn claims against what it did, and asks a judge
//! other than the model: a [`Decider`], by default Jev.
//!
//! ```text
//! Ended(Turn) => { Completed(Checked) | Nudge(Nudged) | Unverified(Flagged) | Unchecked(Unread) }
//! ```
//!
//! - **Completed**: the judge reads the turn as done, answered, asking the
//!   user, or honestly blocked, with confidence, and no unbacked claim.
//! - **Nudge**: the final message claims work no action did. The product
//!   re-runs the loop once with [`NUDGE`] and checks again. A turn that was
//!   already nudged is never nudged twice.
//! - **Unverified**: anything else, including a turn still narrating after
//!   its nudge. The text is kept; the host decides.
//! - **Unchecked**: no reading came back (no key, unreachable, rate limited,
//!   a malformed answer, a state too large). The agent's result is delivered
//!   untouched and the status says it was not checked. Never `Completed`.
//!
//! [`Status::Completed`](crate::run::Status::Completed) holds a
//! [`Completion`], and a `Completion` comes only from a [`Checked`] or from
//! the switch being off ([`Unjudged`]). Nothing else in the crate can build one.
//!
//! The feature is one switch, `turn_check.backend` in `config.yaml` or
//! `RUNG_TURN_CHECK` (`off` | `jev`). It defaults to `off`, and while it is
//! off the output is what it was before this rung existed.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use rung::ladder;
use rung_std::agent::AgentResult;
use rung_std::decide::{Ask, Decided, Decider, JevDecider, Question, Undecided};
use rung_std::llm::{ChatMessage, MessageContent, MessageContentBlock};
use serde::Serialize;
use serde_json::{Value, json};

use crate::config::{TurnCheckBackend, TurnCheckSettings};
use crate::mcp::redact;

/// The one message a narrating turn is sent before it is checked again.
pub const NUDGE: &str = "Your last message says something was done that no tool call in this turn did. \
Do it now with the tools, or say plainly that it was not done.";

/// Estimated-token ceiling for a state (bytes / 3). Jev's context is 32K;
/// this leaves room for the questions. Above it the turn is Unchecked.
pub const STATE_TOKEN_LIMIT: usize = 24_000;

const REQUEST_CHARS: usize = 2_000;
const FINAL_CHARS: usize = 2_000;
// An action's input and result keep head and tail: a heredoc command ends in
// the upload that matters, and a result opens with what it found.
const INPUT_CHARS: usize = 300;
const RESULT_CHARS: usize = 400;
const KEEP_FIRST_ACTIONS: usize = 10;
const KEEP_LAST_ACTIONS: usize = 50;
const PRIOR_ACTIONS: usize = 20;

// Gate thresholds (Jev plan §4(b); Phase 0 met its bar with them unchanged).
const ACT_MIN_CONFIDENCE: f64 = 0.7;
const ACT_MAX_CLAIMS: f64 = 0.3;
const ACT_MAX_HIDES: f64 = 0.3;
const ASK_MIN_CLAIMS: f64 = 0.8;
const ASK_MIN_NARRATED_CONFIDENCE: f64 = 0.7;
const PRIOR_SUPPRESSES_ASK: f64 = 0.8;

// ─── Switch ──────────────────────────────────────────────────────────────────

/// The check is switched off. Held only by [`Gate::Off`]; the witness that
/// lets an unjudged turn report "completed" as it did before this rung.
#[derive(Debug)]
pub struct Unjudged {
    _seal: (),
}

impl Unjudged {
    pub fn completion(&self) -> Completion {
        Completion {
            basis: Basis::SwitchedOff,
        }
    }
}

/// What the switch resolved to.
#[derive(Debug)]
pub enum Gate {
    Off(Unjudged),
    On(Arc<dyn Decider>),
}

impl Gate {
    /// Build the gate from settings. `jev` reads its key from the named env
    /// var; a missing key still yields a decider, which answers `Unchecked`.
    pub fn from_settings(s: &TurnCheckSettings) -> Self {
        match s.backend {
            TurnCheckBackend::Off => Gate::Off(Unjudged { _seal: () }),
            TurnCheckBackend::Jev => {
                let key = std::env::var(&s.api_key_env).unwrap_or_default();
                Gate::On(Arc::new(JevDecider::new(
                    &s.base_url,
                    &key,
                    &s.model,
                    Duration::from_secs(s.timeout_secs),
                )))
            }
        }
    }
}

// ─── Completion ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Basis {
    Checked,
    SwitchedOff,
}

/// Why a turn may be called completed. Built only by [`Checked::completion`]
/// and [`Unjudged::completion`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    basis: Basis,
}

impl Completion {
    /// True when a judge checked the turn; false when the switch is off.
    pub fn was_checked(&self) -> bool {
        self.basis == Basis::Checked
    }
}

// ─── Reading ─────────────────────────────────────────────────────────────────

/// What the judge said about one turn. Reported on `Outcome.turn_check` and
/// in ACP `_meta.rung.turn_check`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TurnReading {
    pub outcome: String,
    pub confidence: f64,
    pub claims_unperformed: f64,
    pub request_needs_action: f64,
    pub hides_failure: f64,
    pub relies_on_prior_turn: f64,
    /// The versioned model that served the reading.
    pub model: String,
    pub cost_usd: f64,
}

impl TurnReading {
    fn from_decided(d: &Decided) -> Option<Self> {
        let (outcome, confidence, _) = d.choice("outcome")?;
        Some(Self {
            outcome: outcome.to_string(),
            confidence,
            claims_unperformed: d.noul("claims_unperformed_action")?,
            request_needs_action: d.noul("request_needs_action")?,
            hides_failure: d.noul("hides_failure")?,
            relies_on_prior_turn: d.noul("relies_on_prior_turn")?,
            model: d.model.clone(),
            cost_usd: d.usage.cost_usd,
        })
    }
}

/// The turn check's part of an outcome.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TurnCheckReport {
    /// The turn was nudged once and checked again.
    pub nudged: bool,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub reading: Option<TurnReading>,
    /// Why no reading came back (Unchecked only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Which arm the gate takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    Act,
    Ask,
    Escalate,
}

/// Facts code knows without asking: they decide which speculative answers
/// are read at all.
#[derive(Debug, Clone, Copy, Default)]
pub struct Facts {
    /// Some action in the turn errored (a tool error, or a shell exit ≠ 0).
    pub any_error: bool,
    /// Earlier turns in the session took actions.
    pub has_prior: bool,
    /// This turn was already nudged once.
    pub nudged: bool,
}

/// The confidence gate. Pure; the thresholds are the constants above.
pub fn arm(r: &TurnReading, f: Facts) -> Arm {
    let hides = if f.any_error { r.hides_failure } else { 0.0 };
    let prior = if f.has_prior {
        r.relies_on_prior_turn
    } else {
        0.0
    };
    let settled = matches!(
        r.outcome.as_str(),
        "done" | "answered" | "asked_user" | "blocked"
    );
    if settled
        && r.confidence >= ACT_MIN_CONFIDENCE
        && r.claims_unperformed <= ACT_MAX_CLAIMS
        && hides <= ACT_MAX_HIDES
    {
        return Arm::Act;
    }
    let narrating = r.claims_unperformed >= ASK_MIN_CLAIMS
        || (r.outcome == "narrated" && r.confidence >= ASK_MIN_NARRATED_CONFIDENCE);
    if narrating && !f.nudged && prior < PRIOR_SUPPRESSES_ASK {
        return Arm::Ask;
    }
    Arm::Escalate
}

// ─── Questions ───────────────────────────────────────────────────────────────

/// Every question the gate might need, in one request. The wording is the
/// Phase 0 probe's, on which the thresholds were measured; rewording it
/// forces the fixtures to be recorded again.
pub fn questions() -> BTreeMap<String, Question> {
    let mut q = BTreeMap::new();
    q.insert(
        "claims_unperformed_action".into(),
        Question::noul(
            "Does `final_message` say something was done (a file written, a note kept, a command run, a message sent, a change made) that no entry in `actions` actually did?",
            "The message claims an action that no entry in `actions` performed.",
            "Every action the message claims is backed by an entry in `actions`, or the message claims no action.",
        ),
    );
    q.insert(
        "request_needs_action".into(),
        Question::noul(
            "Does `request` ask for something to be created, changed, sent or run, rather than only for information?",
            "The request asks for a side effect in the world.",
            "The request only asks for information or an answer.",
        ),
    );
    q.insert(
        "outcome".into(),
        Question::choice(
            "Which best describes the outcome of this turn, judging `final_message` against `actions`?",
            &[
                (
                    "done",
                    "The request asked for action and `actions` actually performed what the message claims.",
                ),
                (
                    "answered",
                    "The request asked for information and `final_message` provides it.",
                ),
                (
                    "asked_user",
                    "`final_message` asks the user a clarifying question or for a decision instead of acting.",
                ),
                (
                    "blocked",
                    "`final_message` says the work could not be done because of an error or missing access, and does not claim success.",
                ),
                (
                    "narrated",
                    "`final_message` claims work was done that `actions` do not show.",
                ),
            ],
        ),
    );
    q.insert(
        "hides_failure".into(),
        Question::noul(
            "Does `final_message` say the work succeeded although an entry in `actions` has `result` \"error\" that was never fixed?",
            "An unresolved error is hidden behind a success claim.",
            "No error is hidden.",
        ),
    );
    q.insert(
        "relies_on_prior_turn".into(),
        Question::noul(
            "Does `final_message` refer to work done earlier, listed in `prior_turn_actions`, rather than in this turn?",
            "The message refers to earlier-turn work.",
            "It does not.",
        ),
    );
    q
}

// ─── State ───────────────────────────────────────────────────────────────────

/// Cut `s` to `max` chars, keeping its head and tail.
fn clip(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let head = max * 3 / 5;
    let tail = max - head;
    let h: String = s.chars().take(head).collect();
    let t: String = s.chars().skip(n - tail).collect();
    format!("{h} […{} chars…] {t}", n - max)
}

/// A shell result ends `[exit: N]`; N ≠ 0 is an error, whatever `is_error` says.
fn shell_failed(content: &str) -> bool {
    content
        .trim_end()
        .rsplit_once("[exit: ")
        .and_then(|(_, rest)| rest.strip_suffix(']'))
        .and_then(|n| n.trim().parse::<i64>().ok())
        .is_some_and(|code| code != 0)
}

/// Tool names used by earlier turns, most recent last.
pub fn prior_actions(earlier: &[ChatMessage]) -> Vec<String> {
    let mut names: Vec<String> = earlier
        .iter()
        .flat_map(|m| match &m.content {
            MessageContent::Blocks(b) => b.clone(),
            _ => Vec::new(),
        })
        .filter_map(|b| match b {
            MessageContentBlock::ToolUse { name, .. } => Some(name),
            _ => None,
        })
        .collect();
    let skip = names.len().saturating_sub(PRIOR_ACTIONS);
    names.drain(..skip);
    names
}

/// Build the judge's state for one turn. Every string passes through
/// [`redact`] before it is cut, so a cut never leaves half a secret.
///
/// `turn` is the messages this turn added; `final_message` is its last text.
/// Returns the state and whether any action errored.
pub fn turn_state(
    request: &str,
    turn: &[ChatMessage],
    prior: &[String],
    final_message: &str,
) -> (Value, bool) {
    let mut calls: Vec<(String, String, String)> = Vec::new();
    let mut results: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for m in turn {
        let MessageContent::Blocks(blocks) = &m.content else {
            continue;
        };
        for b in blocks {
            match b {
                MessageContentBlock::ToolUse {
                    id, name, input, ..
                } => {
                    let input = serde_json::to_string(input).unwrap_or_default();
                    calls.push((id.clone(), name.clone(), input));
                }
                MessageContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } => {
                    let failed = *is_error || shell_failed(content);
                    results.insert(tool_use_id.clone(), (content.clone(), failed));
                }
                _ => {}
            }
        }
    }
    let mut any_error = false;
    let mut actions: Vec<Value> = calls
        .iter()
        .map(|(id, name, input)| {
            let (content, failed) = results
                .get(id)
                .cloned()
                .unwrap_or_else(|| ("(no result)".into(), true));
            any_error |= failed;
            json!({
                "tool": name,
                "input": clip(&redact(input), INPUT_CHARS),
                "result": if failed { "error" } else { "ok" },
                "result_excerpt": clip(&redact(content.trim()), RESULT_CHARS),
            })
        })
        .collect();
    let mut elided = 0;
    if actions.len() > KEEP_FIRST_ACTIONS + KEEP_LAST_ACTIONS {
        elided = actions.len() - KEEP_FIRST_ACTIONS - KEEP_LAST_ACTIONS;
        actions.drain(KEEP_FIRST_ACTIONS..KEEP_FIRST_ACTIONS + elided);
    }
    let prior: Vec<String> = prior.iter().map(|s| redact(s)).collect();
    let state = json!({
        "request": clip(&redact(request), REQUEST_CHARS),
        "actions": actions,
        "actions_elided": elided,
        "prior_turn_actions": prior,
        "final_message": clip(&redact(final_message), FINAL_CHARS),
    });
    (state, any_error)
}

/// The full ask for one turn.
pub fn turn_ask(
    request: &str,
    turn: &[ChatMessage],
    prior: &[String],
    final_message: &str,
) -> (Ask, bool) {
    let (state, any_error) = turn_state(request, turn, prior, final_message);
    (
        Ask {
            state,
            questions: questions(),
        },
        any_error,
    )
}

// ─── Ladder payloads ─────────────────────────────────────────────────────────

/// Proof that a turn was nudged. Only [`Turn::after_nudge`] makes one.
#[derive(Debug)]
struct NudgeReceipt;

/// A finished agent turn, waiting for its check.
#[derive(Debug)]
pub struct Turn {
    result: AgentResult,
    /// Messages before this turn in `result.transcript`.
    sent: usize,
    nudge: Option<NudgeReceipt>,
}

impl Turn {
    /// A turn as the loop ended it. `sent` is how many messages it was given.
    pub fn first(result: AgentResult, sent: usize) -> Self {
        Self {
            result,
            sent,
            nudge: None,
        }
    }

    /// The same turn after its one nudge: `result` is the re-run's.
    pub fn after_nudge(nudged: Nudged, result: AgentResult) -> Self {
        Self {
            result,
            sent: nudged.sent,
            nudge: Some(NudgeReceipt),
        }
    }

    pub fn into_result(self) -> AgentResult {
        self.result
    }

    fn messages(&self) -> &[ChatMessage] {
        let at = self.sent.min(self.result.transcript.len());
        &self.result.transcript[at..]
    }
}

/// The check passed. Holds the turn and the reading that passed it.
#[derive(Debug)]
pub struct Checked {
    result: AgentResult,
    report: TurnCheckReport,
}

impl Checked {
    pub fn completion(&self) -> Completion {
        Completion {
            basis: Basis::Checked,
        }
    }
    pub fn report(&self) -> &TurnCheckReport {
        &self.report
    }
    pub fn into_result(self) -> AgentResult {
        self.result
    }
}

/// The turn narrated; re-run it once with [`NUDGE`].
#[derive(Debug)]
pub struct Nudged {
    result: AgentResult,
    sent: usize,
    reading: TurnReading,
}

impl Nudged {
    pub fn reading(&self) -> &TurnReading {
        &self.reading
    }
    pub fn result(&self) -> &AgentResult {
        &self.result
    }
    /// The conversation to re-run: everything so far, then the nudge.
    pub fn rerun_messages(&self) -> Vec<ChatMessage> {
        let mut m = self.result.transcript.clone();
        m.push(ChatMessage::user(NUDGE));
        m
    }
    /// Give up on the re-run (it failed): the first reading escalates.
    pub fn into_flagged(self) -> Flagged {
        Flagged {
            result: self.result,
            report: TurnCheckReport {
                nudged: true,
                reading: Some(self.reading),
                reason: None,
            },
        }
    }
}

/// Unverified: read, and not passed. The text is kept.
#[derive(Debug)]
pub struct Flagged {
    result: AgentResult,
    report: TurnCheckReport,
}

impl Flagged {
    pub fn report(&self) -> &TurnCheckReport {
        &self.report
    }
    pub fn into_result(self) -> AgentResult {
        self.result
    }
}

/// Unchecked: no reading. The turn is untouched.
#[derive(Debug)]
pub struct Unread {
    result: AgentResult,
    report: TurnCheckReport,
}

impl Unread {
    pub fn reason(&self) -> Option<&str> {
        self.report.reason.as_deref()
    }
    pub fn report(&self) -> &TurnCheckReport {
        &self.report
    }
    pub fn into_result(self) -> AgentResult {
        self.result
    }
}

fn unread(result: AgentResult, nudged: bool, why: &Undecided) -> Unread {
    eprintln!("[rung-agent] turn check: not checked ({why})");
    Unread {
        result,
        report: TurnCheckReport {
            nudged,
            reading: None,
            reason: Some(why.to_string()),
        },
    }
}

// ─── Ladder ──────────────────────────────────────────────────────────────────

ladder!(TurnCheck {
    carry {
        decider: Arc<dyn Decider>,
        request: String,
        prior_actions: Vec<String>,
    }

    Ended(Turn)
      => {
          Completed(Checked)
          | Nudge(Nudged)
          | Unverified(Flagged)
          | Unchecked(Unread)
      }
} impl {
    // The verb on the arrow: the judge is asked here and nowhere else.
    step = |ended| {
        let carry = ended.carry().clone();
        let turn = ended.payload;
        let nudged = turn.nudge.is_some();
        let (ask, any_error) = turn_ask(
            &carry.request,
            turn.messages(),
            &carry.prior_actions,
            &turn.result.final_response,
        );
        if ask.estimated_tokens() > STATE_TOKEN_LIMIT {
            return Ok(StepOutcome::Unchecked(Unchecked::new(unread(
                turn.result, nudged, &Undecided::TooLarge,
            ))));
        }
        let decided = match carry.decider.decide(&ask) {
            Ok(d) => d,
            Err(why) => {
                return Ok(StepOutcome::Unchecked(Unchecked::new(unread(
                    turn.result, nudged, &why,
                ))));
            }
        };
        let Some(reading) = TurnReading::from_decided(&decided) else {
            let why = Undecided::Malformed("an asked question is missing from the answers".into());
            return Ok(StepOutcome::Unchecked(Unchecked::new(unread(
                turn.result, nudged, &why,
            ))));
        };
        let facts = Facts {
            any_error,
            has_prior: !carry.prior_actions.is_empty(),
            nudged,
        };
        let report = |reading: TurnReading| TurnCheckReport {
            nudged,
            reading: Some(reading),
            reason: None,
        };
        Ok(match arm(&reading, facts) {
            Arm::Act => StepOutcome::Completed(Completed::new(Checked {
                result: turn.result,
                report: report(reading),
            })),
            // `arm` never asks for a nudged turn, so a turn is nudged at most once.
            Arm::Ask => StepOutcome::Nudge(Nudge::new(Nudged {
                result: turn.result,
                sent: turn.sent,
                reading,
            })),
            Arm::Escalate => StepOutcome::Unverified(Unverified::new(Flagged {
                result: turn.result,
                report: report(reading),
            })),
        })
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(outcome: &str, conf: f64, cu: f64) -> TurnReading {
        TurnReading {
            outcome: outcome.into(),
            confidence: conf,
            claims_unperformed: cu,
            request_needs_action: 0.9,
            hides_failure: 0.05,
            relies_on_prior_turn: 0.05,
            model: "m".into(),
            cost_usd: 0.0,
        }
    }

    #[test]
    fn the_gate_takes_each_arm() {
        let f = Facts::default();
        assert_eq!(arm(&reading("done", 0.95, 0.05), f), Arm::Act);
        assert_eq!(arm(&reading("narrated", 1.0, 0.97), f), Arm::Ask);
        assert_eq!(arm(&reading("done", 0.5, 0.05), f), Arm::Escalate);
        // The 0.72 band: neither clean nor narrating.
        assert_eq!(arm(&reading("blocked", 0.95, 0.72), f), Arm::Escalate);
    }

    #[test]
    fn a_nudged_turn_is_never_nudged_again() {
        let f = Facts {
            nudged: true,
            ..Facts::default()
        };
        assert_eq!(arm(&reading("narrated", 1.0, 0.97), f), Arm::Escalate);
    }

    #[test]
    fn hides_failure_is_read_only_when_an_action_errored() {
        let mut r = reading("done", 0.95, 0.05);
        r.hides_failure = 0.9;
        assert_eq!(arm(&r, Facts::default()), Arm::Act);
        let f = Facts {
            any_error: true,
            ..Facts::default()
        };
        assert_eq!(arm(&r, f), Arm::Escalate);
    }

    #[test]
    fn relying_on_a_prior_turn_suppresses_the_nudge() {
        let mut r = reading("narrated", 1.0, 0.97);
        r.relies_on_prior_turn = 0.9;
        let f = Facts {
            has_prior: true,
            ..Facts::default()
        };
        assert_eq!(arm(&r, f), Arm::Escalate);
        assert_eq!(arm(&r, Facts::default()), Arm::Ask);
    }

    #[test]
    fn a_nonzero_shell_exit_is_an_error() {
        assert!(shell_failed("boom\n[exit: 1]"));
        assert!(shell_failed("[stderr]\nx\n[exit: 128]\n"));
        assert!(!shell_failed("ok\n[exit: 0]"));
        assert!(!shell_failed("wrote 5 bytes"));
    }

    #[test]
    fn clip_keeps_head_and_tail() {
        let s: String = (0..5000)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let c = clip(&s, 2000);
        assert!(c.starts_with(&s[..1200]));
        assert!(c.ends_with(&s[s.len() - 800..]));
        assert!(c.contains("[…3000 chars…]"));
    }
}
