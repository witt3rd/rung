//! ACP v1 handshake against `rung-agent --acp` (no LLM).

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rung-agent"))
}

fn read_json(reader: &mut BufReader<impl std::io::Read>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("stdout line");
    serde_json::from_str(line.trim()).unwrap_or_else(|e| panic!("json {e}: {line}"))
}

#[test]
fn initialize_new_list_set_mode_close() {
    let tmp = tempfile();
    let cwd = tmp.to_string_lossy().into_owned();
    let mut child = bin()
        .arg("--acp")
        .current_dir(&tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":1}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let init = read_json(&mut stdout);
    assert_eq!(init["result"]["protocolVersion"], 1);
    assert_eq!(init["result"]["agentCapabilities"]["loadSession"], true);
    assert!(init["result"]["agentCapabilities"]["sessionCapabilities"]["list"].is_object());

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":2,"method":"session/new","params":{{"cwd":"{cwd}","mcpServers":[]}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let created = read_json(&mut stdout);
    let sid = created["result"]["sessionId"].as_str().expect("sessionId");
    assert!(!sid.is_empty());
    let modes = &created["result"]["modes"]["availableModes"];
    assert_eq!(modes.as_array().map(|a| a.len()), Some(3));

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":3,"method":"session/list","params":{{"cwd":"{cwd}"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let listed = read_json(&mut stdout);
    assert_eq!(listed["result"]["sessions"].as_array().unwrap().len(), 1);

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":4,"method":"session/set_mode","params":{{"sessionId":"{sid}","modeId":"explore"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let mode = read_json(&mut stdout);
    assert!(mode.get("result").is_some(), "{mode}");

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":5,"method":"session/prompt","params":{{"sessionId":"{sid}","prompt":[]}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let prompt = read_json(&mut stdout);
    assert_eq!(prompt["result"]["stopReason"], "end_turn");

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"session/cancel","params":{{"sessionId":"{sid}"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":6,"method":"session/close","params":{{"sessionId":"{sid}"}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let closed = read_json(&mut stdout);
    assert!(closed.get("result").is_some(), "{closed}");

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success(), "{status:?}");
    let _ = std::fs::remove_dir_all(&tmp);
}

fn tempfile() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rung-agent-acp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}
