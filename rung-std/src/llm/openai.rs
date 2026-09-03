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
    let started = std::time::Instant::now();
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

    if listener.is_some() {
        let mut parser = OpenAiSse::new(started);
        let mut lines = Vec::new();
        let mut saw_data = false;
        super::sse::read_lines_idle_each(response, config.idle_timeout(), |line| {
            saw_data |= line.starts_with("data:");
            lines.push(line.to_string());
            parser.push(line, listener)
        })?;
        return if saw_data {
            parser.finish(listener)
        } else {
            parse_json(&lines.join("\n"))
        };
    }

    let lines = super::sse::read_lines_idle(response, config.idle_timeout())?;
    if lines.iter().any(|line| line.starts_with("data:")) {
        parse_sse(&lines, None)
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

struct OpenAiSse {
    content: String,
    tools: BTreeMap<usize, PendingTool>,
    stop_reason: StopReason,
    id: String,
    model: String,
    usage: Usage,
    saw_text_start: bool,
    saw_think_start: bool,
    saw_message_start: bool,
    started: std::time::Instant,
    first_token_at: Option<std::time::Instant>,
}

impl OpenAiSse {
    fn new(started: std::time::Instant) -> Self {
        Self {
            content: String::new(),
            tools: BTreeMap::new(),
            stop_reason: StopReason::EndTurn,
            id: String::new(),
            model: String::new(),
            usage: Usage::default(),
            saw_text_start: false,
            saw_think_start: false,
            saw_message_start: false,
            started,
            first_token_at: None,
        }
    }

    fn push(
        &mut self,
        line: &str,
        listener: Option<&dyn StreamListener>,
    ) -> Result<(), RawCallError> {
        let line = line.trim();
        let Some(payload) = line.strip_prefix("data:") else {
            return Ok(());
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            return Ok(());
        }
        let Ok(chunk) = serde_json::from_str::<serde_json::Value>(payload) else {
            return Ok(());
        };
        if chunk.get("error").is_some()
            && let Some(err) = parse_sse_error(payload)
        {
            return Err(err);
        }

        let emit = |event: StreamEvent| {
            if let Some(listener) = listener {
                listener.on_event(event);
            }
        };
        if self.id.is_empty() {
            self.id = chunk["id"].as_str().unwrap_or("").to_string();
        }
        if self.model.is_empty() {
            self.model = chunk["model"].as_str().unwrap_or("").to_string();
        }
        if !self.saw_message_start && (!self.id.is_empty() || !self.model.is_empty()) {
            emit(StreamEvent::MessageStart {
                model: self.model.clone(),
                id: self.id.clone(),
            });
            self.saw_message_start = true;
        }
        if let Some(u) = chunk.get("usage") {
            let mut usage = Usage::from_openai(
                u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                u["completion_tokens"].as_u64().unwrap_or(0) as u32,
                u["prompt_tokens_details"]["cached_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32,
                u["completion_tokens_details"]["reasoning_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32,
            );
            usage.cost_usd = u["cost"].as_f64();
            usage.provider = Some(u.clone());
            self.usage = usage;
        }

        let choice = chunk.get("choices").and_then(|choices| choices.get(0));
        if let Some(reason) = choice
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(|value| value.as_str())
        {
            self.stop_reason = map_openai_finish_reason(Some(reason));
        }
        let delta = choice.and_then(|choice| choice.get("delta"));
        if let Some(text) = delta
            .and_then(|delta| {
                delta
                    .get("reasoning_content")
                    .or_else(|| delta.get("reasoning"))
            })
            .and_then(|value| value.as_str())
            && !text.is_empty()
        {
            self.first_token_at
                .get_or_insert_with(std::time::Instant::now);
            if !self.saw_think_start {
                emit(StreamEvent::ContentBlockStart {
                    index: 1,
                    block: ContentBlockStart::Thinking,
                });
                self.saw_think_start = true;
            }
            emit(StreamEvent::ContentBlockDelta {
                index: 1,
                delta: ContentBlockDelta::ThinkingDelta(text.into()),
            });
        }

        if let Some(text) = delta
            .and_then(|delta| delta.get("content"))
            .and_then(|value| value.as_str())
            && !text.is_empty()
        {
            self.first_token_at
                .get_or_insert_with(std::time::Instant::now);
            if !self.saw_text_start {
                emit(StreamEvent::ContentBlockStart {
                    index: 0,
                    block: ContentBlockStart::Text,
                });
                self.saw_text_start = true;
            }
            self.content.push_str(text);
            emit(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentBlockDelta::TextDelta(text.into()),
            });
        }

        if let Some(tool_calls) = delta
            .and_then(|delta| delta.get("tool_calls"))
            .and_then(|value| value.as_array())
        {
            for tool_call in tool_calls {
                let index = tool_call["index"].as_u64().unwrap_or(0) as usize;
                let pending = self.tools.entry(index).or_default();
                let had_identity = !pending.id.is_empty() && !pending.name.is_empty();
                if let Some(id) = tool_call["id"].as_str() {
                    pending.id = id.to_string();
                }
                if let Some(name) = tool_call["function"]["name"].as_str() {
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
                if let Some(arguments) = tool_call["function"]["arguments"].as_str() {
                    pending.arguments.push_str(arguments);
                    emit(StreamEvent::ContentBlockDelta {
                        index,
                        delta: ContentBlockDelta::InputJsonDelta(arguments.into()),
                    });
                }
            }
        }
        Ok(())
    }

    fn finish(
        mut self,
        listener: Option<&dyn StreamListener>,
    ) -> Result<LlmResponse, RawCallError> {
        let finished = std::time::Instant::now();
        self.usage.duration_ms = Some(finished.duration_since(self.started).as_secs_f64() * 1000.0);
        if let Some(first) = self.first_token_at {
            self.usage.ttft_ms = Some(first.duration_since(self.started).as_secs_f64() * 1000.0);
            let generation_seconds = finished.duration_since(first).as_secs_f64();
            if generation_seconds > 0.0 {
                self.usage.output_tokens_per_second =
                    Some(f64::from(self.usage.output_tokens) / generation_seconds);
            }
        }
        let emit = |event: StreamEvent| {
            if let Some(listener) = listener {
                listener.on_event(event);
            }
        };
        if self.saw_text_start {
            emit(StreamEvent::ContentBlockStop { index: 0 });
        }
        if self.saw_think_start {
            emit(StreamEvent::ContentBlockStop { index: 1 });
        }
        let mut blocks = Vec::new();
        if !self.content.is_empty() {
            blocks.push(ContentBlock::Text { text: self.content });
        }
        for (index, tool) in self.tools {
            let input = if tool.arguments.is_empty() {
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
        if matches!(self.stop_reason, StopReason::EndTurn)
            && blocks
                .iter()
                .any(|block| matches!(block, ContentBlock::ToolUse { .. }))
        {
            self.stop_reason = StopReason::ToolUse;
        }
        emit(StreamEvent::MessageDelta {
            stop_reason: Some(self.stop_reason.clone()),
            usage: Some(self.usage.clone()),
        });
        emit(StreamEvent::MessageStop);
        Ok(LlmResponse {
            id: self.id,
            model: self.model,
            stop_reason: self.stop_reason,
            usage: self.usage,
            content: blocks,
        })
    }
}

pub(crate) fn parse_sse(
    lines: &[String],
    listener: Option<&dyn StreamListener>,
) -> Result<LlmResponse, RawCallError> {
    let mut parser = OpenAiSse::new(std::time::Instant::now());
    for line in lines {
        parser.push(line, listener)?;
    }
    parser.finish(listener)
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

    #[test]
    fn sse_openrouter_reasoning_emits_thinking_delta() {
        use std::sync::Mutex;

        #[derive(Default)]
        struct Capture(Mutex<Vec<String>>);
        impl StreamListener for Capture {
            fn on_event(&self, event: StreamEvent) {
                if let StreamEvent::ContentBlockDelta {
                    delta: ContentBlockDelta::ThinkingDelta(text),
                    ..
                } = event
                {
                    self.0.lock().unwrap().push(text);
                }
            }
        }

        let lines = [
            r#"data: {"id":"x","model":"m","choices":[{"delta":{"reasoning":"considering"}}]}"#
                .to_string(),
            r#"data: {"choices":[{"delta":{"content":"done"},"finish_reason":"stop"}]}"#
                .to_string(),
            "data: [DONE]".to_string(),
        ];
        let capture = Capture::default();
        let _ = parse_sse(&lines, Some(&capture)).unwrap();
        assert_eq!(*capture.0.lock().unwrap(), ["considering"]);
    }

    #[test]
    fn raw_call_delivers_a_chunk_before_the_sse_response_finishes() {
        use crate::llm::{CachePolicy, Protocol};
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::mpsc;
        use std::time::Duration;

        struct Capture(mpsc::Sender<String>);
        impl StreamListener for Capture {
            fn on_event(&self, event: StreamEvent) {
                if let StreamEvent::ContentBlockDelta {
                    delta: ContentBlockDelta::TextDelta(text),
                    ..
                } = event
                {
                    let _ = self.0.send(text);
                }
            }
        }

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap();
        let serving = std::thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n")
                .unwrap();
            socket
                .write_all(b"data: {\"id\":\"x\",\"model\":\"m\",\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n")
                .unwrap();
            socket.flush().unwrap();
            std::thread::sleep(Duration::from_millis(500));
            socket
                .write_all(b"data: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":20,\"prompt_tokens_details\":{\"cached_tokens\":80},\"completion_tokens_details\":{\"reasoning_tokens\":5},\"cost\":0.0042}}\n\ndata: [DONE]\n\n")
                .unwrap();
        });

        let (tx, rx) = mpsc::channel();
        let config = LlmConfig {
            base_url: format!("http://{address}/v1"),
            api_key: String::new(),
            model: "m".into(),
            timeout_secs: 5,
            idle_timeout_secs: Some(2),
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
            stream_listener: Some(Arc::new(Capture(tx))),
        };
        let call = std::thread::spawn(move || raw_call(&config, &[ChatMessage::user("hi")], &[]));

        assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), "Hel");
        let response = call.join().unwrap().unwrap();
        serving.join().unwrap();
        assert!(matches!(&response.content[0], ContentBlock::Text { text } if text == "Hello"));
        assert_eq!(response.usage.cache_read_input_tokens, 80);
        assert_eq!(response.usage.cost_usd, Some(0.0042));
        assert!(response.usage.ttft_ms.is_some());
        assert!(response.usage.output_tokens_per_second.is_some());
        assert_eq!(
            response.usage.provider.as_ref().unwrap()["prompt_tokens"],
            100
        );
    }
}
