//! Canonical `LlmCall` rung ladder for reuse across any rung-based project.
//!
//! ## What this is
//!
//! A two-rung ladder (Pending → verdict) that wraps a single blocking HTTP
//! request to any OpenAI-compatible endpoint, including Anthropic's native
//! `/v1/messages` API.  Retryable failures surface as `Err(Failed)` → the
//! `retry` recover edge applies exponential backoff and decrements the
//! attempt counter.  Terminal failures exit as `Ok(LlmError(LlmFailure))`.
//! Success exits as `Ok(Success(LlmResponse))`.
//!
//! ## Configuration — full parity
//!
//! [`LlmConfig`] carries every parameter that may affect request behaviour:
//!
//! | Field | Wire |
//! |---|---|
//! | `base_url` | selects provider path + Anthropic vs. OpenAI format |
//! | `api_key` | `x-api-key` (Anthropic) or `Authorization: Bearer` (OpenAI) |
//! | `model` | `model` field in request body |
//! | `timeout_secs` | HTTP client timeout |
//! | `max_tokens` | `max_tokens` in request body |
//! | `temperature` | `temperature` in request body (both providers) |
//! | `reasoning_level` | `thinking.budget_tokens` (Anthropic) / `reasoning_effort` (OpenAI o-series) |
//!
//! ## How this ladder uses rung's guarantees
//!
//! A reader new to rung should see the distinctive features in play here.
//!
//! - **G2 (sealed construction).** Only the entry rung `Pending::new` is public.
//!   The `Success` / `LlmError` verdicts are built by `step` *inside* the module
//!   — no caller can fabricate a terminal outcome. The sealed constructor is not
//!   merely a fabrication guard; it is the free-category axiom: a verb (the HTTP
//!   call) lives on the arrow (`step`'s body), never in object-position
//!   (constructing a verdict from outside) — see `docs/RUNG-CT.md` §1.
//!
//! - **G7/G9 (recover pairing — error-path semantics).** The single recover edge
//!   `retry: Failed(Pending) => Pending` is an **error-path** recovery (SPEC.md G9):
//!   it re-enters with the *unconsumed token* handed back in `Failed`, and it is
//!   deliberately **unguarded** — a retry after a transient network failure may
//!   legitimately re-send the *identical* request. This is the mirror of Lesson 2's
//!   verdict recover (`Stalled => Active`), which *is* guarded (G8) because a stall
//!   loop must make progress. The two recover forms exist for different intents.
//!
//! - **G5 (carry immutability).** `call_id` is carried on every rung as a private
//!   field, readable only through `&Carry`. The recover edge threads it forward
//!   unchanged — witness data is structurally shared, never mutated in flight.
//!
//! - **G4 (no silent drop).** Every generated token — `Pending`, `Success`,
//!   `LlmError`, `Failed`, `StepOutcome` — is `#[must_use]`. Dropping any of them
//!   without consuming or recovering it is a warning, and an error under
//!   `#![deny(unused_must_use)]`. The non-token `LlmResponse` below carries the
//!   same attribute as a style exemplar.
//!
//! - **G11 (terminal payloads).** `Success(LlmResponse)` carries the structured
//!   result out through the verdict; the caller reads it via `.payload()`.
//!
//! - **Streaming is a side-channel, not a rung.** The `StreamListener` receives
//!   incremental SSE events as a read-only notification. The ladder still blocks
//!   until the stream ends and then resolves to a verdict — streaming does not
//!   introduce a new transition. The verb (HTTP I/O) remains on the arrow.
//!
//! ## Membership criteria (rung-std)
//!
//! This ladder is rung-std because:
//! 1. It recurs across independent domain projects (garden-ladders, inner-loop, …)
//! 2. Its canonical two-rung shape (request-construction rung + verdict-routing
//!    rung) is domain-generic — no caller-specific knowledge is embedded.

use rung::ladder;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ─── Response types ───────────────────────────────────────────────────────────

/// Token-usage counters from the provider response.
///
/// Billing totals: `input_tokens` + `output_tokens` (plus cache-creation tokens).
/// The `output_tokens` field is the authoritative billing count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// Tokens read from the prompt cache (cheaper than input tokens).
    pub cache_read_input_tokens: u32,
    /// Tokens written to the prompt cache.
    pub cache_creation_input_tokens: u32,
    /// Tokens spent on internal reasoning / extended thinking.
    pub thinking_tokens: u32,
    /// Provider service tier: `"standard"`, `"priority"`, or `"batch"`.
    pub service_tier: Option<String>,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StopReason {
    /// Natural stopping point — the model finished its turn.
    EndTurn,
    /// Exceeded `max_tokens` or the model's context limit.
    MaxTokens,
    /// A custom stop sequence was matched.
    StopSequence,
    /// The model invoked one or more tools.
    ToolUse,
    /// Classifier intervened; the model refused to continue.
    Refusal,
}

