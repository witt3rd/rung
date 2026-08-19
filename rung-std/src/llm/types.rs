//! Canonical request/response types for one LLM call.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub const DEFAULT_MAX_ATTEMPTS: u8 = 3;

// ─── Usage ────────────────────────────────────────────────────────────────────

/// Token-usage counters from the provider response.
///
/// **Inclusive totals** (OpenAI / LangChain convention):
/// - `input_tokens` includes cache reads and writes.
/// - `output_tokens` includes reasoning / thinking tokens.
///
/// **Breakdown** (non-overlapping):
/// - `non_cached_input_tokens + cache_read_input_tokens + cache_creation_input_tokens`
///   equals `input_tokens` (clamped against provider bugs).
///
/// Anthropic reports the breakdown natively (`input_tokens` is non-cached);
/// we sum to derive the inclusive total. OpenAI reports inclusive
/// `prompt_tokens`; we subtract cached tokens for the breakdown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub non_cached_input_tokens: u32,
    pub cache_read_input_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub thinking_tokens: u32,
    pub service_tier: Option<String>,
}

impl Usage {
    pub fn from_anthropic(
        non_cached: u32,
        cache_read: u32,
        cache_write: u32,
        output: u32,
        thinking: u32,
        service_tier: Option<String>,
    ) -> Self {
        let input = non_cached
            .saturating_add(cache_read)
            .saturating_add(cache_write);
        Self {
            input_tokens: input,
            output_tokens: output,
            non_cached_input_tokens: non_cached,
            cache_read_input_tokens: cache_read,
            cache_creation_input_tokens: cache_write,
            thinking_tokens: thinking,
            service_tier,
        }
    }

    pub fn from_openai(
        prompt: u32,
        completion: u32,
        cached: u32,
        reasoning: u32,
    ) -> Self {
        let cached = cached.min(prompt);
        Self {
            input_tokens: prompt,
            output_tokens: completion,
            non_cached_input_tokens: prompt.saturating_sub(cached),
            cache_read_input_tokens: cached,
            cache_creation_input_tokens: 0,
            thinking_tokens: reasoning.min(completion),
            service_tier: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    Refusal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Thinking { thinking: String, signature: String },
}

#[must_use = "LlmResponse carries the model's output and usage; dropping it silently loses both — handle it"]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub usage: Usage,
    pub model: String,
    pub id: String,
}

// ─── Protocol / cache / generation ────────────────────────────────────────────

/// Which wire format to use. `Auto` infers Anthropic Messages from an
/// `anthropic.com` host and OpenAI Chat otherwise — wrong for Claude-via-proxy,
/// so callers should set this explicitly when the host is not the vendor's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    #[default]
    Auto,
    #[serde(alias = "anthropic")]
    AnthropicMessages,
    #[serde(alias = "openai", alias = "openai-compatible")]
    OpenAiChat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedProtocol {
    AnthropicMessages,
    OpenAiChat,
}

impl Protocol {
    pub fn resolve(self, base_url: &str) -> ResolvedProtocol {
        match self {
            Self::AnthropicMessages => ResolvedProtocol::AnthropicMessages,
            Self::OpenAiChat => ResolvedProtocol::OpenAiChat,
            Self::Auto => {
                if base_url.contains("anthropic.com") {
                    ResolvedProtocol::AnthropicMessages
                } else {
                    ResolvedProtocol::OpenAiChat
                }
            }
        }
    }
}

/// Prompt-cache placement. `Auto` marks last tool, last system part, and the
/// latest user message (Anthropic/Bedrock explicit cache). OpenAI/Gemini ignore
/// the markers (implicit caching).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CachePolicy {
    #[default]
    Auto,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheHint {
    pub ttl_seconds: Option<u32>,
}

impl CacheHint {
    pub fn ephemeral() -> Self {
        Self { ttl_seconds: None }
    }

    pub fn is_hour(self) -> bool {
        self.ttl_seconds.unwrap_or(0) >= 3600
    }
}

// ─── Config ───────────────────────────────────────────────────────────────────

pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_secs: u64,
    /// Per-chunk idle deadline while reading a stream. `None` → `timeout_secs`.
    pub idle_timeout_secs: Option<u64>,
    pub max_tokens: u32,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub seed: Option<i64>,
    pub stop: Vec<String>,
    pub reasoning_level: Option<String>,
    pub structured_outputs: bool,
    pub protocol: Protocol,
    pub cache: CachePolicy,
    pub stream_listener: Option<Arc<dyn StreamListener>>,
}

impl Clone for LlmConfig {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            timeout_secs: self.timeout_secs,
            idle_timeout_secs: self.idle_timeout_secs,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            top_p: self.top_p,
            top_k: self.top_k,
            seed: self.seed,
            stop: self.stop.clone(),
            reasoning_level: self.reasoning_level.clone(),
            structured_outputs: self.structured_outputs,
            protocol: self.protocol,
            cache: self.cache,
            stream_listener: self.stream_listener.clone(),
        }
    }
}

