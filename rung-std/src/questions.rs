//! Canonical **questions** theory — the second building block in rung-std.
//!
//! ## What this is
//!
//! A complete Het theory over a body of open questions: the sorts, the
//! gate-marked sentences, the roles, the typed dependency edges and what each
//! propagates, the edits, and the lifecycle ladder that carries a question from
//! *open* to a verdict. Nothing here knows any particular body of questions.
//!
//! The shape recurs wherever questions are **posed**, **ruled on by an outside
//! panel**, and **folded back through a lifecycle**: an architecture decision
//! log, a research docket, a standards body's open issues, a review queue. Each
//! instance differs in its ids, its files, and its edges; none of them differs
//! in the *theory*.
//!
//! ## Membership criteria (rung-std)
//!
//! `LlmCall` is rung-std because its two-rung shape recurs across independent
//! domain projects with no caller-specific knowledge embedded. This is the
//! second block admitted on the same test, and it earns it on three counts:
//!
//! 1. **Two carriers already fill it.** rung's own `questions/` tree, read
//!    off disk by `rung-het/tests/questions_of_rung.rs`, and a synthetic decision
//!    docket with a disjoint id space, a disjoint edge set and a different
//!    lifecycle path, in `rung-std/tests/questions_theory.rs`. A theory with one
//!    carrier is a domain model wearing a library's name.
//! 2. **Every deployment coordinate is a parameter.** [`Scheme`] carries the
//!    three things an instance owns — its provenance namespace, the container it
//!    sits in, and the prefix that marks an id as internal. There is no literal
//!    in this file that names anyone's questions.
//! 3. **It fills Het's slots and no more.** Het declares the slots
//!    (`het-declares-the-slots`); a theory fills them
//!    (`theory-declares-four-things`) — sorts, edits, gate-marked sentences, and
//!    a role for each judgmental sentence. rung-CT adds a fifth at the dependency
//!    level: the **edge taxonomy**, which is the governing theory's exactly as an
//!    edit vocabulary is (`edge-taxonomy-is-the-theorys`). The seven kinds below
//!    are this theory's. `rung` and `rung-het` have never heard of `premise`.
//!
//! ## Two sorts, and why
//!
//! `question` states what one question must satisfy; `questions` states what a
//! *relation between* questions must satisfy. That is two sorts of one theory,
//! not two theories — Het's `theory-declares-four-things` says *sorts*, plural.
//! The split into two `theory!` invocations is forced by the DSL, which declares
//! one sort per invocation and emits `SENTENCES` per module; `Sen(Σ)` for this
//! theory is therefore a hand-written concatenation. Recorded as a DSL limit,
//! not chosen as a design.
//!
//! It would be wrong to push `ids_are_unique` down onto `Question` — no
//! individual question can see it — and equally wrong to lift `is_well_posed`
//! up onto the whole set, because a judge rules on *one* question and the
//! `Qualified` token is minted against that one question's `π(a)`. The two sorts
//! are the two things a principal is ever handed.
//!
//! ## What this theory could not say
//!
//! Limits found by using it, recorded rather than worked around:
//!
//! 1. **A `theory!` declares one sort** — see above.
//! 2. **A decidable body returns `bool`.** The failure reason is built from
//!    `stringify!($sentence)`, so a sentence that fails over five items cannot
//!    say *which five*. [`Questions::outbound_drift`] is therefore a plain method,
//!    and the sentence supplies only the verdict.
//! 3. **Verdicts are Boolean.** No metric `d`, no `ε`, so a ruling on an
//!    advisory edge carries no confidence. "This dependent probably survives" is
//!    not expressible.
//! 4. **`#[conditional(..)]` is a parse-time refusal.** [`EdgeKind::Gate`] —
//!    whose whole question is *"has this lifted?"* — is settled per model, which
//!    is exactly Het's conditional gate (`classifier-one-level-up`). `ladder!`
//!    refuses the marker, so this theory classifies `gate` as advisory and says
//!    so here rather than inventing one.

// The declaration surface — `ladder!` (arrows) and `theory!` (sentences) — and
// the pool that mints both capability tokens come from `rung`. Only the *pass*
// comes from `rung-het`: `Applies` is how a theory says what its own edits do
// (`enact-generic-over-edit`), and `enact` refuses to guess.
use rung::{
    Authorized, Pool, Principal, Prov, Provenanced, QualifyError, Role, Settled, Situated, ladder,
    theory,
};
use rung_het::{Applies, EnactError, Verify};
use std::collections::BTreeSet;
use std::path::Path;

// ═════════════════════════════════════════════════════════════════════════
// 0. The deployment parameters — everything an instance owns
// ═════════════════════════════════════════════════════════════════════════

