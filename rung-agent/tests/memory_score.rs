//! `rung-agent --memory-score` on the generic fixture set only (offline).

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_rung-agent");

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rung-memory/tests/fixtures/recall")
}

fn score(args: &[&str]) -> (bool, String, String) {
    score_with(args, None)
}

fn score_with(args: &[&str], config: Option<&str>) -> (bool, String, String) {
    let home = rung_testkit::TempDir::new("memory-score-home");
    let cfg = home.path().join("config.yaml");
    if let Some(c) = config {
        std::fs::write(&cfg, c).unwrap();
    }
    let out = Command::new(BIN)
        .arg("--memory-score")
        .args(args)
        .env("RUNG_CONFIG", &cfg)
        .env("RUNG_HOME", home.path())
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn row<'a>(report: &'a str, arm: &str) -> Vec<&'a str> {
    report
        .lines()
        .find(|l| l.starts_with(&format!("| {arm} |")))
        .unwrap_or_else(|| panic!("no row for {arm} in:\n{report}"))
        .split('|')
        .map(str::trim)
        .collect()
}

#[test]
fn baseline_and_provider_arms_report_on_the_fixture_set() {
    let dir = fixtures();
    let questions = dir.join("questions.jsonl");
    let arm = format!("mcp:{BIN} --memory-fixture");
    let (ok, out, err) = score(&[
        "--notes",
        dir.to_str().unwrap(),
        "--questions",
        questions.to_str().unwrap(),
        "--arm",
        &arm,
    ]);
    assert!(ok, "{err}");
    assert!(out.contains("40 notes, 30 questions"), "{out}");
    let base = row(&out, "baseline");
    // | arm | stored | hit@1 | hit@5 | MRR | ...
    assert_eq!(base[2], "40/40");
    assert!(base[3].parse::<f64>().unwrap() >= 0.8, "{out}");
    assert!(base[4].parse::<f64>().unwrap() >= 0.95, "{out}");
    let fixture = row(&out, &arm);
    assert_eq!(fixture[2], "40/40", "{out}");
    assert!(out.contains("recall $"), "{out}");
}

#[test]
fn a_directory_of_note_files_is_a_store() {
    let dir = rung_testkit::TempDir::new("memory-score-notes");
    std::fs::write(
        dir.path().join("tea.md"),
        "My favourite tea is jasmine green.",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("bike.txt"),
        "The bicycle tyres need 60 psi.",
    )
    .unwrap();
    let q = dir.path().join("q.jsonl");
    std::fs::write(&q, "{\"q\":\"which tea do I like\",\"expect\":[\"tea\"]}\n{\"q\":\"bicycle tyres pressure\",\"expect\":[\"bike\"]}\n").unwrap();
    let (ok, out, err) = score(&[
        "--notes",
        dir.path().to_str().unwrap(),
        "--questions",
        q.to_str().unwrap(),
        "--misses",
    ]);
    assert!(ok, "{err}");
    assert!(out.contains("2 notes, 2 questions"), "{out}");
    assert_eq!(row(&out, "baseline")[3], "1.000", "{out}");
    assert!(!out.contains("Missed by"), "{out}");
}

#[test]
fn a_missing_question_file_is_refused() {
    let dir = fixtures();
    let (ok, _, err) = score(&[
        "--notes",
        dir.to_str().unwrap(),
        "--questions",
        "/nonexistent/q",
    ]);
    assert!(!ok);
    assert!(err.contains("nonexistent"), "{err}");
}

/// A one-thread HTTP endpoint that records each request's `Authorization`
/// header and answers 401, so the configured token provably leaves the
/// process toward the arm.
fn auth_recorder() -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/mcp", l.local_addr().unwrap());
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for mut c in l.incoming().flatten() {
            let mut buf = [0u8; 8192];
            let n = c.read(&mut buf).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]).to_string();
            log.lock().unwrap().push(head);
            let _ = c.write_all(
                b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            );
        }
    });
    (url, seen)
}

#[test]
fn the_memory_token_never_reaches_the_report() {
    const SECRET: &str = "sekrit-token-4f2a9c1d";
    let dir = fixtures();
    let questions = dir.join("questions.jsonl");
    let (url, seen) = auth_recorder();
    let arm = format!("mcp:{url}");
    let cfg = format!("memory:\n  token: {SECRET}\n");
    let (_, out, err) = score_with(
        &[
            "--notes",
            dir.to_str().unwrap(),
            "--questions",
            questions.to_str().unwrap(),
            "--arm",
            &arm,
            "--misses",
            "--timeout",
            "5",
        ],
        Some(&cfg),
    );
    let sent = seen.lock().unwrap().join("\n");
    assert!(
        sent.contains(SECRET),
        "token never sent; test is vacuous:\n{sent}"
    );
    assert!(!out.contains(SECRET), "token in report:\n{out}");
    assert!(!err.contains(SECRET), "token in stderr:\n{err}");
}
