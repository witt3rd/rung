//! The fault injector: what a provider and a routing platform do to a host
//! on a bad day. Each fault is a window of virtual time, optionally limited
//! to one model; a call inside a window fails with it.

use rung_agent_core::engine::{ProviderClass, ProviderFailure};

use crate::clock::{Millis, next_midnight};
use crate::engine::{HostFailure, Origin};

#[derive(Debug, Clone, PartialEq)]
pub enum FaultKind {
    /// The model's provider: 429 with `Retry-After`.
    ProviderRateLimit { retry_after_ms: u64 },
    /// The platform: 429 with `X-RateLimit-Reset` this far ahead.
    PlatformRateLimit { reset_in_ms: Millis },
    /// 5xx.
    ServerError,
    /// Nothing answers.
    Outage,
    /// The credential is refused.
    Auth,
    /// The account's daily request quota is spent (resets at UTC midnight).
    DailyQuota,
    /// The provider drops its prompt cache (the next call is cold).
    CacheEvict,
}

#[derive(Debug, Clone)]
pub struct Fault {
    pub from: Millis,
    pub to: Millis,
    pub kind: FaultKind,
    /// Only calls to this model fail; `None` for every model.
    pub model: Option<String>,
}

#[derive(Debug, Default)]
pub struct FaultInjector {
    pub faults: Vec<Fault>,
    /// Faults fired, by kind label.
    pub fired: Vec<(Millis, String)>,
}

impl FaultInjector {
    pub fn new(faults: Vec<Fault>) -> Self {
        Self {
            faults,
            fired: Vec::new(),
        }
    }

    /// The fault a call to `model` at `now` meets, if any. A cache eviction
    /// fires once and does not fail the call.
    pub fn on_call(&mut self, now: Millis, model: &str) -> (Option<HostFailure>, bool) {
        let mut evict = false;
        let mut failure = None;
        for f in self.faults.iter_mut() {
            if now < f.from || now >= f.to || f.model.as_deref().is_some_and(|m| m != model) {
                continue;
            }
            let fail = |class, retry_after_ms, origin, reset_at| HostFailure {
                failure: ProviderFailure {
                    class,
                    retry_after_ms,
                },
                origin,
                reset_at,
            };
            let hf = match &f.kind {
                FaultKind::CacheEvict => {
                    evict = true;
                    // Once.
                    f.to = now;
                    continue;
                }
                FaultKind::ProviderRateLimit { retry_after_ms } => {
                    fail(ProviderClass::RateLimit, Some(*retry_after_ms), Origin::Provider, None)
                }
                FaultKind::PlatformRateLimit { reset_in_ms } => fail(
                    ProviderClass::RateLimit,
                    None,
                    Origin::Platform,
                    Some(f.from + reset_in_ms),
                ),
                FaultKind::ServerError => fail(ProviderClass::Overloaded, None, Origin::Provider, None),
                FaultKind::Outage => fail(ProviderClass::Transport, None, Origin::Provider, None),
                FaultKind::Auth => fail(ProviderClass::Auth, None, Origin::Provider, None),
                FaultKind::DailyQuota => fail(
                    ProviderClass::RateLimit,
                    None,
                    Origin::Platform,
                    Some(next_midnight(now)),
                ),
            };
            if failure.is_none() {
                self.fired.push((now, format!("{:?}", f.kind)));
                failure = Some(hf);
            }
        }
        (failure, evict)
    }
}