/// The three coordinates a concrete body of questions supplies.
///
/// Held as data rather than as generics because all three are read at runtime
/// by `π`, by the standing predicate, and by the internal-target rule; and
/// because `Applies::territory` returns `&'static str`, so the container name
/// must be one.
///
/// A theory whose provenance tag, container name, or id convention were
/// literals in the library would be a domain model, not a building block. These
/// are what make the difference visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scheme {
    /// The provenance tag every question in this set carries — `π`'s coarse
    /// half. Everyone who files into the set is tagged with it, and is
    /// therefore refused as a judge of everything in it, which is the correct
    /// outcome.
    pub namespace: &'static str,
    /// The container the whole set is, as named in a standing
    /// predicate and as `Applies::territory`.
    pub root: &'static str,
    /// The prefix that marks an id as naming another question *in the same set*.
    /// An id is internal iff it is this prefix followed by digits.
    pub id_prefix: &'static str,
}

/// Does this edge target name another question in the same set?
///
/// With `id_prefix = "q"`: `q7` is internal; `q9-reviews`, `RUNG-CT§6` and
/// `SPEC:G2` are not. The frontmatter format does **not** mark internal against
/// external, so a collective anchor and a typo'd id are indistinguishable
/// except by this rule. A data-format gap, recorded rather than papered over.
pub fn is_internal_id(scheme: Scheme, s: &str) -> bool {
    let Some(rest) = s.strip_prefix(scheme.id_prefix) else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

// ═════════════════════════════════════════════════════════════════════════
// 1. The theory's edge taxonomy — not the library's
// ═════════════════════════════════════════════════════════════════════════

/// **This theory's edge taxonomy** (`edge-taxonomy-is-the-theorys`).
///
/// rung-CT states that an edge type *selects a pushforward*
/// (`edge-type-selects-the-pushforward`) and deliberately never enumerates the
/// types, for the same reason Het never enumerates edits
/// (`governs-who-not-what`). The enumeration is a theory's to make, and this is
/// the theory making it.
///
/// The **lived-instance** discipline is what stops this list growing
/// speculatively: a kind stays in the vocabulary only while some question in
/// the set under audit actually uses it. It is checkable —
/// [`Questions::kinds_in_use`] — and both carriers check it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeKind {
    /// The dependent rests on this as a premise. It was *wrong* until the
    /// change is folded in.
    Premise,
    /// The dependent was motivated by this and stands on its own. It may
    /// survive untouched.
    Justification,
    /// The dependent exists only because this resolved.
    Spawn,
    /// A blocker. Whether it has lifted is settled per-model.
    Gate,
    /// A reference to fix. Mechanical.
    Citation,
    /// Inbound support. Informational.
    Evidence,
    /// See-also.
    Related,
}

/// `Sen(Σ)`'s sibling at the dependency level: the vocabulary as data, so a
/// change to it breaks a test rather than passing silently.
pub const EDGE_KINDS: &[EdgeKind] = &[
    EdgeKind::Premise,
    EdgeKind::Justification,
    EdgeKind::Spawn,
    EdgeKind::Gate,
    EdgeKind::Citation,
    EdgeKind::Evidence,
    EdgeKind::Related,
];

impl EdgeKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "premise" => Self::Premise,
            "justification" => Self::Justification,
            "spawn" => Self::Spawn,
            "gate" => Self::Gate,
            "citation" => Self::Citation,
            "evidence" => Self::Evidence,
            "related" => Self::Related,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Premise => "premise",
            Self::Justification => "justification",
            Self::Spawn => "spawn",
            Self::Gate => "gate",
            Self::Citation => "citation",
            Self::Evidence => "evidence",
            Self::Related => "related",
        }
    }
}

/// What the pushforward along an edge does
/// (`edge-type-selects-the-pushforward`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Propagation {
    /// Lifts to an **obligation**. The dependent must be re-examined.
    Strict,
    /// Lifts to a **coproduct** — *review required* + *survives*
    /// (`advisory-lift-lands-in-a-coproduct`). Which arm is taken is not the
    /// edge's to decide.
    Advisory,
    /// Lifts to the dependent's **existence**.
    Generative,
    /// Lifts to a state update with **no outside**.
    Mechanical,
    /// Surfaced once, chased no further.
    Inert,
}

/// **The one line the strict/advisory test defends.**
///
/// `strict-and-advisory-are-the-gate`: the obligatory/advisory split *is* the
/// gate marker read one level up. Moving [`EdgeKind::Justification`] into the
/// `Strict` arm is type-valid and turns the strict/advisory test red in
/// *both* carriers — which is what makes that test a test, and what makes one
/// theory serving two carriers observable.
pub const fn propagation_of(kind: EdgeKind) -> Propagation {
    match kind {
        EdgeKind::Premise => Propagation::Strict,
        EdgeKind::Justification => Propagation::Advisory,
        // `gate` is Het's *conditional* case — "has this lifted?" is settled per
        // model, one level up (`classifier-one-level-up`). `ladder!` refuses
        // `#[conditional(..)]`, so this theory classifies it advisory and
        // records the approximation rather than inventing a marker.
        EdgeKind::Gate => Propagation::Advisory,
        EdgeKind::Spawn => Propagation::Generative,
        EdgeKind::Citation => Propagation::Mechanical,
        EdgeKind::Evidence | EdgeKind::Related => Propagation::Inert,
    }
}

