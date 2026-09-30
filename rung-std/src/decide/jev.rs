//! Jev over the System One API: `POST {base_url}/systemone`.
//!
//! The body is `{model, state, questions}`. The same body works at TypeSafe
//! directly and through OpenRouter, so only the base URL names the provider.

use std::time::Duration;

use serde_json::Value;

use super::{Ask, Decided, Decider, Undecided, read_answers};

/// OpenRouter's API root. The key is the existing `OPENROUTER_API_KEY`.
pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
/// Pinned: thresholds tuned against one version do not carry to the next.
pub const DEFAULT_MODEL: &str = "typesafe/jev-1.13";

/// Jev's context is 32,000 tokens. Refuse before sending, never cut.
const MAX_ESTIMATED_TOKENS: usize = 32_000;
const ATTEMPTS: u32 = 3;

pub struct JevDecider {
    pub base_url: String,
    api_key: String,
    pub model: String,
    pub timeout: Duration,
}

impl std::fmt::Debug for JevDecider {
    // The key is never printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JevDecider")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("has_key", &!self.api_key.is_empty())
            .finish()
    }
}

impl JevDecider {
    pub fn new(base_url: &str, api_key: &str, model: &str, timeout: Duration) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.trim().to_string(),
            model: model.to_string(),
            timeout,
        }
    }

    /// Send a raw body and return the raw response JSON. [`Recorded`] uses
    /// this to capture an exchange verbatim.
    ///
    /// [`Recorded`]: super::Recorded
    pub fn post(&self, body: &Value) -> Result<Value, Undecided> {
        if self.api_key.is_empty() {
            return Err(Undecided::Unauthorized("no key".into()));
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| Undecided::Unreachable(format!("client: {e}")))?;
        let url = format!("{}/systemone", self.base_url);
        let mut last = Undecided::Unreachable("no attempt made".into());
        for attempt in 0..ATTEMPTS {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(250 << attempt));
            }
            let sent = client
                .post(&url)
                .bearer_auth(&self.api_key)
                .json(body)
                .send();
            let resp = match sent {
                Ok(r) => r,
                Err(e) => {
                    // reqwest's error names the URL, never the headers.
                    last = Undecided::Unreachable(e.to_string());
                    continue;
                }
            };
            let status = resp.status().as_u16();
            let text = resp.text().unwrap_or_default();
            let snippet: String = text.chars().take(300).collect();
            match status {
                200..=299 => {
                    return serde_json::from_str(&text)
                        .map_err(|e| Undecided::Malformed(format!("not JSON: {e}")));
                }
                429 => last = Undecided::RateLimited,
                529 => last = Undecided::Overloaded,
                500 | 502 | 503 | 524 => {
                    last = Undecided::Unreachable(format!("HTTP {status}: {snippet}"))
                }
                400 => return Err(Undecided::Invalid(snippet)),
                413 => return Err(Undecided::TooLarge),
                401 | 403 => return Err(Undecided::Unauthorized(format!("HTTP {status}"))),
                402 => return Err(Undecided::NoCredit),
                _ => return Err(Undecided::Unreachable(format!("HTTP {status}: {snippet}"))),
            }
        }
        Err(last)
    }
}

impl Decider for JevDecider {
    fn decide(&self, ask: &Ask) -> Result<Decided, Undecided> {
        if ask.estimated_tokens() > MAX_ESTIMATED_TOKENS {
            return Err(Undecided::TooLarge);
        }
        let response = self.post(&ask.body(&self.model))?;
        read_answers(ask, &response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn debug_never_prints_the_key() {
        let d = JevDecider::new("http://x", "sk-secret-value", "m", Duration::from_secs(1));
        let s = format!("{d:?}");
        assert!(!s.contains("sk-secret-value"), "{s}");
        assert!(s.contains("has_key: true"));
    }

    #[test]
    fn no_key_is_unauthorized_without_a_request() {
        // Port 9 (discard) would hang or refuse; with no key nothing is sent.
        let d = JevDecider::new("http://127.0.0.1:9", "", "m", Duration::from_secs(1));
        let ask = Ask {
            state: serde_json::json!({}),
            questions: BTreeMap::new(),
        };
        assert!(matches!(d.decide(&ask), Err(Undecided::Unauthorized(_))));
    }

    #[test]
    fn an_oversized_state_is_refused_before_sending() {
        let d = JevDecider::new("http://127.0.0.1:9", "k", "m", Duration::from_secs(1));
        let ask = Ask {
            state: serde_json::json!({"s": "x".repeat(200_000)}),
            questions: BTreeMap::new(),
        };
        assert_eq!(d.decide(&ask), Err(Undecided::TooLarge));
    }
}
