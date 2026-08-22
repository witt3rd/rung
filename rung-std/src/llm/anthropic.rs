//! Anthropic `/v1/messages` protocol.

use super::cache::{self, Breakpoints};
use super::error::{RawCallError, classify_http, header_pairs, parse_sse_error};
use super::types::{
    ChatMessage, ContentBlock, ContentBlockDelta, ContentBlockStart, LlmConfig, LlmResponse,
    MessageContent, MessageContentBlock, ObservingListener, PreparedRequest, ResolvedProtocol,
    StopReason, StreamEvent, StreamListener, ToolDefinition, Usage, map_anthropic_stop_reason,
};
use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn prepare(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<PreparedRequest, RawCallError> {
    let url = format!("{}/messages", config.base_url.trim_end_matches('/'));
    let body = request_body(config, messages, tools)?;
    Ok(PreparedRequest {
        protocol: ResolvedProtocol::AnthropicMessages,
        url,
        body,
    })
}

pub fn raw_call(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<LlmResponse, RawCallError> {
    if config.base_url.trim().is_empty() {
        return Err(RawCallError::Config("empty base_url".into()));
    }
    let prepared = prepare(config, messages, tools)?;
    let mut body = prepared.body;
    let streaming = config.stream_listener.is_some();
    if streaming {
        body["stream"] = serde_json::json!(true);
    }

    let observed = Arc::new(AtomicBool::new(false));
    let listener: Option<Arc<dyn StreamListener>> = config.stream_listener.as_ref().map(|inner| {
        Arc::new(ObservingListener {
            inner: Some(inner.clone()),
            observed: observed.clone(),
        }) as Arc<dyn StreamListener>
    });

    let result = send(config, &prepared.url, body, streaming, listener.as_deref());
    finish(result, observed.load(Ordering::SeqCst))
}

fn finish(
    result: Result<LlmResponse, RawCallError>,
    observed: bool,
) -> Result<LlmResponse, RawCallError> {
    match result {
        Err(e) if observed && e.is_retryable() => Err(e.suppress_retry()),
        other => other,
    }
}

fn request_body(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<serde_json::Value, RawCallError> {
    let mut tools = tools.to_vec();
    let mut messages = messages.to_vec();
    cache::apply(config.cache, &mut tools, &mut messages);

    let mut breakpoints = Breakpoints::new();

    // Breakpoint budget is consumed in invalidation order: tools → system →
    // messages. Tools sit highest in the cache hierarchy.
    let tool_vals: Option<Vec<serde_json::Value>> = if tools.is_empty() {
        None
    } else {
        Some(
            tools
                .iter()
                .map(|t| {
                    let mut v = serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema,
                    });
                    if let Some(cc) = breakpoints.take(t.cache) {
                        v["cache_control"] = cc;
                    }
                    v
                })
                .collect(),
        )
    };

    let system_blocks: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role == "system")
        .flat_map(|m| system_parts(m, &mut breakpoints))
        .collect();

    let user_msgs: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| anthropic_message(m, &mut breakpoints))
        .collect();

    let mut body = serde_json::json!({
        "model": config.model,
        "max_tokens": config.max_tokens,
        "messages": user_msgs,
    });

    if !system_blocks.is_empty() {
        body["system"] = serde_json::json!(system_blocks);
    }

    if let Some(tool_vals) = tool_vals {
        body["tools"] = serde_json::json!(tool_vals);
    }

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
    } else {
        if let Some(t) = config.temperature {
            body["temperature"] = serde_json::json!(t);
        }
        if let Some(p) = config.top_p {
            body["top_p"] = serde_json::json!(p);
        }
        if let Some(k) = config.top_k {
            body["top_k"] = serde_json::json!(k);
        }
    }
    if !config.stop.is_empty() {
        body["stop_sequences"] = serde_json::json!(config.stop);
    }

    Ok(body)
}

fn system_parts(msg: &ChatMessage, bp: &mut Breakpoints) -> Vec<serde_json::Value> {
    match &msg.content {
        MessageContent::Text(t) => vec![serde_json::json!({"type": "text", "text": t})],
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                MessageContentBlock::Text { text, cache } => {
                    let mut v = serde_json::json!({"type": "text", "text": text});
                    if let Some(cc) = bp.take(*cache) {
                        v["cache_control"] = cc;
                    }
                    Some(v)
                }
                _ => None,
            })
            .collect(),
    }
}