/// The gate each propagation is settled under —
/// `strict-and-advisory-are-the-gate` made into a lookup.
///
/// rung-CT legislates only the strict/advisory pair. `generative` → `authorial`
/// is **this theory's** reading (a spawned question is *authored* into the
/// set by someone with standing over it, not classified), and is marked as
/// such rather than attributed upward.
pub const fn gate_of(kind: EdgeKind) -> &'static str {
    match propagation_of(kind) {
        Propagation::Strict | Propagation::Mechanical => "decidable",
        Propagation::Advisory => "judgmental",
        Propagation::Generative => "authorial",
        Propagation::Inert => "none",
    }
}

/// Does a change ripple *through* this edge to the dependent's own dependents?
pub const fn recurses(kind: EdgeKind) -> bool {
    matches!(
        propagation_of(kind),
        Propagation::Strict | Propagation::Advisory | Propagation::Generative
    )
}

// ═════════════════════════════════════════════════════════════════════════
// 2. The first sort — one question
// ═════════════════════════════════════════════════════════════════════════

/// The declared status vocabulary, which is the lifecycle ladder's rungs read
/// as data. Four of the five have a directory; `dissolved` does not — a
/// question that dissolves is deleted, not filed — which
/// status is frontmatter-canonical, so the docket is flat.
pub const STATUSES: &[&str] = &["open", "blocked", "parked", "resolved", "dissolved"];

/// The status whose directory runs a write-guard: the done-pile.
pub const RESOLVED: &str = "resolved";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub target: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// The set this question belongs to.
    pub scheme: Scheme,
    /// The frontmatter `id`.
    pub id: String,
    /// The frontmatter `status`.
    pub status: String,
    /// The directory the file sits in.
    pub dir: String,
    /// The filename without `.md`.
    pub stem: String,
    pub depends_on: Vec<Edge>,
    pub affects: Vec<Edge>,
    /// The declared resolution / adequacy criterion — what would count as an
    /// answer. The structural anchor of well-posedness: existence and
    /// uniqueness are judged *against this*, and whether it is declared is the
    /// one checkable cut (`answerable_is_declared`). A question that does not
    /// declare one is either not yet well-posed, or is being tracked precisely
    /// to repair it into well-posedness (see `is_well_posed`).
    pub answerable: Option<String>,
}

/// **π for a question.**
///
/// The frontmatter has no `author:` field, so the finest provenance available
/// is *the set, and this question within it*. Coarse, but not vacuous: a
/// principal tagged with the set's namespace — which every curator is — is
/// refused as a judge of every question in it, which is the correct outcome.
impl Provenanced for Question {
    fn provenance(&self) -> Prov {
        Prov::of([self.scheme.namespace.to_string(), self.id.clone()])
    }
}

/// **Where a question sits** — the status directory, which is exactly what a
/// curator holds standing over. Moving a question between directories is the
/// set's one real edit, and the pen is minted per directory.
impl Situated for Question {
    fn container(&self) -> &str {
        &self.dir
    }
}

impl Question {
    /// Edge targets that name another question in the same set.
    pub fn internal_depends_on(&self) -> Vec<(&str, &str)> {
        self.depends_on
            .iter()
            .filter(|e| is_internal_id(self.scheme, &e.target))
            .map(|e| (e.target.as_str(), e.kind.as_str()))
            .collect()
    }

