//! Compaction: bounded context is a rung, and "compacted" is reached only by
//! summarizing.
//!
//! rung-agent replays every session line each turn. Without this step the only
//! relief is the one-shot elision of old tool results, and a second overflow
//! ends the turn. This step measures the history against the model window
//! and, past a threshold, replaces the old lines with one summary line
//! (`role: "summary"`, replayed as a fixed user/assistant pair) and keeps
//! the recent tail whole.
//!
//! ```text
//! Measured(Window) => { Rolled(Compacted) | Within(Fits) | Refused(Unsummarized) }
//! ```
//!
//! - **Rolled**: the summarizer ran and the history shrank. Only the step
//!   builds a [`Compacted`], so a caller cannot claim a compaction that did
//!   not happen.
//! - **Within**: under the threshold, or nothing old enough to fold. Untouched.
//! - **Refused**: the summarizer failed, answered nothing, or the result would
//!   not be smaller. The lines are returned untouched and the reason is kept;
//!   a failed compaction is never reported as done.
//!
//! A host that rolls over at an epoch boundary supplies its own
//! [`Summarizer`]; the prompt below is rung's own.

use std::sync::Arc;

use rung::ladder;
use serde::Serialize;

use crate::session::Line;

/// Role of the line a compaction leaves in place of the lines it folded.
pub const SUMMARY_ROLE: &str = "summary";

/// The user text a replayed summary is shown under.
pub const SUMMARY_PREFIX: &str = "[Summary of the earlier conversation, replacing its lines]\n";

/// What the model says after a replayed summary, so roles still alternate.
pub const SUMMARY_ACK: &str = "Understood. I will continue from that summary.";

/// rung's instruction to the summarizer.
pub const SUMMARY_PROMPT: &str = "You are condensing the start of a long working session so it can continue \
without it. Write what a successor needs, in plain prose and short lists: \
the goal and every constraint the user set; facts, names, numbers, paths and \
identifiers stated, exactly as stated; decisions made and why; what was done \
and its result; what is unfinished and the next step. Keep exact values \
verbatim. Leave out pleasantries and tool output that no longer matters. \
Do not invent. A summary line above this text is itself part of the session: \
fold its content in.";

/// Thresholds, in estimated tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// The model's window.
    pub window_tokens: usize,
    /// Compact when the history reaches this percent of the window.
    pub threshold_pct: usize,
    /// The recent tail kept whole.
    pub keep_recent_tokens: usize,
}

impl Policy {
    pub fn for_window(window_tokens: usize) -> Self {
        Self {
            window_tokens,
            threshold_pct: 80,
            keep_recent_tokens: window_tokens / 5,
        }
    }

    fn limit(&self) -> usize {
        self.window_tokens / 100 * self.threshold_pct.min(100)
    }
}

/// Estimated tokens of a line: bytes / 3 of its JSON, a deliberately high
/// estimate (the same rule the decider uses).
pub fn line_tokens(l: &Line) -> usize {
    serde_json::to_string(l).map_or(0, |s| s.len() / 3)
}

/// Estimated tokens of a history.
pub fn history_tokens(lines: &[Line]) -> usize {
    lines.iter().map(line_tokens).sum()
}

/// Writes the summary of the lines being folded.
pub trait Summarizer: Send + Sync + std::fmt::Debug {
    fn summarize(&self, folded: &[Line]) -> Result<String, String>;
}

/// The lines as the summarizer is shown them: [`SUMMARY_PROMPT`], then each
/// line whole.
pub fn summary_request(folded: &[Line]) -> String {
    let mut out = String::from(SUMMARY_PROMPT);
    out.push_str("\n\n--- session ---\n");
    for l in folded {
        out.push_str(&format!("[{}] {}\n", l.role, l.text));
        for m in l.messages.iter().flatten() {
            if let Ok(s) = serde_json::to_string(m) {
                out.push_str(&format!("  step: {s}\n"));
            }
        }
    }
    out
}

/// A history to measure.
#[derive(Debug)]
pub struct Window {
    lines: Vec<Line>,
}

impl Window {
    pub fn new(lines: Vec<Line>) -> Self {
        Self { lines }
    }
}

/// What a compaction did, for the record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Record {
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub lines_folded: usize,
    pub lines_kept: usize,
}

