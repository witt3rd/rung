//! `rung-fleet`: the small command over `rung_fleet::verbs`. It holds no
//! behaviour the library lacks.
//!
//! ```text
//! rung-fleet [--registry DIR] list [--json]
//! rung-fleet [--registry DIR] read ID [--after SEQ] [--limit N] [--crew]
//! rung-fleet [--registry DIR] steer ID --from NAME --id CORRELATION [--wait SECS] TEXT
//! rung-fleet [--registry DIR] assign ID --from NAME --id CORRELATION --task NAME --objective T --done-when T --delivery T --autonomy ask|decide
//! rung-fleet spawn NAME --root DIR --from NAME --task NAME --assignee NAME --objective T --done-when T --delivery T --autonomy ask|decide [--engine KIND] [--host-bin PATH]
//! rung-fleet [--registry DIR] stop ID --from NAME --id CORRELATION [--wait SECS] WHY
//! ```
//!
//! `spawn`, `assign`, `steer` and `stop` also take the principal's word:
//! `--grant-state DIR --grant ID --ledger DIR [--project P --request R]`. The
//! grant is the owner item `ID` in the record under `DIR`; `stop` accepts only
//! a one-time grant, the others a standing one given `--project` and `--request`.

use std::path::PathBuf;
use std::process::ExitCode;

use rung_fleet::delivery::{CorrelationId, Sent};
use rung_fleet::{steer, verbs};

fn usage() -> ExitCode {
    eprintln!(
        "usage: rung-fleet [--registry DIR] list [--json]\n       \
         rung-fleet [--registry DIR] read ID [--after SEQ] [--limit N] [--crew]\n       \
         rung-fleet [--registry DIR] steer ID --from NAME --id CORRELATION [--wait SECS] TEXT\n       \
         rung-fleet [--registry DIR] assign ID --from NAME --id CORRELATION --task NAME \\\n       \
         --objective T --done-when T --delivery T --autonomy ask|decide\n       \
         rung-fleet spawn NAME --root DIR --from NAME --task NAME --assignee NAME \\\n       \
         --objective T --done-when T --delivery T --autonomy ask|decide [--engine KIND] [--host-bin PATH]\n       \
         rung-fleet [--registry DIR] stop ID --from NAME --id CORRELATION [--wait SECS] WHY\n       \
         (spawn, assign, steer, stop: --grant-state DIR --grant ID --ledger DIR [--project P --request R])"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).peekable();
    let mut registry: Option<PathBuf> = None;
    if args.peek().map(String::as_str) == Some("--registry") {
        args.next();
        match args.next() {
            Some(d) => registry = Some(PathBuf::from(d)),
            None => return usage(),
        }
    }
    let dir = registry.unwrap_or_else(rung_host::registry::dir);
    let rest: Vec<String> = args.collect();
    match run(&dir, &rest) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage) => usage(),
        Err(Failure::Verb(e)) => {
            eprintln!("rung-fleet: {e}");
            ExitCode::from(1)
        }
        Err(Failure::Send(e)) => {
            eprintln!("rung-fleet: {e}");
            ExitCode::from(1)
        }
        Err(Failure::Unconfirmed(e)) => {
            eprintln!("rung-fleet: {e}");
            ExitCode::from(3)
        }
    }
}

/// The authority flags every verb that needs the principal's word takes:
/// where the principal's grant is recorded, and which item.
const AUTH_FLAGS: [(&str, &str); 5] = [
    ("--grant-state", "grant_state"),
    ("--grant", "grant"),
    ("--ledger", "ledger"),
    ("--project", "project"),
    ("--request", "request"),
];

type Flags = std::collections::BTreeMap<&'static str, String>;

/// The grant named by the flags, read from the principal's record.
fn granted(f: &Flags) -> Result<(rung_fleet::authority::Granted, PathBuf), Failure> {
    let need = |k: &str| f.get(k).cloned().ok_or(Failure::Usage);
    let g = rung_fleet::authority::Granted::from_record(
        std::path::Path::new(&need("grant_state")?),
        &need("grant")?,
    )
    .map_err(|e| Failure::Unconfirmed(e.to_string()))?;
    Ok((g, PathBuf::from(need("ledger")?)))
}

fn scope(f: &Flags) -> Option<(&str, &str)> {
    Some((f.get("project")?.as_str(), f.get("request")?.as_str()))
}