    /// Whether this question declares a resolution / adequacy criterion —
    /// the structural anchor of well-posedness (`answerable_is_declared`).
    pub fn declares_resolution(&self) -> bool {
        self.answerable
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty())
    }

    pub fn names_in_affects(&self, id: &str) -> bool {
        self.affects.iter().any(|e| e.target == id)
    }

    pub fn has_edge_kind(&self, kind: EdgeKind) -> bool {
        self.depends_on
            .iter()
            .chain(self.affects.iter())
            .any(|e| e.kind == kind.name())
    }

    /// Parse one question from its frontmatter.
    ///
    /// The format is `id`, `status`, and blocks of `- {on: X, kind: Y}` /
    /// `- {target: X, kind: Y}`. Stdlib only, deliberately: the whole
    /// discipline is *clone and read, no service to run*, and a YAML dependency
    /// here would be the first crack in it.
    ///
    /// `dir` and `stem` are the file's coordinates, supplied by the caller —
    /// this function never touches a filesystem.
    pub fn parse(scheme: Scheme, text: &str, dir: &str, stem: &str) -> Option<Self> {
        let rest = text.strip_prefix("---\n")?;
        let end = rest.find("\n---")?;
        let fm = &rest[..end];

        let scalar = |key: &str| -> Option<String> {
            fm.lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix(": ").map(str::trim))
                .map(str::to_string)
        };

        let block = |field: &str, key: &str| -> Vec<Edge> {
            let mut out = Vec::new();
            let mut inside = false;
            for line in fm.lines() {
                if line.trim_end() == format!("{field}:") {
                    inside = true;
                    continue;
                }
                if inside && !line.starts_with("  - ") {
                    inside = false;
                }
                if !inside {
                    continue;
                }
                let inner = line
                    .trim()
                    .trim_start_matches("- ")
                    .trim_matches(['{', '}']);
                let mut target = None;
                let mut kind = None;
                for part in inner.split(',') {
                    let Some((k, v)) = part.split_once(':').map(|(k, v)| (k.trim(), v.trim()))
                    else {
                        continue;
                    };
                    if k == key {
                        target = Some(v.to_string());
                    } else if k == "kind" {
                        kind = Some(v.to_string());
                    }
                }
                if let (Some(target), Some(kind)) = (target, kind) {
                    out.push(Edge { target, kind });
                }
            }
            out
        };

        // Well-posedness frontmatter: `answerable:` as a single line, or a
        // `|`-block scalar carrying the resolution / adequacy criterion.
        let answerable = {
            if let Some(v) = scalar("answerable") {
                Some(v)
            } else {
                let mut inside = false;
                let mut lines: Vec<&str> = Vec::new();
                for line in fm.lines() {
                    if line.trim_end() == "answerable:" || line.trim_end() == "answerable: |" {
                        inside = true;
                        continue;
                    }
                    if inside {
                        if !line.starts_with("  ") {
                            break;
                        }
                        lines.push(line.trim());
                    }
                }
                if lines.is_empty() {
                    None
                } else {
                    Some(lines.join("\n"))
                }
            }
        };

        Some(Question {
            scheme,
            id: scalar("id")?,
            status: scalar("status")?,
            dir: dir.to_string(),
            stem: stem.to_string(),
            depends_on: block("depends_on", "on"),
            affects: block("affects", "target"),
            answerable,
        })
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 3. The second sort — every question at once
// ═════════════════════════════════════════════════════════════════════════

/// The collection of questions, as one model.
///
/// A decidable body takes only its model, so a sentence about a *relation
/// between* questions cannot be stated over one question. It is stated over the
/// carrier instead — the second sort of this theory.
#[derive(Debug, Clone)]
pub struct Questions {
    pub scheme: Scheme,
    pub questions: Vec<Question>,
}

impl Provenanced for Questions {
    fn provenance(&self) -> Prov {
        Prov::of([self.scheme.namespace])
    }
}

impl Situated for Questions {
    fn container(&self) -> &str {
        self.scheme.root
    }
}

impl Questions {
    pub fn new(scheme: Scheme, mut questions: Vec<Question>) -> Self {
        questions.sort_by(|a, b| a.id.cmp(&b.id));
        Self { scheme, questions }
    }

    pub fn by_id(&self, id: &str) -> Option<&Question> {
        self.questions.iter().find(|q| q.id == id)
    }

    /// Internal `depends_on` targets that name no question in the set.
    pub fn dangling_dependencies(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for q in &self.questions {
            for (target, _) in q.internal_depends_on() {
                if self.by_id(target).is_none() {
                    out.push((q.id.clone(), target.to_string()));
                }
            }
        }
        out
    }

