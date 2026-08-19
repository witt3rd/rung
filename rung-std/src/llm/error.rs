//! Error taxonomy for one LLM HTTP attempt.
//!
//! Retryable is a property of the *reason*, not of the status code: a 429 that
//! is a quota is terminal; a 529 (Anthropic overloaded) is retryable; a
//! transport timeout is retryable only if no response body has been observed.

use std::fmt;

const BODY_LIMIT: usize = 16_384;
pub const BASE_DELAY_MS: u64 = 500;
pub const MAX_DELAY_MS: u64 = 10_000;
/// Parse-level cap on `Retry-After` seconds (misbehaving upstream).
pub const RETRY_AFTER_PARSE_CAP_SECS: u64 = 120;

/// Classification for an invalid request that the caller can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestClassification {
    ContextOverflow,
}

impl RequestClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContextOverflow => "context-overflow",
        }
    }
}

/// Redacted snapshot of the HTTP exchange that produced an error.
#[derive(Debug, Clone)]
pub struct HttpContext {
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub request_id: Option<String>,
    pub retry_after_ms: Option<u64>,
    /// `x-should-retry`. Only `false` is a veto; `true` is ignored.
    pub should_retry: Option<bool>,
    pub body: Option<String>,
    pub body_truncated: bool,
}

impl HttpContext {
    pub fn from_parts(
        method: &str,
        url: &str,
        status: Option<u16>,
        headers: &[(String, String)],
        body: &str,
        secret: &str,
    ) -> Self {
        let (body, body_truncated) = redact_and_truncate(body, secret);
        Self {
            method: method.into(),
            url: redact_url(url, secret),
            status,
            request_id: request_id(headers),
            retry_after_ms: retry_after_ms(headers),
            should_retry: should_retry_header(headers),
            body,
            body_truncated,
        }
    }
}

/// Why a single HTTP attempt failed, before retry policy applies.
#[derive(Debug)]
pub enum RawCallError {
    Transport {
        message: String,
        /// True when any stream/body content was already delivered.
        observed: bool,
    },
    Auth {
        message: String,
        context: Option<HttpContext>,
    },
    /// Authenticated, but the action is forbidden (policy / 403). Not a bad key.
    Forbidden {
        message: String,
        context: Option<HttpContext>,
    },
    RateLimit {
        retry_after_ms: Option<u64>,
        context: Option<HttpContext>,
    },
    QuotaExceeded {
        message: String,
        context: Option<HttpContext>,
    },
    ContentPolicy {
        message: String,
        context: Option<HttpContext>,
    },
    InvalidRequest {
        message: String,
        classification: Option<RequestClassification>,
        context: Option<HttpContext>,
    },
    ProviderInternal {
        status: u16,
        retry_after_ms: Option<u64>,
        context: Option<HttpContext>,
    },
    InvalidProviderOutput {
        message: String,
        raw: Option<String>,
    },
    NoContent,
    /// No SSE/body bytes for `elapsed_secs`. Fatal — retrying the same path stalls again.
    IdleTimeout {
        elapsed_secs: u64,
    },
    Config(String),
}

impl RawCallError {
    /// Transient failures that may be retried *before* any output is observed.
    ///
    /// Vetoes (never retried): `x-should-retry: false`, context overflow even
    /// on a 5xx, Cloudflare origin-TLS 525/526, idle timeout, 403.
    pub fn is_retryable(&self) -> bool {
        if self.is_retry_vetoed() {
            return false;
        }
        match self {
            Self::RateLimit { .. } => true,
            Self::ProviderInternal { status, .. } => is_retryable_status(*status),
            Self::Transport { observed, .. } => !*observed,
            _ => false,
        }
    }

    pub fn is_rate_limited(&self) -> bool {
        matches!(self, Self::RateLimit { .. })
    }

    pub fn is_retry_vetoed(&self) -> bool {
        let ctx = self.context();
        if ctx.and_then(|c| c.should_retry) == Some(false) {
            return true;
        }
        if let Some(body) = ctx.and_then(|c| c.body.as_deref())
            && is_context_overflow(body)
        {
            return true;
        }
        match self {
            Self::InvalidRequest {
                classification: Some(RequestClassification::ContextOverflow),
                ..
            } => true,
            Self::ProviderInternal { status, .. } if matches!(*status, 525 | 526) => true,
            _ => false,
        }
    }

