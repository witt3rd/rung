//! GitHub Copilot, the first OAuth provider.
//!
//! 1. **Login (device code).** `POST {github}/login/device/code` with the
//!    editor-client OAuth app id and `scope=read:user`. The user opens the
//!    verification URL on any machine and types the code; meanwhile rung
//!    polls `POST {github}/login/oauth/access_token` (grant `device_code`),
//!    honouring `interval`, `slow_down`, `authorization_pending` and
//!    `expired_token`. The result is a long-lived GitHub token (GitHub does
//!    not rotate it). No browser is needed on the host, so it works over SSH.
//! 2. **Exchange.** `GET {api}/copilot_internal/v2/token` with that token
//!    returns a short-lived Copilot token (about thirty minutes). It is
//!    minted on demand, cached in memory, and shared with every other process
//!    through the login's file, so one exchange serves them all.
//! 3. **Route.** The Copilot token names its proxy (`proxy-ep=`), which maps
//!    to `https://api.<individual|business|enterprise>.githubcopilot.com`; an
//!    enterprise login without one maps to `https://copilot-api.<domain>`.
//! 4. **Identity headers.** The service expects an editor client's
//!    `User-Agent`, `Editor-Version`, `Editor-Plugin-Version` and
//!    `Copilot-Integration-Id`. Their versions go stale, so they are
//!    configuration (`oauth.github-copilot.headers`); the values here are
//!    only defaults. Whether a non-editor client may present them is a policy
//!    question this code does not settle (`docs/rung-agent-oauth.md`, O14).
//! 5. **Models.** `GET {base}/models` lists what the account may use.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rung_std::llm::{CredentialError, CredentialSource, RequestAuth};
use serde::Deserialize;
use serde_json::Value;

use super::store::{Login, Minted, Store};
use super::{Clock, DeviceAnnouncement, LoginError, ModelInfo, Provider};
use crate::redact::redact;

/// The provider's registry name.
pub const ID: &str = "github-copilot";

/// The public OAuth app id that editor clients use. Configuration
/// (`client_id`), not a secret.
pub const DEFAULT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";

/// Editor-identity defaults; every one is overridable in the config file.
const DEFAULT_HEADERS: &[(&str, &str)] = &[
    ("User-Agent", "GitHubCopilotChat/0.35.0"),
    ("Editor-Version", "vscode/1.107.0"),
    ("Editor-Plugin-Version", "copilot-chat/0.35.0"),
    ("Copilot-Integration-Id", "vscode-chat"),
    ("Openai-Intent", "conversation-edits"),
    ("X-Initiator", "agent"),
];
const DEFAULT_API_VERSION: &str = "2026-06-01";
const DEFAULT_BASE: &str = "https://api.individual.githubcopilot.com";

/// A minted token is renewed when it has less than this left.
pub const REFRESH_AHEAD_SECS: u64 = 300;
/// How long a fetched model list is reused.
pub const MODELS_TTL_SECS: u64 = 3600;
/// Added to the poll interval on `slow_down` when the server names none.
const SLOW_DOWN_STEP_SECS: u64 = 5;

/// `oauth.github-copilot:` in `config.yaml`. Every key is optional.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopilotSection {
    /// The OAuth app id for the device flow.
    pub client_id: Option<String>,
    /// A GitHub Enterprise domain (`corp.ghe.com`).
    pub enterprise_domain: Option<String>,
    /// Where the device flow talks (default `https://<domain>`).
    pub github_url: Option<String>,
    /// Where the token exchange talks (default `https://api.<domain>`).
    pub api_url: Option<String>,
    /// The chat route, overriding the token's `proxy-ep`.
    pub base_url: Option<String>,
    /// Request headers, merged over the defaults.
    pub headers: Option<BTreeMap<String, String>>,
    /// `X-GitHub-Api-Version` for the model list.
    pub api_version: Option<String>,
    /// Seconds one request to GitHub may take.
    pub timeout_secs: Option<u64>,
}

/// The resolved settings.
#[derive(Debug, Clone, PartialEq)]
pub struct CopilotSettings {
    pub client_id: String,
    pub enterprise_domain: Option<String>,
    pub github_url: String,
    pub api_url: String,
    pub base_url: Option<String>,
    pub headers: Vec<(String, String)>,
    pub api_version: String,
    pub timeout_secs: u64,
}

