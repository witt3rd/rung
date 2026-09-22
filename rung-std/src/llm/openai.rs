//! OpenAI-compatible `/v1/chat/completions` protocol.

use super::error::{RawCallError, classify_http, header_pairs, parse_sse_error};
use super::types::{
    ChatMessage, ContentBlock, ContentBlockDelta, ContentBlockStart, LlmConfig, LlmResponse,
    MessageContent, MessageContentBlock, ObservingListener, PreparedRequest, ResolvedProtocol,
    StopReason, StreamEvent, StreamListener, ToolDefinition, ToolDiagnostic, ToolErrorKind, Usage,
    map_openai_finish_reason,
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
        "messages": openai_messages(messages),
    });
    // 0 = no cap: the field is optional here, and a cap on a reasoning
    // model cuts off the answer, not the cost.
    if config.max_tokens > 0 {
        body["max_tokens"] = serde_json::json!(config.max_tokens);
    }

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

pub(crate) fn validate_tool_call(
    id: String,
    name: String,
    arguments: &str,
    finish_reason: Option<&str>,
    saw_done: bool,
    saw_malformed_frame: bool,
) -> ContentBlock {
    let received_bytes = arguments.len();
    let received_chars = arguments.chars().count();

    // 1. Missing tool identity: reject without inventing id or name
    if id.is_empty() || name.is_empty() {
        let details = if id.is_empty() && name.is_empty() {
            "missing tool id and name".to_string()
        } else if id.is_empty() {
            "missing tool id".to_string()
        } else {
            "missing tool name".to_string()
        };
        return ContentBlock::InvalidToolUse {
            id,
            name,
            diagnostic: ToolDiagnostic {
                kind: ToolErrorKind::IncompleteStream { details },
                finish_reason: finish_reason.map(str::to_string),
                saw_done,
                received_chars,
                received_bytes,
            },
        };
    }

    // 2. PARSE FIRST: Retain parse error evidence for exact observed truncated JSON + missing finish
    match serde_json::from_str::<serde_json::Value>(arguments) {
        Err(e) => {
            let category = match e.classify() {
                serde_json::error::Category::Io => "io",
                serde_json::error::Category::Syntax => "syntax",
                serde_json::error::Category::Data => "data",
                serde_json::error::Category::Eof => "eof",
            };
            ContentBlock::InvalidToolUse {
                id,
                name,
                diagnostic: ToolDiagnostic {
                    kind: ToolErrorKind::Json {
                        category: category.to_string(),
                        line: e.line(),
                        column: e.column(),
                        message: e.to_string(),
                    },
                    finish_reason: finish_reason.map(str::to_string),
                    saw_done,
                    received_chars,
                    received_bytes,
                },
            }
        }
        Ok(serde_json::Value::Object(map)) => {
            // Check if stream had corrupt/malformed SSE frames
            if saw_malformed_frame {
                return ContentBlock::InvalidToolUse {
                    id,
                    name,
                    diagnostic: ToolDiagnostic {
                        kind: ToolErrorKind::IncompleteStream {
                            details: "corrupted stream: malformed SSE frame encountered".into(),
                        },
                        finish_reason: finish_reason.map(str::to_string),
                        saw_done,
                        received_chars,
                        received_bytes,
                    },
                };
            }

            // Valid complete object BUT missing finish_reason -> IncompleteStream
            let Some(reason) = finish_reason else {
                return ContentBlock::InvalidToolUse {
                    id,
                    name,
                    diagnostic: ToolDiagnostic {
                        kind: ToolErrorKind::IncompleteStream {
                            details: "stream terminated before finish_reason received (EOF)".into(),
                        },
                        finish_reason: None,
                        saw_done,
                        received_chars,
                        received_bytes,
                    },
                };
            };

            // Reject complete object on finish_reason length/content_filter/unrecognized too.
            // Policy: Only "tool_calls", "function_call", and "stop" (for compatible nonstream/stop) permit execution.
            // "length" (MaxTokens) MUST be InvalidToolUse.
            match reason {
                "tool_calls" | "function_call" | "stop" => ContentBlock::ToolUse {
                    id,
                    name,
                    input: serde_json::Value::Object(map),
                },
                "length" => ContentBlock::InvalidToolUse {
                    id,
                    name,
                    diagnostic: ToolDiagnostic {
                        kind: ToolErrorKind::IncompleteStream {
                            details: "generation truncated by max_tokens limit (finish_reason: length)".into(),
                        },
                        finish_reason: Some(reason.to_string()),
                        saw_done,
                        received_chars,
                        received_bytes,
                    },
                },
                "content_filter" => ContentBlock::InvalidToolUse {
                    id,
                    name,
                    diagnostic: ToolDiagnostic {
                        kind: ToolErrorKind::IncompleteStream {
                            details: "generation interrupted by content filter (finish_reason: content_filter)".into(),
                        },
                        finish_reason: Some(reason.to_string()),
                        saw_done,
                        received_chars,
                        received_bytes,
                    },
                },
                other => ContentBlock::InvalidToolUse {
                    id,
                    name,
                    diagnostic: ToolDiagnostic {
                        kind: ToolErrorKind::IncompleteStream {
                            details: format!("unsupported or interrupted finish_reason: {other}"),
                        },
                        finish_reason: Some(reason.to_string()),
                        saw_done,
                        received_chars,
                        received_bytes,
                    },
                },
            }
        }
        Ok(other) => {
            let found = match other {
                serde_json::Value::Null => "null",
                serde_json::Value::Bool(_) => "boolean",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Array(_) => "array",
                serde_json::Value::Object(_) => unreachable!(),
            };
            ContentBlock::InvalidToolUse {
                id,
                name,
                diagnostic: ToolDiagnostic {
                    kind: ToolErrorKind::NotAnObject {
                        found: found.to_string(),
                    },
                    finish_reason: finish_reason.map(str::to_string),
                    saw_done,
                    received_chars,
                    received_bytes,
                },
            }
        }
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
            let args = tc.function.arguments.as_deref().unwrap_or("");
            let block = validate_tool_call(
                tc.id.clone().unwrap_or_default(),
                tc.function.name.clone().unwrap_or_default(),
                args,
                c.finish_reason.as_deref(),
                true,
                false,
            );
            content_blocks.push(block);
        }
    }

    if content_blocks.is_empty() {
        return Err(RawCallError::NoContent);
    }

    let mut stop_reason = match &choice {
        Some(c) => map_openai_finish_reason(c.finish_reason.as_deref()),
        None => StopReason::EndTurn,
    };
    let has_tools = content_blocks.iter().any(|b| {
        matches!(
            b,
            ContentBlock::ToolUse { .. } | ContentBlock::InvalidToolUse { .. }
        )
    });
    if has_tools && matches!(stop_reason, StopReason::EndTurn | StopReason::MaxTokens) {
        stop_reason = StopReason::ToolUse;
    }

    let u = parsed.usage.as_ref();
    Ok(LlmResponse {
        id: parsed.id.unwrap_or_default(),
        model: parsed.model.unwrap_or_default(),
        stop_reason,
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
    raw_finish_reason: Option<String>,
    saw_done: bool,
    saw_malformed_frame: bool,
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
            raw_finish_reason: None,
            saw_done: false,
            saw_malformed_frame: false,
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
            self.saw_done = true;
            return Ok(());
        }
        let chunk = match serde_json::from_str::<serde_json::Value>(payload) {
            Ok(c) => c,
            Err(_) => {
                self.saw_malformed_frame = true;
                return Ok(());
            }
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
            self.raw_finish_reason = Some(reason.to_string());
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
            emit(StreamEvent::ContentBlockStop { index });
            let block = validate_tool_call(
                tool.id,
                tool.name,
                &tool.arguments,
                self.raw_finish_reason.as_deref(),
                self.saw_done,
                self.saw_malformed_frame,
            );
            blocks.push(block);
        }
        if blocks.is_empty() {
            return Err(RawCallError::NoContent);
        }
        let has_tools = blocks.iter().any(|block| {
            matches!(
                block,
                ContentBlock::ToolUse { .. } | ContentBlock::InvalidToolUse { .. }
            )
        });
        if has_tools
            && matches!(
                self.stop_reason,
                StopReason::EndTurn | StopReason::MaxTokens
            )
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
    fn request_body_omits_max_tokens_when_uncapped() {
        use crate::llm::{CachePolicy, Protocol};
        let mut cfg = LlmConfig {
            base_url: "http://127.0.0.1:9/v1".into(),
            api_key: "k".into(),
            model: "m".into(),
            timeout_secs: 10,
            idle_timeout_secs: None,
            max_tokens: 0,
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
        let body = request_body(&cfg, &[ChatMessage::user("hi")], &[]);
        assert!(body.get("max_tokens").is_none());
        cfg.max_tokens = 64;
        let body = request_body(&cfg, &[ChatMessage::user("hi")], &[]);
        assert_eq!(body["max_tokens"], 64);
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

    #[test]
    fn sse_truncated_json_becomes_invalid_tool_use() {
        let lines = [
            r#"data: {"id":"chatcmpl-trunc","model":"gpt","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"search","arguments":"{\"query\":\"unfin"}}]}}]}"#.to_string(),
            r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#.to_string(),
            "data: [DONE]".to_string(),
        ];
        let resp = parse_sse(&lines, None).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.content.len(), 1);
        match &resp.content[0] {
            ContentBlock::InvalidToolUse {
                id,
                name,
                diagnostic,
            } => {
                assert_eq!(id, "c1");
                assert_eq!(name, "search");
                assert_eq!(diagnostic.finish_reason.as_deref(), Some("tool_calls"));
                assert!(diagnostic.saw_done);
                assert_eq!(diagnostic.received_chars, "{\"query\":\"unfin".len());
                match &diagnostic.kind {
                    ToolErrorKind::Json { category, .. } => {
                        assert_eq!(category, "eof");
                    }
                    other => panic!("expected Json eof error, got {other:?}"),
                }
                // Verify no raw args are leaked in formatted diagnostic
                let diag_str = diagnostic.to_string();
                assert!(!diag_str.contains("unfin"));
            }
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }
    }

    #[test]
    fn sse_null_array_string_become_invalid_tool_use() {
        let cases = [
            ("null", "null"),
            ("[1, 2, 3]", "array"),
            ("\"just a string\"", "string"),
            ("12345", "number"),
            ("true", "boolean"),
        ];

        for (arg_str, expected_type) in cases {
            let lines = [
                format!(
                    r#"data: {{"id":"c","model":"m","choices":[{{"index":0,"delta":{{"tool_calls":[{{"index":0,"id":"call_id","function":{{"name":"fn_name","arguments":{}}}}}]}}}}]}}"#,
                    serde_json::to_string(arg_str).unwrap()
                ),
                r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#
                    .to_string(),
                "data: [DONE]".to_string(),
            ];
            let resp = parse_sse(&lines, None).unwrap();
            assert_eq!(resp.stop_reason, StopReason::ToolUse);
            match &resp.content[0] {
                ContentBlock::InvalidToolUse {
                    id,
                    name,
                    diagnostic,
                } => {
                    assert_eq!(id, "call_id");
                    assert_eq!(name, "fn_name");
                    assert!(diagnostic.saw_done);
                    assert_eq!(
                        diagnostic.kind,
                        ToolErrorKind::NotAnObject {
                            found: expected_type.to_string(),
                        }
                    );
                }
                other => panic!("for arg {arg_str}, expected InvalidToolUse, got {other:?}"),
            }
        }
    }

    #[test]
    fn sse_missing_finish_reason_rejects_tool_execution() {
        // Complete valid JSON object, but connection dropped without finish_reason
        let lines = [
            r#"data: {"id":"c","model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"search","arguments":"{\"query\":\"hello\"}"}}]}}]}"#.to_string(),
        ];
        let resp = parse_sse(&lines, None).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        match &resp.content[0] {
            ContentBlock::InvalidToolUse {
                id,
                name,
                diagnostic,
            } => {
                assert_eq!(id, "c1");
                assert_eq!(name, "search");
                assert_eq!(diagnostic.finish_reason, None);
                assert!(!diagnostic.saw_done);
                match &diagnostic.kind {
                    ToolErrorKind::IncompleteStream { details } => {
                        assert!(details.contains("finish_reason"));
                    }
                    other => panic!("expected IncompleteStream, got {other:?}"),
                }
            }
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }
    }

    #[test]
    fn sse_missing_done_with_valid_finish_permits_tool_execution() {
        let lines = [
            r#"data: {"id":"c","model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"search","arguments":"{\"query\":\"hello\"}"}}]}}]}"#.to_string(),
            r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#.to_string(),
            // Notice: no "data: [DONE]" line
        ];
        let resp = parse_sse(&lines, None).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        match &resp.content[0] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "c1");
                assert_eq!(name, "search");
                assert_eq!(input["query"], "hello");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn sse_mixed_valid_and_invalid_tools() {
        let lines = [
            r#"data: {"id":"c","model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"search","arguments":"{\"query\":\"hello\"}"}},{"index":1,"id":"c2","function":{"name":"calc","arguments":"{\"expr\":\"1+2"}}]}}]}"#.to_string(),
            r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#.to_string(),
            "data: [DONE]".to_string(),
        ];
        let resp = parse_sse(&lines, None).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.content.len(), 2);
        match (&resp.content[0], &resp.content[1]) {
            (
                ContentBlock::ToolUse { id: id1, .. },
                ContentBlock::InvalidToolUse { id: id2, .. },
            ) => {
                assert_eq!(id1, "c1");
                assert_eq!(id2, "c2");
            }
            other => panic!("expected (ToolUse, InvalidToolUse), got {other:?}"),
        }
    }

    #[test]
    fn nonstream_parse_json_valid_and_invalid_cases() {
        // Case 1: Valid tool call with finish_reason
        let json_valid = r#"{
            "id": "cmpl-1",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "t1",
                        "function": {"name": "read", "arguments": "{\"path\":\"a.txt\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        }"#;
        let r1 = parse_json(json_valid).unwrap();
        assert_eq!(r1.stop_reason, StopReason::ToolUse);
        match &r1.content[0] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "t1");
                assert_eq!(name, "read");
                assert_eq!(input["path"], "a.txt");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }

        // Case 2: Truncated JSON arguments
        let json_trunc = r#"{
            "id": "cmpl-2",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "t2",
                        "function": {"name": "read", "arguments": "{\"path\":"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        }"#;
        let r2 = parse_json(json_trunc).unwrap();
        assert_eq!(r2.stop_reason, StopReason::ToolUse);
        match &r2.content[0] {
            ContentBlock::InvalidToolUse {
                id,
                name,
                diagnostic,
            } => {
                assert_eq!(id, "t2");
                assert_eq!(name, "read");
                match &diagnostic.kind {
                    ToolErrorKind::Json { category, .. } => assert_eq!(category, "eof"),
                    other => panic!("expected Json eof, got {other:?}"),
                }
            }
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }

        // Case 3: Non-object JSON argument ("null")
        let json_null = r#"{
            "id": "cmpl-3",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "t3",
                        "function": {"name": "read", "arguments": "null"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        }"#;
        let r3 = parse_json(json_null).unwrap();
        assert_eq!(r3.stop_reason, StopReason::ToolUse);
        match &r3.content[0] {
            ContentBlock::InvalidToolUse { diagnostic, .. } => {
                assert_eq!(
                    diagnostic.kind,
                    ToolErrorKind::NotAnObject {
                        found: "null".into()
                    }
                );
            }
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }

        // Case 4: Missing finish_reason rejects tool execution
        let json_nofinish = r#"{
            "id": "cmpl-4",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "t4",
                        "function": {"name": "read", "arguments": "{\"path\":\"a.txt\"}"}
                    }]
                },
                "finish_reason": null
            }]
        }"#;
        let r4 = parse_json(json_nofinish).unwrap();
        assert_eq!(r4.stop_reason, StopReason::ToolUse);
        match &r4.content[0] {
            ContentBlock::InvalidToolUse { diagnostic, .. } => match &diagnostic.kind {
                ToolErrorKind::IncompleteStream { .. } => {}
                other => panic!("expected IncompleteStream, got {other:?}"),
            },
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }

        // Case 5: MaxTokens (length) finish_reason with truncated tool call forces StopReason::ToolUse
        let json_length = r#"{
            "id": "cmpl-5",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "t5",
                        "function": {"name": "read", "arguments": "{\"path\": \"trun"}
                    }]
                },
                "finish_reason": "length"
            }]
        }"#;
        let r5 = parse_json(json_length).unwrap();
        // Crucial test: must force classification ToolUse even when finish_reason was length (MaxTokens)
        assert_eq!(r5.stop_reason, StopReason::ToolUse);
        assert!(matches!(r5.content[0], ContentBlock::InvalidToolUse { .. }));

        // Case 6: Complete valid object with finish_reason: length is REJECTED
        let json_len_complete = r#"{
            "id": "cmpl-6",
            "model": "gpt-4",
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "t6",
                        "function": {"name": "read", "arguments": "{\"path\": \"a.txt\"}"}
                    }]
                },
                "finish_reason": "length"
            }]
        }"#;
        let r6 = parse_json(json_len_complete).unwrap();
        assert_eq!(r6.stop_reason, StopReason::ToolUse);
        match &r6.content[0] {
            ContentBlock::InvalidToolUse { diagnostic, .. } => match &diagnostic.kind {
                ToolErrorKind::IncompleteStream { details } => {
                    assert!(details.contains("length"));
                }
                other => panic!("expected IncompleteStream with length, got {other:?}"),
            },
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }
    }

    #[test]
    fn sse_truncated_json_and_missing_finish_captures_both_parse_and_termination() {
        // Exact observed incident: 242 characters of unterminated JSON, stream EOF with no finish_reason
        let payload_242 = format!(
            r#"{{"path":"/tmp/dir/file.txt","content":"{}"#,
            "x".repeat(203)
        );
        assert_eq!(payload_242.len(), 242);
        assert_eq!(payload_242.chars().count(), 242);

        let lines = [format!(
            r#"data: {{"id":"c1","model":"m","choices":[{{"index":0,"delta":{{"tool_calls":[{{"index":0,"id":"call_inc","function":{{"name":"write","arguments":{}}}}}]}}}}]}}"#,
            serde_json::to_string(&payload_242).unwrap()
        )];
        let resp = parse_sse(&lines, None).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        match &resp.content[0] {
            ContentBlock::InvalidToolUse {
                id,
                name,
                diagnostic,
            } => {
                assert_eq!(id, "call_inc");
                assert_eq!(name, "write");
                // Retains parse error: category EOF
                match &diagnostic.kind {
                    ToolErrorKind::Json { category, .. } => assert_eq!(category, "eof"),
                    other => panic!("expected Json eof, got {other:?}"),
                }
                // Retains stream termination evidence: missing finish_reason and saw_done false
                assert_eq!(diagnostic.finish_reason, None);
                assert!(!diagnostic.saw_done);
                assert_eq!(diagnostic.received_chars, 242);
                assert_eq!(diagnostic.received_bytes, 242);
                // Clear display mentions missing finish_reason
                let display = diagnostic.to_string();
                assert!(display.contains("missing finish_reason (EOF)"));
                assert!(display.contains("saw_done: false"));
            }
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }
    }

    #[test]
    fn sse_complete_object_on_length_finish_reason_rejected() {
        let lines = [
            r#"data: {"id":"c","model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"search","arguments":"{\"query\":\"hello\"}"}}]}}]}"#.to_string(),
            r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"length"}]}"#.to_string(),
            "data: [DONE]".to_string(),
        ];
        let resp = parse_sse(&lines, None).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        match &resp.content[0] {
            ContentBlock::InvalidToolUse { diagnostic, .. } => match &diagnostic.kind {
                ToolErrorKind::IncompleteStream { details } => {
                    assert!(details.contains("length"));
                }
                other => panic!("expected IncompleteStream with length, got {other:?}"),
            },
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }
    }

    #[test]
    fn sse_missing_identity_rejected_without_invention() {
        let lines = [
            r#"data: {"id":"c","model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"","function":{"name":"","arguments":"{}"}}]}}]}"#.to_string(),
            r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#.to_string(),
            "data: [DONE]".to_string(),
        ];
        let resp = parse_sse(&lines, None).unwrap();
        match &resp.content[0] {
            ContentBlock::InvalidToolUse {
                id,
                name,
                diagnostic,
            } => {
                assert!(id.is_empty());
                assert!(name.is_empty());
                match &diagnostic.kind {
                    ToolErrorKind::IncompleteStream { details } => {
                        assert!(details.contains("missing tool id and name"));
                    }
                    other => panic!("expected missing identity details, got {other:?}"),
                }
            }
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }
    }

    #[test]
    fn sse_corrupt_frame_invalidates_tool_call() {
        let lines = [
            r#"data: {"id":"c","model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"search","arguments":"{\"query\":"}}]}}]}"#.to_string(),
            r#"data: {"corrupted_json_chunk_without_closing"#.to_string(),
            r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"hi\"}"}}]}}]}"#.to_string(),
            r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#.to_string(),
            "data: [DONE]".to_string(),
        ];
        let resp = parse_sse(&lines, None).unwrap();
        match &resp.content[0] {
            ContentBlock::InvalidToolUse { diagnostic, .. } => match &diagnostic.kind {
                ToolErrorKind::IncompleteStream { details } => {
                    assert!(details.contains("malformed SSE frame"));
                }
                other => panic!("expected IncompleteStream with malformed SSE, got {other:?}"),
            },
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }
    }

    #[test]
    fn received_chars_vs_bytes_multibyte_utf8() {
        // Multi-byte Unicode: 4 emojis (4 chars, 16 bytes)
        let raw = "{\"emoji\":\"🎉🚀🔥✨\"";
        let chars = raw.chars().count();
        let bytes = raw.len();
        assert_ne!(chars, bytes);

        let lines = [
            format!(
                r#"data: {{"id":"c","model":"m","choices":[{{"index":0,"delta":{{"tool_calls":[{{"index":0,"id":"c1","function":{{"name":"fn","arguments":{}}}}}]}}}}]}}"#,
                serde_json::to_string(raw).unwrap()
            ),
            r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#
                .to_string(),
            "data: [DONE]".to_string(),
        ];
        let resp = parse_sse(&lines, None).unwrap();
        match &resp.content[0] {
            ContentBlock::InvalidToolUse { diagnostic, .. } => {
                assert_eq!(diagnostic.received_chars, chars);
                assert_eq!(diagnostic.received_bytes, bytes);
            }
            other => panic!("expected InvalidToolUse, got {other:?}"),
        }
    }
}
