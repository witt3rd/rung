//! Records that settle the judgmental fragment.
//!
//! A decidable proposition names a test; a judgmental one names a **judgment
//! record** in `judgments/`. This reads them and checks what can be checked.
//!
//! ## A panel, and no opinion about its size
//!
//! [`Record::judges`] is a list. A panel is `⊨` with more than one judge
//! (`panels`), and the schema carries any number.
//!
//! **Nothing here decides how many are enough.** That a claim warrants a deep
//! panel rather than one reasoning model is a judgment about *worth*, and Het
//! declares no worth law (`het-declares-no-worth-law`) — it belongs to HetOpt,
//! which does not exist. So the count is reported and never required, ranked or
//! preferred. When HetOpt arrives it will find the shape already here.
//!
//! ## Receipt or judgment
//!
//! [`Tier`] is the honest distinction, and it is the same one that separated a
//! `trybuild` case from `(rustc)`: whether the thing establishing a claim can
//! itself fail.
//!
//! A `dispatched` record comes from an actual `Pool::consult`, so the judge's
//! provenance came out of a sealed `Judgment`. An `attested` record is a
//! transcription, and nothing here can tell a faithful one from an invention.

use crate::{Doctrine, Kind};
use std::path::Path;

/// Where a record's authority comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// From a real dispatch through the pool. The provenance is the sealed
    /// `Judgment`'s, not a field someone typed.
    Dispatched,
    /// Written down after a judgment that happened out of band. A receipt.
    Attested,
}

/// One judge's ruling.
#[derive(Clone, Debug)]
pub struct Ruling {
    pub id: String,
    /// What this judge authored. Checked disjoint from the proposition.
    pub provenance: Vec<String>,
    /// `conforming` or `non-conforming` — a judgment may go either way, and
    /// Q7's went against the account it was asked about.
    pub verdict: String,
    /// An honest error bar, where the judge gave one
    /// (`epsilon-reported-with-verdict`).
    pub epsilon: Option<String>,
    pub on: String,
}

/// A judgment record.
#[derive(Clone, Debug)]
pub struct Record {
    pub proposition: String,
    pub role: String,
    pub tier: Tier,
    pub judges: Vec<Ruling>,
    /// The argument. Not a restatement of the verdict — the reasoning that
    /// would let a later reader disagree.
    pub reasoning: String,
    /// Where this was read from.
    pub file: String,
}

/// What is wrong with a record, or with the doctrine's reference to one.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    /// The doctrine names a record that is not there.
    Missing { slug: String, path: String },
    /// A record names a proposition no doctrine declares.
    Unknown { file: String, slug: String },
    /// A record settles a proposition that is not judgmental.
    NotJudgmental { file: String, slug: String },
    /// The record's role is not the one the proposition declares. A judge
    /// competent at something else is not competent at this.
    WrongRole {
        file: String,
        declared: String,
        recorded: String,
    },
    /// **P0.** A judge whose provenance overlaps what it judged.
    NonIdentity {
        file: String,
        judge: String,
        shared: String,
    },
    /// No judge, or a judge with no verdict.
    Empty { file: String, why: String },
    /// A verdict with no argument behind it.
    NoReasoning { file: String },
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { slug, path } => {
                write!(f, "#{slug} names ruling `{path}`, which is not there")
            }
            Self::Unknown { file, slug } => {
                write!(f, "{file}: settles #{slug}, which no doctrine declares")
            }
            Self::NotJudgmental { file, slug } => {
                write!(f, "{file}: #{slug} is not judgmental — nothing to settle")
            }
            Self::WrongRole {
                file,
                declared,
                recorded,
            } => write!(
                f,
                "{file}: the proposition declares `{declared}`, the record says `{recorded}`"
            ),
            Self::NonIdentity {
                file,
                judge,
                shared,
            } => write!(
                f,
                "{file}: `{judge}` authored `{shared}` and may not judge it (P0)"
            ),
            Self::Empty { file, why } => write!(f, "{file}: {why}"),
            Self::NoReasoning { file } => {
                write!(f, "{file}: a verdict with no reasoning behind it")
            }
        }
    }
}

