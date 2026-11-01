//! OAuth credentials (`docs/rung-agent-oauth.md`) against a fake provider
//! server. No real login, no network beyond loopback, $0.
//!
//! The fake plays GitHub (device flow, Copilot token exchange) and the
//! Copilot API (chat, models) at once. Time is a fake clock that moves only
//! when something sleeps or the test advances it.

use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rung_agent_core::oauth::copilot::{
    self, Copilot, CopilotSection, CopilotSettings, REFRESH_AHEAD_SECS,
};
use rung_agent_core::oauth::{Clock, DeviceAnnouncement, LoginError, Provider, Store};
use rung_std::llm::{
    ChatMessage, CredentialError, CredentialSource, LlmConfig, RawCallError, raw_call,
};
use rung_testkit::TempDir;
use rung_testkit::http::{Req, Resp, Server};
use serde_json::{Value, json};

const T0: u64 = 1_800_000_000;
const REFRESH: &str = "gho_loginTokenThatIsLongEnoughToLookReal0001";

// ─── fakes ───────────────────────────────────────────────────────────────────

struct FakeClock {
    now: AtomicU64,
    sleeps: Mutex<Vec<u64>>,
}

impl FakeClock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            now: AtomicU64::new(T0),
            sleeps: Mutex::new(Vec::new()),
        })
    }
    fn advance(&self, secs: u64) {
        self.now.fetch_add(secs, Ordering::SeqCst);
    }
    fn slept(&self) -> Vec<u64> {
        self.sleeps.lock().unwrap().clone()
    }
}

impl Clock for FakeClock {
    fn now_secs(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
    fn sleep(&self, d: Duration) {
        self.sleeps.lock().unwrap().push(d.as_secs());
        self.advance(d.as_secs());
    }
}

/// The fake provider's knobs and counters.
struct Gh {
    clock: Arc<FakeClock>,
    /// Answers to the access-token poll, in order; the last one repeats.
    polls: Mutex<VecDeque<Value>>,
    interval: u64,
    expires_in: u64,
    /// What the exchange answers: the status, and how long it takes.
    exchange_status: Mutex<u16>,
    exchange_ms: u64,
    minted: AtomicUsize,
    /// Bearers the chat endpoint refuses with 401, once each.
    refuse_once: Mutex<Vec<String>>,
    /// Refuse every chat call with this status (0: serve).
    chat_status: AtomicUsize,
    model_hits: AtomicUsize,
    models_body: Mutex<Value>,
    /// The long-lived token the exchange accepts.
    good_refresh: Mutex<String>,
}

impl Gh {
    fn new(clock: Arc<FakeClock>) -> Arc<Self> {
        Arc::new(Self {
            clock,
            polls: Mutex::new(VecDeque::new()),
            interval: 5,
            expires_in: 900,
            exchange_status: Mutex::new(200),
            exchange_ms: 0,
            minted: AtomicUsize::new(0),
            refuse_once: Mutex::new(Vec::new()),
            chat_status: AtomicUsize::new(0),
            model_hits: AtomicUsize::new(0),
            models_body: Mutex::new(json!({"data": []})),
            good_refresh: Mutex::new(REFRESH.into()),
        })
    }

