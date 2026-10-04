//! Replay a recorded exchange; record one when told to.
//!
//! A fixture file holds one exchange:
//!
//! ```json
//! {"request": {"model": "...", "state": {...}, "questions": {...}},
//!  "response": {"model": "typesafe/jev-1.13-20260917", "answers": {...}, "usage": {...}},
//!  "model": "typesafe/jev-1.13-20260917",
//!  "recorded_at": "2026-09-30T04:00:00Z"}
//! ```
//!
//! `RUNG_DECIDE` picks the mode. Unset or `replay`: read the file, check the
//! request is the recorded one, return the recorded response. Nothing goes to
//! the network. `record`: replay a fixture that still matches; send any other
//! request to Jev, write the file, and return the response. `rerecord`: send
//! every request. Recording reads the key from `OPENROUTER_API_KEY` and is
//! meant to run under `doppler run -p fleet -c dev_work -- ...`.
//!
//! Replay fails loudly, by panic, when the file is missing or the request has
//! changed (a reworded question, a different state). Either means the
//! fixture no longer records what the code asks, and the only fix is to
//! record it again. This backend exists for tests; it is not a product path.
//!
//! Recording keeps a spend ledger across processes (`RUNG_DECIDE_LEDGER`,
//! default a file in the temp dir) and stops, by panic, before a request
//! would take the total past `RUNG_DECIDE_BUDGET_USD` (default 0.02).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{Value, json};

use super::jev::{DEFAULT_BASE_URL, DEFAULT_MODEL, JevDecider};
use super::{Ask, Decided, Decider, Undecided, read_answers};

/// Jev's list price: $0.042 per million input tokens, output free.
const USD_PER_INPUT_TOKEN: f64 = 0.042e-6;
const DEFAULT_BUDGET_USD: f64 = 0.02;

static LEDGER: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Replay,
    /// Record what is missing or stale.
    Record,
    /// Record everything again.
    Rerecord,
}

impl Mode {
    /// `RUNG_DECIDE=record` / `rerecord`; anything else replays.
    pub fn from_env() -> Self {
        match std::env::var("RUNG_DECIDE").as_deref() {
            Ok("record") => Mode::Record,
            Ok("rerecord") => Mode::Rerecord,
            _ => Mode::Replay,
        }
    }
}

#[derive(Debug)]
pub struct Recorded {
    path: PathBuf,
    model: String,
    live: Option<JevDecider>,
    /// Record even when the fixture still matches.
    always: bool,
}

