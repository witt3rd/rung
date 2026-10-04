//! The governor: world-imposed waits, never rest.
//!
//! - **Quota pacer.** A daily request quota (`rpd`) and a per-minute limit
//!   (`rpm`). A share (`reserve`, 25%) is held for Responding turns; the
//!   rest is released linearly over the UTC day, so a loop that never rests
//!   spreads its requests instead of spending them by noon. A turn starts
//!   only when the quota holds its expected calls. Waits are
//!   `degraded: paced` (or `quota` once the day is spent), interruptible by
//!   stop and by an owner item the reserve can serve.
//! - **Backoff and the model ladder.** A provider failure (429, 5xx,
//!   transport, timeout) waits max(`Retry-After`, a jittered exponential),
//!   capped at 15 minutes, and steps down the ladder; the rung it left cools
//!   down (2 min, doubling to 30 min) and the first boundary after that
//!   probes back up. A platform 429 waits for its reset and never steps
//!   down: switching models does not refill an account quota. An auth
//!   failure is `blocked`: probe every 15 minutes, one owner message per
//!   incident, never exit.
//! - **Spend cap.** Only for paid providers; reaching it halts.
//!
//! The governor's state is a projection of the record; its decisions are
//! pure functions of that state and the time.

use std::collections::{BTreeMap, VecDeque};

use rung_agent_core::engine::ProviderClass;
use serde::Serialize;

use crate::clock::{DAY, MINUTE, Millis, SECOND, day, next_midnight};
use crate::engine::{HostFailure, Origin};
use crate::kernel::TurnKind;
use crate::record::Line;

#[derive(Debug, Clone, Serialize)]
pub struct Quota {
    pub rpd: u64,
    pub rpm: u64,
    /// The share of `rpd` held for Responding turns.
    pub reserve: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GovConfig {
    /// `None`: no request quota (a local model).
    pub quota: Option<Quota>,
    /// Model calls a turn may make (bounds a turn's overdraw).
    pub step_cap: u32,
    pub backoff_base_ms: Millis,
    pub backoff_cap_ms: Millis,
    pub blocked_probe_ms: Millis,
    pub cooldown_base_ms: Millis,
    pub cooldown_cap_ms: Millis,
    /// A bug guard: turns per minute at most.
    pub turns_per_minute: u32,
    /// For paid providers: halt when a day's spend reaches this.
    pub spend_cap_usd_day: Option<f64>,
}

impl Default for GovConfig {
    fn default() -> Self {
        Self {
            quota: None,
            step_cap: 6,
            backoff_base_ms: 2 * SECOND,
            backoff_cap_ms: 15 * MINUTE,
            blocked_probe_ms: 15 * MINUTE,
            cooldown_base_ms: 2 * MINUTE,
            cooldown_cap_ms: 30 * MINUTE,
            turns_per_minute: 600,
            spend_cap_usd_day: None,
        }
    }
}

/// A wait the world imposes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Wait {
    /// `paced`, `quota`, `backoff`, `blocked`, `rate_ceiling`.
    pub class: String,
    pub until: Millis,
    pub why: String,
    /// An owner item may cut this wait short (the reserve can serve it).
    pub owner_wakes: bool,
}

/// What follows a failed turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub wait: Option<Wait>,
    /// Step the ladder down: (from, to).
    pub step_down: Option<(usize, usize)>,
    /// The first failure of a blocked incident: tell the owner once.
    pub incident_start: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct GovState {
    pub day: i64,
    pub requests_today: u64,
    /// Requests today outside Responding turns.
    pub unreserved_today: u64,
    /// Request times within the last minute.
    pub recent: VecDeque<Millis>,
    /// Turn start times within the last minute.
    pub turn_starts: VecDeque<Millis>,
    /// Calls of the last ten finished turns.
    pub calls_per_turn: VecDeque<u32>,
    /// The current wait, if any.
    pub degraded: Option<Wait>,
    /// The ladder rung in use.
    pub rung: usize,
    /// A rung left after failures: (cools down until, cooldown it had).
    pub cooldown: BTreeMap<usize, (Millis, Millis)>,
    /// Consecutive failed turns.
    pub failures: u32,
    /// A blocked incident is open (cleared by the next good turn).
    pub blocked: bool,
    pub paid_spent_today: f64,
}