/// The history was compacted. Only the step makes one.
#[derive(Debug)]
pub struct Compacted {
    lines: Vec<Line>,
    record: Record,
}

impl Compacted {
    pub fn record(&self) -> &Record {
        &self.record
    }
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
    pub fn into_lines(self) -> Vec<Line> {
        self.lines
    }
}

/// Under the threshold, or nothing to fold. Untouched.
#[derive(Debug)]
pub struct Fits {
    lines: Vec<Line>,
    tokens: usize,
}

impl Fits {
    pub fn tokens(&self) -> usize {
        self.tokens
    }
    pub fn into_lines(self) -> Vec<Line> {
        self.lines
    }
}

/// Over the threshold and not compacted. Untouched, with the reason.
#[derive(Debug)]
pub struct Unsummarized {
    lines: Vec<Line>,
    reason: String,
}

impl Unsummarized {
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn into_lines(self) -> Vec<Line> {
        self.lines
    }
}

/// Index where the kept tail starts: the earliest user line such that the
/// tail from it fits `keep`. 0 when nothing can be folded.
fn cut_point(lines: &[Line], keep: usize) -> usize {
    let mut used = 0;
    let mut cut = lines.len();
    for (i, l) in lines.iter().enumerate().rev() {
        used += line_tokens(l);
        if used > keep && cut < lines.len() {
            break;
        }
        if l.role == "user" {
            cut = i;
        }
    }
    if cut == lines.len() { 0 } else { cut }
}

ladder!(Compaction {
    carry {
        policy: Policy,
        summarizer: Arc<dyn Summarizer>,
    }

    Measured(Window)
      => {
          Rolled(Compacted)
          | Within(Fits)
          | Refused(Unsummarized)
      }
} impl {
    step = |measured| {
        let carry = measured.carry().clone();
        let lines = measured.payload.lines;
        let before = history_tokens(&lines);
        let fits = |lines: Vec<Line>| {
            Ok(StepOutcome::Within(Within::new(Fits { lines, tokens: before })))
        };
        if before < carry.policy.limit() {
            return fits(lines);
        }
        let cut = cut_point(&lines, carry.policy.keep_recent_tokens);
        if cut == 0 {
            return fits(lines);
        }
        let refuse = |lines: Vec<Line>, reason: String| {
            Ok(StepOutcome::Refused(Refused::new(Unsummarized { lines, reason })))
        };
        let summary = match carry.summarizer.summarize(&lines[..cut]) {
            Ok(s) if !s.trim().is_empty() => s,
            Ok(_) => return refuse(lines, "the summarizer answered nothing".into()),
            Err(e) => return refuse(lines, format!("the summarizer failed: {e}")),
        };
        let mut out = vec![Line {
            role: SUMMARY_ROLE.into(),
            text: summary,
            messages: None,
            failure: None,
            recalled: None,
        }];
        out.extend(lines[cut..].iter().cloned());
        let after = history_tokens(&out);
        if after >= before {
            return refuse(lines, "the summary is not smaller than what it replaces".into());
        }
        Ok(StepOutcome::Rolled(Rolled::new(Compacted {
            record: Record {
                tokens_before: before,
                tokens_after: after,
                lines_folded: cut,
                lines_kept: out.len() - 1,
            },
            lines: out,
        })))
    },
});