    fn handle(&self, r: &Req) -> Resp {
        let path = r.path.split('?').next().unwrap();
        match (r.method.as_str(), path) {
            ("POST", "/login/device/code") => Resp::json(
                200,
                json!({
                    "device_code": "dev-code-secret",
                    "user_code": "ABCD-1234",
                    "verification_uri": "https://github.com/login/device",
                    "interval": self.interval,
                    "expires_in": self.expires_in,
                }),
            ),
            ("POST", "/login/oauth/access_token") => {
                let mut q = self.polls.lock().unwrap();
                let v = if q.len() > 1 {
                    q.pop_front().unwrap()
                } else {
                    q.front()
                        .cloned()
                        .unwrap_or(json!({"error": "authorization_pending"}))
                };
                Resp::json(200, v)
            }
            ("GET", "/copilot_internal/v2/token") => {
                if self.exchange_ms > 0 {
                    std::thread::sleep(Duration::from_millis(self.exchange_ms));
                }
                let status = *self.exchange_status.lock().unwrap();
                let bearer = r.header("authorization").unwrap_or("");
                let good = format!("Bearer {}", self.good_refresh.lock().unwrap());
                if status == 200 && bearer == good {
                    let n = self.minted.fetch_add(1, Ordering::SeqCst) + 1;
                    let exp = self.clock.now_secs() + 1800;
                    Resp::json(
                        200,
                        json!({
                            "token": format!("tid=t{n};exp={exp};proxy-ep=proxy.individual.githubcopilot.com;sig{n}"),
                            "expires_at": exp,
                        }),
                    )
                } else if status == 200 {
                    Resp::json(401, json!({"message": "Bad credentials"}))
                } else {
                    let mut resp = Resp::json(status, json!({"message": "scripted"}));
                    if status == 429 {
                        resp = resp.with_header("Retry-After", "7");
                    }
                    resp
                }
            }
            ("POST", "/chat/completions") => {
                let bearer = r.header("authorization").unwrap_or("").to_string();
                let forced = self.chat_status.load(Ordering::SeqCst);
                if forced != 0 {
                    let mut resp =
                        Resp::json(forced as u16, json!({"error": {"message": "scripted"}}));
                    if forced == 429 {
                        resp = resp.with_header("Retry-After", "3");
                    }
                    return resp;
                }
                let mut once = self.refuse_once.lock().unwrap();
                if let Some(i) = once.iter().position(|b| format!("Bearer {b}") == bearer) {
                    once.remove(i);
                    return Resp::json(401, json!({"error": {"message": "token expired"}}));
                }
                Resp::json(
                    200,
                    json!({"id": "c1", "model": "m",
                        "choices": [{"message": {"role": "assistant", "content": "hi"},
                                     "finish_reason": "stop"}],
                        "usage": {"prompt_tokens": 3, "completion_tokens": 1}}),
                )
            }
            ("GET", "/models") => {
                self.model_hits.fetch_add(1, Ordering::SeqCst);
                Resp::json(200, self.models_body.lock().unwrap().clone())
            }
            _ => Resp::json(404, json!({"message": "no route"})),
        }
    }
}

struct World {
    gh: Arc<Gh>,
    server: Server,
    clock: Arc<FakeClock>,
    store: Store,
    _tmp: TempDir,
}

fn world_with(tune: impl FnOnce(&mut Gh)) -> World {
    let clock = FakeClock::new();
    let mut gh = Arc::try_unwrap(Gh::new(clock.clone())).ok().unwrap();
    tune(&mut gh);
    let gh = Arc::new(gh);
    let server = {
        let gh = gh.clone();
        Server::start(move |r| gh.handle(r))
    };
    let tmp = TempDir::new("oauth");
    World {
        gh,
        store: Store::new(tmp.path().join("oauth")),
        server,
        clock,
        _tmp: tmp,
    }
}

fn world() -> World {
    world_with(|_| {})
}

impl World {
    fn settings(&self, tweak: impl FnOnce(&mut CopilotSection)) -> CopilotSettings {
        let mut s = CopilotSection {
            github_url: Some(self.server.url().into()),
            api_url: Some(self.server.url().into()),
            base_url: Some(self.server.url().into()),
            ..Default::default()
        };
        tweak(&mut s);
        CopilotSettings::resolve(Some(&s), None).unwrap()
    }

    fn provider(&self) -> Copilot {
        Copilot::new(self.settings(|_| {}))
    }

    fn login(&self, profile: &str, token: &str) {
        self.store
            .save(&rung_agent_core::oauth::Login {
                provider: copilot::ID.into(),
                profile: profile.into(),
                refresh: token.into(),
                access: None,
                enterprise_domain: None,
                created_at: T0,
            })
            .unwrap();
    }

    fn credential(&self, profile: &str) -> Arc<dyn CredentialSource> {
        self.provider()
            .credential(self.store.clone(), profile, self.clock.clone())
    }

    fn llm(&self, cred: Arc<dyn CredentialSource>) -> LlmConfig {
        LlmConfig {
            base_url: String::new(),
            model: "m".into(),
            timeout_secs: 10,
            credential: Some(cred),
            ..rung_agent_core::config::dummy()
        }
    }

