//! Canonical `LlmCall` rung ladder for reuse across any rung-based project.
//!
//! ## What this is
//!
//! A two-rung ladder (Pending → verdict) that wraps a single blocking HTTP
//! request to any OpenAI-compatible endpoint, including Anthropic's native
//! `/v1/messages` API.  Retryable failures surface as `Err(Failed)` → the
//! `retry` recover edge applies exponential backoff and decrements the
//! attempt counter.  Terminal failures exit as `Ok(LlmError(LlmFailure))`.
//! Success exits as `Ok(Success(String))`.
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
//! ## Membership criteria (rung-std)
//!
//! This ladder is rung-std because:
//! 1. It recurs across independent domain projects (garden-ladders, inner-loop, …)
//! 2. Its canonical two-rung shape (request-construction rung + verdict-routing
//!    rung) is domain-generic — no caller-specific knowledge is embedded.

use rung::ladder;
use serde::Serialize;

// ─── Configuration ───────────────────────────────────────────────────────────

/// Maximum HTTP attempts before the ladder terminates with [`LlmFailure::MaxRetries`].
pub const DEFAULT_MAX_ATTEMPTS: u8 = 3;

/// Full configuration for one LLM call.
///
/// Build this once per judge selection and carry it in [`LlmRequest`].
/// All optional fields are `None` → not sent in the request body.
#[derive(Debug, Clone)]
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
}

// ─── Message type ────────────────────────────────────────────────────────────

/// One chat message — role + content, matching the OpenAI wire format.
///
/// For Anthropic requests, `system` messages are extracted and sent in the
/// top-level `system` field; all other roles travel in `messages`.
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

// ─── Rung payload ────────────────────────────────────────────────────────────

/// Everything needed for one LLM call, plus the remaining-attempts counter
/// that the `retry` recover edge decrements before each backoff sleep.
#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub config: LlmConfig,
    pub messages: Vec<ChatMessage>,
    /// How many more HTTP attempts are allowed.
    /// Initialise to [`DEFAULT_MAX_ATTEMPTS`].
    pub attempts_remaining: u8,
}