/// One content block in a model response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentBlock {
    /// A plain-text block.
    Text { text: String },
    /// A client-side tool invocation.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Extended-thinking reasoning text (with a cryptographically-signed
    /// signature that must be re-submitted in multi-turn conversations).
    Thinking { thinking: String, signature: String },
}

/// The full structured response from one LLM call.
///
/// `#[must_use]` follows the same no-silent-drop idiom rung emits on every
/// verdict token (SPEC.md G4): a response dropped silently loses both the
/// model's output and the token-usage accounting. Carrying the attribute here
/// on a non-token result type makes the pattern visible to readers.
#[must_use = "LlmResponse carries the model's output and usage; dropping it silently loses both — handle it"]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// All content blocks in the order the model produced them.
    pub content: Vec<ContentBlock>,
    /// Why the model stopped.
    pub stop_reason: StopReason,
    /// Token-usage counters.
    pub usage: Usage,
    /// The model that produced this response (e.g. `"claude-sonnet-5-20251001"`).
    pub model: String,
    /// Provider-assigned message identifier.
    pub id: String,
}

// ─── Configuration ───────────────────────────────────────────────────────────

/// Maximum HTTP attempts before the ladder terminates with [`LlmFailure::MaxRetries`].
pub const DEFAULT_MAX_ATTEMPTS: u8 = 3;

/// Full configuration for one LLM call.
///
/// Build this once per judge selection and carry it in [`LlmRequest`].
/// All optional fields are `None` → not sent in the request body.
#[derive(Clone)]
pub struct LlmConfig {
    /// Base URL of the provider endpoint, without trailing slash.
    ///
    /// Examples:
    /// - `https://api.anthropic.com/v1`
    /// - `https://api.openai.com/v1`
    /// - `http://localhost:20128/v1`  (local OmniRoute proxy)
    pub base_url: String,

    /// Authentication credential.
    /// - Anthropic: sent as `x-api-key` header.
    /// - OpenAI-compatible: sent as `Authorization: Bearer <key>`.
    pub api_key: String,

    /// Model identifier sent verbatim in the request body.
    pub model: String,

    /// HTTP request timeout in seconds.  Applies to the full response body,
    /// not just the connection.  Increase for large `max_tokens` values.
    pub timeout_secs: u64,

    /// Maximum number of tokens in the generated response.
    pub max_tokens: u32,

    /// Sampling temperature.  `None` → provider default (usually 1.0).
    /// `0.0` makes sampling near-deterministic (greedy).
    pub temperature: Option<f64>,

    /// Provider-specific reasoning level hint.
    ///
    /// - **Anthropic extended thinking** — mapped to `thinking.budget_tokens`:
    ///   `"low"` → 1 024, `"medium"` → 8 192, `"high"` → 32 768.
    ///   Requires a Claude 3.7+ model; also sets `temperature: 1` per Anthropic
    ///   docs (the field is forced to `1` when extended thinking is enabled).
    ///
    /// - **OpenAI o-series** — passed verbatim as `reasoning_effort`:
    ///   `"low"`, `"medium"`, or `"high"`.
    ///
    /// - **All other models** — silently ignored.
    pub reasoning_level: Option<String>,

    /// Optional streaming-side-channel listener.
    ///
    /// When `Some`, the HTTP response is read as SSE and each event is
    /// forwarded to the listener. The ladder still blocks until the stream
    /// ends, then returns the assembled `Success(LlmResponse)` — the listener
    /// is purely a read-only notification side-channel.
    pub stream_listener: Option<Arc<dyn StreamListener>>,
}

impl std::fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"[redacted]")
            .field("model", &self.model)
            .field("timeout_secs", &self.timeout_secs)
            .field("max_tokens", &self.max_tokens)
            .field("temperature", &self.temperature)
            .field("reasoning_level", &self.reasoning_level)
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

// ─── Message types ────────────────────────────────────────────────────────────

/// One block in a message's structured content.
///
/// Used when a message carries more than plain text — images, tool results, etc.
/// Serializes with a `"type"` discriminator per the Anthropic content-block wire
/// format.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum MessageContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// Source metadata for an image content block.
#[derive(Debug, Clone, Serialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String, // "base64"
    pub media_type: String, // "image/png", "image/jpeg", ...
    pub data: String,       // base64-encoded image bytes
}

/// The content of a chat message — either simple text or a list of content blocks.
///
/// Serializes untagged: `Text("hello")` becomes just the string `"hello"`, while
/// `Blocks([...])` becomes the JSON array directly.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<MessageContentBlock>),
}

