//! Run the proofs, and attribute the results to the propositions they prove.
//!
//! ## What this is
//!
//! `cargo test` answers *did the suite pass*. It cannot answer *does rung
//! satisfy its own doctrine*, because nothing maps a test result back to a
//! proposition. The doctrine now holds that join —
//! `Kind::Decidable { proof }` names the test — so the join can be walked.
//!
//! This is the **audit** arm of the pass, `⊨` evaluated over the decidable
//! fragment against the implementation as the model. It needs no principal, no
//! standing and no judge: a decidable sentence is settled by running it
//! (`decidable-runs-pure`).
//!
//! ## What it is not
//!
//! It settles nothing judgmental. 47 propositions need an outside and this
//! cannot supply one; they are reported as unsettled and stay that way.
//!
//! Nor does it check that a proof is **apt** for the proposition citing it. A
//! passing test attributed to the wrong claim reads here exactly like a passing
//! test attributed to the right one — that is `establishes_what_it_cites`,
//! judgmental and unsettled, and this runner inherits the gap rather than
//! closing it.
//!
//! ```text
//! cargo run -p rung-doctrine --bin audit
//! cargo run -p rung-doctrine --bin audit -- --quiet   # verdict line only
//! ```

use rung_doctrine::{Doctrine, Kind, rung, rung_ct, rung_het};
use std::collections::BTreeMap;
use std::process::Command;

/// What running a proof produced.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    Passed,
    Failed,
    /// Present in the suite and `#[ignore]`d — it did not run, so it proves
    /// nothing. Distinct from failing, and worse than it: a failure is visible.
    NotRun,
    /// Cited, and no such test appeared in the run at all.
    Missing,
}

fn main() {
    let quiet = std::env::args().any(|a| a == "--quiet");
    let doctrines = vec![rung::doctrine(), rung_het::doctrine(), rung_ct::doctrine()];

    eprintln!("running the suite…");
    // Merged through a shell, deliberately. Cargo writes `Running tests/foo.rs`
    // to stderr and the test lines to stdout; captured separately they cannot be
    // interleaved afterwards, and a bare fn name is then all there is to key on.
    // Two names already collide across files in this workspace, so that key
    // would attribute one proposition's result to another.
    let out = Command::new("sh")
        .arg("-c")
        .arg("cargo test --workspace --locked --no-fail-fast 2>&1")
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .expect("cargo test runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let results = parse(&text);
    eprintln!("  {} test results", results.len());

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let clashes = ambiguity(&root);
    if !clashes.is_empty() {
        eprintln!("\nambiguous proof references — attribution would be a guess:");
        for c in &clashes {
            eprintln!("  {c}");
        }
        std::process::exit(2);
    }

    eprintln!();

    // ── attribute ──────────────────────────────────────────────────────────
    let mut by_outcome: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut unsettled: Vec<(String, String)> = Vec::new();
    let mut inert = 0usize;

    for d in &doctrines {
        for p in d.props() {
            match &p.kind {
                Kind::Decidable { proof } => {
                    let o = lookup(&results, proof);
                    let key = match o {
                        Outcome::Passed => "satisfied",
                        Outcome::Failed => "VIOLATED",
                        Outcome::NotRun => "not run",
                        Outcome::Missing => "proof missing",
                    };
                    by_outcome
                        .entry(key)
                        .or_default()
                        .push(format!("{}  ({proof})", p.slug));
                }
                Kind::Judgmental { role } => unsettled.push((p.slug.clone(), role.clone())),
                Kind::Owed { why } => {
                    by_outcome
                        .entry("owed")
                        .or_default()
                        .push(format!("{}  — {why}", p.slug));
                }
                _ => inert += 1,
            }
        }
    }

    let n = |k: &str| by_outcome.get(k).map_or(0, Vec::len);
    let decidable = n("satisfied") + n("VIOLATED") + n("not run") + n("proof missing");

    if !quiet {
        for key in ["VIOLATED", "proof missing", "not run", "owed"] {
            if let Some(rows) = by_outcome.get(key) {
                println!("── {key} ({}) ──", rows.len());
                for r in rows {
                    println!("   {r}");
                }
                println!();
            }
        }
    }

    println!("── audit of rung against its own doctrine ──\n");
    println!("  decidable          {decidable}");
    println!("    satisfied        {}", n("satisfied"));
    println!("    violated         {}", n("VIOLATED"));
    println!("    did not run      {}", n("not run"));
    println!("    proof missing    {}", n("proof missing"));
    println!("  owed               {}", n("owed"));
    println!(
        "  unsettled          {}   (need a principal)",
        unsettled.len()
    );
    println!("  not a claim        {inert}");

    let mut roles: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, r) in &unsettled {
        *roles.entry(r.as_str()).or_default() += 1;
    }
    if !quiet && !roles.is_empty() {
        println!("\n  the unsettled, by role they await:");
        for (r, c) in &roles {
            println!("    {c:>3}  {r}");
        }
    }

    println!(
        "\n  rung satisfies {} of {} decidable propositions of its own doctrine.",
        n("satisfied"),
        decidable
    );
    if unsettled.is_empty() {
        println!("  Nothing is unsettled.");
    } else {
        println!(
            "  {} remain unsettled: no test decides them and no judge has ruled.",
            unsettled.len()
        );
    }

    // A violated proposition is the audit finding something, which is the
    // point. Exit non-zero so a driver can act on it.
    if n("VIOLATED") + n("proof missing") > 0 {
        std::process::exit(1);
    }
}

