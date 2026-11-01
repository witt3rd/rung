//! `rung-agent auth …` and an OAuth-credentialed turn, as the binary runs
//! them, against a fake provider server on loopback. No real login.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rung_testkit::TempDir;
use rung_testkit::http::{Req, Resp, Server};
use serde_json::json;

const GH_TOKEN: &str = "gho_BinaryLevelLoginTokenForTheFakeServer01";

struct Fake {
    polls: AtomicUsize,
    exchanges: AtomicUsize,
    chat_bearers: Mutex<Vec<String>>,
}

fn fake() -> (Arc<Fake>, Server) {
    let f = Arc::new(Fake {
        polls: AtomicUsize::new(0),
        exchanges: AtomicUsize::new(0),
        chat_bearers: Mutex::new(Vec::new()),
    });
    let server = {
        let f = f.clone();
        Server::start(move |r: &Req| match (r.method.as_str(), r.path.as_str()) {
            ("POST", "/login/device/code") => Resp::json(
                200,
                json!({"device_code": "dc-secret-1", "user_code": "WXYZ-9876",
                       "verification_uri": "https://github.com/login/device",
                       "interval": 0, "expires_in": 900}),
            ),
            ("POST", "/login/oauth/access_token") => {
                if f.polls.fetch_add(1, Ordering::SeqCst) == 0 {
                    Resp::json(200, json!({"error": "authorization_pending"}))
                } else {
                    Resp::json(200, json!({"access_token": GH_TOKEN}))
                }
            }
            ("GET", "/copilot_internal/v2/token") => {
                if r.header("authorization") != Some(&format!("Bearer {GH_TOKEN}")) {
                    return Resp::json(401, json!({"message": "Bad credentials"}));
                }
                let n = f.exchanges.fetch_add(1, Ordering::SeqCst) + 1;
                let exp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 1800;
                Resp::json(
                    200,
                    json!({"token": format!("tid=bin{n};exp={exp};proxy-ep=proxy.individual.githubcopilot.com;sigbin{n}"),
                           "expires_at": exp}),
                )
            }
            ("GET", "/models") => Resp::json(
                200,
                json!({"data": [{"id": "gpt-4.1"}, {"id": "other", "supported_endpoints": ["/responses"]}]}),
            ),
            ("POST", "/chat/completions") => {
                f.chat_bearers
                    .lock()
                    .unwrap()
                    .push(r.header("authorization").unwrap_or("").to_string());
                let reply = json!({"id": "c", "model": "gpt-4.1",
                    "choices": [{"delta": {"role": "assistant", "content": "oauth says hi"},
                                 "finish_reason": "stop"}]});
                if r.json()["stream"] == true {
                    Resp::sse(format!("data: {reply}\n\ndata: [DONE]\n\n"))
                } else {
                    Resp::json(
                        200,
                        json!({"id": "c", "model": "gpt-4.1", "choices": [{"message":
                            {"role": "assistant", "content": "oauth says hi"}, "finish_reason": "stop"}]}),
                    )
                }
            }
            _ => Resp::json(404, json!({})),
        })
    };
    (f, server)
}

fn agent(home: &Path, server: &Server) -> Command {
    let config = home.join("config.yaml");
    std::fs::write(
        &config,
        format!(
            "llm:\n  model: gpt-4.1\n  auth:\n    oauth:\n      provider: github-copilot\n      profile: work\n\
             oauth:\n  github-copilot:\n    github_url: {u}\n    api_url: {u}\n    base_url: {u}\n",
            u = server.url()
        ),
    )
    .unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_rung-agent"));
    c.current_dir(home)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home)
        .env("RUNG_CONFIG", &config)
        .env("RUNG_HOME", home.join("rung"))
        .env_remove("RUNG_API_KEY")
        .env_remove("XAI_API_KEY")
        .env_remove("RUNG_BASE_URL")
        .env_remove("RUNG_MODEL")
        .env_remove("RUNG_OAUTH_PROFILE")
        .env_remove("RUNG_OAUTH_PROVIDER");
    c
}