    pub fn duplicate_ids(&self) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let mut dup = BTreeSet::new();
        for q in &self.questions {
            if !seen.insert(q.id.clone()) {
                dup.insert(q.id.clone());
            }
        }
        dup.into_iter().collect()
    }

    /// How many internal edges there are at all — the denominator any drift
    /// report needs to mean anything.
    pub fn internal_edge_count(&self) -> usize {
        self.questions
            .iter()
            .map(|q| q.internal_depends_on().len())
            .sum()
    }

    /// Which declared kinds actually occur, and on which questions.
    ///
    /// The **lived-instance** discipline as data: a kind with an empty entry is
    /// a speculative edge type.
    pub fn kinds_in_use(&self) -> Vec<(EdgeKind, Vec<String>)> {
        EDGE_KINDS
            .iter()
            .map(|k| {
                let users: Vec<String> = self
                    .questions
                    .iter()
                    .filter(|q| q.has_edge_kind(*k))
                    .map(|q| q.id.clone())
                    .collect();
                (*k, users)
            })
            .collect()
    }

    /// **Outbound edge drift.** Every inbound `depends_on` should be mirrored
    /// by an outbound `affects` on the target — that is what `affects` is *for*:
    /// "the things that rest on this item."
    ///
    /// Returns `(source, dependent, kind)` for every inbound edge the source
    /// does not acknowledge, sorted.
    pub fn outbound_drift(&self) -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        for q in &self.questions {
            for (target, kind) in q.internal_depends_on() {
                let Some(src) = self.by_id(target) else {
                    continue; // reported by `dangling_dependencies`
                };
                if !src.names_in_affects(&q.id) {
                    out.push((src.id.clone(), q.id.clone(), kind.to_string()));
                }
            }
        }
        out.sort();
        out
    }

    /// Cycles in the **`gate` sub-graph** — the one edge kind whose cycle is a
    /// deadlock.
    ///
    /// A `gate` edge means *blocked by*: the dependent cannot proceed until the
    /// target lifts. A cycle among them is a set of questions each waiting on
    /// another, and no principal can move any of them. That is a deadlock, not
    /// a slow answer, and no ruling resolves it.
    ///
    /// **Mixed-kind cycles are not faults and must not be reported.** Answering
    /// a question routinely raises a second whose framing rests on the first:
    /// the sub-question takes a `premise` edge *upward* while the parent takes
    /// a `gate` edge *downward*. Opposite directions, different kinds — that is
    /// what healthy nesting looks like, and this repository's own Q11 and Q12
    /// are exactly that shape. A traversal over mixed kinds needs a visited
    /// set; it does not need a prohibition.
    ///
    /// Each cycle is returned once, as the ids on it.
    pub fn gate_cycles(&self) -> Vec<Vec<String>> {
        fn walk(
            qs: &Questions,
            id: &str,
            stack: &mut Vec<String>,
            done: &mut BTreeSet<String>,
            out: &mut Vec<Vec<String>>,
        ) {
            if done.contains(id) {
                return;
            }
            if let Some(at) = stack.iter().position(|x| x == id) {
                out.push(stack[at..].to_vec());
                return;
            }
            stack.push(id.to_string());
            if let Some(q) = qs.by_id(id) {
                for e in &q.depends_on {
                    if e.kind == EdgeKind::Gate.name() && is_internal_id(qs.scheme, &e.target) {
                        walk(qs, &e.target, stack, done, out);
                    }
                }
            }
            stack.pop();
            done.insert(id.to_string());
        }

        let mut out = Vec::new();
        let mut done = BTreeSet::new();
        for q in &self.questions {
            walk(self, &q.id, &mut Vec::new(), &mut done, &mut out);
        }
        // One entry point per cycle: dedupe on the member set.
        let mut seen = BTreeSet::new();
        out.retain(|c| seen.insert(c.iter().cloned().collect::<BTreeSet<_>>()));
        out
    }

    /// Read a set of questions from a directory tree of markdown files.
    ///
    /// One question per `*.md`; the directory it sits in is its `dir`. Files
    /// and directories whose name begins with `_` are skipped — the near
    /// universal convention for "not a node" in a tree of this shape.
    ///
    /// The root is a parameter. This function knows no path.
    pub fn load(scheme: Scheme, root: &Path) -> Self {
        // **Flat, and structure is metadata.** Status comes from each
        // question's own frontmatter — never from a folder — so the carrier is
        // a flat set of `*.md` files and `dir` mirrors `status` for standing.
        let mut questions = Vec::new();
        let Ok(entries) = std::fs::read_dir(root) else {
            return Self::new(scheme, questions);
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if name.starts_with('_') || !p.is_file() {
                continue;
            }
            if !p.extension().is_some_and(|x| x == "md") {
                continue;
            }
            let (Ok(text), Some(stem)) = (
                std::fs::read_to_string(&p),
                p.file_stem().and_then(|s| s.to_str()),
            ) else {
                continue;
            };
            if let Some(mut q) = Question::parse(scheme, &text, "", stem) {
                // `dir` mirrors the frontmatter status, not a folder: the
                // container a question sits in is its status as declared.
                q.dir = q.status.clone();
                questions.push(q);
            }
        }
        Self::new(scheme, questions)
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 4. Roles — `role(φ)` for the judgmental sentences, `role(o)` for authorship
// ═════════════════════════════════════════════════════════════════════════

/// The authorial competence — `role(o)`. Who may file, move, and reword a
/// question.
#[derive(Clone, Copy)]
pub struct Curator;
impl Role for Curator {
    const NAME: &'static str = "curator";
}

/// `role(is_well_posed)`. Two judgmental sentences, two roles: Het requires
/// that a role be *declared*, never that it come from a list
/// (`role-declared-not-enumerated`).
#[derive(Clone, Copy)]
pub struct Interrogator;
impl Role for Interrogator {
    const NAME: &'static str = "interrogator";
}

/// `role(resolution_answers_the_question)` and `role(survives_the_change)`.
#[derive(Clone, Copy)]
pub struct Adjudicator;
impl Role for Adjudicator {
    const NAME: &'static str = "adjudicator";
}

// ═════════════════════════════════════════════════════════════════════════
// 5. The sentences
// ═════════════════════════════════════════════════════════════════════════

theory!(question for Question {
    // The id is non-empty and is the filename's first segment.
    decidable id_matches_the_filename = |q: &Question|
        !q.id.is_empty() && q.stem.split('-').next() == Some(q.id.as_str());

    // The status is one of the five declared.
    decidable status_is_declared = |q: &Question|
        STATUSES.contains(&q.status.as_str());

        // Every edge kind this question uses is in the theory's declared taxonomy.
    decidable edge_kinds_are_declared = |q: &Question|
        q.depends_on.iter().chain(q.affects.iter())
            .all(|e| EdgeKind::parse(&e.kind).is_some());

    // The cold first cut of well-posedness: the question must declare what
    // would count as an answer (`answerable:`), or it is not yet a member of
    // the question set — either not a question, or tracked precisely to be
    // repaired. Decidable, because it reads the declaration and nothing else.
    decidable answerable_is_declared = |q: &Question|
        q.declares_resolution();

    // **Well-posedness is the membership criterion of the question set** (`is_well_posed`,
    // in Hadamard's sense, transplanted). A question is well-posed iff, against
    // its declared `answerable:` (the resolution / adequacy criterion), all four
    // cuts hold:
    //
    //   1. **existence** — the structure can actually produce the declared
    //      answer; the resolution condition is reachable from the substance,
    //      not merely nameable.
    //   2. **unique** — exactly one answer, not a family of framings. Watch:
    //      an unpinned equivalence relation ("is X the same as Y" — same in
    //      which sense?), a "what is X" with no pinned adequacy criterion, or a
    //      definitional commitment dressed as a discovery.
    //   3. **stable** — the answer survives rephrasing the question; and
    //      well-posedness itself is stable, so sharpening a well-posed
    //      question never changes *which* question it is.
    //   4. **authentic** — it is a question, not a decision awaiting a ruling,
    //      not a definitional commitment, not a work item. A well-posed
    //      question's answer is *found by the structure*, not *made by the
    //      asker*.
    //
    // The one decidable cut is `answerable_is_declared`; the other three are
    // judgmental, but they are judged *against the declared criterion*, which
    // is what makes the judgment non-vacuous. A ruling that refuses must name
    // the cut it refused (its `reason`).
    judgmental is_well_posed: Interrogator;

    // A resolved question claims a verdict. Does the verdict answer the
    // question that was asked?
    judgmental resolution_answers_the_question: Adjudicator;
});

theory!(questions for Questions {
    // Every internal `depends_on` target resolves to a question that exists.
    decidable every_dependency_resolves = |qs: &Questions|
        qs.dangling_dependencies().is_empty();

    decidable ids_are_unique = |qs: &Questions|
        qs.duplicate_ids().is_empty();

    // Every declared edge kind has a lived instance — the discipline that keeps
    // the taxonomy the theory's rather than the library's, stated as a sentence
    // of the theory rather than as prose.
    decidable every_declared_kind_is_lived = |qs: &Questions|
        qs.kinds_in_use().iter().all(|(_, users)| !users.is_empty());

    // Every inbound edge is acknowledged by its source's `affects`.
    decidable affects_mirrors_inbound = |qs: &Questions|
        qs.outbound_drift().is_empty();

    // No question is blocked, transitively, on itself. Scoped to `gate` because
    // that is the only kind whose cycle is a deadlock — see `gate_cycles`, which
    // says why a mixed-kind cycle is nesting rather than a fault.
    decidable gate_edges_are_acyclic = |qs: &Questions|
        qs.gate_cycles().is_empty();
});

// ═════════════════════════════════════════════════════════════════════════
// 6. Propagation — the same change over two edges, down two paths
// ═════════════════════════════════════════════════════════════════════════

/// One dependent's exposure to one change along one typed edge.
///
/// This is the *model* the propagation sentences are evaluated over: a
/// dependency edge is a dependent optic (`edges-are-dependent-optics`), and the
/// thing to be settled is what the backward pass owes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exposure {
    pub scheme: Scheme,
    pub edge: EdgeKind,
    /// The item that changed.
    pub source: String,
    /// The item that rests on it.
    pub dependent: String,
    /// What happened at the source.
    pub change: String,
}

