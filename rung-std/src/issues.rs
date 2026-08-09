//! Canonical **issues** theory — a body of work items under a ladder.
//!
//! ## What this is
//!
//! A complete Het theory over work items — tasks, defects, requests — the thing
//! a question *becomes* when its authentic cut says *"this was never a
//! determinate question; it is a piece of work."* It is the natural intake
//! destination of a **relegation** from the questions theory (the theory of
//! theories in context). It is deliberately lean: an issue has an id, a status,
//! and a body; it is **well-scoped** (a clear, bounded task) by a judge's ruling.
//!
//! ## The carrier
//!
//! Like `questions`, issues are a **flat, self-describing** carrier: status is
//! frontmatter, not a folder, and the set is a flat pile of `*.md` files. Any
//! concrete issue set (a real GitHub repository, a board, an email queue) is a
//! carrier of this theory; a `GitHubIssuesCarrier` is one concrete backing.

use rung::theory;

pub const STATUSES: &[&str] = &[
    "open",
    "triaged",
    "in-progress",
    "resolved",
    "closed",
    "wontfix",
];

/// This set's coordinates: its provenance namespace, the container it sits in
/// (as named in a standing predicate), and the prefix that marks an id as
/// internal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scheme {
    pub namespace: &'static str,
    pub root: &'static str,
    pub id_prefix: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub scheme: Scheme,
    pub id: String,
    pub status: String,
    /// The directory the file sits in (the container standing is held over).
    pub dir: String,
    /// The filename without `.md`.
    pub stem: String,
    /// The raw body, preserved verbatim for writeback.
    pub body: String,
}

impl rung::Provenanced for Issue {
    fn provenance(&self) -> rung::Prov {
        rung::Prov::of([self.scheme.namespace.to_string(), self.id.clone()])
    }
}
impl rung::Situated for Issue {
    fn container(&self) -> &str {
        &self.dir
    }
}

