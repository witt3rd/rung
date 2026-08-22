//! OpenAI-compatible `/v1/chat/completions` protocol.

use super::error::{RawCallError, classify_http, header_pairs, parse_sse_error};
use super::types::{
    ChatMessage, ContentBlock, ContentBlockDelta, ContentBlockStart, LlmConfig, LlmResponse,
    MessageContent, MessageContentBlock, ObservingListener, PreparedRequest, ResolvedProtocol,
    StopReason, StreamEvent, StreamListener, ToolDefinition, Usage, map_openai_finish_reason,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn prepare(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<PreparedRequest, RawCallError> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    Ok(PreparedRequest {
        protocol: ResolvedProtocol::OpenAiChat,
        url,
        body: request_body(config, messages, tools),
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
    let observed = Arc::new(AtomicBool::new(false));
    let listener: Option<Arc<dyn StreamListener>> = config.stream_listener.as_ref().map(|inner| {
        Arc::new(ObservingListener {
            inner: Some(inner.clone()),
            observed: observed.clone(),
        }) as Arc<dyn StreamListener>
    });

    let result = send(config, &prepared.url, prepared.body, listener.as_deref());
    match result {
        Err(e) if observed.load(Ordering::SeqCst) && e.is_retryable() => Err(e.suppress_retry()),
        other => other,
    }
}

fn request_body(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": config.model,
        "max_tokens": config.max_tokens,
        "messages": openai_messages(messages),
    });

    if config.stream_listener.is_some() {
        body["stream"] = serde_json::json!(true);
    }

    if config.structured_outputs {
        body["response_format"] = serde_json::json!({"type": "json_object"});
    }
    if !tools.is_empty() {
        body["tools"] = serde_json::json!(
            tools
                .iter()
                .map(|t| serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                }))
                .collect::<Vec<_>>()
        );
    }
    if let Some(t) = config.temperature {
        body["temperature"] = serde_json::json!(t);
    }
    if let Some(p) = config.top_p {
        body["top_p"] = serde_json::json!(p);
    }
    if let Some(s) = config.seed {
        body["seed"] = serde_json::json!(s);
    }
    if !config.stop.is_empty() {
        body["stop"] = serde_json::json!(config.stop);
    }
    if let Some(level) = &config.reasoning_level {
        body["reasoning_effort"] = serde_json::json!(level);
    }
    body
}

/// Convert our messages to OpenAI `/v1/chat/completions` wire format.
pub fn audio_format(mime: &str) -> &'static str {
    let m = mime.to_ascii_lowercase();
    if m.contains("mpeg") || m.contains("mp3") {
        "mp3"
    } else {
        "wav"
    }
}

fn openai_messages(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = Vec::new();
    for msg in messages {
        match &msg.content {
            MessageContent::Text(text) => {
                out.push(serde_json::json!({"role": msg.role, "content": text}));
            }
            MessageContent::Blocks(blocks) => {
                let mut text_parts: Vec<serde_json::Value> = Vec::new();
                let mut tool_calls: Vec<serde_json::Value> = Vec::new();
                let mut emitted_tool_result = false;
                for block in blocks {
                    match block {
                        MessageContentBlock::Text { text, .. } => {
                            text_parts.push(serde_json::json!({"type": "text", "text": text}));
                        }
                        MessageContentBlock::Image { source, .. } => {
                            let url = format!("data:{};base64,{}", source.media_type, source.data);
                            text_parts.push(serde_json::json!({
                                "type": "image_url",
                                "image_url": { "url": url }
                            }));
                        }
                        MessageContentBlock::Audio { source, .. } => {
                            text_parts.push(serde_json::json!({
                                "type": "input_audio",
                                "input_audio": {
                                    "data": source.data,
                                    "format": audio_format(&source.media_type),
                                }
                            }));
                        }
                        MessageContentBlock::ToolUse {
                            id, name, input, ..
                        } => {
                            tool_calls.push(serde_json::json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": input.to_string(),
                                }
                            }));
                        }
                        MessageContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } => {
                            out.push(serde_json::json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": content,
                            }));
                            emitted_tool_result = true;
                        }
                        MessageContentBlock::Thinking { .. } => {
                            // OpenAI Chat has no thinking block. Anthropic
                            // round-trip uses the Anthropic protocol.
                        }
                    }
                }

                if !tool_calls.is_empty() {
                    let mut assistant = serde_json::json!({"role": "assistant"});
                    if text_parts.is_empty() {
                        assistant["content"] = serde_json::Value::Null;
                    } else {
                        assistant["content"] = serde_json::json!(text_parts);
                    }
                    assistant["tool_calls"] = serde_json::json!(tool_calls);
                    out.push(assistant);
                } else if !emitted_tool_result {
                    out.push(serde_json::json!({"role": msg.role, "content": text_parts}));
                }
            }
        }
    }
    out
}

