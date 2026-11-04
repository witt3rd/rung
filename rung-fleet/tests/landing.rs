//! `unlanded-work-is-never-retired`: retirement consumes proof that the work
//! has landed; uncommitted or unmerged work in an isolation worktree is
//! refused, and scratch needs a one-time authority to discard. Real git
//! repositories under a scratch directory.

mod common;

use std::path::Path;
use std::process::Command;

use rung_fleet::authority::act;
use rung_fleet::landing::{Landed, Unlanded};
use rung_testkit::TempDir;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A shared bare line `main` with one commit, and a clone to work in.
fn repos(root: &Path) -> std::path::PathBuf {
    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    git(&seed, &["init", "-q", "-b", "main"]);
    std::fs::write(seed.join("a.txt"), "1").unwrap();
    git(&seed, &["add", "a.txt"]);
    git(&seed, &["commit", "-q", "-m", "first"]);
    let shared = root.join("shared.git");
    let work = root.join("work");
    for (src, dst) in [(&seed, &shared), (&shared, &work)] {
        let mut c = Command::new("git");
        c.args(["clone", "-q"]);
        if dst == &shared {
            c.arg("--bare");
        }
        let out = c.arg(src).arg(dst).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    work
}

#[test]
fn an_uncommitted_or_unmerged_worktree_is_refused_retirement() {
    let root = TempDir::new("fleet-landing");
    let work = repos(root.path());

    // Nothing to land: the head is on the line.
    let landed = Landed::merged(&work, "origin/main").expect("clean and merged");
    assert!(landed.describe().starts_with("merged: "));

    // An untracked file, then a modified one: refused, and nothing is touched.
    std::fs::write(work.join("new.txt"), "x").unwrap();
    assert_eq!(
        Landed::merged(&work, "origin/main").unwrap_err(),
        Unlanded::Dirty(vec!["new.txt".into()])
    );
    git(&work, &["add", "new.txt"]);
    git(&work, &["commit", "-q", "-m", "work"]);
    std::fs::write(work.join("a.txt"), "changed").unwrap();
    assert!(matches!(
        Landed::merged(&work, "origin/main"),
        Err(Unlanded::Dirty(p)) if p == ["a.txt"]
    ));
    assert_eq!(
        std::fs::read_to_string(work.join("a.txt")).unwrap(),
        "changed"
    );
    git(&work, &["checkout", "--", "a.txt"]);

    // Committed but not on the line: refused until it is.
    assert!(matches!(
        Landed::merged(&work, "origin/main"),
        Err(Unlanded::NotMerged { .. })
    ));
    git(&work, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
    git(&work, &["fetch", "-q", "origin"]);
    assert!(Landed::merged(&work, "origin/main").is_ok());

    // A revision that does not exist, and a directory that is no repository.
    assert!(matches!(
        Landed::merged(&work, "no-such-line"),
        Err(Unlanded::Git(_))
    ));
    assert!(matches!(
        Landed::merged(root.path(), "origin/main"),
        Err(Unlanded::Git(_))
    ));
}

#[test]
fn scratch_consumes_a_one_time_authority_to_discard() {
    let root = TempDir::new("fleet-landing-scratch");
    let landed = Landed::scratch(common::once::<act::Discard>(root.path()));
    assert!(landed.describe().contains("discarded under grant grant-"));
}