impl GovState {
    pub fn apply(&mut self, l: &Line) {
        let d = day(l.at);
        if d != self.day {
            self.day = d;
            self.requests_today = 0;
            self.unreserved_today = 0;
            self.paid_spent_today = 0.0;
        }
        while self.recent.front().is_some_and(|t| *t <= l.at - MINUTE) {
            self.recent.pop_front();
        }
        while self
            .turn_starts
            .front()
            .is_some_and(|t| *t <= l.at - MINUTE)
        {
            self.turn_starts.pop_front();
        }
        match l.kind.as_str() {
            "llm.call" => {
                self.requests_today += 1;
                if l.str("turn_kind") != "responding" {
                    self.unreserved_today += 1;
                }
                self.recent.push_back(l.at);
                self.paid_spent_today += l.f64("cost_usd");
            }
            "turn.started" => self.turn_starts.push_back(l.at),
            "turn.ended" => {
                self.calls_per_turn.push_back(l.u64("calls") as u32);
                while self.calls_per_turn.len() > 10 {
                    self.calls_per_turn.pop_front();
                }
                if l.get("failure").is_null() {
                    self.failures = 0;
                    if l.str("status") == "completed" {
                        self.blocked = false;
                    }
                } else {
                    self.failures += 1;
                }
            }
            "degraded" => {
                self.degraded = Some(Wait {
                    class: l.str("class").into(),
                    until: l.i64("until"),
                    why: l.str("why").into(),
                    owner_wakes: l.get("owner_wakes").as_bool().unwrap_or(false),
                });
                if l.str("class") == "blocked" {
                    self.blocked = true;
                }
            }
            "degraded.ended" => self.degraded = None,
            "model.switch" => {
                let from = l.u64("rung_from") as usize;
                let to = l.u64("rung_to") as usize;
                if l.str("direction") == "down" {
                    let prev = self.cooldown.get(&from).map(|c| c.1).unwrap_or(0);
                    let ms = l.i64("cooldown_ms").max(prev);
                    self.cooldown.insert(from, (l.at + ms, ms));
                }
                self.rung = to;
            }
            _ => {}
        }
    }

    /// Calls a turn is expected to make: the recent mean, rounded up.
    pub fn expected_calls(&self, cfg: &GovConfig) -> u64 {
        if self.calls_per_turn.is_empty() {
            return 1;
        }
        let sum: u64 = self.calls_per_turn.iter().map(|c| u64::from(*c)).sum();
        sum.div_ceil(self.calls_per_turn.len() as u64)
            .clamp(1, u64::from(cfg.step_cap))
    }
}

/// Must the next turn wait? `kind` is the turn about to run.
pub fn must_wait(st: &GovState, cfg: &GovConfig, kind: TurnKind, now: Millis) -> Option<Wait> {
    if let Some(w) = &st.degraded
        && w.until > now
    {
        return Some(w.clone());
    }
    // Only starts still inside the minute count (the state prunes on
    // apply, which a long idle does not trigger).
    let starts = st.turn_starts.iter().filter(|t| **t > now - MINUTE);
    if starts.clone().count() as u32 >= cfg.turns_per_minute {
        let oldest = starts.clone().next().copied().unwrap_or(now);
        return Some(Wait {
            class: "rate_ceiling".into(),
            until: oldest + MINUTE,
            why: format!("{} turns in a minute", starts.count()),
            owner_wakes: false,
        });
    }
    let q = cfg.quota.as_ref()?;
    let need = st.expected_calls(cfg);
    let today = if day(now) == st.day {
        st.requests_today
    } else {
        0
    };
    let unreserved = if day(now) == st.day {
        st.unreserved_today
    } else {
        0
    };
    if today + need > q.rpd {
        return Some(Wait {
            class: "quota".into(),
            until: next_midnight(now),
            why: format!("daily quota spent ({today}/{})", q.rpd),
            owner_wakes: false,
        });
    }
    let recent = st.recent.iter().filter(|t| **t > now - MINUTE).count() as u64;
    if recent + need > q.rpm {
        let k = (recent + need - q.rpm) as usize;
        let free_at = st
            .recent
            .iter()
            .filter(|t| **t > now - MINUTE)
            .nth(k.saturating_sub(1))
            .map(|t| t + MINUTE)
            .unwrap_or(now + MINUTE);
        return Some(Wait {
            class: "paced".into(),
            until: free_at.max(now + 1),
            why: format!("{recent} requests in the last minute (limit {})", q.rpm),
            owner_wakes: kind != TurnKind::Responding,
        });
    }
    if kind == TurnKind::Responding {
        return None;
    }
    // The unreserved share is released linearly over the day, with one
    // turn's worth of burst.
    let share = (1.0 - q.reserve) * q.rpd as f64;
    let start = day(now) * DAY;
    let allowance = |t: Millis| share * (t - start) as f64 / DAY as f64 + cfg.step_cap as f64;
    if (unreserved + need) as f64 > allowance(now) {
        let target = (unreserved + need) as f64 - cfg.step_cap as f64;
        let at = start + (target / share * DAY as f64).ceil() as Millis;
        let until = at.clamp(now + 1, next_midnight(now));
        return Some(Wait {
            class: "paced".into(),
            until,
            why: format!(
                "pacing the free quota: {unreserved} of {:.0} unreserved requests used",
                share
            ),
            owner_wakes: true,
        });
    }
    None
}