    fn context(&self) -> Option<&HttpContext> {
        match self {
            Self::Auth { context, .. }
            | Self::Forbidden { context, .. }
            | Self::RateLimit { context, .. }
            | Self::QuotaExceeded { context, .. }
            | Self::ContentPolicy { context, .. }
            | Self::InvalidRequest { context, .. }
            | Self::ProviderInternal { context, .. } => context.as_ref(),
            _ => None,
        }
    }

    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            Self::RateLimit { retry_after_ms, .. } => *retry_after_ms,
            Self::ProviderInternal { retry_after_ms, .. } => *retry_after_ms,
            Self::Auth { context, .. }
            | Self::Forbidden { context, .. }
            | Self::QuotaExceeded { context, .. }
            | Self::ContentPolicy { context, .. }
            | Self::InvalidRequest { context, .. } => {
                context.as_ref().and_then(|c| c.retry_after_ms)
            }
            _ => None,
        }
    }

    /// Downgrade a retryable error once tokens have already been seen.
    pub fn suppress_retry(self) -> Self {
        if !self.is_retryable() {
            return self;
        }
        Self::InvalidProviderOutput {
            message: format!("aborted after output began: {self}"),
            raw: None,
        }
    }
}

impl fmt::Display for RawCallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport { message, .. } => write!(f, "transport: {message}"),
            Self::Auth { message, .. } => write!(f, "auth: {message}"),
            Self::Forbidden { message, .. } => write!(f, "forbidden: {message}"),
            Self::RateLimit { retry_after_ms, .. } => match retry_after_ms {
                Some(ms) => write!(f, "rate limited (429), retry after {ms}ms"),
                None => write!(f, "rate limited (429)"),
            },
            Self::QuotaExceeded { message, .. } => write!(f, "quota exceeded: {message}"),
            Self::ContentPolicy { message, .. } => write!(f, "content policy: {message}"),
            Self::InvalidRequest {
                message,
                classification,
                ..
            } => match classification {
                Some(c) => write!(f, "invalid request ({}): {message}", c.as_str()),
                None => write!(f, "invalid request: {message}"),
            },
            Self::ProviderInternal { status, .. } => write!(f, "provider error {status}"),
            Self::InvalidProviderOutput { message, .. } => {
                write!(f, "invalid provider output: {message}")
            }
            Self::NoContent => write!(f, "response contained no message content"),
            Self::IdleTimeout { elapsed_secs } => {
                write!(f, "idle timeout after {elapsed_secs}s with no chunks")
            }
            Self::Config(e) => write!(f, "config: {e}"),
        }
    }
}

/// Terminal failure that rides the `LlmError` verdict out of the ladder.
#[derive(Debug, Clone)]
pub enum LlmFailure {
    Auth(String),
    Forbidden(String),
    MaxRetries {
        last_error: String,
    },
    Config(String),
    ContentPolicy(String),
    InvalidRequest {
        message: String,
        classification: Option<String>,
    },
    QuotaExceeded(String),
    IdleTimeout {
        elapsed_secs: u64,
    },
}

impl LlmFailure {
    pub fn from_raw(e: RawCallError) -> Self {
        match e {
            RawCallError::Auth { message, .. } => Self::Auth(message),
            RawCallError::Forbidden { message, .. } => Self::Forbidden(message),
            RawCallError::IdleTimeout { elapsed_secs } => Self::IdleTimeout { elapsed_secs },
            RawCallError::Config(m) => Self::Config(m),
            RawCallError::ContentPolicy { message, .. } => Self::ContentPolicy(message),
            RawCallError::QuotaExceeded { message, .. } => Self::QuotaExceeded(message),
            RawCallError::InvalidRequest {
                message,
                classification,
                ..
            } => Self::InvalidRequest {
                message,
                classification: classification.map(|c| c.as_str().to_string()),
            },
            other => Self::MaxRetries {
                last_error: other.to_string(),
            },
        }
    }
}