enum Failure {
    Usage,
    Verb(verbs::VerbError),
    Send(steer::SteerError),
    Unconfirmed(String),
}

impl From<steer::SteerError> for Failure {
    fn from(e: steer::SteerError) -> Self {
        Failure::Send(e)
    }
}

/// `spawn`: prepare a member with its brief, and start it with `--host-bin`.
fn spawn_cmd(args: &[String]) -> Result<(), Failure> {
    let name = args.first().ok_or(Failure::Usage)?;
    let mut f: Flags = Flags::new();
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        let key = match a.as_str() {
            "--root" => "root",
            "--from" => "from",
            "--task" => "task",
            "--assignee" => "assignee",
            "--objective" => "objective",
            "--done-when" => "done_when",
            "--delivery" => "delivery",
            "--autonomy" => "autonomy",
            "--engine" => "engine",
            "--host-bin" => "host_bin",
            other => AUTH_FLAGS
                .iter()
                .find(|(n, _)| *n == other)
                .map(|(_, k)| *k)
                .ok_or(Failure::Usage)?,
        };
        f.insert(key, it.next().cloned().ok_or(Failure::Usage)?);
    }
    let get = |k: &str| f.get(k).cloned().ok_or(Failure::Usage);
    let autonomy = match get("autonomy")?.as_str() {
        "ask" => rung_fleet::Autonomy::Ask,
        "decide" => rung_fleet::Autonomy::Decide,
        _ => return Err(Failure::Usage),
    };
    let bad = |e: String| Failure::Unconfirmed(e);
    let brief = rung_fleet::Brief::new(
        get("objective")?,
        get("done_when")?,
        get("delivery")?,
        autonomy,
    )
    .map_err(|e| bad(e.to_string()))?;
    let what = rung_fleet::lifecycle::Spawn::new(brief, get("assignee")?)
        .map_err(|e| bad(e.to_string()))?;
    let engine = f.get("engine").cloned().unwrap_or_else(|| "mock".into());
    let (g, ledger) = granted(&f)?;
    let token = g
        .routine(&ledger, scope(&f))
        .map_err(|e| bad(e.to_string()))?;
    let s = rung_fleet::spawn::spawn(
        std::path::Path::new(&get("root")?),
        name,
        &get("from")?,
        &get("task")?,
        &what,
        &engine,
        token,
    )
    .map_err(|e| bad(e.to_string()))?;
    println!("spawned {name}: {}", s.config.display());
    if let Some(bin) = f.get("host_bin") {
        let child = rung_fleet::spawn::launch(std::path::Path::new(bin), &s.config)
            .map_err(|e| bad(format!("launch {bin}: {e}")))?;
        println!("started pid {}", child.id());
    }
    drop(s.task);
    Ok(())
}

/// `assign`: send a task with its whole brief.
fn assign(dir: &std::path::Path, args: &[String]) -> Result<(), Failure> {
    let member = args.first().ok_or(Failure::Usage)?;
    let mut f: Flags = Flags::new();
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        let key = match a.as_str() {
            "--from" => "from",
            "--id" => "id",
            "--task" => "task",
            "--objective" => "objective",
            "--done-when" => "done_when",
            "--delivery" => "delivery",
            "--autonomy" => "autonomy",
            other => AUTH_FLAGS
                .iter()
                .find(|(n, _)| *n == other)
                .map(|(_, k)| *k)
                .ok_or(Failure::Usage)?,
        };
        f.insert(key, it.next().cloned().ok_or(Failure::Usage)?);
    }
    let get = |k: &str| f.get(k).cloned().ok_or(Failure::Usage);
    let autonomy = match get("autonomy")?.as_str() {
        "ask" => rung_fleet::Autonomy::Ask,
        "decide" => rung_fleet::Autonomy::Decide,
        _ => return Err(Failure::Usage),
    };
    let brief = rung_fleet::Brief::new(
        get("objective")?,
        get("done_when")?,
        get("delivery")?,
        autonomy,
    )
    .map_err(|e| Failure::Unconfirmed(e.to_string()))?;
    let id = CorrelationId::new(get("id")?)
        .map_err(|e| Failure::Send(steer::SteerError::Delivery(e)))?;
    let (g, ledger) = granted(&f)?;
    let token = g
        .routine(&ledger, scope(&f))
        .map_err(|e| Failure::Unconfirmed(e.to_string()))?;
    let sent = steer::assign(dir, member, &get("from")?, id, &get("task")?, &brief, token)?;
    println!("sent {}", sent.correlation().as_str());
    Ok(())
}