/// `https://corp.example/x` or `corp.example` → `corp.example`.
pub fn normalize_domain(input: &str) -> Option<String> {
    let t = input.trim();
    let rest = t.split_once("://").map_or(t, |(_, r)| r);
    let host = rest
        .split(['/', '?', '#'])
        .next()?
        .trim()
        .to_ascii_lowercase();
    let ok = !host.is_empty()
        && host.contains('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    ok.then_some(host)
}

fn http_url(what: &str, v: &str) -> Result<String, String> {
    let v = v.trim().trim_end_matches('/');
    if (v.starts_with("https://") || v.starts_with("http://")) && v.len() > 8 {
        Ok(v.to_string())
    } else {
        Err(format!("oauth.{ID}.{what}: not an http(s) URL"))
    }
}

impl CopilotSettings {
    /// Resolve the section. `explicit_base` is `llm.base_url` when the user
    /// set one: it wins over the route the token names.
    pub fn resolve(
        section: Option<&CopilotSection>,
        explicit_base: Option<&str>,
    ) -> Result<Self, String> {
        let default = CopilotSection::default();
        let s = section.unwrap_or(&default);
        let enterprise_domain =
            match s.enterprise_domain.as_deref() {
                None => None,
                Some(d) => Some(normalize_domain(d).ok_or_else(|| {
                    format!("oauth.{ID}.enterprise_domain: '{d}' is not a domain")
                })?),
            };
        let (github_default, api_default) = match &enterprise_domain {
            Some(d) => (format!("https://{d}"), format!("https://api.{d}")),
            None => ("https://github.com".into(), "https://api.github.com".into()),
        };
        let github_url = http_url(
            "github_url",
            s.github_url.as_deref().unwrap_or(&github_default),
        )?;
        let api_url = http_url("api_url", s.api_url.as_deref().unwrap_or(&api_default))?;
        let base_url = match (explicit_base, s.base_url.as_deref()) {
            (Some(b), _) | (None, Some(b)) if !b.trim().is_empty() => {
                Some(http_url("base_url", b)?)
            }
            _ => None,
        };
        let mut headers: BTreeMap<String, String> = DEFAULT_HEADERS
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        for (k, v) in s.headers.iter().flatten() {
            if k.trim().is_empty() || v.contains(['\r', '\n']) || k.contains(['\r', '\n', ':']) {
                return Err(format!("oauth.{ID}.headers: bad header '{k}'"));
            }
            headers.insert(k.trim().to_string(), v.clone());
        }
        Ok(Self {
            client_id: s
                .client_id
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .unwrap_or(DEFAULT_CLIENT_ID)
                .to_string(),
            enterprise_domain,
            github_url,
            api_url,
            base_url,
            headers: headers.into_iter().collect(),
            api_version: s
                .api_version
                .clone()
                .unwrap_or_else(|| DEFAULT_API_VERSION.into()),
            timeout_secs: s.timeout_secs.unwrap_or(30),
        })
    }
}

/// The API base a Copilot token names: its `proxy-ep` with `proxy.` turned
/// into `api.`. `None` when the token names none.
pub fn base_url_from_token(token: &str) -> Option<String> {
    let host = token
        .split(';')
        .find_map(|kv| kv.strip_prefix("proxy-ep="))?
        .trim();
    if host.is_empty() {
        return None;
    }
    let host = host
        .strip_prefix("proxy.")
        .map_or_else(|| host.to_string(), |rest| format!("api.{rest}"));
    Some(format!("https://{host}"))
}

/// The expiry a Copilot token states (`exp=<epoch seconds>`).
pub fn expiry_from_token(token: &str) -> Option<u64> {
    token
        .split(';')
        .find_map(|kv| kv.strip_prefix("exp="))?
        .parse()
        .ok()
}

/// The chat base for a token: an explicit one, else the token's, else the
/// enterprise domain's, else the individual endpoint.
pub fn base_url_for(settings: &CopilotSettings, token: &str) -> String {
    if let Some(b) = &settings.base_url {
        return b.clone();
    }
    if let Some(b) = base_url_from_token(token) {
        return b;
    }
    match &settings.enterprise_domain {
        Some(d) => format!("https://copilot-api.{d}"),
        None => DEFAULT_BASE.into(),
    }
}

fn client(settings: &CopilotSettings) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(settings.timeout_secs))
        .build()
        .map_err(|e| format!("http client: {e}"))
}

/// A provider's own error text, made safe to show: the `message` or
/// `error_description` field of a JSON body, redacted and cut short. Never
/// the raw body.
fn provider_message(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let m = ["message", "error_description", "error"]
        .iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .unwrap_or("");
    let m: String = redact(m).chars().take(200).collect();
    m
}

