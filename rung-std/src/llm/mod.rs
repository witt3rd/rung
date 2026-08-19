//! One blocking LLM call, with retry — the first canonical building block.
//!
//! The verb lives on the arrow: a state cannot call an endpoint, only a
//! transition can (`the-law`). Provider quirks live in [`anthropic`] and
//! [`openai`]; callers see one request, one verdict.

mod anthropic;
mod cache;
mod error;
mod openai;
mod sse;
mod types;

pub use error::{HttpContext, LlmFailure, RawCallError, RequestClassification, retry_delay_ms};
pub use types::{
    CacheHint, CachePolicy, ChatMessage, ContentBlock, ContentBlockDelta, ContentBlockStart,
    DEFAULT_MAX_ATTEMPTS, ImageSource, LlmConfig, LlmRequest, LlmResponse, MessageContent,
    MessageContentBlock, PreparedRequest, Protocol, ResolvedProtocol, StopReason, StreamEvent,
    StreamListener, ToolDefinition, Usage,
};

use rung::ladder;

/// Dispatch a single request. Protocol is resolved from [`LlmConfig::protocol`],
/// not from sniffing the hostname (except `Protocol::Auto`).
pub fn raw_call(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<LlmResponse, RawCallError> {
    match config.resolved_protocol() {
        ResolvedProtocol::AnthropicMessages => anthropic::raw_call(config, messages, tools),
        ResolvedProtocol::OpenAiChat => openai::raw_call(config, messages, tools),
    }
}

/// Compile the provider-native body without sending. For inspection and tests.
pub fn prepare(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<PreparedRequest, RawCallError> {
    match config.resolved_protocol() {
        ResolvedProtocol::AnthropicMessages => anthropic::prepare(config, messages, tools),
        ResolvedProtocol::OpenAiChat => openai::prepare(config, messages, tools),
    }
}

// Canonical rung ladder for a blocking LLM call with retry.
// Pending(LlmRequest) → Success(LlmResponse) | LlmError(LlmFailure)
// recover: retry: Failed(Pending) => Pending
ladder!(LlmCall {
    carry { call_id: String }

    Pending(LlmRequest)
      => {
          Success(LlmResponse)
          | LlmError(LlmFailure)
      }

    recover {
        retry: Failed(Pending) => Pending
    }
} impl {
    step = |pending| {
        if pending.payload.attempts_remaining == 0 {
            return Ok(StepOutcome::LlmError(LlmError::new(
                LlmFailure::MaxRetries {
                    last_error: "attempts counter reached zero".into(),
                },
            )));
        }
        match raw_call(&pending.payload.config, &pending.payload.messages, &pending.payload.tools) {
            Ok(response) => Ok(StepOutcome::Success(Success::new(response))),
            Err(e) if e.is_retryable() => {
                let attempt_index =
                    DEFAULT_MAX_ATTEMPTS.saturating_sub(pending.payload.attempts_remaining) as u32;
                let delay = error::retry_delay_ms(
                    attempt_index,
                    e.retry_after_ms(),
                    e.is_rate_limited(),
                );
                let msg = e.to_string();
                let mut token = pending;
                token.payload.next_delay_ms = Some(delay);
                Err(Failed { token, error: msg })
            }
            Err(e) => Ok(StepOutcome::LlmError(LlmError::new(LlmFailure::from_raw(e)))),
        }
    },

    retry = |f| {
        let carry = f.token.carry().clone();
        let call_id = carry.call_id.clone();
        let mut req = f.token.payload;
        let attempt_index =
            DEFAULT_MAX_ATTEMPTS.saturating_sub(req.attempts_remaining) as u32;
        req.attempts_remaining = req.attempts_remaining.saturating_sub(1);
        if req.attempts_remaining > 0 {
            let delay_ms = req
                .next_delay_ms
                .take()
                .unwrap_or_else(|| error::retry_delay_ms(attempt_index, None, false));
            eprintln!("[rung-std] {call_id}: retrying after {delay_ms}ms (attempt {attempt_index})");
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
        Pending::new(req, carry)
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_protocol_sniffs_anthropic_host() {
        assert_eq!(
            Protocol::Auto.resolve("https://api.anthropic.com/v1"),
            ResolvedProtocol::AnthropicMessages
        );
        assert_eq!(
            Protocol::Auto.resolve("http://localhost:20128/v1"),
            ResolvedProtocol::OpenAiChat
        );
        assert_eq!(
            Protocol::AnthropicMessages.resolve("http://localhost:20128/v1"),
            ResolvedProtocol::AnthropicMessages
        );
    }

    #[test]
    fn prepare_openai_compatible_proxy() {
        let config = LlmConfig {
            base_url: "http://localhost:20128/v1".into(),
            api_key: "k".into(),
            model: "anthropic/claude-sonnet-4-5".into(),
            timeout_secs: 30,
            idle_timeout_secs: None,
            max_tokens: 64,
            temperature: None,
            top_p: None,
            top_k: None,
            seed: None,
            stop: vec![],
            reasoning_level: None,
            structured_outputs: false,
            protocol: Protocol::OpenAiChat,
            cache: CachePolicy::None,
            stream_listener: None,
        };
        let p = prepare(&config, &[ChatMessage::user("hi")], &[]).unwrap();
        assert!(p.url.ends_with("/chat/completions"));
    }
}
