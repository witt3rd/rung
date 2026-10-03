//! The pack: the agent's context as a cache artefact.
//!
//! ```text
//! STABLE  tool superset · system (identity, host contract, free-time rules, pinned memory)
//! SLOW    one message, fixed for the epoch: header, carried note, register digest, previous-epoch outline, kept segments
//! LOG     turn header · the turn's messages, verbatim · next header · …   (append-only)
//! ```
//!
//! Inside an epoch nothing is ever rewritten: each request is the previous
//! one plus what came after it. A rollover starts a new epoch (and a new
//! session id) and rebuilds the slow layer; the stable layer changes only by
//! a recorded swap. The request bytes the host reasons about are the
//! canonical serialization [`request_bytes`] — tools, system, then one line
//! per message — the same bytes a provider cache sees.

use rung_std::agent::Thread;
use rung_std::llm::{ChatMessage, MessageContent, MessageContentBlock, ToolDefinition};
use serde::Serialize;

use crate::canon;
use crate::desk::pack::Segment;

/// The canonical bytes of one request: the tools, the system text, then
/// each message, one per line. A request that extends another by appended
/// messages has the other's bytes as a prefix.
pub fn request_bytes(tools: &[ToolDefinition], system: &str, messages: &[ChatMessage]) -> Vec<u8> {
    let mut out = canon::of(tools);
    out.push(b'\n');
    out.extend(canon::of(system));
    for m in messages {
        out.push(b'\n');
        out.extend(canon::of(m));
    }
    out
}

