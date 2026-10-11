//! Retain distillation: keep what a later recall could use, drop the rest.
//!
//! A turn is offered to retain as its user text and the assistant's final
//! answer. Much of that is noise to recall: fenced code, tool chatter
//! ("Running cargo test…"), openers ("Sure, let me…"), and turns that carry
//! nothing durable ("thanks"). [`distill`] drops code and chatter, trims the
//! assistant side to its conclusion, and returns `None` for a turn with no
//! durable content. It is opt-in; the default retains the turn whole.

use crate::provider::{Body, Observation};

/// Acknowledgements that carry nothing to recall.
const ACKS: &[&str] = &[
    "ok",
    "okay",
    "thanks",
    "thank you",
    "thx",
    "hi",
    "hello",
    "hey",
    "yes",
    "no",
    "yep",
    "nope",
    "great",
    "cool",
    "got it",
    "continue",
    "go on",
    "next",
    "done",
    "lgtm",
];

/// Line openers that are chatter about doing, not a result.
const CHATTER: &[&str] = &[
    "running ",
    "ran ",
    "calling ",
    "reading ",
    "searching ",
    "opening ",
    "let me ",
    "i'll ",
    "i will ",
    "sure",
    "okay",
    "ok,",
    "$ ",
    "> ",
    "tool:",
    "output:",
    "exit code",
    "you're welcome",
    "anything else",
];

/// `Some(observation)` trimmed, or `None` when the turn has no durable
/// content. Notes (the agent chose to keep them) pass through unchanged.
pub fn distill(o: &Observation) -> Option<Observation> {
    let Body::Turn { user, assistant } = &o.body else {
        return Some(o.clone());
    };
    let user = strip_code(user);
    let user = user.trim();
    let user = if is_ack(user) { "" } else { user };
    let assistant = conclusion(assistant);
    if assistant.is_empty() && user.split_whitespace().count() < 4 {
        return None;
    }
    Some(Observation {
        body: Body::Turn {
            user: user.to_string(),
            assistant,
        },
        attrs: o.attrs.clone(),
    })
}

fn is_ack(text: &str) -> bool {
    let t: String = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    t.is_empty() || ACKS.contains(&t.as_str())
}

/// Remove fenced code blocks (an unclosed fence runs to the end).
fn strip_code(text: &str) -> String {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if !fenced {
            out.push(line);
        }
    }
    out.join("\n")
}

/// The assistant's text without code or chatter, every paragraph whole.
fn conclusion(text: &str) -> String {
    let cleaned: Vec<String> = strip_code(text)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| {
            let low = l.to_lowercase();
            !CHATTER.iter().any(|c| low.starts_with(c))
        })
        .collect();
    let paras: Vec<String> = cleaned
        .split(|l| l.is_empty())
        .map(|p| p.join(" "))
        .filter(|p| !p.is_empty())
        .collect();
    paras.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(user: &str, assistant: &str) -> Observation {
        Observation {
            body: Body::Turn {
                user: user.into(),
                assistant: assistant.into(),
            },
            attrs: Default::default(),
        }
    }

    #[test]
    fn a_bare_ack_keeps_the_assistants_durable_conclusion() {
        let kept = distill(&turn("thanks", "The release freeze now starts Thursday.")).unwrap();
        let Body::Turn { user, assistant } = kept.body else {
            panic!()
        };
        assert!(user.is_empty() && assistant.contains("freeze"));
        assert!(distill(&turn("thanks", "Okay, noted.")).is_none());
    }

    #[test]
    fn a_long_conclusion_is_kept_whole() {
        let long = format!("{} end-marker.", "The plan holds. ".repeat(2_000));
        let kept = distill(&turn("what is the plan", &long)).unwrap();
        let Body::Turn { assistant, .. } = kept.body else {
            panic!()
        };
        assert_eq!(assistant, long.trim());
    }
}
