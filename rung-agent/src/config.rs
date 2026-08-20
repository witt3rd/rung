//! [`rung_std::llm::LlmConfig`] from the environment. No file of secrets.

use rung_std::llm::{CachePolicy, LlmConfig, Protocol};

const DEFAULT_BASE: &str = "https://api.x.ai/v1";
const DEFAULT_MODEL: &str = "grok-4";

pub fn from_env() -> Result<LlmConfig, String> {
    let api_key = std::env::var("RUNG_API_KEY")
        .or_else(|_| std::env::var("XAI_API_KEY"))
        .map_err(|_| "missing RUNG_API_KEY or XAI_API_KEY".to_string())?;
    let timeout_secs = parse_u64("RUNG_TIMEOUT_SECS", 120)?;
    Ok(LlmConfig {
        base_url: std::env::var("RUNG_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE.into()),
        api_key,
        model: std::env::var("RUNG_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into()),
        timeout_secs,
        idle_timeout_secs: match std::env::var("RUNG_IDLE_TIMEOUT_SECS") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| format!("RUNG_IDLE_TIMEOUT_SECS: not a number ({s})"))?,
            ),
            Err(_) => None,
        },
        max_tokens: parse_u32("RUNG_MAX_TOKENS", 8192)?,
        temperature: match std::env::var("RUNG_TEMPERATURE") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| format!("RUNG_TEMPERATURE: not a number ({s})"))?,
            ),
            Err(_) => None,
        },
        top_p: None,
        top_k: None,
        seed: None,
        stop: Vec::new(),
        reasoning_level: std::env::var("RUNG_REASONING_LEVEL").ok(),
        structured_outputs: false,
        protocol: parse_protocol(std::env::var("RUNG_PROTOCOL").ok().as_deref())?,
        cache: CachePolicy::Auto,
        stream_listener: None,
    })
}

fn parse_protocol(value: Option<&str>) -> Result<Protocol, String> {
    match value.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(Protocol::Auto),
        Some(s) => match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(Protocol::Auto),
            "anthropic-messages" | "anthropic" => Ok(Protocol::AnthropicMessages),
            "openai-chat" | "openai" | "openai-compatible" => Ok(Protocol::OpenAiChat),
            other => Err(format!("RUNG_PROTOCOL: unknown '{other}'")),
        },
    }
}

fn parse_u64(key: &str, default: u64) -> Result<u64, String> {
    match std::env::var(key) {
        Err(_) => Ok(default),
        Ok(s) => s.parse().map_err(|_| format!("{key}: not a number ({s})")),
    }
}

fn parse_u32(key: &str, default: u32) -> Result<u32, String> {
    match std::env::var(key) {
        Err(_) => Ok(default),
        Ok(s) => s.parse().map_err(|_| format!("{key}: not a number ({s})")),
    }
}

/// Config that never hits the network. Tests only.
pub fn dummy() -> LlmConfig {
    LlmConfig {
        base_url: "http://127.0.0.1:9/v1".into(),
        api_key: "test".into(),
        model: "dummy".into(),
        timeout_secs: 1,
        idle_timeout_secs: None,
        max_tokens: 16,
        temperature: None,
        top_p: None,
        top_k: None,
        seed: None,
        stop: Vec::new(),
        reasoning_level: None,
        structured_outputs: false,
        protocol: Protocol::OpenAiChat,
        cache: CachePolicy::None,
        stream_listener: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_names() {
        assert_eq!(parse_protocol(None).unwrap(), Protocol::Auto);
        assert_eq!(
            parse_protocol(Some("anthropic-messages")).unwrap(),
            Protocol::AnthropicMessages
        );
        assert_eq!(
            parse_protocol(Some("openai")).unwrap(),
            Protocol::OpenAiChat
        );
        assert!(parse_protocol(Some("nope")).is_err());
    }
}