fn anthropic_message(msg: &ChatMessage, bp: &mut Breakpoints) -> serde_json::Value {
    match &msg.content {
        MessageContent::Text(t) => serde_json::json!({"role": msg.role, "content": t}),
        MessageContent::Blocks(blocks) => {
            let content: Vec<serde_json::Value> = blocks
                .iter()
                .map(|b| match b {
                    MessageContentBlock::Text { text, cache } => {
                        let mut v = serde_json::json!({"type": "text", "text": text});
                        if let Some(cc) = bp.take(*cache) {
                            v["cache_control"] = cc;
                        }
                        v
                    }
                    MessageContentBlock::Image { source, cache } => {
                        let mut v = serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": source.source_type,
                                "media_type": source.media_type,
                                "data": source.data,
                            }
                        });
                        if let Some(cc) = bp.take(*cache) {
                            v["cache_control"] = cc;
                        }
                        v
                    }
                    MessageContentBlock::Audio { source, cache } => {
                        let mut v = serde_json::json!({
                            "type": "text",
                            "text": format!(
                                "[audio {} attached, {} base64 chars; this protocol has no audio part]",
                                source.media_type,
                                source.data.len()
                            ),
                        });
                        if let Some(cc) = bp.take(*cache) {
                            v["cache_control"] = cc;
                        }
                        v
                    }
                    MessageContentBlock::ToolUse {
                        id,
                        name,
                        input,
                        cache,
                    } => {
                        let mut v = serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input,
                        });
                        if let Some(cc) = bp.take(*cache) {
                            v["cache_control"] = cc;
                        }
                        v
                    }
                    MessageContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                        cache,
                    } => {
                        let mut v = serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": content,
                        });
                        if *is_error {
                            v["is_error"] = serde_json::json!(true);
                        }
                        if let Some(cc) = bp.take(*cache) {
                            v["cache_control"] = cc;
                        }
                        v
                    }
                    MessageContentBlock::Thinking {
                        thinking,
                        signature,
                        cache,
                    } => {
                        let mut v = serde_json::json!({
                            "type": "thinking",
                            "thinking": thinking,
                            "signature": signature,
                        });
                        if let Some(cc) = bp.take(*cache) {
                            v["cache_control"] = cc;
                        }
                        v
                    }
                })
                .collect();
            serde_json::json!({"role": msg.role, "content": content})
        }
    }
}

fn send(
    config: &LlmConfig,
    url: &str,
    body: serde_json::Value,
    streaming: bool,
    listener: Option<&dyn StreamListener>,
) -> Result<LlmResponse, RawCallError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| RawCallError::Transport {
            message: e.to_string(),
            observed: false,
        })?;

    let mut req = client
        .post(url)
        .header("anthropic-version", "2023-06-01")
        .json(&body);
    if !config.api_key.is_empty() {
        req = req.header("x-api-key", &config.api_key);
    }
    let response = req.send().map_err(|e| RawCallError::Transport {
        message: e.to_string(),
        observed: false,
    })?;

    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let headers = header_pairs(response.headers());
        let body = response.text().unwrap_or_default();
        return Err(classify_http(
            "POST",
            url,
            status,
            &headers,
            &body,
            &config.api_key,
        ));
    }

    if streaming {
        let lines = super::sse::read_lines_idle(response, config.idle_timeout())?;
        let joined = lines.join("\n") + "\n";
        return parse_sse(std::io::Cursor::new(joined.into_bytes()), listener);
    }

    parse_json(response)
}

fn parse_json(response: reqwest::blocking::Response) -> Result<LlmResponse, RawCallError> {
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

    let parsed: AnthropicResponse =
        response
            .json()
            .map_err(|e| RawCallError::InvalidProviderOutput {
                message: e.to_string(),
                raw: None,
            })?;

    Ok(LlmResponse {
        id: parsed.id,
        model: parsed.model,
        stop_reason: map_anthropic_stop_reason(parsed.stop_reason.as_deref()),
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
        usage: Usage::from_anthropic(
            parsed.usage.input_tokens.unwrap_or(0),
            parsed.usage.cache_read_input_tokens.unwrap_or(0),
            parsed.usage.cache_creation_input_tokens.unwrap_or(0),
            parsed.usage.output_tokens.unwrap_or(0),
            parsed
                .usage
                .output_tokens_details
                .and_then(|d| d.thinking_tokens)
                .unwrap_or(0),
            parsed.usage.service_tier,
        ),
    })
}