impl fmt::Display for LlmFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auth(e) => write!(f, "auth: {e}"),
            Self::Forbidden(e) => write!(f, "forbidden: {e}"),
            Self::IdleTimeout { elapsed_secs } => {
                write!(f, "idle timeout after {elapsed_secs}s with no chunks")
            }
            Self::MaxRetries { last_error } => {
                write!(f, "max retries exhausted — last error: {last_error}")
            }
            Self::Config(e) => write!(f, "config: {e}"),
            Self::ContentPolicy(e) => write!(f, "content policy: {e}"),
            Self::InvalidRequest {
                message,
                classification,
            } => match classification {
                Some(c) => write!(f, "invalid request ({c}): {message}"),
                None => write!(f, "invalid request: {message}"),
            },
            Self::QuotaExceeded(e) => write!(f, "quota exceeded: {e}"),
        }
    }
}

pub fn classify_http(
    method: &str,
    url: &str,
    status: u16,
    headers: &[(String, String)],
    body: &str,
    secret: &str,
) -> RawCallError {
    let ctx = HttpContext::from_parts(method, url, Some(status), headers, body, secret);
    let retry_after = ctx.retry_after_ms;
    let mut message = user_facing_message(status, body);
    if secret.len() >= 4 {
        message = message.replace(secret, "<redacted>");
    }

    // Overflow is deterministic on any status — including a 500 wrap.
    if is_context_overflow(body) {
        return RawCallError::InvalidRequest {
            classification: Some(RequestClassification::ContextOverflow),
            message,
            context: Some(ctx),
        };
    }
    if !looks_like_html(body) && is_content_policy(body) {
        return RawCallError::ContentPolicy {
            message,
            context: Some(ctx),
        };
    }
    if status == 401 {
        return RawCallError::Auth {
            message,
            context: Some(ctx),
        };
    }
    if status == 403 {
        return RawCallError::Forbidden {
            message,
            context: Some(ctx),
        };
    }
    if status == 429 {
        if is_quota(body) {
            return RawCallError::QuotaExceeded {
                message,
                context: Some(ctx),
            };
        }
        return RawCallError::RateLimit {
            retry_after_ms: retry_after,
            context: Some(ctx),
        };
    }
    if matches!(status, 400 | 404 | 409 | 413 | 422) {
        return RawCallError::InvalidRequest {
            classification: None,
            message,
            context: Some(ctx),
        };
    }
    if status >= 500 || matches!(status, 520..=530) {
        return RawCallError::ProviderInternal {
            status,
            retry_after_ms: retry_after,
            context: Some(ctx),
        };
    }
    RawCallError::InvalidRequest {
        classification: None,
        message,
        context: Some(ctx),
    }
}

/// 429 and 5xx except Cloudflare origin-TLS 525/526.
pub fn is_retryable_status(status: u16) -> bool {
    if matches!(status, 525 | 526) {
        return false;
    }
    status == 429 || (500..600).contains(&status)
}

pub fn header_pairs(headers: &reqwest::header::HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(k, v)| {
            Some((
                k.as_str().to_ascii_lowercase(),
                v.to_str().ok()?.to_string(),
            ))
        })
        .collect()
}

fn request_id(headers: &[(String, String)]) -> Option<String> {
    const NAMES: &[&str] = &[
        "x-request-id",
        "request-id",
        "x-amzn-requestid",
        "x-amz-request-id",
        "x-goog-request-id",
        "cf-ray",
    ];
    for name in NAMES {
        if let Some((_, v)) = headers.iter().find(|(k, _)| k == name) {
            return Some(v.clone());
        }
    }
    None
}

fn retry_after_ms(headers: &[(String, String)]) -> Option<u64> {
    if let Some((_, v)) = headers.iter().find(|(k, _)| k == "retry-after-ms") {
        let n: u64 = v.parse().ok()?;
        return Some(n);
    }
    let v = headers.iter().find(|(k, _)| k == "retry-after")?.1.as_str();
    if let Ok(secs) = v.parse::<u64>() {
        return Some(secs.min(RETRY_AFTER_PARSE_CAP_SECS).saturating_mul(1000));
    }
    let date = httpdate_secs(v)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(date.saturating_sub(now).saturating_mul(1000))
}

