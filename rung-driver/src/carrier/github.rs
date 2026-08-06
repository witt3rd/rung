//! [`GitHubIssuesCarrier`] — an **external** carrier backed by GitHub issues,
//! resolved through the `gh` CLI.
//!
//! The interesting case for the carrier setup: it needs **authentication**. It
//! deliberately takes **no secret**: authentication is the ambient `gh` CLI's
//! (its keyring, or `GH_TOKEN`/`GITHUB_TOKEN` in the environment) — the same
//! "no secret lives in config" rule that governs `population.yaml`. A
//! `config.yaml` names only the repositories; the credential is never in it.
//!
//! Content stays **opaque**: we enumerate issue numbers and read issue text via
//! `gh --jq`, so no JSON is parsed here and no field vocabulary is imposed.
//! Item ids are `<owner>/<repo>#<number>`.
//!
//! This is the external half of the carrier story (Q18's "colocated *and*
//! external"): the strategy is the same `Carrier` trait, the resolution is a
//! CLI call instead of a filesystem walk.

use std::process::Command;

use super::{Carrier, CarrierError, ObjectId};

/// Scroll through issues of one or more repositories; issues only for now.
#[derive(Debug, Clone)]
pub struct GitHubIssuesCarrier {
    repos: Vec<String>,
    /// `gh issue list` page size.
    limit: usize,
}

impl GitHubIssuesCarrier {
    pub fn new(repos: Vec<String>) -> Result<Self, String> {
        if repos.is_empty() {
            return Err("a GitHub issues carrier requires at least one repo".into());
        }
        Ok(Self { repos, limit: 200 })
    }

    fn err(&self, reason: impl Into<String>) -> CarrierError {
        CarrierError::new(self.id(), reason)
    }

    fn gh(&self, args: &[&str]) -> Result<String, CarrierError> {
        let out = Command::new("gh").args(args).output().map_err(|e| {
            self.err(format!(
                "gh CLI unavailable (is it installed and authed?): {e}"
            ))
        })?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(self.err(format!(
                "gh {} failed ({}): {}",
                args.join(" "),
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            )))
        }
    }

    /// Enumerate issue numbers for one repo, as opaque lines.
    fn numbers(&self, repo: &str) -> Result<Vec<ObjectId>, CarrierError> {
        let limit = self.limit.to_string();
        let text = self.gh(&[
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            "all",
            "--json",
            "number",
            "--jq",
            ".[].number",
            "--limit",
            limit.as_str(),
        ])?;
        Ok(text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|n| ObjectId::new(format!("{repo}#{n}")))
            .collect())
    }
}

impl Carrier for GitHubIssuesCarrier {
    fn id(&self) -> ObjectId {
        ObjectId::new(format!("github:issues:{}", self.repos.join(",")))
    }

    fn exists(&self) -> bool {
        // accessible iff the ambient gh CLI is present; authorization is tested
        // at use, because a network/auth probe on every `exists()` is heavy.
        Command::new("gh").arg("--version").output().is_ok()
    }

    fn iter(&self) -> Box<dyn Iterator<Item = Result<ObjectId, CarrierError>> + '_> {
        let mut all = Vec::new();
        for repo in &self.repos {
            match self.numbers(repo) {
                Ok(ns) => all.extend(ns.into_iter().map(Ok)),
                Err(e) => all.push(Err(e)),
            }
        }
        Box::new(all.into_iter())
    }

    fn read(&self, item: &ObjectId) -> Result<String, CarrierError> {
        let (repo, number) = item
            .as_str()
            .rsplit_once('#')
            .ok_or_else(|| self.err(format!("{item} is not `owner/repo#number`")))?;
        // opaque: the issue body, nothing parsed here
        let body = self.gh(&[
            "issue", "view", number, "--repo", repo, "--json", "body", "--jq", ".body",
        ])?;
        Ok(body.trim_end().to_string())
    }
}