// ─── The provider ────────────────────────────────────────────────────────────

/// GitHub Copilot: the registry entry.
#[derive(Debug, Clone)]
pub struct Copilot {
    settings: Arc<CopilotSettings>,
}

impl Copilot {
    pub fn new(settings: CopilotSettings) -> Self {
        Self {
            settings: Arc::new(settings),
        }
    }

    pub fn from_section(
        section: Option<&serde_yaml::Value>,
        explicit_base: Option<&str>,
    ) -> Result<Self, String> {
        let parsed: Option<CopilotSection> = match section {
            None | Some(serde_yaml::Value::Null) => None,
            Some(v) => {
                Some(serde_yaml::from_value(v.clone()).map_err(|e| format!("oauth.{ID}: {e}"))?)
            }
        };
        Ok(Self::new(CopilotSettings::resolve(
            parsed.as_ref(),
            explicit_base,
        )?))
    }

    pub fn settings(&self) -> &CopilotSettings {
        &self.settings
    }

    /// Ask GitHub for a device code.
    pub fn start_device_flow(&self) -> Result<DeviceCode, LoginError> {
        let s = &self.settings;
        let http = client(s).map_err(LoginError::Failed)?;
        let resp = http
            .post(format!("{}/login/device/code", s.github_url))
            .header("Accept", "application/json")
            .header("User-Agent", user_agent(s))
            .form(&[("client_id", s.client_id.as_str()), ("scope", "read:user")])
            .send()
            .map_err(|e| LoginError::Failed(format!("device code request: {e}")))?;
        let status = resp.status().as_u16();
        let body = resp.text().unwrap_or_default();
        if status != 200 {
            return Err(LoginError::Failed(format!(
                "device code request: HTTP {status} {}",
                provider_message(&body)
            )));
        }
        let v: Value = serde_json::from_str(&body)
            .map_err(|_| LoginError::Failed("device code response is not JSON".into()))?;
        let field = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let (Some(device_code), Some(user_code), Some(uri)) = (
            field("device_code"),
            field("user_code"),
            field("verification_uri"),
        ) else {
            return Err(LoginError::Failed(
                "device code response is missing fields".into(),
            ));
        };
        // A URL that is shown to the user must be one a browser opens.
        if !(uri.starts_with("https://") || uri.starts_with("http://")) {
            return Err(LoginError::Failed(
                "the verification URL is not an http(s) URL".into(),
            ));
        }
        Ok(DeviceCode {
            device_code,
            user_code,
            verification_uri: uri,
            interval: v.get("interval").and_then(Value::as_u64).unwrap_or(5),
            expires_in: v.get("expires_in").and_then(Value::as_u64).unwrap_or(900),
        })
    }

    /// Poll until the user approves; returns the long-lived GitHub token.
    ///
    /// The wait before each poll is the server's `interval`, grown by
    /// `slow_down` (to the interval it names, else by five seconds), and the
    /// whole flow ends when `expires_in` is spent.
    pub fn poll_device_flow(
        &self,
        dc: &DeviceCode,
        clock: &dyn Clock,
    ) -> Result<String, LoginError> {
        let s = &self.settings;
        let http = client(s).map_err(LoginError::Failed)?;
        let deadline = clock.now_secs().saturating_add(dc.expires_in);
        let mut interval = dc.interval;
        let mut transient = 0u32;
        loop {
            clock.sleep(Duration::from_secs(interval));
            if clock.now_secs() >= deadline {
                return Err(LoginError::Expired);
            }
            let sent = http
                .post(format!("{}/login/oauth/access_token", s.github_url))
                .header("Accept", "application/json")
                .header("User-Agent", user_agent(s))
                .form(&[
                    ("client_id", s.client_id.as_str()),
                    ("device_code", dc.device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send();
            let body = match sent {
                Ok(r) if r.status().is_success() => r.text().unwrap_or_default(),
                // A hiccup is not a verdict: allow a few in a row.
                Ok(_) | Err(_) => {
                    transient += 1;
                    if transient >= 3 {
                        return Err(LoginError::Failed(
                            "the access-token poll keeps failing".into(),
                        ));
                    }
                    continue;
                }
            };
            transient = 0;
            let v: Value = serde_json::from_str(&body)
                .map_err(|_| LoginError::Failed("access-token response is not JSON".into()))?;
            if let Some(token) = v.get("access_token").and_then(Value::as_str) {
                return Ok(token.to_string());
            }
            match v.get("error").and_then(Value::as_str) {
                Some("authorization_pending") => {}
                Some("slow_down") => {
                    interval = match v.get("interval").and_then(Value::as_u64) {
                        Some(i) if i > interval => i,
                        _ => interval + SLOW_DOWN_STEP_SECS,
                    };
                }
                Some("expired_token") => return Err(LoginError::Expired),
                Some("access_denied") => return Err(LoginError::Denied),
                Some(other) => {
                    return Err(LoginError::Failed(format!(
                        "device flow failed: {other} {}",
                        provider_message(&body)
                    )));
                }
                None => {
                    return Err(LoginError::Failed(
                        "access-token response has neither a token nor an error".into(),
                    ));
                }
            }
        }
    }
}

fn user_agent(s: &CopilotSettings) -> &str {
    s.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("user-agent"))
        .map_or("rung-agent", |(_, v)| v.as_str())
}

/// What GitHub returns for a device-code request.
#[derive(Clone, PartialEq)]
pub struct DeviceCode {
    /// Secret: proves this device's request. Never shown.
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

impl std::fmt::Debug for DeviceCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCode")
            .field("device_code", &"[redacted]")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .finish()
    }
}

impl Provider for Copilot {
    fn id(&self) -> &'static str {
        ID
    }