fn should_retry_header(headers: &[(String, String)]) -> Option<bool> {
    let v = headers
        .iter()
        .find(|(k, _)| k == "x-should-retry")?
        .1
        .as_str();
    if v.eq_ignore_ascii_case("true") {
        Some(true)
    } else if v.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

fn looks_like_html(body: &str) -> bool {
    let t = body.trim();
    let lower = t.to_ascii_lowercase();
    t.starts_with('<') || lower.contains("<html") || lower.contains("<!doctype")
}

/// JSON envelopes only. HTML edge pages never become the user-facing string.
pub fn user_facing_message(status: u16, body: &str) -> String {
    if let Some(parsed) = parse_provider_error(body)
        && !parsed_is_markup(&parsed)
    {
        return parsed;
    }
    status_user_message(status)
}

fn status_user_message(status: u16) -> String {
    match status {
        502..=504 => format!("provider temporarily unavailable (HTTP {status})"),
        529 => format!("provider overloaded (HTTP {status})"),
        520..=524 | 530 => format!("connection to provider interrupted (HTTP {status})"),
        525 | 526 => format!("secure connection to provider failed (HTTP {status})"),
        s if s >= 500 => format!("provider error (HTTP {s})"),
        401 => "authentication failed (HTTP 401)".into(),
        403 => "request forbidden (HTTP 403)".into(),
        429 => "rate limited (HTTP 429)".into(),
        s => format!("request failed (HTTP {s})"),
    }
}

fn parsed_is_markup(s: &str) -> bool {
    looks_like_html(s)
}

/// Pull a human message out of common provider error envelopes.
pub fn parse_provider_error(body: &str) -> Option<String> {
    let text = body.trim();
    if text.is_empty() || looks_like_html(text) {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    walk_error(&value)
}

fn walk_error(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() || looks_like_html(s) {
                None
            } else if let Some(inner) = parse_provider_error(s) {
                Some(inner)
            } else {
                Some(s.to_string())
            }
        }
        serde_json::Value::Array(items) => items.iter().find_map(walk_error),
        serde_json::Value::Object(obj) => {
            if let Some(err) = obj.get("error") {
                if let Some(s) = walk_error(err) {
                    return Some(s);
                }
            }
            for key in ["message", "detail", "msg", "description"] {
                if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
                    let s = s.trim();
                    if !s.is_empty() && !looks_like_html(s) {
                        return Some(s.to_string());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Mid-stream SSE error payload (`event: error` or a `data:` object with `error`).
pub fn parse_sse_error(data: &str) -> Option<RawCallError> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    let err = v.get("error").cloned().or_else(|| {
        matches!(v.get("type").and_then(|t| t.as_str()), Some("error")).then(|| v.clone())
    })?;
    let err_type = err
        .get("type")
        .and_then(|t| t.as_str())
        .or_else(|| v.get("type").and_then(|t| t.as_str()))
        .unwrap_or("");
    let message = err
        .get("message")
        .and_then(|m| m.as_str())
        .or_else(|| err.as_str())
        .unwrap_or("stream error")
        .to_string();
    if is_context_overflow(&message) {
        return Some(RawCallError::InvalidRequest {
            classification: Some(RequestClassification::ContextOverflow),
            message,
            context: None,
        });
    }
    // Classify stream errors on the parsed type, not message text.
    if err_type.eq_ignore_ascii_case("overloaded_error")
        || err_type.eq_ignore_ascii_case("service_unavailable_error")
    {
        return Some(RawCallError::ProviderInternal {
            status: 529,
            retry_after_ms: None,
            context: None,
        });
    }
    if err_type.contains("invalid") || err_type.contains("authentication") {
        return Some(RawCallError::InvalidRequest {
            classification: None,
            message,
            context: None,
        });
    }
    Some(RawCallError::InvalidProviderOutput {
        message: if err_type.is_empty() || err_type == "error" {
            message
        } else {
            format!("{err_type}: {message}")
        },
        raw: Some(data.chars().take(512).collect()),
    })
}

fn httpdate_secs(_v: &str) -> Option<u64> {
    // HTTP-date retry-after is rare; numeric seconds cover the providers we hit.
    None
}

fn redact_url(url: &str, secret: &str) -> String {
    if secret.len() >= 4 && url.contains(secret) {
        url.replace(secret, "<redacted>")
    } else {
        url.to_string()
    }
}

fn redact_and_truncate(body: &str, secret: &str) -> (Option<String>, bool) {
    if body.is_empty() {
        return (None, false);
    }
    let mut text = body.to_string();
    if secret.len() >= 4 {
        text = text.replace(secret, "<redacted>");
    }
    if text.len() <= BODY_LIMIT {
        (Some(text), false)
    } else {
        text.truncate(BODY_LIMIT);
        (Some(text), true)
    }
}

fn is_quota(body: &str) -> bool {
    let m = body.to_ascii_lowercase();
    m.contains("insufficient") && m.contains("quota")
        || m.contains("quota_exceeded")
        || m.contains("quota exceeded")
}

fn is_content_policy(body: &str) -> bool {
    let m = body.to_ascii_lowercase();
    m.contains("content_filter")
        || m.contains("content-filter")
        || m.contains("content policy")
        || m.contains("content_policy")
}

pub fn is_context_overflow(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    if m.contains("rate limit") || m.contains("too many requests") || m.contains("throttl") {
        return false;
    }
    const NEEDLES: &[&str] = &[
        "prompt is too long",
        "request_too_large",
        "context window",
        "maximum context",
        "context_length_exceeded",
        "context length exceeded",
        "reduce the length of the messages",
        "too many tokens",
        "token limit exceeded",
        "request entity too large",
        "model_context_window_exceeded",
        "exceeds the context",
        "maximum prompt length",
    ];
    NEEDLES.iter().any(|n| m.contains(n))
}

/// Jittered exponential backoff.
///
/// 429: honor the full `Retry-After` (already parse-capped at 120s).
/// 5xx / edge: clamp to [`MAX_DELAY_MS`] and jitter — Cloudflare sends the
/// same 60–120s `Retry-After` to every client at once.
pub fn retry_delay_ms(attempt_index: u32, retry_after_ms: Option<u64>, rate_limited: bool) -> u64 {
    if rate_limited {
        return retry_after_ms
            .unwrap_or_else(|| jittered_exp(attempt_index))
            .min(RETRY_AFTER_PARSE_CAP_SECS.saturating_mul(1000));
    }
    if let Some(ms) = retry_after_ms {
        return jitter_around(ms.min(MAX_DELAY_MS));
    }
    jittered_exp(attempt_index)
}

fn jittered_exp(attempt_index: u32) -> u64 {
    let exp = BASE_DELAY_MS
        .saturating_mul(1u64 << attempt_index.min(8))
        .min(MAX_DELAY_MS);
    jitter_around(exp)
}

fn jitter_around(base_ms: u64) -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let factor = 80 + (nanos % 41); // 80–120%
    (base_ms.saturating_mul(factor as u64) / 100).min(MAX_DELAY_MS.max(base_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_429_is_not_retryable() {
        let e = classify_http(
            "POST",
            "https://api.example/v1",
            429,
            &[],
            "insufficient_quota: you exceeded your current quota",
            "sk-secret",
        );
        assert!(!e.is_retryable());
        assert!(matches!(e, RawCallError::QuotaExceeded { .. }));
    }

    #[test]
    fn rate_limit_429_is_retryable_and_honors_retry_after() {
        let e = classify_http(
            "POST",
            "https://api.example/v1",
            429,
            &[("retry-after".into(), "8".into())],
            "rate limit exceeded",
            "sk",
        );
        assert!(e.is_retryable());
        assert_eq!(e.retry_after_ms(), Some(8000));
    }

    #[test]
    fn anthropic_529_is_retryable() {
        let e = classify_http(
            "POST",
            "https://api.anthropic.com/v1/messages",
            529,
            &[],
            "overloaded",
            "k",
        );
        assert!(e.is_retryable());
        assert!(matches!(
            e,
            RawCallError::ProviderInternal { status: 529, .. }
        ));
    }

    #[test]
    fn context_overflow_is_not_retryable() {
        let e = classify_http(
            "POST",
            "https://api.example/v1",
            400,
            &[],
            "prompt is too long: maximum context length exceeded",
            "k",
        );
        assert!(!e.is_retryable());
        match e {
            RawCallError::InvalidRequest {
                classification: Some(RequestClassification::ContextOverflow),
                ..
            } => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn transport_retryable_only_before_output() {
        assert!(
            RawCallError::Transport {
                message: "timeout".into(),
                observed: false
            }
            .is_retryable()
        );
        assert!(
            !RawCallError::Transport {
                message: "timeout".into(),
                observed: true
            }
            .is_retryable()
        );
    }

    #[test]
    fn secret_is_redacted_from_body() {
        let e = classify_http(
            "POST",
            "https://api.example/v1?key=sk-secret-value",
            401,
            &[],
            r#"{"error":"bad key sk-secret-value"}"#,
            "sk-secret-value",
        );
        match e {
            RawCallError::Auth {
                context: Some(ctx), ..
            } => {
                assert!(!ctx.body.unwrap_or_default().contains("sk-secret-value"));
                assert!(!ctx.url.contains("sk-secret-value"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn retry_after_caps_generic_delay_but_not_429() {
        let generic = retry_delay_ms(0, Some(50_000), false);
        assert!(generic <= MAX_DELAY_MS * 12 / 10);
        assert_eq!(retry_delay_ms(0, Some(50_000), true), 50_000);
        assert_eq!(
            retry_delay_ms(0, Some(200_000), true),
            RETRY_AFTER_PARSE_CAP_SECS * 1000
        );
    }

    #[test]
    fn forbidden_403_is_not_auth() {
        let e = classify_http(
            "POST",
            "https://api.example/v1",
            403,
            &[],
            r#"{"error":"nope"}"#,
            "k",
        );
        assert!(!e.is_retryable());
        assert!(matches!(e, RawCallError::Forbidden { .. }));
        let auth = classify_http(
            "POST",
            "https://api.example/v1",
            401,
            &[],
            r#"{"error":"bad key"}"#,
            "k",
        );
        assert!(matches!(auth, RawCallError::Auth { .. }));
    }

    #[test]
    fn overflow_on_500_is_not_retryable() {
        let e = classify_http(
            "POST",
            "https://api.example/v1",
            500,
            &[],
            "none: The prompt is too long for this model's context window.",
            "k",
        );
        assert!(!e.is_retryable());
        assert!(e.is_retry_vetoed());
        match e {
            RawCallError::InvalidRequest {
                classification: Some(RequestClassification::ContextOverflow),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn should_retry_false_vetoes_500() {
        let e = classify_http(
            "POST",
            "https://api.example/v1",
            500,
            &[("x-should-retry".into(), "false".into())],
            r#"{"error":{"message":"malformed tool in history"}}"#,
            "k",
        );
        assert!(!e.is_retryable());
        assert!(e.is_retry_vetoed());
    }

    #[test]
    fn should_retry_true_does_not_force_retry_on_525() {
        let e = classify_http(
            "POST",
            "https://api.example/v1",
            525,
            &[("x-should-retry".into(), "true".into())],
            "<html>origin tls</html>",
            "k",
        );
        assert!(!e.is_retryable());
        assert!(matches!(
            e,
            RawCallError::ProviderInternal { status: 525, .. }
        ));
    }

    #[test]
    fn html_502_does_not_surface_markup() {
        let e = classify_http(
            "POST",
            "https://api.example/v1",
            502,
            &[],
            "<!DOCTYPE html><html><body>Bad Gateway</body></html>",
            "k",
        );
        let shown = e.to_string();
        assert!(!shown.to_ascii_lowercase().contains("<html"));
        assert!(e.is_retryable());
    }

    #[test]
    fn sse_overloaded_is_retryable_529() {
        let e = parse_sse_error(
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        )
        .unwrap();
        assert!(e.is_retryable());
        assert!(matches!(
            e,
            RawCallError::ProviderInternal { status: 529, .. }
        ));
    }

    #[test]
    fn sse_invalid_request_mentioning_overloaded_is_not_retryable() {
        let e = parse_sse_error(
            r#"{"error":{"type":"invalid_request_error","message":"field `overloaded` is not valid"}}"#,
        )
        .unwrap();
        assert!(!e.is_retryable());
    }

    #[test]
    fn parse_provider_error_corpus() {
        assert_eq!(
            parse_provider_error(
                r#"{"error":{"message":"Incorrect API key","type":"invalid_request_error"}}"#
            )
            .as_deref(),
            Some("Incorrect API key")
        );
        assert_eq!(
            parse_provider_error(r#"{"error":{"message":"Provider returned error","code":429}}"#)
                .as_deref(),
            Some("Provider returned error")
        );
        assert_eq!(
            parse_provider_error(r#"{"error":"{\"error\":{\"message\":\"max_tokens too big\"}}"}"#)
                .as_deref(),
            Some("max_tokens too big")
        );
        assert!(parse_provider_error("<!DOCTYPE html><html></html>").is_none());
    }
}