impl MessageContent {
    /// Returns the text if this is a `Text` variant, `None` otherwise.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s.as_str()),
            MessageContent::Blocks(_) => None,
        }
    }
}

/// One chat message — role + content.
///
/// For Anthropic requests, `system` messages are extracted and sent in the
/// top-level `system` field; all other roles travel in `messages`.
///
/// Content can be a simple string (`ChatMessage::user("Hello")`) or a list of
/// content blocks for multi-modal / tool-result messages.
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: MessageContent,
}

impl ChatMessage {
    /// Plain-text system prompt.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: MessageContent::Text(content.into()),
        }
    }
    /// Plain-text user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: MessageContent::Text(content.into()),
        }
    }
    /// Plain-text assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: MessageContent::Text(content.into()),
        }
    }
    /// User message with structured content blocks (images, tool results, etc.).
    pub fn user_with_blocks(blocks: Vec<MessageContentBlock>) -> Self {
        Self {
            role: "user".into(),
            content: MessageContent::Blocks(blocks),
        }
    }
    /// Convenience: a user-role tool-result message (Anthropic wire format).
    pub fn tool_result(tool_use_id: &str, result: &str) -> Self {
        Self {
            role: "user".into(),
            content: MessageContent::Blocks(vec![MessageContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: result.into(),
            }]),
        }
    }
}

// ─── Tool definition ──────────────────────────────────────────────────────────

/// A tool the model may invoke.
///
/// `input_schema` is a JSON Schema object describing the tool's parameters.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ─── Rung payload ────────────────────────────────────────────────────────────

/// Everything needed for one LLM call, plus the remaining-attempts counter
/// that the `retry` recover edge decrements before each backoff sleep.
#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub config: LlmConfig,
    pub messages: Vec<ChatMessage>,
    /// Tool definitions sent to the model.  Empty → no tools available.
    pub tools: Vec<ToolDefinition>,
    /// How many more HTTP attempts are allowed.
    /// Initialise to [`DEFAULT_MAX_ATTEMPTS`].
    pub attempts_remaining: u8,
}

impl LlmRequest {
    /// Convenience constructor — uses [`DEFAULT_MAX_ATTEMPTS`].
    pub fn new(config: LlmConfig, messages: Vec<ChatMessage>) -> Self {
        Self {
            config,
            messages,
            tools: Vec::new(),
            attempts_remaining: DEFAULT_MAX_ATTEMPTS,
        }
    }
}

// ─── Failure types ───────────────────────────────────────────────────────────

/// Terminal failure — why the call ultimately could not succeed.
/// Rides the `LlmError` verdict out of the ladder.
#[derive(Debug, Clone)]
pub enum LlmFailure {
    /// HTTP 401 Unauthorized — bad key or missing auth.  Non-retryable.
    Auth(String),
    /// All retry attempts exhausted after retryable failures.
    MaxRetries { last_error: String },
    /// Required configuration was absent (e.g. empty `base_url`).
    Config(String),
}

impl std::fmt::Display for LlmFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmFailure::Auth(e) => write!(f, "auth (401): {e}"),
            LlmFailure::MaxRetries { last_error } => {
                write!(f, "max retries exhausted — last error: {last_error}")
            }
            LlmFailure::Config(e) => write!(f, "config: {e}"),
        }
    }
}

/// Single-attempt HTTP error, before retry logic applies.
/// Not `Clone` — not carried in rung tokens; classified immediately.
#[derive(Debug)]
pub enum RawCallError {
    Http(String),
    Auth(String),
    RateLimit,
    Server { status: u16, body: String },
    NoContent,
}

impl RawCallError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            RawCallError::RateLimit | RawCallError::Http(_) | RawCallError::Server { .. }
        )
    }
}

impl std::fmt::Display for RawCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RawCallError::Http(e) => write!(f, "http error: {e}"),
            RawCallError::Auth(e) => write!(f, "auth error: {e}"),
            RawCallError::RateLimit => write!(f, "rate limited (429)"),
            RawCallError::Server { status, body } => write!(f, "server error {status}: {body}"),
            RawCallError::NoContent => write!(f, "response contained no message content"),
        }
    }
}

// ─── Streaming listener ──────────────────────────────────────────────────────

/// Observer that receives streaming events as a read-only side channel.
///
/// When `LlmConfig::stream_listener` is `Some`, the HTTP response is read as
/// SSE and each parsed event is forwarded to `on_event()`. The ladder still
/// blocks until the stream ends, then returns the assembled `Success(LlmResponse)`.
pub trait StreamListener: Send + Sync {
    fn on_event(&self, event: StreamEvent);
}

