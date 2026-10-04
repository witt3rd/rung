//! Time: the host's clock, real or simulated.
//!
//! Every time the host records is milliseconds since the Unix epoch, UTC.
//! [`RealClock`] reads the system clock and sleeps. [`SimClock`] is a
//! virtual clock: it moves only when the engine charges simulated work
//! ([`Clock::advance`]) or a wait jumps to its deadline or to the next
//! external event. A seeded run on a `SimClock` is deterministic, and 30
//! simulated minutes take milliseconds.
//!
//! The host's own processing costs no virtual time on a `SimClock`; the
//! host measures it with the wall clock and records it in `wall_us`
//! fields, which a determinism check ignores (gate G-a reads them).

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch, UTC.
pub type Millis = i64;

pub const SECOND: Millis = 1_000;
pub const MINUTE: Millis = 60 * SECOND;
pub const HOUR: Millis = 60 * MINUTE;
pub const DAY: Millis = 24 * HOUR;

/// Why a wait returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Woke {
    /// The deadline passed.
    Elapsed,
    /// The wake test held (stop, an item the wait serves, ...).
    Woken,
}

pub trait Clock: Send + Sync + std::fmt::Debug {
    fn now(&self) -> Millis;

    /// Spend `ms` of work: a real clock sleeps, a sim clock moves.
    fn advance(&self, ms: Millis);

    /// Wait until `until`, returning early when `wake(now)` holds. `next`
    /// is the time of the next scheduled external event, if any is known
    /// (a sim clock jumps there; a real clock polls). `wake` is called at
    /// least every [`POLL`] on a real clock, and once per jump on a sim one.
    fn wait(
        &self,
        until: Millis,
        next: &dyn Fn() -> Option<Millis>,
        wake: &mut dyn FnMut(Millis) -> bool,
    ) -> Woke;

    /// A simulated clock (virtual time).
    fn is_sim(&self) -> bool;
}

/// How often a real wait looks at its wake test.
pub const POLL: Duration = Duration::from_millis(20);

#[derive(Debug, Default)]
pub struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> Millis {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as Millis)
            .unwrap_or(0)
    }

    fn advance(&self, ms: Millis) {
        if ms > 0 {
            std::thread::sleep(Duration::from_millis(ms as u64));
        }
    }

    fn wait(
        &self,
        until: Millis,
        _next: &dyn Fn() -> Option<Millis>,
        wake: &mut dyn FnMut(Millis) -> bool,
    ) -> Woke {
        loop {
            let now = self.now();
            if wake(now) {
                return Woke::Woken;
            }
            if now >= until {
                return Woke::Elapsed;
            }
            let left = Duration::from_millis((until - now) as u64);
            std::thread::sleep(left.min(POLL));
        }
    }

    fn is_sim(&self) -> bool {
        false
    }
}

/// A virtual clock. See the module docs.
#[derive(Debug)]
pub struct SimClock {
    now: Mutex<Millis>,
}

impl SimClock {
    pub fn new(start: Millis) -> Self {
        Self {
            now: Mutex::new(start),
        }
    }

    fn set(&self, t: Millis) {
        let mut now = self.now.lock().expect("clock");
        if t > *now {
            *now = t;
        }
    }
}

impl Clock for SimClock {
    fn now(&self) -> Millis {
        *self.now.lock().expect("clock")
    }

    fn advance(&self, ms: Millis) {
        if ms > 0 {
            *self.now.lock().expect("clock") += ms;
        }
    }

    fn wait(
        &self,
        until: Millis,
        next: &dyn Fn() -> Option<Millis>,
        wake: &mut dyn FnMut(Millis) -> bool,
    ) -> Woke {
        loop {
            let now = self.now();
            if wake(now) {
                return Woke::Woken;
            }
            if now >= until {
                return Woke::Elapsed;
            }
            let to = match next() {
                Some(t) if t > now && t < until => t,
                _ => until,
            };
            self.set(to);
        }
    }

    fn is_sim(&self) -> bool {
        true
    }
}

/// The UTC day number of `t` (days since 1970-01-01).
pub fn day(t: Millis) -> i64 {
    t.div_euclid(DAY)
}

/// The start of the UTC day after the one holding `t`.
pub fn next_midnight(t: Millis) -> Millis {
    (day(t) + 1) * DAY
}

/// `YYYY-MM-DDTHH:MM:SSZ`.
pub fn iso(t: Millis) -> String {
    rung_std::time::iso_utc(t.div_euclid(1000))
}

/// A short human span: `3h12m`, `45s`, `2d4h`.
pub fn span(ms: Millis) -> String {
    let s = ms.max(0) / 1000;
    match s {
        s if s < 60 => format!("{s}s"),
        s if s < 3_600 => format!("{}m{:02}s", s / 60, s % 60),
        s if s < 86_400 => format!("{}h{:02}m", s / 3_600, s % 3_600 / 60),
        s => format!("{}d{}h", s / 86_400, s % 86_400 / 3_600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_and_span() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(1_790_000_000_000), "2026-09-21T14:13:20Z");
        assert_eq!(span(11_520_000), "3h12m");
        assert_eq!(span(45_000), "45s");
    }

    #[test]
    fn a_sim_wait_jumps_to_the_next_event() {
        let c = SimClock::new(1_000);
        let mut seen = Vec::new();
        let woke = c.wait(10_000, &|| Some(4_000), &mut |now| {
            seen.push(now);
            now >= 4_000
        });
        assert_eq!(woke, Woke::Woken);
        assert_eq!(c.now(), 4_000);
        assert_eq!(seen, vec![1_000, 4_000]);
        assert_eq!(c.wait(9_000, &|| None, &mut |_| false), Woke::Elapsed);
        assert_eq!(c.now(), 9_000);
    }
}
