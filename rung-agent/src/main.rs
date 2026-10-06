//! `rung-agent` — fleet utility task agent.

use rung_agent::args::{self, Args};
use rung_agent::run::run_job;
use std::io::{self, IsTerminal, Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    match argv.get(1).map(String::as_str) {
        Some("--memory-fixture") => {
            return match rung_agent::memory_fixture::serve(&argv[2..]) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("rung-agent: {e}");
                    ExitCode::from(2)
                }
            };
        }
        Some("--memory-scope") => return memory_scope(&argv[2..]),
        Some("--memory-check") => return memory_check(argv.get(2)),
        _ => {}
    }
    let mut args = match Args::parse(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("rung-agent: {e}\n{}", args::usage());
            return ExitCode::from(2);
        }
    };
    if args.help {
        print!("{}", args::usage());
        return ExitCode::SUCCESS;
    }
    if args.acp || args.acp_http.is_some() {
        let ran = rung_agent::acp::run(args);
        rung_agent::memory::settle();
        return match ran {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("rung-agent: {e}");
                ExitCode::from(1)
            }
        };
    }
    if args.prompt.is_none() && args.task_id.is_none() && !io::stdin().is_terminal() {
        let mut buf = String::new();
        if io::stdin().read_to_string(&mut buf).is_ok() {
            let t = buf.trim();
            if !t.is_empty() {
                args.prompt = Some(t.to_string());
            }
        }
    }
    if args.prompt.is_none() && args.task_id.is_none() {
        eprintln!("rung-agent: missing prompt\n{}", args::usage());
        return ExitCode::from(2);
    }
    let origin = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("rung-agent: cwd: {e}");
            return ExitCode::from(1);
        }
    };
    let result = run_job(&args, &origin);
    let code = match result {
        Ok(out) => {
            if args.stream {
                // NDJSON stream mode: the final result line was already
                // emitted by the stream emitter during the run. Nothing else
                // goes on stdout.
            } else if args.json {
                // Single-shot JSON contract: one Outcome object on stdout,
                // used by non-interactive callers.
                match serde_json::to_string(&out) {
                    Ok(j) => println!("{j}"),
                    Err(e) => {
                        eprintln!("rung-agent: json: {e}");
                        return ExitCode::from(1);
                    }
                }
            } else if args.background && !rung_agent::background::in_child() {
                println!("task_id={}", out.task_id);
                println!("{}", out.text);
            } else if args.prompt.is_none() {
                println!("task_id={} status={}", out.task_id, out.status);
                if let Some(p) = out.isolation_path {
                    println!("isolation={p}");
                }
                if !out.text.is_empty() {
                    println!("{}", out.text);
                }
            } else {
                println!("{}", out.text);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("rung-agent: {e}");
            ExitCode::from(1)
        }
    };
    let _ = io::stdout().flush();
    rung_agent::memory::settle();
    code
}

/// `--memory-check SETTING`: run the provider contract and print each clause.
fn memory_check(setting: Option<&String>) -> ExitCode {
    let Some(setting) = setting else {
        eprintln!("rung-agent: --memory-check needs a provider (baseline, mcp:URL, mcp:COMMAND)");
        return ExitCode::from(2);
    };
    let dir = std::env::var("RUNG_MEMORY_DIR")
        .ok()
        .filter(|d| !d.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("rung-memory-check-{}", std::process::id()))
        });
    let timeout = std::time::Duration::from_secs(10);
    let token = match rung_agent::config::load_memory(None) {
        Ok(s) => s.token,
        Err(e) => {
            eprintln!("rung-agent: {e}");
            return ExitCode::from(2);
        }
    };
    match rung_agent::memory_fixture::check(setting, &dir, timeout, token) {
        Ok(clauses) if rung_agent::memory_fixture::report(&clauses) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(e) => {
            eprintln!("rung-agent: {e}");
            ExitCode::from(2)
        }
    }
}

/// `--memory-scope ls|rm ID|drop`: inspect the baseline store for the scope in
/// `RUNG_MEMORY_SCOPE` under `RUNG_MEMORY_DIR`. `ls` prints count, newest,
/// bytes, then one line per record; `rm ID` deletes a record; `drop` the scope.
fn memory_scope(rest: &[String]) -> ExitCode {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    let (Some(scope), Some(dir)) = (env("RUNG_MEMORY_SCOPE"), env("RUNG_MEMORY_DIR")) else {
        eprintln!("rung-agent: --memory-scope needs RUNG_MEMORY_SCOPE and RUNG_MEMORY_DIR");
        return ExitCode::from(2);
    };
    let scope = rung_memory::Scope::new(scope);
    let store = rung_memory::baseline::Baseline::new(dir);
    let fail = |e: String| {
        eprintln!("rung-agent: {e}");
        ExitCode::from(1)
    };
    match rest.first().map(String::as_str) {
        Some("ls") => match store.records(&scope) {
            Ok(rs) => {
                let bytes = std::fs::metadata(store.file(&scope)).map_or(0, |m| m.len());
                let newest = rs.iter().filter_map(|r| r.observed_at.as_deref()).max();
                println!(
                    "records: {}\nnewest: {}\nbytes: {bytes}",
                    rs.len(),
                    newest.unwrap_or("-")
                );
                for r in &rs {
                    let t: String = r.text.chars().take(60).collect();
                    println!(
                        "{}\t{}\t{}",
                        r.id.as_str(),
                        r.observed_at.as_deref().unwrap_or("-"),
                        t.replace(['\n', '\t'], " ")
                    );
                }
                ExitCode::SUCCESS
            }
            Err(m) => fail(m.why.to_string()),
        },
        Some("rm") => match rest.get(1) {
            Some(id) => match store.delete_record(&scope, &rung_memory::RecordId::new(id)) {
                Ok(true) => {
                    println!("deleted {id}");
                    ExitCode::SUCCESS
                }
                Ok(false) => fail(format!("no record {id}")),
                Err(e) => fail(e),
            },
            None => fail("rm needs a record id".into()),
        },
        Some("drop") => match store.delete_scope(&scope) {
            Ok(b) => {
                println!(
                    "{}",
                    if b {
                        "scope deleted"
                    } else {
                        "scope was empty"
                    }
                );
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        _ => fail("usage: --memory-scope ls | rm ID | drop".into()),
    }
}