    fn chat(&self, cred: Arc<dyn CredentialSource>) -> Result<String, RawCallError> {
        let r = raw_call(&self.llm(cred), &[ChatMessage::user("hello")], &[])?;
        Ok(format!("{:?}", r.content))
    }
}

// ─── login (device flow) ─────────────────────────────────────────────────────

#[test]
fn device_flow_waits_out_pending_and_slow_down_then_stores_the_login() {
    let w = world();
    *w.gh.polls.lock().unwrap() = VecDeque::from([
        json!({"error": "authorization_pending"}),
        json!({"error": "slow_down"}),
        json!({"error": "authorization_pending"}),
        json!({"access_token": REFRESH, "token_type": "bearer"}),
    ]);
    let mut shown: Vec<DeviceAnnouncement> = Vec::new();
    w.provider()
        .login(&w.store, "default", w.clock.as_ref(), &mut |a| {
            shown.push(a.clone())
        })
        .unwrap();

    // The user is told the code and the URL, once.
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].user_code, "ABCD-1234");
    assert_eq!(shown[0].verification_uri, "https://github.com/login/device");
    // The interval is honoured; slow_down adds five seconds for the rest.
    assert_eq!(w.clock.slept(), vec![5, 5, 10, 10]);
    // The request names the app and the scope; the code is the device's.
    let reqs = w.server.requests();
    let first = reqs
        .iter()
        .find(|r| r.path == "/login/device/code")
        .unwrap();
    assert_eq!(first.form()["client_id"], copilot::DEFAULT_CLIENT_ID);
    assert_eq!(first.form()["scope"], "read:user");
    let poll = reqs
        .iter()
        .find(|r| r.path == "/login/oauth/access_token")
        .unwrap();
    assert_eq!(poll.form()["device_code"], "dev-code-secret");
    assert_eq!(
        poll.form()["grant_type"],
        "urn:ietf:params:oauth:grant-type:device_code"
    );
    // The login is stored, and is the token GitHub issued.
    let stored = w.store.load(copilot::ID, "default").unwrap().unwrap();
    assert_eq!(stored.refresh, REFRESH);
    assert!(stored.access.is_none());
}

#[test]
fn device_flow_ends_on_expired_token_denied_and_a_spent_code() {
    let w = world();
    *w.gh.polls.lock().unwrap() = VecDeque::from([json!({"error": "expired_token"})]);
    let e = w
        .provider()
        .login(&w.store, "default", w.clock.as_ref(), &mut |_| {})
        .unwrap_err();
    assert_eq!(e, LoginError::Expired);

    *w.gh.polls.lock().unwrap() = VecDeque::from([json!({"error": "access_denied"})]);
    let e = w
        .provider()
        .login(&w.store, "default", w.clock.as_ref(), &mut |_| {})
        .unwrap_err();
    assert_eq!(e, LoginError::Denied);

    // Pending forever: the code's own lifetime ends the flow.
    *w.gh.polls.lock().unwrap() = VecDeque::from([json!({"error": "authorization_pending"})]);
    let before = w.clock.slept().len();
    let e = w
        .provider()
        .login(&w.store, "default", w.clock.as_ref(), &mut |_| {})
        .unwrap_err();
    assert_eq!(e, LoginError::Expired);
    assert!(w.clock.slept().len() - before <= 900 / 5 + 1);
    // No failed login leaves a login behind.
    assert!(w.store.load(copilot::ID, "default").unwrap().is_none());
}

#[test]
fn an_unknown_device_flow_error_is_reported_without_a_token() {
    let w = world();
    *w.gh.polls.lock().unwrap() =
        VecDeque::from([json!({"error": "unsupported_grant_type", "error_description": "no"})]);
    let e = w
        .provider()
        .login(&w.store, "default", w.clock.as_ref(), &mut |_| {})
        .unwrap_err();
    assert!(
        matches!(e, LoginError::Failed(ref m) if m.contains("unsupported_grant_type")),
        "{e}"
    );
}

// ─── storage ─────────────────────────────────────────────────────────────────

#[test]
fn a_login_is_stored_at_mode_0600_in_a_private_directory() {
    let w = world();
    w.login("default", REFRESH);
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(w.store.dir()), 0o700);
    assert_eq!(mode(&w.store.login_path(copilot::ID, "default")), 0o600);
    // The lock and the minted-token update keep the mode too.
    w.credential("default").authorize().unwrap();
    for entry in std::fs::read_dir(w.store.dir()).unwrap() {
        let p = entry.unwrap().path();
        assert_eq!(mode(&p), 0o600, "{}", p.display());
    }
}