impl Issue {
    pub fn parse(scheme: Scheme, text: &str, dir: &str, stem: &str) -> Option<Self> {
        let rest = text.strip_prefix("---\n")?;
        let end = rest.find("\n---")?;
        let fm = &rest[..end];
        let body = rest[end + 4..].trim_start().to_string();
        let scalar = |key: &str| -> Option<String> {
            fm.lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix(": ").map(str::trim))
                .map(str::to_string)
        };
        Some(Issue {
            scheme,
            id: scalar("id")?,
            status: scalar("status")?,
            dir: dir.to_string(),
            stem: stem.to_string(),
            body,
        })
    }

    /// Re-render as a markdown file (frontmatter + body), for writeback.
    pub fn to_markdown(&self) -> String {
        format!(
            "---\nid: {}\nstatus: {}\n---\n\n{}\n",
            self.id, self.status, self.body
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueEdit {
    /// Move the issue to a declared status.
    Triage { to: &'static str },
    /// A piece of work is done.
    Resolve,
    /// Close for good (a completed or knowingly-unfixed work item).
    Close,
    /// Re-open a closed issue.
    Reopen,
}

/// The work-item set.
#[derive(Debug, Clone)]
pub struct Issues {
    pub scheme: Scheme,
    pub issues: Vec<Issue>,
    /// The carrier directory, kept so an enacted edit can persist (writeback).
    pub source: Option<std::path::PathBuf>,
}

impl rung_het::Applies<IssueEdit> for Issues {
    fn territory(&self) -> &'static str {
        self.scheme.root
    }
    fn apply(&mut self, object: &str, edit: &IssueEdit) -> Result<(), rung_het::EnactError> {
        let idx = self
            .issues
            .iter()
            .position(|q| q.id == object)
            .ok_or_else(|| rung_het::EnactError::ObjectNotFound {
                object: object.to_string(),
            })?;
        match edit {
            IssueEdit::Triage { to } => {
                if !STATUSES.contains(to) {
                    return Err(rung_het::EnactError::TargetRefused {
                        target: (*to).to_string(),
                        reason: format!("`{to}` is not a declared issue status"),
                    });
                }
                self.issues[idx].status = (*to).to_string();
            }
            IssueEdit::Resolve => {
                self.issues[idx].status = "resolved".into();
            }
            IssueEdit::Close => self.issues[idx].status = "closed".into(),
            IssueEdit::Reopen => self.issues[idx].status = "open".into(),
        }
        Ok(())
    }
}

impl rung_het::Verify<IssueEdit> for Issues {
    fn confirms(&self, edit: &IssueEdit, object: &str) -> bool {
        let Some(idx) = self.issues.iter().position(|q| q.id == object) else {
            return false;
        };
        let q = &self.issues[idx];
        match edit {
            IssueEdit::Triage { to } => q.status == *to,
            IssueEdit::Resolve => q.status == "resolved",
            IssueEdit::Close => q.status == "closed",
            IssueEdit::Reopen => q.status == "open",
        }
    }
}

/// The Issues theory’s intake gate ([`rung_het::Admits`] — the catalog note’s
/// Intake/Discharge). Admission is a **re-audit under the destination law** — a
/// candidate subject is admitted only if it parses as an `Issue` with a
/// non-empty id and a declared status. The source theory may say "not a
/// question"; only this theory can say whether it is a well-formed issue.
impl rung_het::Admits for Issues {
    fn content_is_admissible(&self, content: &str) -> bool {
        match Issue::parse(self.scheme, content, "", self.scheme.id_prefix) {
            Some(it) => !it.id.is_empty() && STATUSES.contains(&it.status.as_str()),
            None => false,
        }
    }

    fn render(&self, content: &str) -> String {
        match Issue::parse(self.scheme, content, "", self.scheme.id_prefix) {
            Some(it) => it.to_markdown(),
            // Unreachable in the driver's contract: `render` is called only on
            // a content the gate just admitted. Preserve it verbatim rather
            // than fabricate.
            None => content.to_string(),
        }
    }
}

impl Issues {
    pub fn new(scheme: Scheme, mut issues: Vec<Issue>) -> Self {
        issues.sort_by(|a, b| a.id.cmp(&b.id));
        Self {
            scheme,
            issues,
            source: None,
        }
    }

    pub fn load(scheme: Scheme, root: &std::path::Path) -> Self {
        let mut issues = Vec::new();
        if let Ok(entries) = std::fs::read_dir(root) {
            for e in entries.flatten() {
                let p = e.path();
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                if name.starts_with('_')
                    || !p.is_file()
                    || !p.extension().is_some_and(|x| x == "md")
                {
                    continue;
                }
                let (text, stem) = (
                    std::fs::read_to_string(&p),
                    p.file_stem().and_then(|s| s.to_str()),
                );
                if let (Ok(text), Some(stem)) = (text, stem)
                    && let Some(mut it) = Issue::parse(scheme, &text, "", stem)
                {
                    it.dir = it.status.clone();
                    issues.push(it);
                }
            }
        }
        issues.sort_by(|a, b| a.id.cmp(&b.id));
        Self {
            scheme,
            issues,
            source: Some(root.to_path_buf()),
        }
    }

    pub fn by_id(&self, id: &str) -> Option<&Issue> {
        self.issues.iter().find(|q| q.id == id)
    }
}

/// `role(well_scoped)` — an issue is a clear, bounded task.
#[derive(Clone, Copy)]
pub struct Reviewer;
impl rung::Role for Reviewer {
    const NAME: &'static str = "reviewer";
}

/// `role(o)` for filing, moving and closing issues — authorial.
#[derive(Clone, Copy)]
pub struct Triager;
impl rung::Role for Triager {
    const NAME: &'static str = "triager";
}

theory!(issue for Issue {
    decidable id_matches_the_filename = |q: &Issue|
        !q.id.is_empty() && q.stem.split('-').next() == Some(q.id.as_str());

    decidable status_is_declared = |q: &Issue|
        STATUSES.contains(&q.status.as_str());

    // Whether an issue is a clear, well-scoped work item — one bounded task,
    // not two, with a reachable definition of "done". No predicate settles it.
    judgmental well_scoped: Reviewer;
});

theory!(issues for Issues {
    decidable ids_are_unique = |qs: &Issues|
        qs.issues
            .iter()
            .map(|q| &q.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == qs.issues.len();
});

/// `Sen(Σ)` for the whole theory, across both sorts.
pub fn sentences() -> Vec<(&'static str, &'static str)> {
    issue::SENTENCES
        .iter()
        .chain(issues::SENTENCES)
        .copied()
        .collect()
}