    fn login(
        &self,
        store: &Store,
        profile: &str,
        clock: &dyn Clock,
        announce: &mut dyn FnMut(&DeviceAnnouncement),
    ) -> Result<(), LoginError> {
        super::store::valid_name(profile).map_err(LoginError::Failed)?;
        let dc = self.start_device_flow()?;
        announce(&DeviceAnnouncement {
            user_code: dc.user_code.clone(),
            verification_uri: dc.verification_uri.clone(),
            expires_in: dc.expires_in,
        });
        let token = self.poll_device_flow(&dc, clock)?;
        // One writer at a time per login; a new login drops the old minted token.
        let _lock = store.lock(ID, profile).map_err(LoginError::Failed)?;
        store
            .save(&Login {
                provider: ID.into(),
                profile: profile.into(),
                refresh: token,
                access: None,
                enterprise_domain: self.settings.enterprise_domain.clone(),
                created_at: clock.now_secs(),
            })
            .map_err(LoginError::Failed)
    }

    fn credential(
        &self,
        store: Store,
        profile: &str,
        clock: Arc<dyn Clock>,
    ) -> Arc<dyn CredentialSource> {
        Arc::new(CopilotCredential::new(
            self.settings.clone(),
            store,
            profile,
            clock,
        ))
    }

    fn models(
        &self,
        store: &Store,
        profile: &str,
        clock: Arc<dyn Clock>,
        refresh: bool,
    ) -> Result<Vec<ModelInfo>, String> {
        CopilotCredential::new(self.settings.clone(), store.clone(), profile, clock).models(refresh)
    }
}

// ─── The credential ──────────────────────────────────────────────────────────

#[derive(Default)]
struct Mem {
    token: Option<Minted>,
    /// A long-lived token GitHub has rejected: no exchange is tried with it
    /// again in this process (a new login has another value).
    revoked: Option<String>,
}

/// The Copilot credential of one login (provider + profile).
///
/// **Single flight.** [`authorize`](CredentialSource::authorize) returns the
/// in-memory token while it is good. Otherwise it takes this process's lock
/// (callers of one source queue behind one exchange) and then the login's
/// file lock (processes queue behind one exchange), re-reads the file, and
/// exchanges only if the file holds no good token either. The result goes
/// back to the file, so the next process finds it.
pub struct CopilotCredential {
    settings: Arc<CopilotSettings>,
    store: Store,
    profile: String,
    clock: Arc<dyn Clock>,
    mem: Mutex<Mem>,
}

impl std::fmt::Debug for CopilotCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CopilotCredential")
            .field("provider", &ID)
            .field("profile", &self.profile)
            .finish()
    }
}