/// Look a proof up.
///
/// A proof names `rung/tests/compile_pass.rs::fn`; cargo reports
/// `tests/compile_pass.rs::fn`, because it prints paths relative to the crate
/// and does not say which crate. So the key is matched as a **suffix**.
///
/// That is sound only while `basename::fn` is unique across the workspace, and
/// it is not obviously so — two fn names already repeat across files. The
/// pairs differ in basename, which is why this works; [`ambiguity`] refuses
/// the run if that ever stops being true, rather than crediting one
/// proposition with another's result.
fn lookup(results: &BTreeMap<String, Outcome>, proof: &str) -> Outcome {
    let tail = proof.split_once('/').map_or(proof, |(_, rest)| rest);
    results.get(tail).copied().unwrap_or(Outcome::Missing)
}

/// `basename::fn` pairs claimed by more than one file.
fn ambiguity(root: &std::path::Path) -> Vec<String> {
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in rung_doctrine::workspace_tests(root) {
        let Some((file, func)) = t.rsplit_once("::") else {
            continue;
        };
        let base = file.rsplit('/').next().unwrap_or(file);
        seen.entry(format!("{base}::{func}"))
            .or_default()
            .push(file.to_string());
    }
    seen.into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(k, files)| format!("{k} claimed by {}", files.join(", ")))
        .collect()
}

/// Parse `cargo test` output into `fn -> outcome`.
fn parse(text: &str) -> BTreeMap<String, Outcome> {
    let mut out = BTreeMap::new();
    let mut file = String::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Running ")
            && let Some((path, _)) = rest.split_once(" (")
        {
            file = path.to_string();
            continue;
        }
        if t.starts_with("Doc-tests ") {
            file = String::new();
            continue;
        }
        let Some(rest) = t.strip_prefix("test ") else {
            continue;
        };
        let Some((name, tail)) = rest.rsplit_once(" ... ") else {
            continue;
        };
        let outcome = if tail.starts_with("ok") {
            Outcome::Passed
        } else if tail.starts_with("ignored") {
            Outcome::NotRun
        } else if tail.starts_with("FAILED") {
            Outcome::Failed
        } else {
            continue;
        };
        // A `#[should_panic]` test reports as `name - should panic ... ok`.
        // Splitting naively leaves the suffix on the name and the proof stops
        // resolving — which reads as "proof missing" for a test that ran and
        // passed. Ten propositions looked unproven for exactly this reason.
        let name = name.trim().trim_end_matches(" - should panic").trim();
        // Skip trybuild's own per-case lines (`tests/ui/foo.rs`); the harness
        // fn that owns them is reported separately and is what a proof cites.
        if name.ends_with(".rs") {
            continue;
        }
        // A failing test is reported twice — once in the running list and
        // once in the failures summary. Never let a later `ok` overwrite a
        // `FAILED`, or a violation would vanish into a pass.
        let key = if file.is_empty() {
            name.to_string()
        } else {
            format!("{file}::{name}")
        };
        out.entry(key)
            .and_modify(|e| {
                if outcome == Outcome::Failed {
                    *e = Outcome::Failed;
                }
            })
            .or_insert(outcome);
    }
    out
}

/// Unused outside `main`, but kept so the binary compiles standalone if the
/// doctrine list is ever built elsewhere.
#[allow(dead_code)]
fn all() -> Vec<Doctrine> {
    vec![rung::doctrine(), rung_het::doctrine(), rung_ct::doctrine()]
}