/// Run the step over `lines`.
pub fn compact(
    lines: Vec<Line>,
    policy: Policy,
    summarizer: Arc<dyn Summarizer>,
) -> compaction::StepOutcome {
    let m = compaction::Measured::new(Window::new(lines), compaction::Carry { policy, summarizer });
    match compaction::step(m) {
        Ok(o) => o,
        // The step returns no error: a refusal is a verdict.
        Err(_) => unreachable!("the compaction step returns no error"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Remembers every `fact:` it is shown, and writes them verbatim.
    #[derive(Debug)]
    struct Scripted {
        calls: Mutex<usize>,
        fail: bool,
    }

    impl Summarizer for Scripted {
        fn summarize(&self, folded: &[Line]) -> Result<String, String> {
            *self.calls.lock().unwrap() += 1;
            if self.fail {
                return Err("unreachable".into());
            }
            let facts: Vec<&str> = folded
                .iter()
                .flat_map(|l| l.text.lines())
                .filter(|t| t.starts_with("fact:"))
                .collect();
            Ok(format!("Facts stated:\n{}", facts.join("\n")))
        }
    }

    fn scripted(fail: bool) -> Arc<Scripted> {
        Arc::new(Scripted {
            calls: Mutex::new(0),
            fail,
        })
    }

    fn session(turns: usize) -> Vec<Line> {
        let mut v = Vec::new();
        for i in 0..turns {
            let ask = if i == 0 {
                "fact: the deploy key is kiwi-42".to_string()
            } else {
                format!("turn {i}: {}", "padding ".repeat(40))
            };
            v.push(Line::user(ask));
            v.push(Line::assistant(format!("ok {i} {}", "reply ".repeat(40))));
        }
        v
    }

    fn replay(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn rolled(o: compaction::StepOutcome) -> Compacted {
        match o {
            compaction::StepOutcome::Rolled(r) => r.into_payload(),
            _ => panic!("not rolled"),
        }
    }

    #[test]
    fn a_200_turn_session_stays_inside_the_window_and_recalls_an_early_fact() {
        let window = 8_000;
        let policy = Policy::for_window(window);
        let s = scripted(false);
        let mut lines = Vec::new();
        let mut rollovers = 0;
        let mut peak = 0;
        for i in 0..200 {
            let mut fresh = session(1);
            if i > 0 {
                fresh = vec![
                    Line::user(format!("turn {i}: {}", "padding ".repeat(40))),
                    Line::assistant(format!("ok {i} {}", "reply ".repeat(40))),
                ];
            }
            lines.extend(fresh);
            match compact(std::mem::take(&mut lines), policy, s.clone()) {
                compaction::StepOutcome::Rolled(r) => {
                    rollovers += 1;
                    lines = r.into_payload().into_lines();
                }
                compaction::StepOutcome::Within(f) => lines = f.into_payload().into_lines(),
                compaction::StepOutcome::Refused(u) => panic!("{}", u.payload().reason()),
            }
            peak = peak.max(history_tokens(&lines));
        }
        assert!(rollovers >= 2, "rolled {rollovers} times");
        assert!(peak <= window, "peak {peak} passed the window {window}");
        // Without compaction 200 turns would be far past the window.
        assert!(history_tokens(&session(200)) > window * 3);
        // A fact stated at turn 0 survives two or more rollovers.
        assert!(replay(&lines).contains("kiwi-42"));
        assert_eq!(lines[0].role, SUMMARY_ROLE);
        assert_eq!(lines.iter().filter(|l| l.role == SUMMARY_ROLE).count(), 1);
        assert_eq!(lines.last().unwrap().role, "assistant");
    }

    #[test]
    fn under_the_threshold_nothing_is_summarized() {
        let s = scripted(false);
        let o = compact(session(3), Policy::for_window(100_000), s.clone());
        assert!(matches!(o, compaction::StepOutcome::Within(_)));
        assert_eq!(*s.calls.lock().unwrap(), 0);
    }

    #[test]
    fn a_failed_summarizer_is_refused_and_the_lines_come_back_whole() {
        let lines = session(40);
        let n = lines.len();
        match compact(lines, Policy::for_window(2_000), scripted(true)) {
            compaction::StepOutcome::Refused(u) => {
                let u = u.into_payload();
                assert!(u.reason().contains("unreachable"));
                assert_eq!(u.into_lines().len(), n);
            }
            _ => panic!("not refused"),
        }
    }

    #[test]
    fn a_summary_that_is_not_smaller_is_refused() {
        #[derive(Debug)]
        struct Bloat;
        impl Summarizer for Bloat {
            fn summarize(&self, _: &[Line]) -> Result<String, String> {
                Ok("x".repeat(1_000_000))
            }
        }
        let o = compact(session(40), Policy::for_window(2_000), Arc::new(Bloat));
        assert!(matches!(o, compaction::StepOutcome::Refused(_)));
    }

    #[test]
    fn the_record_matches_the_lines() {
        let lines = session(40);
        let before = history_tokens(&lines);
        let c = rolled(compact(lines, Policy::for_window(4_000), scripted(false)));
        let r = c.record().clone();
        assert_eq!(r.tokens_before, before);
        assert_eq!(r.tokens_after, history_tokens(c.lines()));
        assert_eq!(r.lines_folded + r.lines_kept, 80);
        // The kept tail starts at a user line.
        assert_eq!(c.lines()[1].role, "user");
    }
}