impl CopilotCredential {
    pub fn new(
        settings: Arc<CopilotSettings>,
        store: Store,
        profile: &str,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            settings,
            store,
            profile: profile.to_string(),
            clock,
            mem: Mutex::new(Mem::default()),
        }
    }

    fn login_hint(&self) -> String {
        format!(
            "run `rung-agent auth login {ID} --profile {}`",
            self.profile
        )
    }

    fn fresh(&self, m: &Minted) -> bool {
        m.expires_at > self.clock.now_secs().saturating_add(REFRESH_AHEAD_SECS)
    }

    fn auth_for(&self, token: &str) -> RequestAuth {
        RequestAuth {
            bearer: token.to_string(),
            base_url: Some(base_url_for(&self.settings, token)),
            headers: self.settings.headers.clone(),
        }
    }

    /// Exchange the long-lived token for a Copilot token. One request.
    fn exchange(&self, refresh: &str) -> Result<Minted, CredentialError> {
        let s = &self.settings;
        let http = client(s).map_err(CredentialError::Unavailable)?;
        let mut req = http
            .get(format!("{}/copilot_internal/v2/token", s.api_url))
            .header("Accept", "application/json")
            .bearer_auth(refresh);
        for (k, v) in &s.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = req.send().map_err(|e| {
            // reqwest's error names the URL, never the header.
            CredentialError::Unavailable(format!("token exchange failed: {e}"))
        })?;
        let status = resp.status().as_u16();
        let retry_after_ms = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|secs| secs.saturating_mul(1000));
        let body = resp.text().unwrap_or_default();
        match status {
            200 => {}
            401 => {
                return Err(CredentialError::Revoked(format!(
                    "GitHub rejected the stored {ID} login for profile '{}' \
                     (revoked or expired): {}",
                    self.profile,
                    self.login_hint()
                )));
            }
            403 | 404 if retry_after_ms.is_none() => {
                return Err(CredentialError::Revoked(format!(
                    "this GitHub account has no Copilot access for profile '{}' (HTTP {status}): \
                     check the subscription, then {}",
                    self.profile,
                    self.login_hint()
                )));
            }
            403 | 429 => {
                return Err(CredentialError::RateLimited {
                    retry_after_ms,
                    message: format!("GitHub throttled the Copilot token exchange (HTTP {status})"),
                });
            }
            other => {
                return Err(CredentialError::Unavailable(format!(
                    "Copilot token exchange: HTTP {other} {}",
                    provider_message(&body)
                )));
            }
        }
        let v: Value = serde_json::from_str(&body).map_err(|_| {
            CredentialError::Unavailable("Copilot token response is not JSON".into())
        })?;
        let Some(token) = v
            .get("token")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
        else {
            return Err(CredentialError::Unavailable(
                "Copilot token response has no token".into(),
            ));
        };
        let expires_at = v
            .get("expires_at")
            .and_then(Value::as_u64)
            .or_else(|| expiry_from_token(token))
            .ok_or_else(|| {
                CredentialError::Unavailable("Copilot token response has no expiry".into())
            })?;
        Ok(Minted {
            token: token.to_string(),
            expires_at,
        })
    }

    /// The model list for the login. Cached on disk for [`MODELS_TTL_SECS`];
    /// `refresh` ignores the cache.
    pub fn models(&self, refresh: bool) -> Result<Vec<ModelInfo>, String> {
        let now = self.clock.now_secs();
        if !refresh
            && let Some(c) = self.load_models_cache()
            && c.fetched_at + MODELS_TTL_SECS > now
        {
            return Ok(c.models);
        }
        let list = self.fetch_models().map_err(|e| e.to_string())?;
        self.save_models_cache(&ModelsCache {
            fetched_at: now,
            models: list.clone(),
        });
        Ok(list)
    }

    fn fetch_models(&self) -> Result<Vec<ModelInfo>, CredentialError> {
        let http = client(&self.settings).map_err(CredentialError::Unavailable)?;
        let mut auth = self.authorize()?;
        let mut retried = false;
        let body = loop {
            let base = auth.base_url.clone().unwrap_or_else(|| DEFAULT_BASE.into());
            let mut req = http
                .get(format!("{}/models", base.trim_end_matches('/')))
                .header("Accept", "application/json")
                .header("X-GitHub-Api-Version", self.settings.api_version.as_str())
                .bearer_auth(&auth.bearer);
            for (k, v) in &auth.headers {
                req = req.header(k.as_str(), v.as_str());
            }
            let resp = req
                .send()
                .map_err(|e| CredentialError::Unavailable(format!("model list failed: {e}")))?;
            let status = resp.status().as_u16();
            if status == 401 && !retried {
                self.rejected(&auth);
                auth = self.authorize()?;
                retried = true;
                continue;
            }
            let text = resp.text().unwrap_or_default();
            if status != 200 {
                return Err(CredentialError::Unavailable(format!(
                    "model list: HTTP {status} {}",
                    provider_message(&text)
                )));
            }
            break text;
        };
        parse_models(&body).map_err(CredentialError::Unavailable)
    }

    fn models_cache_path(&self) -> std::path::PathBuf {
        self.store
            .dir()
            .join(format!("{ID}--{}.models.json", self.profile))
    }

    fn load_models_cache(&self) -> Option<ModelsCache> {
        serde_json::from_str(&std::fs::read_to_string(self.models_cache_path()).ok()?).ok()
    }

    fn save_models_cache(&self, c: &ModelsCache) {
        // Best effort: a cache that cannot be written is only a cache miss.
        if let Ok(body) = serde_json::to_string_pretty(c) {
            let _ = self
                .store
                .write_aside(&self.models_cache_path(), body.as_bytes());
        }
    }
}

