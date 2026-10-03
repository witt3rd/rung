//! `rung-host`: run the continuous host.
//!
//! In this slice the only engine is the scripted mock, so the binary runs
//! the host against the fake world — the harness the process-level gates
//! (stop, watchdog, `kill -9` restarts) drive. It makes no network call.
//!
//! ```text
//! rung-host sim --state DIR [--seed N] [--turns N] [--clock real|sim]
//!               [--call-ms A,B] [--wedge-at TURN] [--inbox DIR] [--stop-file PATH]
//!               [--fault outage|none] [--backoff-ms MS] [--quota RPD,RPM]
//!               [--owner-per-hour X] [--no-memory]
//! rung-host canon --state DIR [--seed N]   # print the hash of a seeded run's request bytes
//! ```

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use rung_host::clock::{Clock, DAY, RealClock};
use rung_host::inbox::{DirSource, Source};
use rung_host::notify::Notifier;
use rung_host::sim::{self, Fault, FaultKind, Scenario, WorldConfig};
use rung_host::stop::{self, StopAuthority, Why};

struct Opts {
    cmd: String,
    state: Option<PathBuf>,
    seed: u64,
    turns: Option<u64>,
    real: bool,
    call_ms: Option<(i64, i64)>,
    wedge_at: Option<u64>,
    inbox: Option<PathBuf>,
    stop_file: Option<PathBuf>,
    fault: Option<String>,
    backoff_ms: Option<i64>,
    quota: Option<(u64, u64)>,
    owner_per_hour: f64,
    memory: bool,
}

fn parse() -> Result<Opts, String> {
    let mut a = std::env::args().skip(1);
    let cmd = a.next().ok_or("usage: rung-host sim|canon --state DIR ...")?;
    let mut o = Opts {
        cmd,
        state: None,
        seed: 1,
        turns: None,
        real: false,
        call_ms: None,
        wedge_at: None,
        inbox: None,
        stop_file: None,
        fault: None,
        backoff_ms: None,
        quota: None,
        owner_per_hour: 0.0,
        memory: true,
    };
    while let Some(f) = a.next() {
        let mut v = || a.next().ok_or(format!("{f} needs a value"));
        let pair = |s: String| -> Result<(String, String), String> {
            s.split_once(',')
                .map(|(x, y)| (x.to_string(), y.to_string()))
                .ok_or(format!("{f}: A,B"))
        };
        match f.as_str() {
            "--state" => o.state = Some(PathBuf::from(v()?)),
            "--seed" => o.seed = v()?.parse().map_err(|e| format!("{f}: {e}"))?,
            "--turns" => o.turns = Some(v()?.parse().map_err(|e| format!("{f}: {e}"))?),
            "--clock" => o.real = v()? == "real",
            "--call-ms" => {
                let (x, y) = pair(v()?)?;
                o.call_ms = Some((x.parse().map_err(|_| "--call-ms")?, y.parse().map_err(|_| "--call-ms")?));
            }
            "--wedge-at" => o.wedge_at = Some(v()?.parse().map_err(|e| format!("{f}: {e}"))?),
            "--inbox" => o.inbox = Some(PathBuf::from(v()?)),
            "--stop-file" => o.stop_file = Some(PathBuf::from(v()?)),
            "--fault" => o.fault = Some(v()?),
            "--backoff-ms" => o.backoff_ms = Some(v()?.parse().map_err(|e| format!("{f}: {e}"))?),
            "--quota" => {
                let (x, y) = pair(v()?)?;
                o.quota = Some((x.parse().map_err(|_| "--quota")?, y.parse().map_err(|_| "--quota")?));
            }
            "--owner-per-hour" => o.owner_per_hour = v()?.parse().map_err(|e| format!("{f}: {e}"))?,
            "--no-memory" => o.memory = false,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(o)
}

fn scenario(o: &Opts, state: &std::path::Path) -> Result<Scenario, String> {
    let mut sc = Scenario::new(state, o.seed);
    sc.max_turns = o.turns;
    sc.memory = o.memory;
    if let Some(ms) = o.call_ms {
        sc.mock.call_ms = ms;
    }
    sc.mock.wedge_at_turn = o.wedge_at;
    sc.mock.call_log = Some(state.join("mock-calls.log"));
    let clock: Arc<dyn Clock> = if o.real {
        Arc::new(RealClock)
    } else {
        Arc::new(rung_host::clock::SimClock::new(sim::SIM_START))
    };
    let start = clock.now();
    sc.start = start;
    sc.world = WorldConfig {
        owner_per_hour: o.owner_per_hour,
        ..WorldConfig::quiet(o.seed, start, DAY)
    };
    sc.clock = Some(clock);
    if let Some(f) = &o.fault {
        match f.as_str() {
            "outage" => sc.faults.push(Fault {
                from: 0,
                to: i64::MAX,
                kind: FaultKind::Outage,
                model: None,
            }),
            "none" => {}
            other => return Err(format!("unknown fault {other}")),
        }
    }
    if let Some(ms) = o.backoff_ms {
        sc.config.governor.backoff_base_ms = ms;
    }
    if let Some((rpd, rpm)) = o.quota {
        sc = sc.quota(rpd, rpm);
    }
    if let Some(dir) = &o.inbox {
        let src: Box<dyn Source> = Box::new(DirSource::new(dir).map_err(|e| format!("inbox: {e}"))?);
        sc.sources.push(src);
    }
    sc.notifier = Some(Notifier::from_env());
    stop::install_signals();
    sc.stop = Some(Arc::new(StopAuthority::new(o.stop_file.clone(), true)));
    Ok(sc)
}

fn main() -> ExitCode {
    let o = match parse() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("rung-host: {e}");
            return ExitCode::from(2);
        }
    };
    let Some(state) = o.state.clone() else {
        eprintln!("rung-host: --state DIR is required");
        return ExitCode::from(2);
    };
    let sc = match scenario(&o, &state) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rung-host: {e}");
            return ExitCode::from(2);
        }
    };
    match o.cmd.as_str() {
        "sim" => {
            let (host, rec, _mock) = sim::build(sc);
            match host.run(rec) {
                Why::Stopped { .. } => ExitCode::SUCCESS,
                Why::Revoked { .. } | Why::SpendCap { .. } => ExitCode::from(3),
            }
        }
        "canon" => {
            let out = sim::run(sc);
            println!("{}", rung_host::canon::hash(&out.host.request_bytes()));
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("rung-host: unknown command {other}");
            ExitCode::from(2)
        }
    }
}