impl Provenanced for Exposure {
    fn provenance(&self) -> Prov {
        Prov::of([self.scheme.namespace.to_string(), self.dependent.clone()])
    }
}

theory!(propagation for Exposure {
    // **The strict lift.** Machine-checked from the edge type alone: no
    // principal is consulted, and `holds` has no parameter one could enter
    // through. A `premise` edge obliges re-examination and nobody rules on it.
    decidable must_reexamine = |e: &Exposure|
        propagation_of(e.edge) == Propagation::Strict;

    // **The advisory lift.** The coproduct `ReviewRequired + Survives`, and
    // collapsing it is not the edge's work — a judge disjoint from the
    // dependent rules. `Verdict::Conforming` is *survives*; non-conforming is
    // *review required*.
    judgmental survives_the_change: Adjudicator;
});

/// Which path a change took along one edge. The variants are the *observable*
/// difference between the two gates: `Reexamined` never names a principal
/// because none was consulted, and `Ruled` always does.
#[derive(Debug)]
pub enum Propagated {
    /// Strict — settled inside the algebra.
    Reexamined(Settled),
    /// Advisory — settled by a qualified outside.
    Ruled(Settled),
    /// Mechanical — a reference to fix, no outside.
    Mechanical(Settled),
    /// Generative — the dependent exists because of the source; an author with
    /// standing revisits it. Not settled here: authorship is not classification.
    Authored { dependent: String },
    /// Surfaced, not chased.
    Inert,
}