/// One event in a streaming SSE response.
pub enum StreamEvent {
    /// Stream opened; initial metadata.
    MessageStart { model: String, id: String },
    /// A new content block begins.
    ContentBlockStart {
        index: usize,
        block: ContentBlockStart,
    },
    /// Incremental data for the current content block.
    ContentBlockDelta {
        index: usize,
        delta: ContentBlockDelta,
    },
    /// The current content block is complete.
    ContentBlockStop { index: usize },
    /// Final metadata before the stream ends.
    MessageDelta {
        stop_reason: Option<StopReason>,
        usage: Option<Usage>,
    },
    /// Stream ended.
    MessageStop,
}

/// Metadata announced when a content block begins.
pub enum ContentBlockStart {
    Text,
    ToolUse { id: String, name: String },
    Thinking,
}

/// Incremental data for a content block in progress.
pub enum ContentBlockDelta {
    TextDelta(String),
    /// Tool-use input arrives as incremental JSON fragments.
    InputJsonDelta(String),
    ThinkingDelta(String),
    /// Extended-thinking signature (arrives once, at the end of a thinking block).
    SignatureDelta(String),
}

// ─── Raw HTTP call (single attempt) ──────────────────────────────────────────

/// Dispatch a single request to the appropriate provider wire format.
///
/// Routes to [`raw_call_anthropic`] when `base_url` contains `anthropic.com`,
/// otherwise to [`raw_call_openai`].
pub fn raw_call(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<LlmResponse, RawCallError> {
    if config.base_url.contains("anthropic.com") {
        raw_call_anthropic(config, messages, tools)
    } else {
        raw_call_openai(config, messages, tools)
    }
}

/// Map an Anthropic stop_reason string to [`StopReason`].
fn map_stop_reason(s: Option<&str>) -> StopReason {
    match s {
        Some("end_turn") => StopReason::EndTurn,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("stop_sequence") => StopReason::StopSequence,
        Some("tool_use") => StopReason::ToolUse,
        Some("refusal") => StopReason::Refusal,
        _ => StopReason::EndTurn,
    }
}

/// Anthropic `/v1/messages` wire format.
///
/// Sends `temperature` when set.  When `reasoning_level` is set, enables
/// extended thinking via `thinking.budget_tokens` and forces `temperature: 1`
/// per Anthropic's requirement.
///
/// When `config.stream_listener` is `Some`, sends `stream: true` and reads the
/// SSE response, forwarding events to the listener while accumulating the full
/// `LlmResponse`.
fn raw_call_anthropic(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<LlmResponse, RawCallError> {
    let system = messages
        .iter()
        .find(|m| m.role == "system")
        .and_then(|m| m.content.as_text())
        .unwrap_or("");
    let user_msgs: Vec<_> = messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
        .collect();

    let url = format!("{}/messages", config.base_url.trim_end_matches('/'));

    let mut body = serde_json::json!({
        "model": config.model,
        "max_tokens": config.max_tokens,
        "system": system,
        "messages": user_msgs,
    });

    // Tools
    if !tools.is_empty() {
        body["tools"] = serde_json::json!(tools);
    }

    // Extended thinking: temperature must be 1; budget_tokens from level name.
    if let Some(level) = &config.reasoning_level {
        let budget_tokens: u32 = match level.to_lowercase().as_str() {
            "low" => 1_024,
            "medium" => 8_192,
            "high" => 32_768,
            other => other.parse::<u32>().unwrap_or(8_192),
        };
        body["thinking"] = serde_json::json!({
            "type": "enabled",
            "budget_tokens": budget_tokens
        });
        body["temperature"] = serde_json::json!(1);
    } else if let Some(t) = config.temperature {
        body["temperature"] = serde_json::json!(t);
    }

    let streaming = config.stream_listener.is_some();
    if streaming {
        body["stream"] = serde_json::json!(true);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| RawCallError::Http(e.to_string()))?;

    let response = client
        .post(&url)
        .header("x-api-key", &config.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .map_err(|e| RawCallError::Http(e.to_string()))?;

    classify_status(response.status().as_u16(), &url)?;

    if streaming {
        return parse_anthropic_stream(response, config);
    }

    // Non-streaming: deserialize full JSON response.
    #[derive(serde::Deserialize)]
    struct AnthropicResponse {
        id: String,
        model: String,
        stop_reason: Option<String>,
        content: Vec<AnthropicContentBlock>,
        usage: AnthropicUsage,
    }

    #[derive(serde::Deserialize)]
    struct AnthropicContentBlock {
        #[serde(rename = "type")]
        kind: String,
        text: Option<String>,
        id: Option<String>,
        name: Option<String>,
        input: Option<serde_json::Value>,
        thinking: Option<String>,
        signature: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct AnthropicUsage {
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
        cache_read_input_tokens: Option<u32>,
        cache_creation_input_tokens: Option<u32>,
        output_tokens_details: Option<AnthropicThinkingDetails>,
        service_tier: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct AnthropicThinkingDetails {
        thinking_tokens: Option<u32>,
    }

    let parsed: AnthropicResponse = response
        .json()
        .map_err(|e| RawCallError::Http(e.to_string()))?;

    Ok(LlmResponse {
        id: parsed.id,
        model: parsed.model,
        stop_reason: map_stop_reason(parsed.stop_reason.as_deref()),
        content: parsed
            .content
            .into_iter()
            .filter_map(|b| match b.kind.as_str() {
                "text" => Some(ContentBlock::Text {
                    text: b.text.unwrap_or_default(),
                }),
                "tool_use" => Some(ContentBlock::ToolUse {
                    id: b.id.unwrap_or_default(),
                    name: b.name.unwrap_or_default(),
                    input: b.input.unwrap_or(serde_json::Value::Null),
                }),
                "thinking" => Some(ContentBlock::Thinking {
                    thinking: b.thinking.unwrap_or_default(),
                    signature: b.signature.unwrap_or_default(),
                }),
                _ => None,
            })
            .collect(),
        usage: Usage {
            input_tokens: parsed.usage.input_tokens.unwrap_or(0),
            output_tokens: parsed.usage.output_tokens.unwrap_or(0),
            cache_read_input_tokens: parsed.usage.cache_read_input_tokens.unwrap_or(0),
            cache_creation_input_tokens: parsed.usage.cache_creation_input_tokens.unwrap_or(0),
            thinking_tokens: parsed
                .usage
                .output_tokens_details
                .and_then(|d| d.thinking_tokens)
                .unwrap_or(0),
            service_tier: parsed.usage.service_tier,
        },
    })
}

/// Read an Anthropic SSE streaming response, forwarding events to the
/// [`StreamListener`] and accumulating the final [`LlmResponse`].
fn parse_anthropic_stream(
    response: reqwest::blocking::Response,
    config: &LlmConfig,
) -> Result<LlmResponse, RawCallError> {
    use std::io::BufRead;

    let listener = config.stream_listener.as_ref().unwrap();
    let mut reader = std::io::BufReader::new(response);

    // Accumulators for the final LlmResponse
    let mut id = String::new();
    let mut model = String::new();
    let mut stop_reason = StopReason::EndTurn;
    let mut usage = Usage {
        input_tokens: 0,
        output_tokens: 0,
        cache_read_input_tokens: 0,
        cache_creation_input_tokens: 0,
        thinking_tokens: 0,
        service_tier: None,
    };
    let mut content_blocks: Vec<ContentBlock> = Vec::new();
    let mut current_block_kind = String::new();
    let mut current_text = String::new();
    let mut current_tool_id = String::new();
    let mut current_tool_name = String::new();
    let mut current_input_json = String::new();
    let mut current_thinking = String::new();
    let mut current_signature = String::new();

    let mut line = String::new();
    let mut event_type = String::new();
    let mut data = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => return Err(RawCallError::Http(format!("body read error: {e}"))),
        }
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("event: ") {
            event_type.clear();
            event_type.push_str(rest.trim());
        } else if let Some(rest) = trimmed.strip_prefix("data: ") {
            // Append successive data: lines (some events span multiple).
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim());
        } else if trimmed.is_empty() {
            // Blank line = end of event. Process the accumulated event_type + data.
            if event_type.is_empty() || data.is_empty() {
                data.clear();
                continue;
            }

            let payload: serde_json::Value = serde_json::from_str(&data)
                .map_err(|e| RawCallError::Http(format!("SSE JSON parse error: {e}")))?;

            match event_type.as_str() {
                "message_start" => {
                    if let Some(msg) = payload.get("message") {
                        id = msg["id"].as_str().unwrap_or("").into();
                        model = msg["model"].as_str().unwrap_or("").into();
                        if let Some(u) = msg.get("usage") {
                            usage.input_tokens = u["input_tokens"].as_u64().unwrap_or(0) as u32;
                        }
                    }
                    listener.on_event(StreamEvent::MessageStart {
                        model: model.clone(),
                        id: id.clone(),
                    });
                }

                "content_block_start" => {
                    let index = payload["index"].as_u64().unwrap_or(0) as usize;
                    let block = &payload["content_block"];
                    let kind = block["type"].as_str().unwrap_or("");
                    current_block_kind = kind.to_owned();
                    // Capture tool_use id/name BEFORE clearing (they are needed
                    // by the committed ContentBlock on content_block_stop).
                    current_tool_id.clear();
                    current_tool_name.clear();
                    if kind == "tool_use" {
                        current_tool_id = block["id"].as_str().unwrap_or("").into();
                        current_tool_name = block["name"].as_str().unwrap_or("").into();
                    }
                    current_text.clear();
                    current_input_json.clear();
                    current_thinking.clear();
                    current_signature.clear();

                    let cb_start = match kind {
                        "text" => ContentBlockStart::Text,
                        "tool_use" => ContentBlockStart::ToolUse {
                            id: current_tool_id.clone(),
                            name: current_tool_name.clone(),
                        },
                        "thinking" => ContentBlockStart::Thinking,
                        _ => ContentBlockStart::Text, // fallback
                    };
                    listener.on_event(StreamEvent::ContentBlockStart {
                        index,
                        block: cb_start,
                    });
                }

                "content_block_delta" => {
                    let index = payload["index"].as_u64().unwrap_or(0) as usize;
                    let delta = &payload["delta"];
                    let delta_kind = delta["type"].as_str().unwrap_or("");

                    match delta_kind {
                        "text_delta" => {
                            let t = delta["text"].as_str().unwrap_or("");
                            current_text.push_str(t);
                            listener.on_event(StreamEvent::ContentBlockDelta {
                                index,
                                delta: ContentBlockDelta::TextDelta(t.into()),
                            });
                        }
                        "input_json_delta" => {
                            let j = delta["partial_json"].as_str().unwrap_or("");
                            current_input_json.push_str(j);
                            listener.on_event(StreamEvent::ContentBlockDelta {
                                index,
                                delta: ContentBlockDelta::InputJsonDelta(j.into()),
                            });
                        }
                        "thinking_delta" => {
                            let t = delta["thinking"].as_str().unwrap_or("");
                            current_thinking.push_str(t);
                            listener.on_event(StreamEvent::ContentBlockDelta {
                                index,
                                delta: ContentBlockDelta::ThinkingDelta(t.into()),
                            });
                        }
                        "signature_delta" => {
                            let s = delta["signature"].as_str().unwrap_or("");
                            current_signature.push_str(s);
                            listener.on_event(StreamEvent::ContentBlockDelta {
                                index,
                                delta: ContentBlockDelta::SignatureDelta(s.into()),
                            });
                        }
                        _ => {}
                    }
                }

                "content_block_stop" => {
                    let index = payload["index"].as_u64().unwrap_or(0) as usize;

                    // Commit the accumulated block
                    match current_block_kind.as_str() {
                        "text" => {
                            content_blocks.push(ContentBlock::Text {
                                text: std::mem::take(&mut current_text),
                            });
                        }
                        "tool_use" => {
                            let input: serde_json::Value = if current_input_json.is_empty() {
                                serde_json::Value::Null
                            } else {
                                serde_json::from_str(&current_input_json)
                                    .unwrap_or(serde_json::Value::Null)
                            };
                            content_blocks.push(ContentBlock::ToolUse {
                                id: std::mem::take(&mut current_tool_id),
                                name: std::mem::take(&mut current_tool_name),
                                input,
                            });
                        }
                        "thinking" => {
                            content_blocks.push(ContentBlock::Thinking {
                                thinking: std::mem::take(&mut current_thinking),
                                signature: std::mem::take(&mut current_signature),
                            });
                        }
                        _ => {}
                    }

                    listener.on_event(StreamEvent::ContentBlockStop { index });
                }

                "message_delta" => {
                    if let Some(delta) = payload.get("delta")
                        && let Some(sr) = delta.get("stop_reason").and_then(|v| v.as_str())
                    {
                        stop_reason = map_stop_reason(Some(sr));
                    }
                    if let Some(u) = payload.get("usage") {
                        usage.output_tokens = u["output_tokens"].as_u64().unwrap_or(0) as u32;
                    }
                    listener.on_event(StreamEvent::MessageDelta {
                        stop_reason: Some(stop_reason.clone()),
                        usage: Some(usage.clone()),
                    });
                }

                "message_stop" => {
                    listener.on_event(StreamEvent::MessageStop);
                }

                _ => {}
            }
            data.clear();
        }
    }

    Ok(LlmResponse {
        id,
        model,
        stop_reason,
        usage,
        content: content_blocks,
    })
}

/// Map an OpenAI `finish_reason` string to [`StopReason`].
fn map_openai_finish_reason(s: Option<&str>) -> StopReason {
    match s {
        Some("stop") => StopReason::EndTurn,
        Some("tool_calls") => StopReason::ToolUse,
        Some("length") => StopReason::MaxTokens,
        Some("content_filter") => StopReason::Refusal,
        _ => StopReason::EndTurn,
    }
}

/// OpenAI-compatible `/v1/chat/completions` wire format.
///
/// Handles both non-streaming JSON and SSE streaming (some endpoints stream
/// even when `stream: false` is sent).  Sends `temperature` when set.
/// When `reasoning_level` is set, passes it as `reasoning_effort` (o-series).
fn raw_call_openai(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<LlmResponse, RawCallError> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let mut body = serde_json::json!({
        "model": config.model,
        "max_tokens": config.max_tokens,
        "messages": messages,
        "response_format": {"type": "json_object"},
    });

    if !tools.is_empty() {
        body["tools"] = serde_json::json!(
            tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema,
                        }
                    })
                })
                .collect::<Vec<_>>()
        );
    }

    if let Some(t) = config.temperature {
        body["temperature"] = serde_json::json!(t);
    }

    // OpenAI o-series reasoning effort.
    if let Some(level) = &config.reasoning_level {
        body["reasoning_effort"] = serde_json::json!(level);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| RawCallError::Http(e.to_string()))?;

    let response = client
        .post(&url)
        .bearer_auth(&config.api_key)
        .json(&body)
        .send()
        .map_err(|e| RawCallError::Http(e.to_string()))?;

    classify_status(response.status().as_u16(), &url)?;

    // Read line-by-line so the timeout applies per-chunk, not to the full body.
    use std::io::BufRead;
    let mut reader = std::io::BufReader::new(response);
    let mut first_data_line: Option<String> = None;
    let mut lines: Vec<String> = Vec::new();

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim().to_owned();
                if first_data_line.is_none() && trimmed.starts_with("data:") {
                    first_data_line = Some(trimmed.clone());
                }
                lines.push(trimmed);
            }
            Err(e) => return Err(RawCallError::Http(format!("body read error: {e}"))),
        }
    }

    if first_data_line.is_some() {
        parse_openai_sse(&lines)
    } else {
        parse_openai_json(&lines.join("\n"))
    }
}

