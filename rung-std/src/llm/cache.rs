//! Prompt-cache placement. Runs before protocol lowering so each wire format
//! only has to honour `CacheHint`s already on the parts.

use super::types::{CacheHint, CachePolicy, ChatMessage, MessageContent, ToolDefinition};

const ANTHROPIC_BREAKPOINT_CAP: u32 = 4;

/// Mutable budget of remaining Anthropic `cache_control` markers (max 4).
#[derive(Debug)]
pub struct Breakpoints {
    remaining: u32,
    pub dropped: u32,
}

impl Breakpoints {
    pub fn new() -> Self {
        Self {
            remaining: ANTHROPIC_BREAKPOINT_CAP,
            dropped: 0,
        }
    }

    /// Take one breakpoint if a hint is present. Returns the wire object, or
    /// `None` if the cap is exhausted (hint is dropped, never 400s the API).
    pub fn take(&mut self, hint: Option<CacheHint>) -> Option<serde_json::Value> {
        let hint = hint?;
        if self.remaining == 0 {
            self.dropped += 1;
            return None;
        }
        self.remaining -= 1;
        if hint.is_hour() {
            Some(serde_json::json!({"type": "ephemeral", "ttl": "1h"}))
        } else {
            Some(serde_json::json!({"type": "ephemeral"}))
        }
    }
}

/// Clone tools/messages and stamp `CacheHint`s for `CachePolicy::Auto`.
pub fn apply(policy: CachePolicy, tools: &mut [ToolDefinition], messages: &mut [ChatMessage]) {
    if policy != CachePolicy::Auto {
        return;
    }
    let hint = CacheHint::ephemeral();

    if let Some(last) = tools.last_mut()
        && last.cache.is_none()
    {
        last.cache = Some(hint);
    }

    // Last system message (text or last block).
    if let Some(sys) = messages.iter_mut().rev().find(|m| m.role == "system") {
        mark_message(sys, hint);
    }

    // Latest user message — the load-bearing boundary for tool-use loops.
    if let Some(user) = messages.iter_mut().rev().find(|m| m.role == "user") {
        mark_message(user, hint);
    }
}

fn mark_message(msg: &mut ChatMessage, hint: CacheHint) {
    match &mut msg.content {
        MessageContent::Text(text) => {
            let t = std::mem::take(text);
            msg.content = MessageContent::Blocks(vec![super::types::MessageContentBlock::Text {
                text: t,
                cache: Some(hint),
            }]);
        }
        MessageContent::Blocks(blocks) => {
            if let Some(last) = blocks.last_mut()
                && last.cache_mut().is_none()
            {
                *last.cache_mut() = Some(hint);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{ChatMessage, ToolDefinition};

    #[test]
    fn auto_marks_last_tool_system_and_user() {
        let mut tools = vec![
            ToolDefinition::new("a", "a", serde_json::json!({})),
            ToolDefinition::new("b", "b", serde_json::json!({})),
        ];
        let mut msgs = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("hello"),
            ChatMessage::assistant("hi"),
            ChatMessage::user("again"),
        ];
        apply(CachePolicy::Auto, &mut tools, &mut msgs);
        assert!(tools[0].cache.is_none());
        assert!(tools[1].cache.is_some());
        match &msgs[0].content {
            MessageContent::Blocks(b) => {
                assert!(matches!(
                    b[0],
                    crate::llm::types::MessageContentBlock::Text { cache: Some(_), .. }
                ));
            }
            _ => panic!("system should be promoted to blocks"),
        }
        match &msgs[3].content {
            MessageContent::Blocks(b) => assert!(matches!(
                b[0],
                crate::llm::types::MessageContentBlock::Text { cache: Some(_), .. }
            )),
            _ => panic!("latest user should be marked"),
        }
        match &msgs[1].content {
            MessageContent::Text(_) => {}
            MessageContent::Blocks(b) => {
                assert!(matches!(
                    b[0],
                    crate::llm::types::MessageContentBlock::Text { cache: None, .. }
                ));
            }
        }
    }

    #[test]
    fn none_places_nothing() {
        let mut tools = vec![ToolDefinition::new("a", "a", serde_json::json!({}))];
        let mut msgs = vec![ChatMessage::user("x")];
        apply(CachePolicy::None, &mut tools, &mut msgs);
        assert!(tools[0].cache.is_none());
        assert!(matches!(msgs[0].content, MessageContent::Text(_)));
    }

    #[test]
    fn breakpoints_cap_at_four() {
        let mut bp = Breakpoints::new();
        let hint = Some(CacheHint::ephemeral());
        for _ in 0..4 {
            assert!(bp.take(hint).is_some());
        }
        assert!(bp.take(hint).is_none());
        assert_eq!(bp.dropped, 1);
    }
}