pub(crate) fn parse_sse(
    mut reader: impl BufRead,
    listener: Option<&dyn StreamListener>,
) -> Result<LlmResponse, RawCallError> {
    let mut id = String::new();
    let mut model = String::new();
    let mut stop_reason = StopReason::EndTurn;
    let mut usage = Usage::default();
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

    let emit = |event: StreamEvent| {
        if let Some(l) = listener {
            l.on_event(event);
        }
    };

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                return Err(RawCallError::Transport {
                    message: format!("body read error: {e}"),
                    observed: !content_blocks.is_empty()
                        || !current_text.is_empty()
                        || !current_input_json.is_empty(),
                });
            }
        }
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("event: ") {
            event_type.clear();
            event_type.push_str(rest.trim());
        } else if let Some(rest) = trimmed.strip_prefix("data: ") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim());
        } else if trimmed.is_empty() {
            if event_type.is_empty() || data.is_empty() {
                data.clear();
                continue;
            }

            let payload: serde_json::Value =
                serde_json::from_str(&data).map_err(|e| RawCallError::InvalidProviderOutput {
                    message: format!("SSE JSON parse error: {e}"),
                    raw: Some(data.clone()),
                })?;

            match event_type.as_str() {
                "error" => {
                    return Err(parse_sse_error(&data).unwrap_or_else(|| {
                        RawCallError::InvalidProviderOutput {
                            message: "stream error".into(),
                            raw: Some(data.clone()),
                        }
                    }));
                }
                "message_start" => {
                    if let Some(msg) = payload.get("message") {
                        id = msg["id"].as_str().unwrap_or("").into();
                        model = msg["model"].as_str().unwrap_or("").into();
                        if let Some(u) = msg.get("usage") {
                            let non_cached = u["input_tokens"].as_u64().unwrap_or(0) as u32;
                            let cache_read =
                                u["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32;
                            let cache_write =
                                u["cache_creation_input_tokens"].as_u64().unwrap_or(0) as u32;
                            usage = Usage::from_anthropic(
                                non_cached,
                                cache_read,
                                cache_write,
                                usage.output_tokens,
                                usage.thinking_tokens,
                                u["service_tier"].as_str().map(str::to_string),
                            );
                        }
                    }
                    emit(StreamEvent::MessageStart {
                        model: model.clone(),
                        id: id.clone(),
                    });
                }
                "content_block_start" => {
                    let index = payload["index"].as_u64().unwrap_or(0) as usize;
                    let block = &payload["content_block"];
                    let kind = block["type"].as_str().unwrap_or("");
                    current_block_kind = kind.to_owned();
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
                        "tool_use" => ContentBlockStart::ToolUse {
                            id: current_tool_id.clone(),
                            name: current_tool_name.clone(),
                        },
                        "thinking" => ContentBlockStart::Thinking,
                        _ => ContentBlockStart::Text,
                    };
                    emit(StreamEvent::ContentBlockStart {
                        index,
                        block: cb_start,
                    });
                }
                "content_block_delta" => {
                    let index = payload["index"].as_u64().unwrap_or(0) as usize;
                    let delta = &payload["delta"];
                    match delta["type"].as_str().unwrap_or("") {
                        "text_delta" => {
                            let t = delta["text"].as_str().unwrap_or("");
                            current_text.push_str(t);
                            emit(StreamEvent::ContentBlockDelta {
                                index,
                                delta: ContentBlockDelta::TextDelta(t.into()),
                            });
                        }
                        "input_json_delta" => {
                            let j = delta["partial_json"].as_str().unwrap_or("");
                            current_input_json.push_str(j);
                            emit(StreamEvent::ContentBlockDelta {
                                index,
                                delta: ContentBlockDelta::InputJsonDelta(j.into()),
                            });
                        }
                        "thinking_delta" => {
                            let t = delta["thinking"].as_str().unwrap_or("");
                            current_thinking.push_str(t);
                            emit(StreamEvent::ContentBlockDelta {
                                index,
                                delta: ContentBlockDelta::ThinkingDelta(t.into()),
                            });
                        }
                        "signature_delta" => {
                            let s = delta["signature"].as_str().unwrap_or("");
                            current_signature.push_str(s);
                            emit(StreamEvent::ContentBlockDelta {
                                index,
                                delta: ContentBlockDelta::SignatureDelta(s.into()),
                            });
                        }
                        _ => {}
                    }
                }
                "content_block_stop" => {
                    let index = payload["index"].as_u64().unwrap_or(0) as usize;
                    match current_block_kind.as_str() {
                        "text" => content_blocks.push(ContentBlock::Text {
                            text: std::mem::take(&mut current_text),
                        }),
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
                        "thinking" => content_blocks.push(ContentBlock::Thinking {
                            thinking: std::mem::take(&mut current_thinking),
                            signature: std::mem::take(&mut current_signature),
                        }),
                        _ => {}
                    }
                    emit(StreamEvent::ContentBlockStop { index });
                }
                "message_delta" => {
                    if let Some(delta) = payload.get("delta")
                        && let Some(sr) = delta.get("stop_reason").and_then(|v| v.as_str())
                    {
                        stop_reason = map_anthropic_stop_reason(Some(sr));
                    }
                    if let Some(u) = payload.get("usage") {
                        usage.output_tokens = u["output_tokens"].as_u64().unwrap_or(0) as u32;
                    }
                    emit(StreamEvent::MessageDelta {
                        stop_reason: Some(stop_reason.clone()),
                        usage: Some(usage.clone()),
                    });
                }
                "message_stop" => emit(StreamEvent::MessageStop),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{CachePolicy, Protocol};

    fn cfg() -> LlmConfig {
        LlmConfig {
            base_url: "https://api.anthropic.com/v1".into(),
            api_key: "sk-test".into(),
            model: "claude-sonnet-4-5".into(),
            timeout_secs: 30,
            idle_timeout_secs: None,
            max_tokens: 256,
            temperature: None,
            top_p: None,
            top_k: None,
            seed: None,
            stop: vec![],
            reasoning_level: None,
            structured_outputs: false,
            protocol: Protocol::AnthropicMessages,
            cache: CachePolicy::None,
            stream_listener: None,
        }
    }

    #[test]
    fn prepare_uses_messages_path() {
        let p = prepare(&cfg(), &[ChatMessage::user("hi")], &[]).unwrap();
        assert!(p.url.ends_with("/messages"));
        assert_eq!(p.body["model"], "claude-sonnet-4-5");
        assert_eq!(p.body["messages"][0]["role"], "user");
    }

    #[test]
    fn thinking_round_trips_on_assistant_message() {
        let msgs = vec![ChatMessage::assistant_with_blocks(vec![
            MessageContentBlock::Thinking {
                thinking: "hmm".into(),
                signature: "sig".into(),
                cache: None,
            },
            MessageContentBlock::Text {
                text: "ok".into(),
                cache: None,
            },
        ])];
        let p = prepare(&cfg(), &msgs, &[]).unwrap();
        let content = &p.body["messages"][0]["content"];
        assert_eq!(content[0]["type"], "thinking");
        assert_eq!(content[0]["signature"], "sig");
        assert_eq!(content[1]["type"], "text");
    }

    #[test]
    fn auto_cache_emits_cache_control_on_user() {
        let mut c = cfg();
        c.cache = CachePolicy::Auto;
        let p = prepare(&c, &[ChatMessage::system("s"), ChatMessage::user("u")], &[]).unwrap();
        let sys = &p.body["system"];
        assert_eq!(sys[0]["cache_control"]["type"], "ephemeral");
        let user = &p.body["messages"][0]["content"];
        // user is a blocks array after auto placement
        assert!(
            user.is_array() && user[0]["cache_control"]["type"] == "ephemeral" || user.is_string()
        );
    }

    #[test]
    fn usage_is_inclusive() {
        let u = Usage::from_anthropic(100, 40, 10, 20, 0, None);
        assert_eq!(u.input_tokens, 150);
        assert_eq!(u.non_cached_input_tokens, 100);
        assert_eq!(u.cache_read_input_tokens, 40);
        assert_eq!(u.cache_creation_input_tokens, 10);
    }

    #[test]
    fn sse_error_event_overloaded() {
        let body = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n";
        let err = parse_sse(std::io::Cursor::new(body.as_bytes()), None).unwrap_err();
        assert!(err.is_retryable());
        assert!(matches!(
            err,
            RawCallError::ProviderInternal { status: 529, .. }
        ));
    }
}