fn send(
    config: &LlmConfig,
    url: &str,
    body: serde_json::Value,
    listener: Option<&dyn StreamListener>,
) -> Result<LlmResponse, RawCallError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| RawCallError::Transport {
            message: e.to_string(),
            observed: false,
        })?;

    let mut req = client.post(url).json(&body);
    if !config.api_key.is_empty() {
        req = req.bearer_auth(&config.api_key);
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

    let lines = super::sse::read_lines_idle(response, config.idle_timeout())?;
    let first_data_line = lines.iter().find(|l| l.starts_with("data:"));

    if first_data_line.is_some() {
        parse_sse(&lines, listener)
    } else {
        parse_json(&lines.join("\n"))
    }
}

pub(crate) fn parse_json(text: &str) -> Result<LlmResponse, RawCallError> {
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

    let parsed: OpenAiResponse =
        serde_json::from_str(text).map_err(|e| RawCallError::InvalidProviderOutput {
            message: format!("JSON parse error: {e}"),
            raw: Some(text.chars().take(512).collect()),
        })?;

    let choice = parsed.choices.into_iter().next();
    let mut content_blocks = Vec::new();

    if let Some(c) = &choice
        && let Some(ref text) = c.message.content
        && !text.is_empty()
    {
        content_blocks.push(ContentBlock::Text { text: text.clone() });
    }

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
        usage: Usage::from_openai(
            u.and_then(|u| u.prompt_tokens).unwrap_or(0),
            u.and_then(|u| u.completion_tokens).unwrap_or(0),
            u.and_then(|u| u.prompt_tokens_details.as_ref())
                .and_then(|d| d.cached_tokens)
                .unwrap_or(0),
            u.and_then(|u| u.completion_tokens_details.as_ref())
                .and_then(|d| d.reasoning_tokens)
                .unwrap_or(0),
        ),
        content: content_blocks,
    })
}

#[derive(Default)]
struct PendingTool {
    id: String,
    name: String,
    arguments: String,
}

