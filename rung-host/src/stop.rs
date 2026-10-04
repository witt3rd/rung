//! The stop authority: the only road to `Halted`.
//!
//! A stop comes from a signal (SIGTERM, SIGINT), a stop file appearing, or
//! an explicit request (the owner's stop, a test). The loop checks it at
//! every boundary; every wait polls it; a running turn's cancel flag is
//! raised from it, so the engine stops before its next model call or tool.
//!
//! Nothing here needs the loop to reach an exit: the process supervisor
//! (systemd or docker) and the watchdog ([`crate::notify`]) sit outside it.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use serde::Serialize;

/// Why the host halted. Never the model's choice.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "why")]
pub enum Why {
    /// A deliberate stop.
    Stopped { by: String },
    /// The owner revoked the host's authority to run.
    Revoked { by: String },
    /// A paid provider's daily spend cap was reached.
    SpendCap { spent_usd: f64, cap_usd: f64 },
}

static SIGNALLED: AtomicI32 = AtomicI32::new(0);

extern "C" fn on_signal(sig: libc::c_int) {
    SIGNALLED.store(sig, Ordering::SeqCst);
}

/// Route SIGTERM and SIGINT to the stop authority. The handler only stores
/// the signal number.
pub fn install_signals() {
    // SAFETY: `on_signal` is async-signal-safe: one atomic store.
    unsafe {
        libc::signal(
            libc::SIGTERM,
            on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGINT,
            on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t,
        );
    }
}

#[derive(Debug, Default)]
pub struct StopAuthority {
    raised: AtomicBool,
    why: Mutex<Option<Why>>,
    stop_file: Option<PathBuf>,
    listen_signals: bool,
}

impl StopAuthority {
    pub fn new(stop_file: Option<PathBuf>, listen_signals: bool) -> Self {
        Self {
            stop_file,
            listen_signals,
            ..Self::default()
        }
    }

    /// Ask the host to stop.
    pub fn request(&self, why: Why) {
        let mut w = self.why.lock().expect("stop");
        if w.is_none() {
            *w = Some(why);
        }
        self.raised.store(true, Ordering::SeqCst);
    }

    /// Why the host must halt, if it must.
    pub fn check(&self) -> Option<Why> {
        if !self.raised.load(Ordering::SeqCst) {
            if self.listen_signals {
                let sig = SIGNALLED.load(Ordering::SeqCst);
                if sig != 0 {
                    let name = if sig == libc::SIGINT {
                        "SIGINT"
                    } else {
                        "SIGTERM"
                    };
                    self.request(Why::Stopped {
                        by: format!("signal:{name}"),
                    });
                }
            }
            if let Some(f) = &self.stop_file
                && f.exists()
            {
                self.request(Why::Stopped {
                    by: format!("stop-file:{}", f.display()),
                });
            }
        }
        if self.raised.load(Ordering::SeqCst) {
            self.why.lock().expect("stop").clone()
        } else {
            None
        }
    }

    pub fn raised(&self) -> bool {
        self.check().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_reason_wins() {
        let s = StopAuthority::default();
        assert_eq!(s.check(), None);
        s.request(Why::Stopped { by: "test".into() });
        s.request(Why::Revoked { by: "owner".into() });
        assert_eq!(s.check(), Some(Why::Stopped { by: "test".into() }));
    }

    #[test]
    fn a_stop_file_stops() {
        let guard = crate::sim::temp_dir_guard("stop-file");
        let f = guard.path().join("stop");
        let s = StopAuthority::new(Some(f.clone()), false);
        assert!(!s.raised());
        std::fs::write(&f, "").unwrap();
        assert!(matches!(s.check(), Some(Why::Stopped { .. })));
    }
}