impl Recorded {
    /// Replay only. Never touches the network.
    pub fn replay(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            model: DEFAULT_MODEL.into(),
            live: None,
            always: false,
        }
    }

    /// Record through `live` when the fixture is missing or stale.
    pub fn record(path: impl Into<PathBuf>, live: JevDecider) -> Self {
        Self {
            path: path.into(),
            model: live.model.clone(),
            live: Some(live),
            always: false,
        }
    }

    /// The mode `RUNG_DECIDE` names. Recording builds a Jev decider from
    /// `OPENROUTER_API_KEY` (base `RUNG_DECIDE_BASE_URL`, default OpenRouter).
    pub fn from_env(path: impl Into<PathBuf>) -> Self {
        let mode = Mode::from_env();
        if mode == Mode::Replay {
            return Self::replay(path);
        }
        let key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
        let base =
            std::env::var("RUNG_DECIDE_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into());
        let mut r = Self::record(
            path,
            JevDecider::new(&base, &key, DEFAULT_MODEL, Duration::from_secs(30)),
        );
        r.always = mode == Mode::Rerecord;
        r
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The raw exchange: a request body in, the recorded (or live) response out.
    pub fn exchange(&self, request: &Value) -> Result<Value, Undecided> {
        match &self.live {
            None => self.replayed(request),
            Some(live) if self.always || !self.matches(request) => self.recorded(live, request),
            Some(_) => self.replayed(request),
        }
    }

    /// The fixture exists and records exactly this request.
    fn matches(&self, request: &Value) -> bool {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .is_some_and(|f| &f["request"] == request)
    }

    fn replayed(&self, request: &Value) -> Result<Value, Undecided> {
        let name = self.path.display();
        let body = std::fs::read_to_string(&self.path).unwrap_or_else(|e| {
            panic!(
                "fixture {name}: {e}\nrecord it with:\n  RUNG_DECIDE=record doppler run -p fleet -c dev_work -- cargo test ..."
            )
        });
        let fixture: Value =
            serde_json::from_str(&body).unwrap_or_else(|e| panic!("fixture {name}: {e}"));
        if &fixture["request"] != request {
            panic!(
                "fixture {name} records a different request; the code now asks:\n{}\nre-record with:\n  RUNG_DECIDE=record doppler run -p fleet -c dev_work -- cargo test ...",
                serde_json::to_string_pretty(request).unwrap_or_default()
            );
        }
        Ok(fixture["response"].clone())
    }

    fn recorded(&self, live: &JevDecider, request: &Value) -> Result<Value, Undecided> {
        let _guard = LEDGER.lock().unwrap_or_else(|p| p.into_inner());
        let ledger = std::env::var("RUNG_DECIDE_LEDGER")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("rung-decide-spend.json"));
        let budget = std::env::var("RUNG_DECIDE_BUDGET_USD")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(DEFAULT_BUDGET_USD);
        let spent = std::fs::read_to_string(&ledger)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v["spent_usd"].as_f64())
            .unwrap_or(0.0);
        let estimate = (request.to_string().len() / 3) as f64 * USD_PER_INPUT_TOKEN;
        if spent + estimate > budget {
            panic!(
                "budget stop: spent ${spent:.6}, next request ~${estimate:.6}, budget ${budget}"
            );
        }
        let response = live.post(request)?;
        let cost = response["usage"]["cost"].as_f64().unwrap_or(estimate);
        let _ = std::fs::write(&ledger, json!({"spent_usd": spent + cost}).to_string());
        let served = response["model"].as_str().unwrap_or("").to_string();
        let fixture = json!({
            "request": request,
            "response": response,
            "model": served,
            "recorded_at": utc_now(),
        });
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = serde_json::to_string_pretty(&fixture).unwrap_or_default() + "\n";
        std::fs::write(&self.path, text)
            .unwrap_or_else(|e| panic!("write {}: {e}", self.path.display()));
        crate::events::emit(
            "rung-std",
            "decide.recorded",
            &format!(
                "[rung-std] recorded {} (cost ${cost:.6}, total ${:.6})",
                self.path.display(),
                spent + cost
            ),
        );
        Ok(response)
    }
}

impl Decider for Recorded {
    fn decide(&self, ask: &Ask) -> Result<Decided, Undecided> {
        let response = self.exchange(&ask.body(&self.model))?;
        read_answers(ask, &response)
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` from the system clock.
fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decide::Question;
    use std::collections::BTreeMap;

    fn ask() -> Ask {
        let mut questions = BTreeMap::new();
        questions.insert("q".into(), Question::noul("Is it?", "Yes.", "No."));
        Ask {
            state: json!({"x": 1}),
            questions,
        }
    }

    fn fixture_file(request: &Value) -> (rung_testkit::TempDir, PathBuf) {
        let dir = rung_testkit::TempDir::new("recorded");
        let path = dir.join("f.json");
        let fixture = json!({
            "request": request,
            "response": {"model": "typesafe/jev-1.13-20260917",
                         "answers": {"q": {"type": "noul", "noul": 0.9}},
                         "usage": {"input_tokens": 10, "cost": 4.2e-7}},
            "model": "typesafe/jev-1.13-20260917",
            "recorded_at": "2026-09-30T00:00:00Z",
        });
        std::fs::write(&path, fixture.to_string()).unwrap();
        (dir, path)
    }

    #[test]
    fn replay_returns_the_recorded_answers() {
        let (_dir, path) = fixture_file(&ask().body(DEFAULT_MODEL));
        let d = Recorded::replay(&path).decide(&ask()).unwrap();
        assert_eq!(d.noul("q"), Some(0.9));
        assert_eq!(d.model, "typesafe/jev-1.13-20260917");
    }

    #[test]
    #[should_panic(expected = "records a different request")]
    fn a_changed_request_fails_loudly() {
        let (_dir, path) = fixture_file(&ask().body(DEFAULT_MODEL));
        let mut changed = ask();
        changed.state = json!({"x": 2});
        let _ = Recorded::replay(&path).decide(&changed);
    }

    #[test]
    #[should_panic(expected = "RUNG_DECIDE=record")]
    fn a_missing_fixture_names_the_record_command() {
        let _ = Recorded::replay("/no/such/fixture.json").decide(&ask());
    }

    #[test]
    fn utc_now_is_iso_shaped() {
        let s = utc_now();
        assert_eq!(s.len(), 20, "{s}");
        assert!(s.ends_with('Z') && s.as_bytes()[10] == b'T', "{s}");
    }
}