fn parse_openai_json(text: &str) -> Result<LlmResponse, RawCallError> {
    #[derive(serde::Deserialize)]
    struct OpenAiResponse {
        id: Option<String>,
        model: Option<String>,
        choices: Vec<OpenAiChoice>,
        usage: Option<OpenAiUsage>,
    }
    #[derive(serde::Deserialize)]
    struct OpenAiChoice {
        message: OpenAiMessage,
        finish_reason: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct OpenAiMessage {
        content: Option<String>,
        tool_calls: Option<Vec<OpenAiToolCall>>,
    }
    #[derive(serde::Deserialize)]
    struct OpenAiToolCall {
        id: Option<String>,
        #[serde(rename = "type")]
        _kind: Option<String>,
        function: OpenAiFunction,
    }
    #[derive(serde::Deserialize)]
    struct OpenAiFunction {
        name: Option<String>,
        arguments: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct OpenAiUsage {
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
        prompt_tokens_details: Option<OpenAiPromptDetails>,
        completion_tokens_details: Option<OpenAiCompletionDetails>,
    }
    #[derive(serde::Deserialize)]
    struct OpenAiPromptDetails {
        cached_tokens: Option<u32>,
    }
    #[derive(serde::Deserialize)]
    struct OpenAiCompletionDetails {
        reasoning_tokens: Option<u32>,
    }

    let parsed: OpenAiResponse = serde_json::from_str(text)
        .map_err(|e| RawCallError::Http(format!("JSON parse error: {e}")))?;

    let choice = parsed.choices.into_iter().next();

    let mut content_blocks = Vec::new();

    // Text content
    if let Some(c) = &choice
        && let Some(ref text) = c.message.content
        && !text.is_empty()
    {
        content_blocks.push(ContentBlock::Text { text: text.clone() });
    }

    // Tool calls
    if let Some(c) = &choice
        && let Some(tool_calls) = &c.message.tool_calls
    {
        for tc in tool_calls {
            let input: serde_json::Value = tc
                .function
                .arguments
                .as_ref()
                .and_then(|args| serde_json::from_str(args).ok())
                .unwrap_or(serde_json::Value::Null);
            content_blocks.push(ContentBlock::ToolUse {
                id: tc.id.clone().unwrap_or_default(),
                name: tc.function.name.clone().unwrap_or_default(),
                input,
            });
        }
    }

    if content_blocks.is_empty() {
        return Err(RawCallError::NoContent);
    }

    let u = parsed.usage.as_ref();

    Ok(LlmResponse {
        id: parsed.id.unwrap_or_default(),
        model: parsed.model.unwrap_or_default(),
        stop_reason: match &choice {
            Some(c) => map_openai_finish_reason(c.finish_reason.as_deref()),
            None => StopReason::EndTurn,
        },
        usage: Usage {
            input_tokens: u.and_then(|u| u.prompt_tokens).unwrap_or(0),
            output_tokens: u.and_then(|u| u.completion_tokens).unwrap_or(0),
            cache_read_input_tokens: u
                .and_then(|u| u.prompt_tokens_details.as_ref())
                .and_then(|d| d.cached_tokens)
                .unwrap_or(0),
            cache_creation_input_tokens: 0,
            thinking_tokens: u
                .and_then(|u| u.completion_tokens_details.as_ref())
                .and_then(|d| d.reasoning_tokens)
                .unwrap_or(0),
            service_tier: None,
        },
        content: content_blocks,
    })
}

/// Parse accumulated SSE lines from an OpenAI-compatible endpoint.
/// Extracts `choices[0].delta.content` for text and builds a text-only
/// `LlmResponse`.  Tool-call stream accumulation is deferred.
fn parse_openai_sse(lines: &[String]) -> Result<LlmResponse, RawCallError> {
    let mut content = String::new();
    for line in lines {
        let line = line.trim();
        let Some(payload) = line.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            break;
        }
        #[allow(clippy::collapsible_if)]
        if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(payload) {
            if let Some(s) = chunk
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|v| v.as_str())
            {
                content.push_str(s);
            }
        }
    }
    if content.is_empty() {
        Err(RawCallError::NoContent)
    } else {
        Ok(LlmResponse {
            id: String::new(),
            model: String::new(),
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
                thinking_tokens: 0,
                service_tier: None,
            },
            content: vec![ContentBlock::Text { text: content }],
        })
    }
}