/// Read every record in a directory.
pub fn read_all(dir: &Path) -> Vec<Record> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Record> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .filter(|p| p.file_name().is_some_and(|n| n != "README.md"))
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            parse(&text, &p.file_name()?.to_string_lossy())
        })
        .collect();
    out.sort_by(|a, b| a.file.cmp(&b.file));
    out
}

/// Parse one record. Frontmatter between `---` fences, then the reasoning.
fn parse(text: &str, file: &str) -> Option<Record> {
    let body = text.strip_prefix("---\n")?;
    let (front, reasoning) = body.split_once("\n---\n")?;

    let mut proposition = String::new();
    let mut role = String::new();
    let mut tier = Tier::Attested;
    let mut judges: Vec<Ruling> = Vec::new();

    for line in front.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("proposition:") {
            proposition = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("role:") {
            role = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("tier:") {
            tier = if v.trim() == "dispatched" {
                Tier::Dispatched
            } else {
                Tier::Attested
            };
        } else if let Some(v) = t.strip_prefix("- id:") {
            judges.push(Ruling {
                id: v.trim().to_string(),
                provenance: Vec::new(),
                verdict: String::new(),
                epsilon: None,
                on: String::new(),
            });
        } else if let Some(j) = judges.last_mut() {
            if let Some(v) = t.strip_prefix("provenance:") {
                j.provenance = v
                    .trim()
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            } else if let Some(v) = t.strip_prefix("verdict:") {
                j.verdict = v.trim().to_string();
            } else if let Some(v) = t.strip_prefix("epsilon:") {
                j.epsilon = Some(v.trim().to_string());
            } else if let Some(v) = t.strip_prefix("on:") {
                j.on = v.trim().to_string();
            }
        }
    }

    Some(Record {
        proposition,
        role,
        tier,
        judges,
        reasoning: reasoning.trim().to_string(),
        file: file.to_string(),
    })
}

/// Every fault, across the doctrine and the records.
///
/// Checks what a machine can: that a record settles something that exists and
/// is judgmental, that the role matches, that no judge ruled on its own work,
/// and that a verdict has an argument behind it.
///
/// It cannot check that the judge said it, or that the reasoning is sound. The
/// first is what [`Tier::Dispatched`] is for; the second is what a judge is for.
pub fn check(doctrines: &[Doctrine], records: &[Record], dir: &Path) -> Vec<Fault> {
    let mut faults = Vec::new();

    for d in doctrines {
        for p in d.props() {
            if let Kind::Judgmental {
                ruling: Some(path), ..
            } = &p.kind
                && !dir.join(path).exists()
            {
                faults.push(Fault::Missing {
                    slug: p.slug.clone(),
                    path: path.clone(),
                });
            }
        }
    }

    for r in records {
        let prop = doctrines
            .iter()
            .flat_map(|d| d.props())
            .find(|p| p.slug == r.proposition);
        let Some(prop) = prop else {
            faults.push(Fault::Unknown {
                file: r.file.clone(),
                slug: r.proposition.clone(),
            });
            continue;
        };
        let Kind::Judgmental { role, .. } = &prop.kind else {
            faults.push(Fault::NotJudgmental {
                file: r.file.clone(),
                slug: r.proposition.clone(),
            });
            continue;
        };
        if role != &r.role {
            faults.push(Fault::WrongRole {
                file: r.file.clone(),
                declared: role.clone(),
                recorded: r.role.clone(),
            });
        }
        if r.judges.is_empty() {
            faults.push(Fault::Empty {
                file: r.file.clone(),
                why: "no judge — a record settles nothing on its own".into(),
            });
        }
        for j in &r.judges {
            if j.verdict.is_empty() {
                faults.push(Fault::Empty {
                    file: r.file.clone(),
                    why: format!("`{}` rendered no verdict", j.id),
                });
            }
            // P0, against the proposition being judged.
            if let Some(shared) = j.provenance.iter().find(|s| *s == &r.proposition) {
                faults.push(Fault::NonIdentity {
                    file: r.file.clone(),
                    judge: j.id.clone(),
                    shared: shared.clone(),
                });
            }
        }
        if r.reasoning.is_empty() {
            faults.push(Fault::NoReasoning {
                file: r.file.clone(),
            });
        }
    }
    faults
}