#[derive(serde::Serialize, Deserialize)]
struct ModelsCache {
    fetched_at: u64,
    models: Vec<ModelInfo>,
}

/// The usable models of a `/models` body: tool-calling, picker-enabled and
/// not disabled by policy. `chat_completions` says whether the engine's
/// OpenAI chat-completions style serves it (v1 serves only those).
pub fn parse_models(body: &str) -> Result<Vec<ModelInfo>, String> {
    let v: Value = serde_json::from_str(body).map_err(|_| "model list is not JSON".to_string())?;
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or("model list has no data")?;
    let mut out: Vec<ModelInfo> = data
        .iter()
        .filter_map(|m| {
            let id = m.get("id")?.as_str()?.to_string();
            if m.pointer("/capabilities/supports/tool_calls") == Some(&Value::Bool(false)) {
                return None;
            }
            if m.get("model_picker_enabled") == Some(&Value::Bool(false)) {
                return None;
            }
            if m.pointer("/policy/state").and_then(Value::as_str) == Some("disabled") {
                return None;
            }
            // Without a stated endpoint list, assume the common style.
            let chat_completions = m
                .get("supported_endpoints")
                .and_then(Value::as_array)
                .is_none_or(|eps| eps.iter().any(|e| e.as_str() == Some("/chat/completions")));
            Some(ModelInfo {
                id,
                chat_completions,
            })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    Ok(out)
}

impl CredentialSource for CopilotCredential {
    fn authorize(&self) -> Result<RequestAuth, CredentialError> {
        let mut mem = self.mem.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(m) = &mem.token
            && self.fresh(m)
        {
            return Ok(self.auth_for(&m.token));
        }
        mem.token = None;

        let _file = self
            .store
            .lock(ID, &self.profile)
            .map_err(CredentialError::Unavailable)?;
        let mut login = self
            .store
            .load(ID, &self.profile)
            .map_err(CredentialError::Unavailable)?
            .ok_or_else(|| {
                CredentialError::Missing(format!(
                    "no {ID} login for profile '{}': {}",
                    self.profile,
                    self.login_hint()
                ))
            })?;
        if mem.revoked.as_deref() == Some(login.refresh.as_str()) {
            return Err(CredentialError::Revoked(format!(
                "GitHub rejected the stored {ID} login for profile '{}': {}",
                self.profile,
                self.login_hint()
            )));
        }
        // Another process may have minted while this one waited for the lock.
        if let Some(m) = &login.access
            && self.fresh(m)
        {
            let auth = self.auth_for(&m.token);
            mem.token = Some(m.clone());
            return Ok(auth);
        }
        match self.exchange(&login.refresh) {
            Ok(minted) => {
                login.access = Some(minted.clone());
                self.store
                    .save(&login)
                    .map_err(CredentialError::Unavailable)?;
                let auth = self.auth_for(&minted.token);
                mem.token = Some(minted);
                Ok(auth)
            }
            Err(CredentialError::Revoked(m)) => {
                mem.revoked = Some(login.refresh.clone());
                Err(CredentialError::Revoked(m))
            }
            Err(e) => Err(e),
        }
    }

    fn rejected(&self, auth: &RequestAuth) {
        let mut mem = self.mem.lock().unwrap_or_else(|p| p.into_inner());
        if mem.token.as_ref().is_some_and(|m| m.token == auth.bearer) {
            mem.token = None;
        }
        // Drop the shared copy too, but only if it is the refused one: a
        // process that already minted a replacement keeps it.
        let Ok(_file) = self.store.lock(ID, &self.profile) else {
            return;
        };
        if let Ok(Some(mut login)) = self.store.load(ID, &self.profile)
            && login
                .access
                .as_ref()
                .is_some_and(|m| m.token == auth.bearer)
        {
            login.access = None;
            let _ = self.store.save(&login);
        }
    }
}