fn classify_status(status: u16, url: &str) -> Result<(), RawCallError> {
    match status {
        200..=299 => Ok(()),
        401 => Err(RawCallError::Auth(format!("401 Unauthorized from {url}"))),
        429 => Err(RawCallError::RateLimit),
        s => Err(RawCallError::Server {
            status: s,
            body: String::new(),
        }),
    }
}

// ─── LlmCall ladder ──────────────────────────────────────────────────────────

// Canonical rung ladder for a blocking LLM call with retry.
// Carry: call_id — opaque identifier threaded through for tracing.
// Pending(LlmRequest) → Success(LlmResponse) | LlmError(LlmFailure)
// recover: retry: Failed(Pending) => Pending — exponential backoff, decrement counter.
ladder!(LlmCall {
    carry { call_id: String }

    Pending(LlmRequest)
      => {
          Success(LlmResponse)
          | LlmError(LlmFailure)
      }

    // Error-path recovery (SPEC.md G9): `Failed(Pending)` hands the *unconsumed*
    // token back, so `retry` re-enters with the live request. Unlike a verdict
    // recover (`Stalled => Active`, SPEC.md G8), this edge is **unguarded** —
    // a retry after a transient error may legitimately re-send the identical
    // request. The external bound is `attempts_remaining`, which the recover
    // edge decrements so the ladder cannot retry forever.
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
        // The verb lives HERE, on the arrow (RUNG-CT §1 law): a single blocking
        // HTTP POST. A state cannot call an endpoint — only a transition can.
        match raw_call(&pending.payload.config, &pending.payload.messages, &pending.payload.tools) {
            Ok(response) => Ok(StepOutcome::Success(Success::new(response))),
            Err(e) if e.is_retryable() => {
                let msg = e.to_string();
                // Hand the unspent token back in `Failed { token, error }` so the
                // recover edge can re-enter. (This is also why a transition is a
                // Prism, not a monad — Q7: a failing `g` hands back `B`, but the
                // composite domain is `A`.)
                Err(Failed { token: pending, error: msg })
            }
            Err(RawCallError::Auth(msg)) => Ok(StepOutcome::LlmError(LlmError::new(
                LlmFailure::Auth(msg),
            ))),
            Err(e) => Ok(StepOutcome::LlmError(LlmError::new(
                LlmFailure::MaxRetries { last_error: e.to_string() },
            ))),
        }
    },

    // Exponential backoff: 1 s, 2 s, 4 s … capped at 30 s.
    // attempt_index = DEFAULT_MAX_ATTEMPTS - attempts_remaining (0-indexed).
    // `call_id` (G5 carry) threads forward unchanged — witness data is
    // structurally shared, never mutated in flight.
    retry = |f| {
        let carry = f.token.carry().clone();
        let call_id = carry.call_id.clone(); // witness data, read through &Carry
        let mut req = f.token.payload;
        let attempt_index =
            DEFAULT_MAX_ATTEMPTS.saturating_sub(req.attempts_remaining) as u32;
        req.attempts_remaining = req.attempts_remaining.saturating_sub(1);
        if req.attempts_remaining > 0 {
            let delay_ms = (1000u64.saturating_mul(1u64 << attempt_index)).min(30_000);
            eprintln!("[rung-std] {call_id}: retrying after {delay_ms}ms (attempt {attempt_index})");
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
        Pending::new(req, carry)
    },
});