pub(crate) fn parse_sse(
    lines: &[String],
    listener: Option<&dyn StreamListener>,
) -> Result<LlmResponse, RawCallError> {
    let mut content = String::new();
    let mut tools: BTreeMap<usize, PendingTool> = BTreeMap::new();
    let mut stop_reason = StopReason::EndTurn;
    let mut id = String::new();
    let mut model = String::new();
    let mut usage = Usage::default();
    let mut saw_text_start = false;

    let emit = |event: StreamEvent| {
        if let Some(l) = listener {
            l.on_event(event);
        }
    };

    for line in lines {
        let line = line.trim();
        let Some(payload) = line.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            break;
        }
        let Ok(chunk) = serde_json::from_str::<serde_json::Value>(payload) else {
            continue;
        };
        if chunk.get("error").is_some()
            && let Some(err) = parse_sse_error(payload)
        {
            return Err(err);
        }

        if id.is_empty() {
            id = chunk["id"].as_str().unwrap_or("").to_string();
        }
        if model.is_empty() {
            model = chunk["model"].as_str().unwrap_or("").to_string();
        }
        if let Some(u) = chunk.get("usage") {
            usage = Usage::from_openai(
                u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                u["completion_tokens"].as_u64().unwrap_or(0) as u32,
                u["prompt_tokens_details"]["cached_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32,
                u["completion_tokens_details"]["reasoning_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32,
            );
        }

        let choice = chunk.get("choices").and_then(|c| c.get(0));
        if let Some(fr) = choice
            .and_then(|c| c.get("finish_reason"))
            .and_then(|v| v.as_str())
        {
            stop_reason = map_openai_finish_reason(Some(fr));
        }
        let delta = choice.and_then(|c| c.get("delta"));

        if let Some(s) = delta
            .and_then(|d| d.get("content"))
            .and_then(|v| v.as_str())
            && !s.is_empty()
        {
            if !saw_text_start {
                emit(StreamEvent::ContentBlockStart {
                    index: 0,
                    block: ContentBlockStart::Text,
                });
                saw_text_start = true;
            }
            content.push_str(s);
            emit(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentBlockDelta::TextDelta(s.into()),
            });
        }

        if let Some(tcs) = delta
            .and_then(|d| d.get("tool_calls"))
            .and_then(|v| v.as_array())
        {
            for tc in tcs {
                let index = tc["index"].as_u64().unwrap_or(0) as usize;
                let pending = tools.entry(index).or_default();
                let had_identity = !pending.id.is_empty() && !pending.name.is_empty();
                if let Some(tid) = tc["id"].as_str() {
                    pending.id = tid.to_string();
                }
                if let Some(name) = tc["function"]["name"].as_str() {
                    pending.name = name.to_string();
                }
                if !had_identity && !pending.id.is_empty() && !pending.name.is_empty() {
                    emit(StreamEvent::ContentBlockStart {
                        index,
                        block: ContentBlockStart::ToolUse {
                            id: pending.id.clone(),
                            name: pending.name.clone(),
                        },
                    });
                }
                if let Some(args) = tc["function"]["arguments"].as_str() {
                    pending.arguments.push_str(args);
                    emit(StreamEvent::ContentBlockDelta {
                        index,
                        delta: ContentBlockDelta::InputJsonDelta(args.into()),
                    });
                }
            }
        }
    }

    if saw_text_start {
        emit(StreamEvent::ContentBlockStop { index: 0 });
    }

    let mut blocks = Vec::new();
    if !content.is_empty() {
        blocks.push(ContentBlock::Text { text: content });
    }
    for (index, tool) in tools {
        let input: serde_json::Value = if tool.arguments.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&tool.arguments).unwrap_or(serde_json::Value::Null)
        };
        emit(StreamEvent::ContentBlockStop { index });
        blocks.push(ContentBlock::ToolUse {
            id: tool.id,
            name: tool.name,
            input,
        });
    }

    if blocks.is_empty() {
        return Err(RawCallError::NoContent);
    }
    if matches!(stop_reason, StopReason::EndTurn)
        && blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
    {
        stop_reason = StopReason::ToolUse;
    }

    emit(StreamEvent::MessageDelta {
        stop_reason: Some(stop_reason.clone()),
        usage: Some(usage.clone()),
    });
    emit(StreamEvent::MessageStop);

    Ok(LlmResponse {
        id,
        model,
        stop_reason,
        usage,
        content: blocks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_use(id: &str, name: &str, input: serde_json::Value) -> MessageContentBlock {
        MessageContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
            cache: None,
        }
    }
    fn tool_result(id: &str, content: &str) -> MessageContentBlock {
        MessageContentBlock::ToolResult {
            tool_use_id: id.into(),
            content: content.into(),
            is_error: false,
            cache: None,
        }
    }

    #[test]
    fn openai_messages_plain_text_passes_through() {
        let msgs = vec![ChatMessage::system("sys"), ChatMessage::user("hello")];
        let out = openai_messages(&msgs);
        assert_eq!(
            out,
            vec![
                serde_json::json!({"role": "system", "content": "sys"}),
                serde_json::json!({"role": "user", "content": "hello"}),
            ]
        );
    }

    #[test]
    fn openai_messages_tool_use_becomes_tool_calls() {
        let assistant = ChatMessage::assistant_with_blocks(vec![tool_use(
            "t1",
            "list_files",
            serde_json::json!({"path": "."}),
        )]);
        let out = openai_messages(&[assistant]);
        assert_eq!(out.len(), 1);
        let m = &out[0];
        assert_eq!(m["role"], "assistant");
        assert_eq!(m["content"], serde_json::Value::Null);
        assert_eq!(m["tool_calls"][0]["id"], "t1");
        assert_eq!(m["tool_calls"][0]["function"]["name"], "list_files");
    }

    #[test]
    fn openai_messages_tool_result_becomes_tool_role() {
        let result = ChatMessage::user_with_blocks(vec![tool_result("t1", "Cargo.toml, src/")]);
        let out = openai_messages(&[result]);
        assert_eq!(
            out,
            vec![serde_json::json!({
                "role": "tool",
                "tool_call_id": "t1",
                "content": "Cargo.toml, src/"
            })]
        );
    }

    #[test]
    fn openai_messages_round_trip_tool_conversation() {
        let msgs = vec![
            ChatMessage::user("What files are here?"),
            ChatMessage::assistant_with_blocks(vec![tool_use(
                "t1",
                "list_files",
                serde_json::json!({"path": "."}),
            )]),
            ChatMessage::user_with_blocks(vec![tool_result("t1", "Cargo.toml, src/")]),
        ];
        let out = openai_messages(&msgs);
        assert_eq!(out.len(), 3);
        assert_eq!(out[1]["tool_calls"][0]["id"], "t1");
        assert_eq!(out[2]["role"], "tool");
    }

    #[test]
    fn request_body_streams_when_listener_present() {
        use crate::llm::{CachePolicy, Protocol, StreamEvent, StreamListener};
        struct Noop;
        impl StreamListener for Noop {
            fn on_event(&self, _: StreamEvent) {}
        }
        let cfg = || LlmConfig {
            base_url: "http://127.0.0.1:9/v1".into(),
            api_key: "k".into(),
            model: "m".into(),
            timeout_secs: 10,
            idle_timeout_secs: None,
            max_tokens: 32,
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
        let plain = request_body(&cfg(), &[ChatMessage::user("hi")], &[]);
        assert!(plain.get("stream").is_none());

        let mut with_listener = cfg();
        with_listener.stream_listener = Some(std::sync::Arc::new(Noop) as _);
        let body = request_body(&with_listener, &[ChatMessage::user("hi")], &[]);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn sse_accumulates_split_tool_arguments() {
        let lines = [
            r#"data: {"id":"chatcmpl-1","model":"gpt","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"lookup","arguments":"{\"q\":"}}]}}]}"#.to_string(),
            r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"hi\"}"}}]}}]}"#.to_string(),
            r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#.to_string(),
            "data: [DONE]".to_string(),
        ];
        let resp = parse_sse(&lines, None).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        match &resp.content[0] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "c1");
                assert_eq!(name, "lookup");
                assert_eq!(input["q"], "hi");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn sse_error_frame_is_fatal_for_invalid_request() {
        let lines = [
            r#"data: {"error":{"type":"invalid_request_error","message":"bad tool json"}}"#
                .to_string(),
        ];
        let err = parse_sse(&lines, None).unwrap_err();
        assert!(!err.is_retryable());
    }

    #[test]
    fn sse_text_still_works() {
        let lines = [
            r#"data: {"id":"x","model":"m","choices":[{"delta":{"content":"Hel"}}]}"#.to_string(),
            r#"data: {"choices":[{"delta":{"content":"lo"},"finish_reason":"stop"}]}"#.to_string(),
            "data: [DONE]".to_string(),
        ];
        let resp = parse_sse(&lines, None).unwrap();
        match &resp.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hello"),
            other => panic!("{other:?}"),
        }
    }
}