#[test]
fn a_profile_name_cannot_leave_the_store() {
    let w = world();
    for bad in ["../x", "a/b", ".hidden", "", "a--b", "x y"] {
        assert!(w.store.load(copilot::ID, bad).is_err(), "{bad:?}");
    }
    assert!(
        w.store
            .load(copilot::ID, "work.2025_a-b")
            .unwrap()
            .is_none()
    );
}

#[test]
fn logins_list_and_remove() {
    let w = world();
    w.login("personal", "gho_a1111111111111111111111111");
    w.login("work", "gho_b2222222222222222222222222");
    assert_eq!(
        w.store.list().unwrap(),
        vec![
            (copilot::ID.to_string(), "personal".to_string()),
            (copilot::ID.to_string(), "work".to_string())
        ]
    );
    assert!(w.store.remove(copilot::ID, "work").unwrap());
    assert!(!w.store.remove(copilot::ID, "work").unwrap());
    assert_eq!(w.store.list().unwrap().len(), 1);
}

// ─── exchange, expiry, route ─────────────────────────────────────────────────

#[test]
fn the_token_names_the_route_and_enterprise_has_its_own_rule() {
    assert_eq!(
        copilot::base_url_from_token("tid=1;exp=2;proxy-ep=proxy.individual.githubcopilot.com;x"),
        Some("https://api.individual.githubcopilot.com".into())
    );
    assert_eq!(
        copilot::base_url_from_token("tid=1;proxy-ep=proxy.business.githubcopilot.com"),
        Some("https://api.business.githubcopilot.com".into())
    );
    assert_eq!(copilot::base_url_from_token("tid=1;exp=2"), None);
    assert_eq!(copilot::expiry_from_token("tid=1;exp=1234;x"), Some(1234));

    let ent = CopilotSettings::resolve(
        Some(&CopilotSection {
            enterprise_domain: Some("https://Corp.Example.com/path".into()),
            ..Default::default()
        }),
        None,
    )
    .unwrap();
    assert_eq!(ent.enterprise_domain.as_deref(), Some("corp.example.com"));
    assert_eq!(ent.github_url, "https://corp.example.com");
    assert_eq!(ent.api_url, "https://api.corp.example.com");
    // A token with no proxy-ep falls back to the enterprise API.
    assert_eq!(
        copilot::base_url_for(&ent, "tid=1;exp=2"),
        "https://copilot-api.corp.example.com"
    );
    // An explicit route wins over the token's.
    let explicit = CopilotSettings::resolve(None, Some("http://localhost:9/v1/")).unwrap();
    assert_eq!(
        copilot::base_url_for(
            &explicit,
            "tid=1;proxy-ep=proxy.individual.githubcopilot.com"
        ),
        "http://localhost:9/v1"
    );
}

#[test]
fn authorize_returns_a_bearer_the_route_and_the_editor_headers() {
    let w = world();
    w.login("default", REFRESH);
    let auth = w.credential("default").authorize().unwrap();
    assert!(auth.bearer.starts_with("tid=t1;"), "{}", "[bearer]");
    assert_eq!(auth.base_url.as_deref(), Some(w.server.url()));
    let h = |n: &str| {
        auth.headers
            .iter()
            .find(|(k, _)| k == n)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(h("Copilot-Integration-Id"), Some("vscode-chat"));
    assert!(h("Editor-Version").is_some());
    assert!(h("Editor-Plugin-Version").is_some());
    assert!(h("User-Agent").is_some());
}

#[test]
fn the_token_route_is_used_when_nothing_overrides_it() {
    let w = world();
    w.login("default", REFRESH);
    let s = CopilotSettings::resolve(
        Some(&CopilotSection {
            github_url: Some(w.server.url().into()),
            api_url: Some(w.server.url().into()),
            ..Default::default()
        }),
        None,
    )
    .unwrap();
    let cred = Copilot::new(s).credential(w.store.clone(), "default", w.clock.clone());
    assert_eq!(
        cred.authorize().unwrap().base_url.as_deref(),
        Some("https://api.individual.githubcopilot.com")
    );
}

#[test]
fn editor_identity_headers_are_configuration() {
    let w = world();
    w.login("default", REFRESH);
    let s = w.settings(|s| {
        s.headers = Some(
            [("Editor-Version".to_string(), "vscode/9.9.9".to_string())]
                .into_iter()
                .collect(),
        );
    });
    let cred = Copilot::new(s).credential(w.store.clone(), "default", w.clock.clone());
    w.chat(cred).unwrap();
    let chat = w
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == "/chat/completions")
        .unwrap();
    assert_eq!(chat.header("editor-version"), Some("vscode/9.9.9"));
    // A default the config did not touch is still sent.
    assert_eq!(chat.header("copilot-integration-id"), Some("vscode-chat"));
}