/// The theory's pass over one edge, dispatched on the **edge type's gate**.
///
/// `strict-and-advisory-are-the-gate` as a function. The pool is in scope for
/// the whole function and is *unreachable* from the strict branch — not by
/// discipline, but because `must_reexamine::holds` takes one argument.
///
/// **No verdict parameter.** There used to be one: `propagate(e, pool, ruling)`
/// took the advisory ruling from its caller and handed it to `settle`, so a
/// driver could decide the outcome and the receipt would name whichever
/// principal the filter happened to select. `Pool::consult` asks the principal
/// that qualified, and the answer is what lands on the receipt.
pub fn propagate<P: Principal>(e: &Exposure, pool: &Pool<P>) -> Result<Propagated, QualifyError> {
    Ok(match propagation_of(e.edge) {
        Propagation::Strict => Propagated::Reexamined(propagation::must_reexamine::holds(e)),
        Propagation::Mechanical => Propagated::Mechanical(propagation::must_reexamine::holds(e)),
        Propagation::Advisory => {
            let (q, judgment) = pool.consult::<Adjudicator>(e, "survives_the_change")?;
            Propagated::Ruled(
                propagation::survives_the_change::settle(e, q, judgment)
                    .expect("the licence and the judgment are the same principal's"),
            )
        }
        Propagation::Generative => Propagated::Authored {
            dependent: e.dependent.clone(),
        },
        Propagation::Inert => Propagated::Inert,
    })
}

// ═════════════════════════════════════════════════════════════════════════
// 7. The theory's edits, and the write-guard on the done-pile
// ═════════════════════════════════════════════════════════════════════════

/// **This theory's edits** (`edit-required-not-typed`). Not Het's.
///
/// `Dissolve` is the case with no analogue in an issue tracker or a cabinet: it
/// removes a question on the ground that it was the *wrong question* — the
/// answer is a diagnosis, never a resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestionEdit {
    Relocate { to: &'static str },
    Dissolve { why: &'static str },
    AddEdge { target: String, kind: EdgeKind },
}

impl Applies<QuestionEdit> for Questions {
    fn territory(&self) -> &'static str {
        self.scheme.root
    }

    fn apply(&mut self, object: &str, edit: &QuestionEdit) -> Result<(), EnactError> {
        let idx = self
            .questions
            .iter()
            .position(|q| q.id == object)
            .ok_or_else(|| EnactError::ObjectNotFound {
                object: object.to_string(),
            })?;

        match edit {
            QuestionEdit::Relocate { to } => {
                // **The write-guard** (`target-runs-its-own-models`). The
                // done-pile runs its own law at its boundary: a question with
                // work still owed is in the wrong folder, enforced here as *a
                // resolved question cites its evidence*. An authorization to
                // edit is not a licence to violate the destination's law.
                if *to == RESOLVED && !self.questions[idx].has_edge_kind(EdgeKind::Evidence) {
                    return Err(EnactError::TargetRefused {
                        target: (*to).to_string(),
                        reason: format!("{object} cites no evidence; {to}/ is a done-pile"),
                    });
                }
                self.questions[idx].dir = (*to).to_string();
                self.questions[idx].status = (*to).to_string();
            }
            QuestionEdit::Dissolve { .. } => {
                self.questions[idx].status = "dissolved".into();
            }
            QuestionEdit::AddEdge { target, kind } => {
                self.questions[idx].affects.push(Edge {
                    target: target.clone(),
                    kind: kind.name().to_string(),
                });
            }
        }
        Ok(())
    }
}