impl LlmRequest {
    /// Convenience constructor — uses [`DEFAULT_MAX_ATTEMPTS`].
    pub fn new(config: LlmConfig, messages: Vec<ChatMessage>) -> Self {
        Self { config, messages, attempts_remaining: DEFAULT_MAX_ATTEMPTS }
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

// ─── Raw HTTP call (single attempt) ──────────────────────────────────────────

/// Dispatch a single request to the appropriate provider wire format.
///
/// Routes to [`raw_call_anthropic`] when `base_url` contains `anthropic.com`,
/// otherwise to [`raw_call_openai`].
pub fn raw_call(config: &LlmConfig, messages: &[ChatMessage]) -> Result<String, RawCallError> {
    if config.base_url.contains("anthropic.com") {
        raw_call_anthropic(config, messages)
    } else {
        raw_call_openai(config, messages)
    }
}

/// Anthropic `/v1/messages` wire format.
///
/// Sends `temperature` when set.  When `reasoning_level` is set, enables
/// extended thinking via `thinking.budget_tokens` and forces `temperature: 1`
/// per Anthropic's requirement.
fn raw_call_anthropic(config: &LlmConfig, messages: &[ChatMessage]) -> Result<String, RawCallError> {
    let system = messages.iter().find(|m| m.role == "system")
        .map(|m| m.content.as_str()).unwrap_or("");
    let user_msgs: Vec<_> = messages.iter()
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

    // Extended thinking: temperature must be 1; budget_tokens from level name.
    if let Some(level) = &config.reasoning_level {
        let budget_tokens: u32 = match level.to_lowercase().as_str() {
            "low"    => 1_024,
            "medium" => 8_192,
            "high"   => 32_768,
            other => {
                // Treat unknown strings as raw token counts if numeric, else medium.
                other.parse::<u32>().unwrap_or(8_192)
            }
        };
        body["thinking"] = serde_json::json!({
            "type": "enabled",
            "budget_tokens": budget_tokens
        });
        // Anthropic requires temperature=1 when extended thinking is enabled.
        body["temperature"] = serde_json::json!(1);
    } else if let Some(t) = config.temperature {
        body["temperature"] = serde_json::json!(t);
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

    #[derive(serde::Deserialize)]
    struct Resp { content: Vec<Block> }
    #[derive(serde::Deserialize)]
    struct Block {
        #[serde(rename = "type")] kind: String,
        text: Option<String>,
    }

    let parsed: Resp = response.json().map_err(|e| RawCallError::Http(e.to_string()))?;
    // Skip `thinking` blocks; take first `text` block.
    parsed.content.into_iter()
        .find(|b| b.kind == "text")
        .and_then(|b| b.text)
        .ok_or(RawCallError::NoContent)
}

/// OpenAI-compatible `/v1/chat/completions` wire format.
///
/// Handles both non-streaming JSON and SSE streaming (some endpoints stream
/// even when `stream: false` is sent).  Sends `temperature` when set.
/// When `reasoning_level` is set, passes it as `reasoning_effort` (o-series).
fn raw_call_openai(config: &LlmConfig, messages: &[ChatMessage]) -> Result<String, RawCallError> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let mut body = serde_json::json!({
        "model": config.model,
        "max_tokens": config.max_tokens,
        "messages": messages,
        "response_format": {"type": "json_object"},
    });

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
        parse_sse_lines(&lines)
    } else {
        parse_openai_json(&lines.join("\n"))
    }
}

fn parse_openai_json(text: &str) -> Result<String, RawCallError> {
    #[derive(serde::Deserialize)]
    struct Resp { choices: Vec<Choice> }
    #[derive(serde::Deserialize)]
    struct Choice { message: Msg }
    #[derive(serde::Deserialize)]
    struct Msg { content: Option<String> }

    let parsed: Resp = serde_json::from_str(text)
        .map_err(|e| RawCallError::Http(format!("JSON parse error: {e}")))?;
    parsed.choices.into_iter().next()
        .and_then(|c| c.message.content)
        .ok_or(RawCallError::NoContent)
}

/// Parse accumulated SSE lines, extracting `choices[0].delta.content`.
/// Skips reasoning/thinking content blocks; stops at `[DONE]`.
fn parse_sse_lines(lines: &[String]) -> Result<String, RawCallError> {
    let mut content = String::new();
    for line in lines {
        let line = line.trim();
        let Some(payload) = line.strip_prefix("data:") else { continue };
        let payload = payload.trim();
        if payload == "[DONE]" { break; }
        if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(payload) {
            if let Some(s) = chunk
                .get("choices").and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|v| v.as_str())
            {
                content.push_str(s);
            }
        }
    }
    if content.is_empty() { Err(RawCallError::NoContent) } else { Ok(content) }
}

fn classify_status(status: u16, url: &str) -> Result<(), RawCallError> {
    match status {
        200..=299 => Ok(()),
        401 => Err(RawCallError::Auth(format!("401 Unauthorized from {url}"))),
        429 => Err(RawCallError::RateLimit),
        s => Err(RawCallError::Server { status: s, body: String::new() }),
    }
}

// ─── LlmCall ladder ──────────────────────────────────────────────────────────

/// Canonical rung ladder for a blocking LLM call with retry.
///
/// # Carry
/// `call_id` — opaque identifier threaded through for tracing.
///
/// # Rungs
/// `Pending(LlmRequest)` → `Success(String)` | `LlmError(LlmFailure)`
///
/// # Recover
/// `retry: Failed(Pending) => Pending` — exponential backoff, decrement counter.
ladder!(LlmCall {
    carry { call_id: String }

    Pending(LlmRequest)
      => {
          Success(String)
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
        match raw_call(&pending.payload.config, &pending.payload.messages) {
            Ok(text) => Ok(StepOutcome::Success(Success::new(text))),
            Err(e) if e.is_retryable() => {
                let msg = e.to_string();
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
    retry = |f| {
        let carry = f.token.carry().clone();
        let mut req = f.token.payload;
        let attempt_index =
            DEFAULT_MAX_ATTEMPTS.saturating_sub(req.attempts_remaining) as u32;
        req.attempts_remaining = req.attempts_remaining.saturating_sub(1);
        if req.attempts_remaining > 0 {
            let delay_ms = (1000u64.saturating_mul(1u64 << attempt_index)).min(30_000);
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
        Pending::new(req, carry)
    },
});