#[test]
fn a_good_token_is_reused_and_renewed_ahead_of_its_expiry() {
    let w = world();
    w.login("default", REFRESH);
    let cred = w.credential("default");
    let a = cred.authorize().unwrap();
    // Nearly the whole lifetime later it is still the same token.
    w.clock.advance(1800 - REFRESH_AHEAD_SECS - 10);
    assert_eq!(cred.authorize().unwrap().bearer, a.bearer);
    assert_eq!(w.gh.minted.load(Ordering::SeqCst), 1);
    // Inside the renewal window, before it has actually expired, it is replaced.
    w.clock.advance(20);
    let b = cred.authorize().unwrap();
    assert_ne!(b.bearer, a.bearer);
    assert_eq!(w.gh.minted.load(Ordering::SeqCst), 2);
}

#[test]
fn a_chat_call_goes_through_the_minted_credential() {
    let w = world();
    w.login("default", REFRESH);
    let out = w.chat(w.credential("default")).unwrap();
    assert!(out.contains("hi"), "{out}");
    let chat = w
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == "/chat/completions")
        .unwrap();
    let bearer = chat.header("authorization").unwrap();
    assert!(bearer.starts_with("Bearer tid=t1;"));
    // The login's own token never goes to the chat endpoint.
    assert!(!bearer.contains(REFRESH));
}

// ─── single flight ───────────────────────────────────────────────────────────

#[test]
fn parallel_callers_cause_exactly_one_exchange() {
    let w = world_with(|g| g.exchange_ms = 150);
    w.login("default", REFRESH);
    // Four "processes" (separate sources over one store) of four threads each.
    let sources: Vec<Arc<dyn CredentialSource>> = (0..4).map(|_| w.credential("default")).collect();
    let mut handles = Vec::new();
    for i in 0..16 {
        let c = sources[i % 4].clone();
        handles.push(std::thread::spawn(move || c.authorize().unwrap().bearer));
    }
    let bearers: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(
        w.gh.minted.load(Ordering::SeqCst),
        1,
        "one exchange for all"
    );
    assert_eq!(w.server.count("GET", "/copilot_internal/v2/token"), 1);
    assert!(bearers.iter().all(|b| *b == bearers[0]));
}

#[test]
fn a_second_process_reuses_the_token_the_first_stored() {
    let w = world();
    w.login("default", REFRESH);
    let first = w.credential("default").authorize().unwrap();
    let second = w.credential("default").authorize().unwrap();
    assert_eq!(first.bearer, second.bearer);
    assert_eq!(w.gh.minted.load(Ordering::SeqCst), 1);
}

// ─── 401, revocation, throttling ─────────────────────────────────────────────

#[test]
fn a_401_forces_one_refresh_and_one_retry() {
    let w = world();
    w.login("default", REFRESH);
    // The first minted token is refused once (expired server-side).
    w.gh.refuse_once.lock().unwrap().push(
        // The fake mints `tid=t1;exp=<T0+1800>;proxy-ep=…;sig1`.
        format!(
            "tid=t1;exp={};proxy-ep=proxy.individual.githubcopilot.com;sig1",
            T0 + 1800
        ),
    );
    let out = w.chat(w.credential("default")).unwrap();
    assert!(out.contains("hi"));
    assert_eq!(w.server.count("POST", "/chat/completions"), 2);
    assert_eq!(w.gh.minted.load(Ordering::SeqCst), 2);
}

