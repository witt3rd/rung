//! Relegate a work item across two instances — the first real cross-theory
//! rectification, from the command line (the second-order note: the router).
//!
//! ```text
//! cargo run -p rung-driver --bin relegate -- q-7            # dry-run (prints the plan)
//! cargo run -p rung-driver --bin relegate -- q-7 --live     # discharge + admit
//! ```
//!
//! Reads the real state sidecars: the Questions instance (`.het/rung-questions/
//! config.yaml`, a colocated folder) is the source; the Issues instance
//! (`.het/rung-issues/config.yaml`) is the destination. A work item is
//! **discharged** from the questions carrier and **admitted** onto the issues
//! carrier, gated on the Issues theory's own membership screen.
//!
//! The **destination carrier's add is the actual write**: for a folder carrier
//! it writes a file; for the external GitHub carrier it creates an issue via
//! the ambient `gh` CLI. Because that is a real, external side effect, the
//! default is a **dry-run** — it prints the exact plan (source → destination,
//! id, and whether the destination's gate admits the re-formed candidate) and
//! touches nothing. Pass `--live` to discharge and admit for real.

use std::path::{Path, PathBuf};

use rung_driver::{
    CarrierKind, FolderCarrier, GitHubIssuesCarrier, Instance, ObjectCarrier, ObjectId, admit,
};
use rung_het::Admits;
use rung_std::issues::{Issues, Scheme};

const ISSUES_SCHEME: Scheme = Scheme {
    namespace: "rung-issues",
    root: "issues",
    id_prefix: "issue",
};

fn ws_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-driver sits in the workspace")
        .to_path_buf()
}

/// Load an instance's config and resolve its colocated path against the
/// config's own directory (Q18). Returns (Instance, resolved_base).
fn sidecar(root: &Path, rel: &str) -> (Instance, PathBuf) {
    let text = std::fs::read_to_string(root.join(rel)).expect("instance config.yaml");
    let inst = Instance::from_yaml(&text).expect("config parses");
    let base = root.join(Path::new(rel).parent().expect("config has a directory"));
    (inst, base)
}

/// Build a writable (ObjectCarrier) destination for an instance config.
fn object_carrier(inst: &Instance, base: &Path) -> Box<dyn ObjectCarrier> {
    let path = |p: &str| -> PathBuf {
        let pb = PathBuf::from(p);
        if pb.is_absolute() { pb } else { base.join(pb) }
    };
    match inst.carrier.kind {
        CarrierKind::Folder => Box::new(FolderCarrier::new(path(
            inst.carrier
                .path
                .as_deref()
                .expect("folder carrier needs a path"),
        ))),
        CarrierKind::GitHub => Box::new(
            GitHubIssuesCarrier::new(inst.carrier.repos.clone())
                .expect("github carrier needs repos"),
        ),
        other => {
            eprintln!("no writable carrier for kind `{other:?}`");
            std::process::exit(4);
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let id = args.next().unwrap_or_else(|| {
        eprintln!("usage: relegate <object-id> [--live]");
        std::process::exit(2);
    });
    let live = args.any(|a| a == "--live");
    let root = ws_root();

    let (src_inst, src_base) = sidecar(&root, ".het/rung-questions/config.yaml");
    let (dst_inst, dst_base) = sidecar(&root, ".het/rung-issues/config.yaml");
    let src = object_carrier(&src_inst, &src_base);
    let dst = object_carrier(&dst_inst, &dst_base);

    println!("── relegation ───────────────────────────────────────────────");
    println!("  source   : questions @ .het/rung-questions (colocated folder)");
    println!(
        "  dest     : issues @ .het/rung-issues (carrier kind {:?})",
        dst_inst.carrier.kind
    );
    println!("  subject  : {id}");
    println!(
        "  mode     : {}",
        if live {
            "LIVE — will discharge + admit"
        } else {
            "dry-run — nothing written"
        }
    );
    println!();

    let obj = ObjectId::new(id.clone());

    // Does the source carrier still hold it?
    match src.read(&obj) {
        Ok(content) => {
            println!(
                "  [1] source holds `{id}` ({} bytes) — DISCHARGE",
                content.len()
            );
            // Re-form the subject as a member of the *destination* sort: an open
            // issue carrying the work item's body.
            let candidate = format!("---\nid: {id}\nstatus: open\n---\n\n{}\n", content.trim());
            let world = Issues::new(ISSUES_SCHEME, vec![]);
            if world.content_is_admissible(&candidate) {
                println!("  [2] Issues gate ADMITS the re-formed issue");
            } else {
                eprintln!(
                    "  [2] Issues gate REFUSES the re-formed issue — not a well-formed issue"
                );
                std::process::exit(3);
            }
            if !live {
                println!("  [3] (dry-run) would ADMIT onto the issues carrier");
                println!();
                println!("  nothing written — pass `--live` to discharge and admit for real");
                return;
            }
            rung_driver::discharge(&*src, &obj).expect("discharge from the questions carrier");
            let assigned =
                admit(&world, &*dst, &obj, &candidate).expect("admit onto the issues carrier");
            println!(
                "  [3] LIVE — discharged from questions, admitted onto issues as `{assigned}`"
            );
        }
        Err(e) => {
            eprintln!("  source carrier does not hold `{id}`: {e}");
            std::process::exit(1);
        }
    }
}
