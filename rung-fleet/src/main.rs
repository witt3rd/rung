//! `rung-fleet`: the small command over `rung_fleet::verbs`. It holds no
//! behaviour the library lacks.
//!
//! ```text
//! rung-fleet [--registry DIR] list [--json]
//! rung-fleet [--registry DIR] read ID [--after SEQ] [--limit N] [--crew]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use rung_fleet::verbs;

fn usage() -> ExitCode {
    eprintln!(
        "usage: rung-fleet [--registry DIR] list [--json]\n       \
         rung-fleet [--registry DIR] read ID [--after SEQ] [--limit N] [--crew]"
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
    }
}

enum Failure {
    Usage,
    Verb(verbs::VerbError),
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
