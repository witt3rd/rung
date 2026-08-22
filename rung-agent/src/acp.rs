//! ACP agent on stdio: JSON-RPC, one object per line.
//! initialize → session/new → session/prompt (session/update chunks, then
//! stopReason). The kernel stays a CLI; this is the door anvil holds.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::args::{Args, IsolationMode};
use crate::catalog::Kind;
use crate::run::run_job;
use crate::session;

/// Speak ACP until stdin closes.
pub fn run() -> Result<(), String> {
    let stdin = io::stdin();
    let mut out = io::stdout();
    let mut session_id: Option<String> = None;
    let origin = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| format!("stdin: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value =
            serde_json::from_str(&line).map_err(|e| format!("json: {e}"))?;
        for reply in handle(&mut session_id, &origin, &msg) {
            writeln!(out, "{reply}").map_err(|e| format!("stdout: {e}"))?;
            out.flush().map_err(|e| format!("stdout: {e}"))?;
        }
    }
    Ok(())
}

fn handle(session_id: &mut Option<String>, origin: &PathBuf, msg: &Value) -> Vec<Value> {
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = msg.get("id").cloned();
    match method {
        "initialize" => vec![ok(
            id,
            json!({
                "protocolVersion": 1,
                "agentCapabilities": {
                    "loadSession": false,
                    "promptCapabilities": { "image": false, "audio": false }
                },
                "agentInfo": { "name": "rung-agent", "version": "0.1.0" },
                "authMethods": []
            }),
        )],
        "session/new" => {
            let sid = session::new_id();
            *session_id = Some(sid.clone());
            vec![ok(id, json!({ "sessionId": sid }))]
        }
        "session/prompt" => prompt(session_id, origin, id, msg),
        "session/cancel" => vec![ok(id, json!({}))],
        _ if id.is_some() => vec![err(id, -32601, "method not found")],
        _ => Vec::new(),
    }
}

fn prompt(
    session_id: &mut Option<String>,
    origin: &PathBuf,
    id: Option<Value>,
    msg: &Value,
) -> Vec<Value> {
    let Some(sid) = session_id.clone() else {
        return vec![err(id, -32600, "no session")];
    };
    let text = prompt_text(msg);
    if text.is_empty() {
        return vec![ok(id, json!({ "stopReason": "end_turn" }))];
    }
    let args = Args {
        task_id: Some(sid.clone()),
        kind: Kind::Implement,
        isolation: IsolationMode::None,
        background: false,
        max_iterations: None,
        prompt: Some(text),
        help: false,
        acp: false,
    };
    match run_job(&args, origin) {
        Ok(out) => {
            let mut replies = Vec::new();
            if !out.text.is_empty() {
                replies.push(json!({
                    "jsonrpc": "2.0",
                    "method": "session/update",
                    "params": {
                        "sessionId": sid,
                        "update": {
                            "sessionUpdate": "agent_message_chunk",
                            "content": { "type": "text", "text": out.text }
                        }
                    }
                }));
            }
            replies.push(ok(id, json!({ "stopReason": "end_turn" })));
            replies
        }
        Err(e) => vec![err(id, -32000, &e)],
    }
}

fn prompt_text(msg: &Value) -> String {
    let Some(blocks) = msg
        .pointer("/params/prompt")
        .and_then(|v| v.as_array())
    else {
        return String::new();
    };
    let mut out = String::new();
    for b in blocks {
        if b.get("type").and_then(|t| t.as_str()) != Some("text") {
            continue;
        }
        if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    out
}

fn ok(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_and_session_new() {
        let mut sid = None;
        let origin = PathBuf::from(".");
        let init = handle(
            &mut sid,
            &origin,
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        );
        assert_eq!(init[0]["result"]["protocolVersion"], 1);
        let created = handle(
            &mut sid,
            &origin,
            &json!({"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"."}}),
        );
        assert!(created[0]["result"]["sessionId"].as_str().unwrap().len() > 1);
        assert!(sid.is_some());
    }

    #[test]
    fn prompt_text_joins_blocks() {
        let msg = json!({
            "params": { "prompt": [
                {"type":"text","text":"hello"},
                {"type":"text","text":"world"}
            ]}
        });
        assert_eq!(prompt_text(&msg), "hello\nworld");
    }
}