/// What to do after a failed turn on `rungs` rungs, at `now`. `jitter` is
/// in [0, 1).
pub fn on_failure(
    st: &GovState,
    cfg: &GovConfig,
    f: &HostFailure,
    rungs: usize,
    now: Millis,
    jitter: f64,
) -> Plan {
    let class = f.failure.class;
    let retry = f.failure.retry_after_ms.map(|m| m as Millis).unwrap_or(0);
    let backoff = || {
        let exp = cfg.backoff_base_ms.saturating_mul(1 << st.failures.min(20)) as f64;
        let jittered = (exp * (0.5 + 0.5 * jitter)) as Millis;
        retry.max(jittered).min(cfg.backoff_cap_ms)
    };
    let down = || (st.rung + 1 < rungs).then_some((st.rung, st.rung + 1));
    match (f.origin, class) {
        (Origin::Platform, _) | (_, ProviderClass::Quota) => {
            // A reset already past (the platform still refuses) backs off
            // like any repeated failure rather than retrying at once.
            let until = match f.reset_at {
                Some(r) if r > now => r,
                Some(_) => now + backoff(),
                None if retry > 0 => now + retry,
                None => next_midnight(now),
            };
            Plan {
                wait: Some(Wait {
                    class: "quota".into(),
                    until: until.max(now + 1),
                    why: format!("{} quota: wait for its reset", f.class_name()),
                    owner_wakes: false,
                }),
                step_down: None,
                incident_start: false,
            }
        }
        (
            Origin::Provider,
            ProviderClass::RateLimit
            | ProviderClass::Overloaded
            | ProviderClass::Transport
            | ProviderClass::Timeout,
        ) => Plan {
            wait: Some(Wait {
                class: "backoff".into(),
                until: now + backoff(),
                why: format!("provider {}", f.class_name()),
                owner_wakes: false,
            }),
            step_down: down(),
            incident_start: false,
        },
        (Origin::Provider, ProviderClass::Auth | ProviderClass::Config) => Plan {
            wait: Some(Wait {
                class: "blocked".into(),
                until: now + cfg.blocked_probe_ms,
                why: format!("provider {}: probing every 15 minutes", f.class_name()),
                owner_wakes: false,
            }),
            step_down: None,
            incident_start: !st.blocked,
        },
        // Refusals, invalid requests, unusable output, overflow: the turn
        // failed for this content; the next boundary goes on.
        _ => Plan {
            wait: Some(Wait {
                class: "backoff".into(),
                until: now + cfg.backoff_base_ms,
                why: format!("provider {}", f.class_name()),
                owner_wakes: false,
            }),
            step_down: None,
            incident_start: false,
        },
    }
}

/// The rung to probe back up to, when the one above has cooled down.
pub fn probe_up(st: &GovState, now: Millis) -> Option<usize> {
    let up = st.rung.checked_sub(1)?;
    match st.cooldown.get(&up) {
        Some((until, _)) if *until > now => None,
        _ => Some(up),
    }
}