/// `steer` or `stop`: send, and with `--wait` confirm against the member's record.
fn send(dir: &std::path::Path, stop: bool, args: &[String]) -> Result<(), Failure> {
    let member = args.first().ok_or(Failure::Usage)?;
    let (mut from, mut id, mut wait, mut text) = (None, None, 0u64, None);
    let mut f: Flags = Flags::new();
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--from" => from = it.next().cloned(),
            "--id" => id = it.next().cloned(),
            "--wait" => {
                wait = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or(Failure::Usage)?
            }
            t if text.is_none() && !t.starts_with("--") => text = Some(t.to_string()),
            other => {
                let key = AUTH_FLAGS
                    .iter()
                    .find(|(n, _)| *n == other)
                    .map(|(_, k)| *k)
                    .ok_or(Failure::Usage)?;
                f.insert(key, it.next().cloned().ok_or(Failure::Usage)?);
            }
        }
    }
    let (from, id, text) = (
        from.ok_or(Failure::Usage)?,
        id.ok_or(Failure::Usage)?,
        text.ok_or(Failure::Usage)?,
    );
    let id = CorrelationId::new(id).map_err(|e| Failure::Send(steer::SteerError::Delivery(e)))?;
    let (g, ledger) = granted(&f)?;
    let bad = |e: rung_fleet::authority::AuthorityError| Failure::Unconfirmed(e.to_string());
    let mut sent: Sent = if stop {
        let token = g.once(&ledger).map_err(bad)?;
        steer::stop(dir, member, &from, id, &text, token)?
    } else {
        let token = g.routine(&ledger, scope(&f)).map_err(bad)?;
        steer::steer(dir, member, &from, id, &text, token)?
    };
    let end = std::time::Instant::now() + std::time::Duration::from_secs(wait);
    loop {
        if wait == 0 {
            println!("sent {}", sent.correlation().as_str());
            return Ok(());
        }
        match sent.confirm() {
            Ok(r) => {
                println!("recorded at line {}", r.seq());
                r.discard();
                return Ok(());
            }
            Err(p) => {
                if std::time::Instant::now() >= end {
                    return Err(Failure::Unconfirmed(format!(
                        "{} not in the record after {wait}s",
                        p.into_sent().correlation().as_str()
                    )));
                }
                sent = p.into_sent();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
}

impl From<verbs::VerbError> for Failure {
    fn from(e: verbs::VerbError) -> Self {
        Failure::Verb(e)
    }
}

fn run(dir: &std::path::Path, args: &[String]) -> Result<(), Failure> {
    match args.first().map(String::as_str) {
        Some("list") => {
            let rows = verbs::list(dir)?;
            match args.get(1).map(String::as_str) {
                None => print!("{}", rung_host::registry::render(&rows)),
                Some("--json") if args.len() == 2 => {
                    println!("{}", rung_host::registry::rows_json(&rows))
                }
                _ => return Err(Failure::Usage),
            }
            Ok(())
        }
        Some("assign") => assign(dir, &args[1..]),
        Some("spawn") => spawn_cmd(&args[1..]),
        Some(w @ ("steer" | "stop")) => send(dir, w == "stop", &args[1..]),
        Some("read") => {
            let id = args.get(1).ok_or(Failure::Usage)?;
            let (mut after, mut limit, mut crew) = (0u64, 100usize, false);
            let mut it = args[2..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--crew" => crew = true,
                    "--after" => {
                        after = it
                            .next()
                            .and_then(|v| v.parse().ok())
                            .ok_or(Failure::Usage)?
                    }
                    "--limit" => {
                        limit = it
                            .next()
                            .and_then(|v| v.parse().ok())
                            .ok_or(Failure::Usage)?
                    }
                    _ => return Err(Failure::Usage),
                }
            }
            if crew {
                for (seq, r) in verbs::crew(dir, id)? {
                    println!("{seq}\t{}\t{}", r.kind(), r.crew());
                }
            } else {
                let r = verbs::read(dir, id, after, limit)?;
                for l in &r.lines {
                    println!("{}", l.text());
                }
                if let Some(d) = r.damaged {
                    eprintln!("rung-fleet: {d}");
                }
            }
            Ok(())
        }
        _ => Err(Failure::Usage),
    }
}