impl std::fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"[redacted]")
            .field("model", &self.model)
            .field("timeout_secs", &self.timeout_secs)
            .field("idle_timeout_secs", &self.idle_timeout_secs)
            .field("max_tokens", &self.max_tokens)
            .field("temperature", &self.temperature)
            .field("top_p", &self.top_p)
            .field("top_k", &self.top_k)
            .field("seed", &self.seed)
            .field("stop", &self.stop)
            .field("reasoning_level", &self.reasoning_level)
            .field("structured_outputs", &self.structured_outputs)
            .field("protocol", &self.protocol)
            .field("cache", &self.cache)
            .field(
                "stream_listener",
                if self.stream_listener.is_some() {
                    &"Some(...)"
                } else {
                    &"None"
                },
            )
            .finish()
    }
}

impl LlmConfig {
    pub fn resolved_protocol(&self) -> ResolvedProtocol {
        self.protocol.resolve(&self.base_url)
    }

    pub fn idle_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.idle_timeout_secs.unwrap_or(self.timeout_secs).max(1))
    }
}

// ─── Messages ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum MessageContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip)]
        cache: Option<CacheHint>,
    },
    #[serde(rename = "image")]
    Image {
        source: ImageSource,
        #[serde(skip)]
        cache: Option<CacheHint>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        #[serde(skip)]
        cache: Option<CacheHint>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip)]
        cache: Option<CacheHint>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        signature: String,
        #[serde(skip)]
        cache: Option<CacheHint>,
    },
}

impl MessageContentBlock {
    pub fn cache_mut(&mut self) -> &mut Option<CacheHint> {
        match self {
            Self::Text { cache, .. }
            | Self::Image { cache, .. }
            | Self::ToolUse { cache, .. }
            | Self::ToolResult { cache, .. }
            | Self::Thinking { cache, .. } => cache,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<MessageContentBlock>),
}

impl MessageContent {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s.as_str()),
            MessageContent::Blocks(_) => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: MessageContent,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: MessageContent::Text(content.into()),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: MessageContent::Text(content.into()),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: MessageContent::Text(content.into()),
        }
    }
    pub fn user_with_blocks(blocks: Vec<MessageContentBlock>) -> Self {
        Self {
            role: "user".into(),
            content: MessageContent::Blocks(blocks),
        }
    }
    pub fn assistant_with_blocks(blocks: Vec<MessageContentBlock>) -> Self {
        Self {
            role: "assistant".into(),
            content: MessageContent::Blocks(blocks),
        }
    }
    pub fn tool_result(tool_use_id: &str, result: &str) -> Self {
        Self {
            role: "user".into(),
            content: MessageContent::Blocks(vec![MessageContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: result.into(),
                cache: None,
            }]),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(skip)]
    pub cache: Option<CacheHint>,
}

impl ToolDefinition {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            cache: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub config: LlmConfig,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub attempts_remaining: u8,
    /// Set by `step` from a retryable error's `Retry-After` before recover.
    pub next_delay_ms: Option<u64>,
}

impl LlmRequest {
    pub fn new(config: LlmConfig, messages: Vec<ChatMessage>) -> Self {
        Self {
            config,
            messages,
            tools: Vec::new(),
            attempts_remaining: DEFAULT_MAX_ATTEMPTS,
            next_delay_ms: None,
        }
    }
}

// ─── Streaming ────────────────────────────────────────────────────────────────

pub trait StreamListener: Send + Sync {
    fn on_event(&self, event: StreamEvent);
}

pub enum StreamEvent {
    MessageStart { model: String, id: String },
    ContentBlockStart {
        index: usize,
        block: ContentBlockStart,
    },
    ContentBlockDelta {
        index: usize,
        delta: ContentBlockDelta,
    },
    ContentBlockStop { index: usize },
    MessageDelta {
        stop_reason: Option<StopReason>,
        usage: Option<Usage>,
    },
    MessageStop,
}

pub enum ContentBlockStart {
    Text,
    ToolUse { id: String, name: String },
    Thinking,
}

pub enum ContentBlockDelta {
    TextDelta(String),
    InputJsonDelta(String),
    ThinkingDelta(String),
    SignatureDelta(String),
}

/// Marks that the caller (or the assembler) has already seen tokens, so retry
/// must not re-fire.
pub struct ObservingListener {
    pub inner: Option<Arc<dyn StreamListener>>,
    pub observed: Arc<AtomicBool>,
}

impl StreamListener for ObservingListener {
    fn on_event(&self, event: StreamEvent) {
        self.observed.store(true, Ordering::SeqCst);
        if let Some(inner) = &self.inner {
            inner.on_event(event);
        }
    }
}

/// Body that would be sent, without performing I/O.
#[derive(Debug, Clone)]
pub struct PreparedRequest {
    pub protocol: ResolvedProtocol,
    pub url: String,
    pub body: serde_json::Value,
}

/// Shared mapping of Anthropic `stop_reason` strings.
pub fn map_anthropic_stop_reason(s: Option<&str>) -> StopReason {
    match s {
        Some("end_turn") => StopReason::EndTurn,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("stop_sequence") => StopReason::StopSequence,
        Some("tool_use") => StopReason::ToolUse,
        Some("refusal") => StopReason::Refusal,
        _ => StopReason::EndTurn,
    }
}

/// Shared mapping of OpenAI `finish_reason` strings.
pub fn map_openai_finish_reason(s: Option<&str>) -> StopReason {
    match s {
        Some("stop") => StopReason::EndTurn,
        Some("tool_calls") | Some("function_call") => StopReason::ToolUse,
        Some("length") => StopReason::MaxTokens,
        Some("content_filter") => StopReason::Refusal,
        _ => StopReason::EndTurn,
    }
}

