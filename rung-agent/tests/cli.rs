use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rung-agent"))
}

#[test]
fn help_exits_zero() {
    let out = bin().arg("--help").output().unwrap();
    assert!(out.status.success(), "{:?}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--toolset"), "{text}");
    assert!(text.contains("--type"), "{text}");
    assert!(text.contains("--tools"), "{text}");
    assert!(text.contains("--isolation"), "{text}");
    assert!(text.contains("--background"), "{text}");
    assert!(text.contains("--acp"), "{text}");
}

#[test]
fn missing_prompt_is_usage() {
    let out = bin().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn poll_missing_session() {
    let tmp = tempfile();
    let out = bin()
        .current_dir(&tmp)
        .args(["--task-id", "no-such-id"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{:?}", out);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no session"), "{err}");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn poll_completed_session() {
    let tmp = tempfile();
    let dir = tmp.join(".rung").join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("abc-1.json"),
        r#"{
  "id": "abc-1",
  "kind": "explore",
  "status": "completed",
  "cwd": ".",
  "lines": [
    {"role": "user", "text": "look"},
    {"role": "assistant", "text": "found it"}
  ]
}"#,
    )
    .unwrap();
    let out = bin()
        .current_dir(&tmp)
        .args(["--task-id", "abc-1"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("status=completed"), "{text}");
    assert!(text.contains("found it"), "{text}");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn poll_completed_session_json() {
    let tmp = tempfile();
    let dir = tmp.join(".rung").join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("abc-1.json"),
        r#"{
  "id": "abc-1",
  "kind": "implement",
  "status": "completed",
  "cwd": ".",
  "lines": [
    {"role": "user", "text": "look"},
    {"role": "assistant", "text": "found it"}
  ]
}"#,
    )
    .unwrap();
    let out = bin()
        .current_dir(&tmp)
        .args(["--json", "--task-id", "abc-1"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let text = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["task_id"], "abc-1");
    assert_eq!(parsed["status"], "completed");
    assert_eq!(parsed["text"], "found it");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn background_unreachable_endpoint_records_error() {
    let tmp = tempfile();
    let out = bin()
        .current_dir(&tmp)
        .env("RUNG_CONFIG", tmp.join("no-such-config.yaml"))
        .env_remove("RUNG_API_KEY")
        .env_remove("XAI_API_KEY")
        .env("RUNG_BASE_URL", "http://127.0.0.1:9/v1")
        .env("RUNG_MODEL", "dummy")
        .env("RUNG_TIMEOUT_SECS", "1")
        .args(["--background", "--type", "explore", "look around"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("task_id="), "{text}");
    let id = text
        .lines()
        .find_map(|l| l.strip_prefix("task_id="))
        .expect("task_id line");
    let sess = tmp
        .join(".rung")
        .join("sessions")
        .join(format!("{id}.json"));
    let mut body = String::new();
    for _ in 0..80 {
        if let Ok(s) = std::fs::read_to_string(&sess) {
            body = s;
            if body.contains("\"status\": \"error\"") || body.contains("\"status\":\"error\"") {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(body.contains("error"), "session never failed: {body}");
    let _ = std::fs::remove_dir_all(&tmp);
}

fn tempfile() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rung-agent-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}
