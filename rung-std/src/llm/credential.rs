//! A credential that is minted, not configured: the seam between a call and
//! whatever holds a login.
//!
//! A static key rides in [`LlmConfig::api_key`](super::LlmConfig). Some
//! providers instead hand out a short-lived bearer in exchange for a
//! long-lived login, and the route (base URL, extra headers) can come with
//! it. A [`CredentialSource`] is that exchange as an object: the call asks it
//! for the [`RequestAuth`] of one request, and tells it when the server
//! refused the bearer. Everything else in the call keeps seeing "an
//! OpenAI-compatible route plus a bearer".
//!
//! The kernel owns only this seam. How a login is made, where it is kept and
//! how it is refreshed belong to the source (the agent crate's `oauth` module).

use std::fmt;

use super::error::RawCallError;

/// What one request carries for authentication.
#[derive(Clone)]
pub struct RequestAuth {
    /// The bearer token. Never shown by `Debug`.
    pub bearer: String,
    /// The route's base URL when the credential decides it; `None` keeps
    /// [`LlmConfig::base_url`](super::LlmConfig).
    pub base_url: Option<String>,
    /// Extra request headers the provider expects with this credential.
    pub headers: Vec<(String, String)>,
}

impl fmt::Debug for RequestAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestAuth")
            .field("bearer", &"[redacted]")
            .field("base_url", &self.base_url)
            .field(
                "headers",
                &self.headers.iter().map(|(k, _)| k).collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Why a source could not give a bearer.
#[derive(Debug, Clone, PartialEq)]
pub enum CredentialError {
    /// There is no login to use. Carries the command that makes one.
    Missing(String),
    /// The provider rejected the stored login: it was revoked or expired.
    /// Retrying changes nothing until someone logs in again.
    Revoked(String),
    /// Throttled while minting; the wait the provider asked for, if any.
    RateLimited {
        retry_after_ms: Option<u64>,
        message: String,
    },
    /// Could not reach the provider, or it failed. Transient.
    Unavailable(String),
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(m) | Self::Revoked(m) | Self::Unavailable(m) => f.write_str(m),
            Self::RateLimited { message, .. } => f.write_str(message),
        }
    }
}

impl From<CredentialError> for RawCallError {
    /// A missing or revoked login is an auth failure, which no retry cures
    /// (a host reads it as `blocked`, not as load). A throttle is a rate
    /// limit, so the call's own backoff waits it out. Anything else is
    /// transport, retried before any output is seen.
    fn from(e: CredentialError) -> Self {
        match e {
            CredentialError::Missing(message) | CredentialError::Revoked(message) => {
                RawCallError::Auth {
                    message,
                    context: None,
                }
            }
            CredentialError::RateLimited { retry_after_ms, .. } => RawCallError::RateLimit {
                retry_after_ms,
                context: None,
            },
            CredentialError::Unavailable(message) => RawCallError::Transport {
                message,
                observed: false,
            },
        }
    }
}

/// A minted credential. `Send + Sync`: one source is shared by every call of
/// a process, and by many processes through its own storage.
pub trait CredentialSource: Send + Sync + fmt::Debug {
    /// The auth for one request. Returns a cached bearer while it is good and
    /// mints a new one when it is not; callers racing for a new bearer cause
    /// one mint, not many.
    fn authorize(&self) -> Result<RequestAuth, CredentialError>;

    /// The server refused `auth` (a 401). The source drops that bearer, so
    /// the next [`authorize`](Self::authorize) mints another.
    fn rejected(&self, auth: &RequestAuth);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_or_revoked_login_is_auth_a_throttle_is_a_rate_limit_the_rest_transport() {
        let auth: RawCallError = CredentialError::Revoked("no".into()).into();
        assert!(matches!(auth, RawCallError::Auth { .. }) && !auth.is_retryable());
        let auth: RawCallError = CredentialError::Missing("no".into()).into();
        assert!(matches!(auth, RawCallError::Auth { .. }));
        let rate: RawCallError = CredentialError::RateLimited {
            retry_after_ms: Some(5),
            message: "slow".into(),
        }
        .into();
        assert!(rate.is_rate_limited() && rate.is_retryable());
        let net: RawCallError = CredentialError::Unavailable("down".into()).into();
        assert!(net.is_retryable());
    }

    #[test]
    fn a_request_auth_never_shows_its_bearer() {
        let a = RequestAuth {
            bearer: "tid=secret-bearer".into(),
            base_url: None,
            headers: vec![("X-Name".into(), "secret-header-value".into())],
        };
        let shown = format!("{a:?}");
        assert!(!shown.contains("secret-bearer") && !shown.contains("secret-header-value"));
        assert!(shown.contains("X-Name"));
    }
}
