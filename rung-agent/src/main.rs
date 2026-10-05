//! `rung-agent` — fleet utility task agent.

use rung_agent::args::{self, Args};
use rung_agent::run::run_job;
use std::io::{self, IsTerminal, Read};
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
    rung_agent::memory::settle();
    match result {
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
    }
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