#[test]
fn a_second_401_is_the_answer_not_a_loop() {
    let w = world();
    w.login("default", REFRESH);
    w.gh.chat_status.store(401, Ordering::SeqCst);
    let e = w.chat(w.credential("default")).unwrap_err();
    assert!(matches!(e, RawCallError::Auth { .. }), "{e}");
    assert_eq!(w.server.count("POST", "/chat/completions"), 2);
    assert_eq!(w.gh.minted.load(Ordering::SeqCst), 2);
}

#[test]
fn a_revoked_login_is_an_actionable_auth_failure_and_is_not_retried_hot() {
    let w = world();
    w.login("default", "gho_revokedRevokedRevokedRevoked0001");
    let cred = w.credential("default");
    let e = cred.authorize().unwrap_err();
    let CredentialError::Revoked(msg) = &e else {
        panic!("{e:?}")
    };
    assert!(
        msg.contains("auth login github-copilot --profile default"),
        "{msg}"
    );
    assert!(!msg.contains("gho_"), "{msg}");
    // Through a call it is an auth failure (a host reads it as blocked).
    let call = w.chat(cred.clone()).unwrap_err();
    assert!(matches!(call, RawCallError::Auth { .. }), "{call}");
    assert!(!call.is_retryable());
    // Asking again touches no network: one exchange total.
    assert_eq!(w.server.count("GET", "/copilot_internal/v2/token"), 1);
    assert_eq!(w.server.count("POST", "/chat/completions"), 0);
    // A new login (another token) is tried again, and works.
    w.login("default", REFRESH);
    assert!(cred.authorize().is_ok());
}

#[test]
fn a_missing_login_says_how_to_make_one() {
    let w = world();
    let e = w.credential("work").authorize().unwrap_err();
    let CredentialError::Missing(m) = e else {
        panic!("{e:?}")
    };
    assert!(
        m.contains("auth login github-copilot --profile work"),
        "{m}"
    );
    assert_eq!(w.server.requests().len(), 0);
}

#[test]
fn no_account_access_is_a_revoked_style_failure_and_a_throttle_waits() {
    let w = world();
    w.login("default", REFRESH);
    *w.gh.exchange_status.lock().unwrap() = 404;
    let e = w.credential("default").authorize().unwrap_err();
    assert!(
        matches!(e, CredentialError::Revoked(ref m) if m.contains("no Copilot access")),
        "{e:?}"
    );

    *w.gh.exchange_status.lock().unwrap() = 429;
    let e = w.credential("default").authorize().unwrap_err();
    assert!(
        matches!(
            e,
            CredentialError::RateLimited {
                retry_after_ms: Some(7000),
                ..
            }
        ),
        "{e:?}"
    );
    let raw: RawCallError = e.into();
    assert!(raw.is_retryable() && raw.is_rate_limited());

    *w.gh.exchange_status.lock().unwrap() = 503;
    let e = w.credential("default").authorize().unwrap_err();
    assert!(matches!(e, CredentialError::Unavailable(_)), "{e:?}");
}

#[test]
fn quota_and_rate_refusals_from_the_api_are_a_backoff_not_a_hot_retry() {
    let w = world();
    w.login("default", REFRESH);
    w.gh.chat_status.store(429, Ordering::SeqCst);
    let e = w.chat(w.credential("default")).unwrap_err();
    // A rate limit carries the wait the provider asked for, so the call's
    // retry (and a host's governor) sleeps that long instead of retrying hot.
    match &e {
        RawCallError::RateLimit { retry_after_ms, .. } => assert_eq!(*retry_after_ms, Some(3000)),
        other => panic!("{other}"),
    }
    assert!(e.is_retryable());
    // No token was minted again by the refusal.
    assert_eq!(w.gh.minted.load(Ordering::SeqCst), 1);
}

// ─── profiles ────────────────────────────────────────────────────────────────