/// **The observer's check on an edit** (`enact-verify`): read back whether an
/// edit is observably in effect, independent of the author's report. Without
/// this, success is whatever the author says it is — the third failure point
/// of `enact` (7.53).
impl Verify<QuestionEdit> for Questions {
    fn confirms(&self, edit: &QuestionEdit, object: &str) -> bool {
        let Some(idx) = self.questions.iter().position(|q| q.id == object) else {
            return false;
        };
        let q = &self.questions[idx];
        match edit {
            QuestionEdit::Relocate { to } => q.dir == *to && q.status == *to,
            QuestionEdit::Dissolve { .. } => q.status == "dissolved",
            QuestionEdit::AddEdge { target, kind } => q
                .affects
                .iter()
                .any(|e| e.target == *target && e.kind == kind.name()),
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 8. The lifecycle ladder — both gates, on the theory's own vocabulary
// ═════════════════════════════════════════════════════════════════════════

/// Evidence gathered toward answering a question. Authored, not judged.
#[derive(Clone, Debug, PartialEq)]
pub struct Dossier {
    pub question: Question,
    pub sources: Vec<String>,
}

impl Situated for Dossier {
    fn container(&self) -> &str {
        self.question.container()
    }
}

/// A drafted resolution, before anyone has ruled on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub dossier: Dossier,
    pub claim: String,
    /// What the drafter believes. The judge is not bound by it.
    pub answerable: bool,
}

impl Provenanced for Draft {
    fn provenance(&self) -> Prov {
        self.dossier.question.provenance()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Resolution {
    pub id: String,
    pub landed_in: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnosis {
    pub why_malformed: String,
}

// `open → {blocked | parked} → … → {resolved | dissolved}`, with both gates on
// the arrows that need them.
//
// - `gathered` and `drafted` are **authorial**: gathering evidence and drafting
//   a resolution *transform* the question, and transformation demands standing
//   over the folder it sits in (`one-pool-two-filters`). The pen is minted per
//   directory, so a curator authorized over `open/` cannot draft in `resolved/`.
// - `step` is **judgmental**: it is the `dispose` position of Het's pass
//   (`the-pass`). A judge disjoint from the question rules; nobody rules on
//   their own question.
// - `blocked` and `parked` are **continue arms**. They are not terminals — a
//   blocked question re-enters at `Gathered` when its gate lifts, and Het places
//   no bound on that re-entry (`no-bound-on-reentry`).
ladder!(QuestionLifecycle {
    Open(Question)
        => #[authorial(Curator)] Gathered(Dossier)
        => #[authorial(Curator)] Drafted(Draft)
        => #[judgmental(Adjudicator)] {
              Resolved(Resolution)
            | Dissolved(Diagnosis)
            | Blocked -> Gathered
            | Parked  -> Gathered
        }
} impl {
    gathered = |open, pen| {
        assert_eq!(pen.role_name(), "curator");
        let q = open.payload;
        Gathered::new(Dossier {
            // flat docket: the subject's file is at the root
            sources: vec![format!("{}/{}.md", q.scheme.root, q.stem)],
            question: q,
        })
    },

    drafted = |gathered, pen| {
        assert_eq!(pen.over(), gathered.payload.container());
        let d = gathered.payload;
        let answerable = !d.sources.is_empty();
        Drafted::new(Draft {
            claim: format!("{}: drafted from {} source(s)", d.question.id, d.sources.len()),
            answerable,
            dossier: d,
        })
    },

    // The judge's ruling picks a declared, legal edge. It cannot fabricate a
    // state, and it cannot author one either: `Resolution` and `Diagnosis` are
    // built here from what the draft carries.
    step = |drafted, q| {
        assert_eq!(q.role_name(), "adjudicator");
        let d = drafted.payload;
        if !d.answerable {
            return Ok(StepOutcome::Dissolved(Dissolved::new(Diagnosis {
                why_malformed: "no evidence was gatherable; the frame did not survive".into(),
            })));
        }
        if d.dossier.question.status == "blocked" {
            return Ok(StepOutcome::Blocked(Gathered::new(d.dossier)));
        }
        if d.dossier.question.status == "parked" {
            return Ok(StepOutcome::Parked(Gathered::new(d.dossier)));
        }
        Ok(StepOutcome::Resolved(Resolved::new(Resolution {
            id: d.dossier.question.id.clone(),
            landed_in: d.claim,
        })))
    },
});

/// `Sen(Σ)` for the whole theory, across all three sorts.
///
/// Hand-written, because `theory!` declares one sort and emits `SENTENCES` per
/// module. That is limit 1 in this module's header, made concrete: a consumer
/// that wants "every sentence of this theory" cannot get it from the DSL.
pub fn sentences() -> Vec<(&'static str, &'static str)> {
    question::SENTENCES
        .iter()
        .chain(questions::SENTENCES)
        .chain(propagation::SENTENCES)
        .copied()
        .collect()
}

/// Convenience for the authorial hops: a pen over the folder a question sits
/// in. Present so a consumer does not have to re-derive that standing is held
/// over the *directory*, not over the whole set, for the two ladder transitions.
pub fn pen_over<'a, P: rung::Steward>(
    pool: &Pool<P>,
    principal: &'a P,
    folder: &'a str,
) -> Result<Authorized<'a, Curator>, rung::AuthorizeError> {
    pool.authorize::<Curator, _>(principal, folder)
}