/// The cooldown a rung gets when the ladder steps down from it.
pub fn cooldown_for(st: &GovState, cfg: &GovConfig, rung: usize) -> Millis {
    match st.cooldown.get(&rung) {
        Some((_, ms)) => (ms * 2).min(cfg.cooldown_cap_ms),
        None => cfg.cooldown_base_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rung_agent_core::engine::ProviderFailure;

    fn quota() -> GovConfig {
        GovConfig {
            quota: Some(Quota {
                rpd: 1_000,
                rpm: 20,
                reserve: 0.25,
            }),
            ..GovConfig::default()
        }
    }

    #[test]
    fn the_unreserved_share_is_paced_over_the_day() {
        let cfg = quota();
        let mut st = GovState {
            day: 20_000,
            ..GovState::default()
        };
        let noon = 20_000 * DAY + DAY / 2;
        // Half the day: 375 of 750 unreserved requests released (+ a turn's burst).
        st.unreserved_today = 400;
        st.requests_today = 400;
        let w = must_wait(&st, &cfg, TurnKind::Free, noon).expect("paced");
        assert_eq!(w.class, "paced");
        assert!(w.owner_wakes);
        // A Responding turn draws on the reserve.
        assert!(must_wait(&st, &cfg, TurnKind::Responding, noon).is_none());
        st.unreserved_today = 300;
        assert!(must_wait(&st, &cfg, TurnKind::Free, noon).is_none());
    }

    #[test]
    fn a_provider_429_steps_down_and_a_platform_429_waits() {
        let cfg = GovConfig::default();
        let st = GovState::default();
        let f = |origin, retry| HostFailure {
            failure: ProviderFailure {
                class: ProviderClass::RateLimit,
                retry_after_ms: retry,
            },
            origin,
            reset_at: Some(5_000_000),
        };
        let p = on_failure(&st, &cfg, &f(Origin::Provider, Some(30_000)), 3, 1_000, 0.0);
        assert_eq!(p.step_down, Some((0, 1)));
        assert_eq!(p.wait.unwrap().until, 31_000);
        let p = on_failure(&st, &cfg, &f(Origin::Platform, None), 3, 1_000, 0.0);
        assert_eq!(p.step_down, None);
        assert_eq!(p.wait.unwrap().until, 5_000_000);
    }

    #[test]
    fn the_turn_rate_ceiling_ignores_starts_older_than_a_minute() {
        let cfg = GovConfig {
            turns_per_minute: 2,
            ..GovConfig::default()
        };
        let mut st = GovState::default();
        st.turn_starts.extend([1_000, 2_000]);
        assert!(must_wait(&st, &cfg, TurnKind::Free, 3_000).is_some());
        // No line has been applied since; the starts are stale at 70 s.
        assert!(must_wait(&st, &cfg, TurnKind::Free, 70_000).is_none());
    }

    #[test]
    fn backoff_is_capped() {
        let cfg = GovConfig::default();
        let st = GovState {
            failures: 30,
            ..GovState::default()
        };
        let f = HostFailure {
            failure: ProviderFailure {
                class: ProviderClass::Overloaded,
                retry_after_ms: Some(10 * 3_600_000),
            },
            origin: Origin::Provider,
            reset_at: None,
        };
        let p = on_failure(&st, &cfg, &f, 1, 0, 0.9);
        assert_eq!(p.wait.unwrap().until, cfg.backoff_cap_ms);
        assert_eq!(p.step_down, None, "no rung below the last");
    }

    #[test]
    fn stale_turn_starts_do_not_trip_the_rate_ceiling() {
        let cfg = GovConfig {
            turns_per_minute: 2,
            ..GovConfig::default()
        };
        let mut st = GovState::default();
        st.turn_starts.extend([1_000, 2_000]);
        let now = 10 * MINUTE;
        assert!(must_wait(&st, &cfg, TurnKind::Free, now).is_none());
        let w = must_wait(&st, &cfg, TurnKind::Free, 2_500).expect("fresh starts count");
        assert_eq!(w.class, "rate_ceiling");
        assert_eq!(w.until, 1_000 + MINUTE);
    }
}