fn text(o: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn login_status_models_and_logout_never_print_a_token() {
    let (_f, server) = fake();
    let home = TempDir::new("oauth-cli");
    let login = agent(home.path(), &server)
        .args(["auth", "login", "github-copilot", "--profile", "work"])
        .output()
        .unwrap();
    let (out, err) = text(&login);
    assert!(login.status.success(), "{out}{err}");
    // The code and the URL are for the user to act on, on any machine.
    assert!(
        err.contains("WXYZ-9876") && err.contains("https://github.com/login/device"),
        "{err}"
    );
    assert!(
        out.contains("logged in: github-copilot profile 'work'"),
        "{out}"
    );
    let file = home.path().join("rung/oauth/github-copilot--work.json");
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    // The login is stored outside any repo (under RUNG_HOME), and is the only
    // place the token is.
    assert!(std::fs::read_to_string(&file).unwrap().contains(GH_TOKEN));
    for secret in [GH_TOKEN, "dc-secret-1"] {
        assert!(
            !out.contains(secret) && !err.contains(secret),
            "login printed a secret"
        );
    }

    let status = agent(home.path(), &server)
        .args(["auth", "status"])
        .output()
        .unwrap();
    let (out, err) = text(&status);
    assert!(status.status.success(), "{err}");
    assert!(
        out.contains("github-copilot profile 'work': logged in"),
        "{out}"
    );
    assert!(!out.contains(GH_TOKEN));

    let models = agent(home.path(), &server)
        .args(["auth", "models", "github-copilot", "--profile", "work"])
        .output()
        .unwrap();
    let (out, err) = text(&models);
    assert!(models.status.success(), "{err}");
    assert!(
        out.contains("gpt-4.1") && out.contains("other  (not chat-completions"),
        "{out}"
    );
    assert!(!out.contains("tid=") && !err.contains("tid="));

    let logout = agent(home.path(), &server)
        .args(["auth", "logout", "github-copilot", "--profile", "work"])
        .output()
        .unwrap();
    assert!(logout.status.success());
    assert!(!file.exists());
    let again = agent(home.path(), &server)
        .args(["auth", "status"])
        .output()
        .unwrap();
    assert!(text(&again).0.contains("no logins"));
}

#[test]
fn auth_usage_errors_exit_two_and_a_failed_login_leaves_nothing() {
    let (_f, server) = fake();
    let home = TempDir::new("oauth-cli-usage");
    let bad = agent(home.path(), &server)
        .args(["auth", "login"])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(1));
    assert!(text(&bad).1.contains("missing provider"));
    let bad = agent(home.path(), &server)
        .args(["auth", "login", "github-copilot", "--profile", "../x"])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
    let bad = agent(home.path(), &server)
        .args(["auth", "login", "nope"])
        .output()
        .unwrap();
    assert!(text(&bad).1.contains("not known"));
    assert!(
        !home.path().join("rung/oauth").exists()
            || std::fs::read_dir(home.path().join("rung/oauth"))
                .unwrap()
                .count()
                == 0
    );
}

#[test]
fn a_turn_on_an_oauth_credential_mints_once_and_sends_the_bearer() {
    let (f, server) = fake();
    let home = TempDir::new("oauth-turn");
    let login = agent(home.path(), &server)
        .args(["auth", "login", "github-copilot", "--profile", "work"])
        .output()
        .unwrap();
    assert!(login.status.success());

    for _ in 0..2 {
        let run = agent(home.path(), &server)
            .args(["--json", "--tools", "none", "say hi"])
            .output()
            .unwrap();
        let (out, err) = text(&run);
        assert!(run.status.success(), "{out}{err}");
        assert!(out.contains("oauth says hi"), "{out}");
        for secret in [GH_TOKEN, "tid=bin"] {
            assert!(
                !out.contains(secret) && !err.contains(secret),
                "a turn printed a secret"
            );
        }
    }
    // Two processes, one exchange: the second reused the stored token.
    assert_eq!(f.exchanges.load(Ordering::SeqCst), 1);
    let bearers = f.chat_bearers.lock().unwrap();
    assert!(bearers.len() >= 2);
    assert!(
        bearers.iter().all(|b| b.starts_with("Bearer tid=bin1;")),
        "{bearers:?}"
    );
    // Nothing the run wrote (sessions, memory) holds a token.
    let mut stack = vec![home.path().to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if !p.starts_with(home.path().join("rung/oauth")) {
                let body = std::fs::read_to_string(&p).unwrap_or_default();
                assert!(
                    !body.contains(GH_TOKEN) && !body.contains("tid=bin"),
                    "{} holds a token",
                    p.display()
                );
            }
        }
    }
}

#[test]
fn a_turn_with_no_login_fails_with_the_command_that_fixes_it() {
    let (_f, server) = fake();
    let home = TempDir::new("oauth-nologin");
    let run = agent(home.path(), &server)
        .args(["--json", "--tools", "none", "say hi"])
        .output()
        .unwrap();
    let (out, err) = text(&run);
    let all = format!("{out}{err}");
    assert!(!run.status.success() || all.contains("auth login"), "{all}");
    assert!(
        all.contains("auth login github-copilot --profile work"),
        "{all}"
    );
}