/// The text of a message, flattened (for outlines and kept segments).
pub fn flat_text(m: &ChatMessage) -> String {
    match &m.content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(bs) => bs
            .iter()
            .map(|b| match b {
                MessageContentBlock::Text { text, .. } => text.clone(),
                MessageContentBlock::ToolUse { name, input, .. } => {
                    format!("[call {name} {}]", canon::string(input))
                }
                MessageContentBlock::ToolResult { content, .. } => format!("[result {content}]"),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// A turn's span in the log.
#[derive(Debug, Clone, Serialize)]
pub struct Span {
    pub turn: u64,
    /// Index of its header message.
    pub start: usize,
    /// One past its last message.
    pub end: usize,
    pub bytes: usize,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct Pack {
    tools: Vec<ToolDefinition>,
    system: String,
    stable_bytes: usize,
    s_hash: String,
    pub epoch: u64,
    slow: String,
    slow_bytes: usize,
    l_hash: String,
    log: Vec<ChatMessage>,
    /// Canonical bytes of each log message.
    log_bytes: Vec<usize>,
    log_total: usize,
    spans: Vec<Span>,
    pub budget: usize,
}

impl Pack {
    /// A pack with its stable layer and a first (empty) epoch.
    pub fn new(tools: Vec<ToolDefinition>, system: String, budget: usize) -> Self {
        let mut p = Self {
            tools,
            system,
            stable_bytes: 0,
            s_hash: String::new(),
            epoch: 0,
            slow: String::new(),
            slow_bytes: 0,
            l_hash: String::new(),
            log: Vec::new(),
            log_bytes: Vec::new(),
            log_total: 0,
            spans: Vec::new(),
            budget,
        };
        p.restable();
        p
    }

    fn restable(&mut self) {
        let mut b = canon::of(&self.tools);
        b.push(b'\n');
        b.extend(canon::of(&self.system));
        self.stable_bytes = b.len();
        self.s_hash = canon::hash(&b);
    }

    /// Replace the stable layer (a batched swap; record it).
    pub fn swap(&mut self, tools: Vec<ToolDefinition>, system: String) {
        self.tools = tools;
        self.system = system;
        self.restable();
    }

    /// Start epoch `epoch` with slow text `slow`; the log empties.
    pub fn rollover(&mut self, epoch: u64, slow: String) {
        self.epoch = epoch;
        let msg = ChatMessage::user(slow.clone());
        let b = canon::of(&msg);
        self.slow_bytes = b.len() + 1;
        self.l_hash = canon::hash(&b);
        self.slow = slow;
        self.log.clear();
        self.log_bytes.clear();
        self.log_total = 0;
        self.spans.clear();
    }

    fn push(&mut self, m: ChatMessage) {
        let n = canon::of(&m).len() + 1;
        self.log_bytes.push(n);
        self.log_total += n;
        self.log.push(m);
    }

    /// Append a turn's header (the only new bytes at a boundary).
    pub fn begin_turn(&mut self, turn: u64, kind: &str, header: String) {
        let start = self.log.len();
        self.push(ChatMessage::user(header));
        self.spans.push(Span {
            turn,
            start,
            end: self.log.len(),
            bytes: *self.log_bytes.last().unwrap_or(&0),
            kind: kind.into(),
        });
    }

    /// Append what the turn produced, verbatim.
    pub fn end_turn(&mut self, messages: Vec<ChatMessage>) {
        for m in messages {
            self.push(m);
        }
        let end = self.log.len();
        let bytes: usize = match self.spans.last() {
            Some(s) => self.log_bytes[s.start..end].iter().sum(),
            None => 0,
        };
        if let Some(s) = self.spans.last_mut() {
            s.end = end;
            s.bytes = bytes;
        }
    }

    /// The thread for the engine: the system text, the slow message, the log.
    pub fn thread(&self) -> Thread {
        let mut messages = Vec::with_capacity(self.log.len() + 1);
        messages.push(ChatMessage::user(self.slow.clone()));
        messages.extend(self.log.iter().cloned());
        Thread {
            system_prompt: self.system.clone(),
            messages,
        }
    }

    pub fn tools(&self) -> &[ToolDefinition] {
        &self.tools
    }

    pub fn system(&self) -> &str {
        &self.system
    }

    /// Bytes of the current request.
    pub fn bytes(&self) -> usize {
        self.stable_bytes + self.slow_bytes + self.log_total
    }

    /// Estimated tokens of the current request.
    pub fn tokens(&self) -> usize {
        canon::tokens(self.bytes())
    }

    pub fn stable_tokens(&self) -> usize {
        canon::tokens(self.stable_bytes)
    }

    pub fn slow_tokens(&self) -> usize {
        canon::tokens(self.slow_bytes)
    }

    pub fn s_hash(&self) -> &str {
        &self.s_hash
    }

    pub fn l_hash(&self) -> &str {
        &self.l_hash
    }

    pub fn log_len_bytes(&self) -> usize {
        self.log_total
    }

    pub fn log(&self) -> &[ChatMessage] {
        &self.log
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub fn turns_in_epoch(&self) -> u64 {
        self.spans.len() as u64
    }

    /// The epoch's turns grouped into at most `max` segments of equal turn
    /// counts, oldest first. `is_ref(first, last)` marks the ones an open
    /// commitment or expectation refers to.
    pub fn segments(&self, max: usize, is_ref: &dyn Fn(u64, u64) -> (bool, bool)) -> Vec<Segment> {
        if self.spans.is_empty() || max == 0 {
            return Vec::new();
        }
        let per = self.spans.len().div_ceil(max);
        self.spans
            .chunks(per)
            .enumerate()
            .map(|(i, ch)| {
                let first = ch[0].turn;
                let last = ch[ch.len() - 1].turn;
                let bytes: usize = ch.iter().map(|s| s.bytes).sum();
                let text: String = ch
                    .iter()
                    .flat_map(|s| self.log[s.start..s.end].iter())
                    .map(flat_text)
                    .collect::<Vec<_>>()
                    .join(" ");
                let (c, e) = is_ref(first, last);
                Segment {
                    id: format!("s{}", i + 1),
                    first_turn: first,
                    last_turn: last,
                    tokens: canon::tokens(bytes),
                    gist: text.chars().take(150).collect(),
                    referenced_by_open_commitment: c,
                    referenced_by_open_expectation: e,
                }
            })
            .collect()
    }

    /// The verbatim text of the segments `ids` (as [`Pack::segments`] named
    /// them), for the next epoch's slow layer.
    pub fn segment_text(&self, max: usize, ids: &[String]) -> String {
        if self.spans.is_empty() || max == 0 {
            return String::new();
        }
        let per = self.spans.len().div_ceil(max);
        let mut out = String::new();
        for (i, ch) in self.spans.chunks(per).enumerate() {
            if !ids.contains(&format!("s{}", i + 1)) {
                continue;
            }
            for s in ch {
                for m in &self.log[s.start..s.end] {
                    out.push_str(&format!("{}: {}\n", m.role, flat_text(m)));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> Pack {
        let tools = crate::toolbox::superset(true);
        let mut p = Pack::new(tools, "system".into(), 10_000);
        p.rollover(1, "slow".into());
        p
    }

    #[test]
    fn the_request_only_grows_inside_an_epoch() {
        let mut p = pack();
        let mut prev = request_bytes(p.tools(), p.system(), &p.thread().messages);
        assert_eq!(prev.len(), p.bytes(), "the host counts the request bytes");
        for t in 1..5 {
            p.begin_turn(t, "free", format!("[turn {t}]"));
            p.end_turn(vec![ChatMessage::assistant(format!("answer {t}"))]);
            let now = request_bytes(p.tools(), p.system(), &p.thread().messages);
            assert!(now.starts_with(&prev));
            assert_eq!(now.len(), p.bytes());
            prev = now;
        }
        let (s, l) = (p.s_hash().to_string(), p.l_hash().to_string());
        p.rollover(2, "slow 2".into());
        assert_eq!(p.s_hash(), s);
        assert_ne!(p.l_hash(), l);
        assert!(p.log().is_empty());
    }

    #[test]
    fn segments_cover_the_epoch() {
        let mut p = pack();
        for t in 1..=25 {
            p.begin_turn(t, "free", format!("[turn {t}]"));
            p.end_turn(vec![ChatMessage::assistant("x")]);
        }
        let segs = p.segments(10, &|_, _| (false, false));
        assert!(segs.len() <= 10);
        assert_eq!(segs[0].first_turn, 1);
        assert_eq!(segs.last().unwrap().last_turn, 25);
        let text = p.segment_text(10, &["s1".into()]);
        assert!(text.contains("[turn 1]") && !text.contains("[turn 25]"));
    }
}
