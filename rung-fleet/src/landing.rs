//! Proof that work has landed (`unlanded-work-is-never-retired`).
//!
//! A task is retired only by a transition that consumes a [`Landed`]. There
//! are two ways to hold one: [`Landed::merged`], which reads the isolation
//! worktree and finds nothing uncommitted and its head already contained in
//! the line it was to land on; and [`Landed::scratch`], which records that the
//! work is intentionally thrown away and consumes a one-time authority to
//! discard. Retirement never discards uncommitted work by itself, and a
//! refusal is an [`Unlanded`] the steward reports.

use std::fmt;
use std::path::Path;
use std::process::Command;

use crate::authority::{Grant, act};

/// Why work is not landed.
#[derive(Debug, PartialEq, Eq)]
pub enum Unlanded {
    /// The worktree has uncommitted or untracked paths.
    Dirty(Vec<String>),
    /// The worktree's head is not contained in the line it was to land on.
    NotMerged { head: String, into: String },
    /// Git could not be asked.
    Git(String),
}

impl fmt::Display for Unlanded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unlanded::Dirty(p) => write!(f, "uncommitted work: {}", p.join(", ")),
            Unlanded::NotMerged { head, into } => {
                write!(f, "head {head} is not contained in {into}")
            }
            Unlanded::Git(e) => write!(f, "git: {e}"),
        }
    }
}

impl std::error::Error for Unlanded {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum How {
    Merged { head: String, into: String },
    Scratch { grant: String },
}

/// Proof that a task's work has landed or was intentionally discarded. Its
/// only constructors are [`Landed::merged`] and [`Landed::scratch`].
#[must_use = "a Landed is what retirement consumes"]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Landed {
    how: How,
}

fn git(worktree: &Path, args: &[&str]) -> Result<std::process::Output, Unlanded> {
    Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(args)
        .output()
        .map_err(|e| Unlanded::Git(e.to_string()))
}

impl Landed {
    /// The work in `worktree` is clean and its head is an ancestor of (or is)
    /// `into`, a revision that resolves in that worktree.
    pub fn merged(worktree: &Path, into: &str) -> Result<Landed, Unlanded> {
        let status = git(
            worktree,
            &["status", "--porcelain", "--untracked-files=all"],
        )?;
        if !status.status.success() {
            return Err(Unlanded::Git(
                String::from_utf8_lossy(&status.stderr).trim().to_string(),
            ));
        }
        let dirty: Vec<String> = String::from_utf8_lossy(&status.stdout)
            .lines()
            .map(|l| l.get(3..).unwrap_or(l).to_string())
            .collect();
        if !dirty.is_empty() {
            return Err(Unlanded::Dirty(dirty));
        }
        let head = git(worktree, &["rev-parse", "HEAD"])?;
        if !head.status.success() {
            return Err(Unlanded::Git(
                String::from_utf8_lossy(&head.stderr).trim().to_string(),
            ));
        }
        let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
        let anc = git(worktree, &["merge-base", "--is-ancestor", &head, into])?;
        match anc.status.code() {
            Some(0) => Ok(Landed {
                how: How::Merged {
                    head,
                    into: into.to_string(),
                },
            }),
            Some(1) => Err(Unlanded::NotMerged {
                head,
                into: into.to_string(),
            }),
            _ => Err(Unlanded::Git(
                String::from_utf8_lossy(&anc.stderr).trim().to_string(),
            )),
        }
    }

    /// The work is intentionally scratch: the principal's one-time authority
    /// to discard is consumed.
    pub fn scratch(authority: impl Grant<act::Discard>) -> Landed {
        Landed {
            how: How::Scratch {
                grant: authority.grant_id().to_string(),
            },
        }
    }

    /// Words for the record: how it landed.
    pub fn describe(&self) -> String {
        match &self.how {
            How::Merged { head, into } => format!("merged: {head} is in {into}"),
            How::Scratch { grant } => format!("scratch, discarded under grant {grant}"),
        }
    }
}