#[test]
fn two_profiles_of_one_provider_use_their_own_logins() {
    let w = world();
    let personal = "gho_personalPersonalPersonalPersonal01";
    let work = "gho_workWorkWorkWorkWorkWorkWorkWork0002";
    w.login("personal", personal);
    w.login("work", work);
    *w.gh.good_refresh.lock().unwrap() = work.into();

    // The work login exchanges; the personal one is rejected by the fake.
    assert!(w.credential("work").authorize().is_ok());
    assert!(matches!(
        w.credential("personal").authorize(),
        Err(CredentialError::Revoked(_))
    ));
    let exchanges = w.server.requests();
    let sent: Vec<&str> = exchanges
        .iter()
        .filter(|r| r.path == "/copilot_internal/v2/token")
        .filter_map(|r| r.header("authorization"))
        .collect();
    assert!(sent.contains(&format!("Bearer {work}").as_str()));
    assert!(sent.contains(&format!("Bearer {personal}").as_str()));
    // Each profile has its own minted token on disk.
    assert!(
        w.store
            .load(copilot::ID, "work")
            .unwrap()
            .unwrap()
            .access
            .is_some()
    );
    assert!(
        w.store
            .load(copilot::ID, "personal")
            .unwrap()
            .unwrap()
            .access
            .is_none()
    );
}

// ─── models ──────────────────────────────────────────────────────────────────

#[test]
fn the_model_list_is_fetched_filtered_and_cached() {
    let w = world();
    w.login("default", REFRESH);
    *w.gh.models_body.lock().unwrap() = json!({"data": [
        {"id": "gpt-4.1", "model_picker_enabled": true,
         "capabilities": {"supports": {"tool_calls": true}},
         "supported_endpoints": ["/chat/completions"], "policy": {"state": "enabled"}},
        {"id": "claude-sonnet-5.5", "model_picker_enabled": true,
         "supported_endpoints": ["/v1/messages", "/chat/completions"]},
        {"id": "codex-x", "model_picker_enabled": true,
         "supported_endpoints": ["/responses"]},
        {"id": "embed-1", "capabilities": {"supports": {"tool_calls": false}}},
        {"id": "off-1", "policy": {"state": "disabled"}},
    ]});
    let p = w.provider();
    let list = p
        .models(&w.store, "default", w.clock.clone(), false)
        .unwrap();
    let ids: Vec<(&str, bool)> = list
        .iter()
        .map(|m| (m.id.as_str(), m.chat_completions))
        .collect();
    assert_eq!(
        ids,
        vec![
            ("claude-sonnet-5.5", true),
            ("codex-x", false),
            ("gpt-4.1", true)
        ]
    );
    let req = w
        .server
        .requests()
        .into_iter()
        .find(|r| r.path == "/models")
        .unwrap();
    assert_eq!(req.header("x-github-api-version"), Some("2026-06-01"));
    assert!(
        req.header("authorization")
            .unwrap()
            .starts_with("Bearer tid=")
    );

    // Cached: no second fetch until forced or the TTL passes.
    p.models(&w.store, "default", w.clock.clone(), false)
        .unwrap();
    assert_eq!(w.gh.model_hits.load(Ordering::SeqCst), 1);
    p.models(&w.store, "default", w.clock.clone(), true)
        .unwrap();
    assert_eq!(w.gh.model_hits.load(Ordering::SeqCst), 2);
    w.clock.advance(copilot::MODELS_TTL_SECS + 1);
    p.models(&w.store, "default", w.clock.clone(), false)
        .unwrap();
    assert_eq!(w.gh.model_hits.load(Ordering::SeqCst), 3);
}

// ─── no secret becomes text ──────────────────────────────────────────────────

#[test]
fn no_debug_or_error_text_holds_a_token() {
    let w = world();
    w.login("default", REFRESH);
    let cred = w.credential("default");
    let auth = cred.authorize().unwrap();
    let stored = w.store.load(copilot::ID, "default").unwrap().unwrap();
    let cfg = w.llm(cred.clone());
    let seen = [
        format!("{auth:?}"),
        format!("{cred:?}"),
        format!("{stored:?}"),
        format!("{cfg:?}"),
    ]
    .join("\n");
    for secret in [REFRESH, auth.bearer.as_str(), "sig1"] {
        assert!(!seen.contains(secret), "leaked {secret}");
    }
    // Failures name the login, not the token.
    w.gh.chat_status.store(401, Ordering::SeqCst);
    let e = format!("{:?}", w.chat(cred).unwrap_err());
    assert!(!e.contains(REFRESH) && !e.contains(&auth.bearer), "{e}");
}
