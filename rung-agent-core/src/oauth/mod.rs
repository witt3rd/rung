//! OAuth credentials for the agent: a login made once, a bearer minted on
//! demand.
//!
//! `llm.auth.oauth: {provider, profile}` in `config.yaml` (or
//! `RUNG_OAUTH_PROVIDER` / `RUNG_OAUTH_PROFILE`) replaces `api_key_env`. A
//! small registry ([`provider`]) maps the provider name to the code that
//! owns its login flow, its token exchange and its base-URL rule; the turn
//! engine sees only a [`rung_std::llm::CredentialSource`], that is "an
//! OpenAI-compatible route plus a bearer".
//!
//! - [`store`]: where a login lives (machine-local, mode 0600).
//! - [`copilot`]: GitHub Copilot, the first provider (device flow).
//! - [`cli`]: `rung-agent auth login | status | models | logout`.
//!
//! The contract is `docs/reference/rung-agent-oauth.md`.

pub mod cli;
pub mod copilot;
pub mod store;

use std::sync::Arc;
use std::time::Duration;

use rung_std::llm::CredentialSource;

pub use store::{DEFAULT_PROFILE, Login, Minted, Store};

/// Time as the oauth code sees it, so a test can move it without waiting.
pub trait Clock: Send + Sync {
    /// Seconds since the Unix epoch.
    fn now_secs(&self) -> u64;
    fn sleep(&self, d: Duration);
}

/// The wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// What the user must do to finish a device-code login.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceAnnouncement {
    pub user_code: String,
    pub verification_uri: String,
    /// Seconds until the code stops working.
    pub expires_in: u64,
}

/// Why a login did not complete. Messages never hold a token.
#[derive(Debug, Clone, PartialEq)]
pub enum LoginError {
    /// The code expired before it was approved.
    Expired,
    /// The user (or the account's policy) refused it.
    Denied,
    /// The provider or the network failed.
    Failed(String),
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Expired => {
                f.write_str("the login code expired before it was approved; run login again")
            }
            Self::Denied => f.write_str("the login was denied"),
            Self::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for LoginError {}

/// One model an account may use.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    pub id: String,
    /// Served over the OpenAI chat-completions style, the one the engine
    /// speaks. Others (responses, Anthropic messages) are out of v1.
    pub chat_completions: bool,
}

/// A provider of OAuth logins. The registry's unit.
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;

    /// Run the login, calling `announce` once with what the user must do,
    /// and store the result under `profile`.
    fn login(
        &self,
        store: &Store,
        profile: &str,
        clock: &dyn Clock,
        announce: &mut dyn FnMut(&DeviceAnnouncement),
    ) -> Result<(), LoginError>;

    /// The credential for a stored login. Touches no network until used.
    fn credential(
        &self,
        store: Store,
        profile: &str,
        clock: Arc<dyn Clock>,
    ) -> Arc<dyn CredentialSource>;

    /// The models the login's account may use, from the provider (cached).
    fn models(
        &self,
        store: &Store,
        profile: &str,
        clock: Arc<dyn Clock>,
        refresh: bool,
    ) -> Result<Vec<ModelInfo>, String>;
}

/// The provider names the registry knows.
pub const PROVIDERS: &[&str] = &[copilot::ID];

/// Build a provider from its `oauth.<name>:` section of the config file.
///
/// `explicit_base` is the route the user set (`llm.base_url`); it wins over
/// the route the provider would derive.
pub fn provider(
    name: &str,
    section: Option<&serde_yaml::Value>,
    explicit_base: Option<&str>,
) -> Result<Box<dyn Provider>, String> {
    match name {
        copilot::ID => Ok(Box::new(copilot::Copilot::from_section(
            section,
            explicit_base,
        )?)),
        other => Err(format!(
            "oauth provider '{other}' is not known (known: {})",
            PROVIDERS.join(", ")
        )),
    }
}
