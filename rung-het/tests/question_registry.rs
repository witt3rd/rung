//! **The registry governs itself.** A Het theory over `docs/questions/`.
//!
//! The four domains that came before this one were fixtures: a soul document, a
//! cabinet, an issue tracker, a review loop. Each proved something about the
//! library. None of them governed anything that exists.
//!
//! This one does. Its sort is a **question file in this repository**, its models
//! are read off disk at test time from `docs/questions/**/*.md`, and every
//! decidable sentence below is evaluated against all eleven of them. If a
//! sentence is wrong about the real registry the suite says so
//! (`the_real_registry_reports_its_outbound_edge_drift`) rather than being
//! relaxed until it agrees.
//!
//! It also carries the vocabulary that `docs/EDGES.md` used to hold. Het
//! declares the slots ([`het-declares-the-slots`]); a theory fills them
//! ([`theory-declares-four-things`]) — sorts, edits, gate-marked sentences, and
//! a role for each judgmental sentence. rung-CT adds a fifth slot at the
//! dependency level: the **edge taxonomy**, which is the governing theory's
//! exactly as an edit vocabulary is ([`edge-taxonomy-is-the-theorys`]). The
//! seven kinds below are this theory's, and they are the seven `EDGES.md`
//! declared. The library has never heard of `premise`.
//!
//! The load-bearing claim under test is [`strict-and-advisory-are-the-gate`]:
//! *a strict edge propagates decidably; an advisory edge requires a ruling.*
//! Until now that proposition had no test. It has one:
//! `a_strict_edge_propagates_decidably_and_an_advisory_edge_is_ruled_on`.
//!
//! ## What this domain could not say
//!
//! Recorded here rather than worked around, because a limit discovered by a real
//! domain is worth more than one predicted from the armchair:
//!
//! 1. **A `theory!` declares one sort.** Het's [`theory-declares-four-things`]
//!    says *sorts*, plural. The per-question sentences and the cross-question
//!    sentences are two `theory!` invocations (`question`, `registry`) with
//!    nothing tying them into one theory — `SENTENCES` is per-module, so `Sen(Σ)`
//!    for this theory is a hand-written concatenation.
//! 2. **A decidable body returns `bool`.** `__decidable!` builds the failure
//!    reason from `stringify!($sentence)`, so a sentence that fails over five
//!    items cannot say *which five*. The audit below therefore computes the
//!    detail through a plain method and uses the sentence only for the verdict.
//! 3. **Verdicts are Boolean.** [`Verdict`] has no metric `d` and no `ε`, so a
//!    judgmental ruling on an advisory edge carries no confidence. "This
//!    dependent probably survives" is not expressible; the judge must say
//!    *survives* or *review required*, flat.
//! 4. **`#[conditional(..)]` is a parse-time refusal.** An edge whose
//!    propagation is settled per-model — `gate`, whose whole question is
//!    *"has this lifted?"* — is exactly Het's conditional gate, and `ladder!`
//!    refuses the marker. This theory classifies `gate` as advisory and says so.
//! 5. **The judgmental branch of `standing-conditional-gated` has no term.**
//!    `Pool::authorize` returns [`AuthorizeError::StandingIsJudgmental`] and
//!    stops. There is no way to dispatch that ruling, so a registrar whose
//!    standing over `resolved/` is not settled by containment simply cannot
//!    act. Exhibited by `standing_over_a_folder_can_be_refused_with_nowhere_to_appeal`.
//!
//! [`het-declares-the-slots`]: ../../docs/rung-het-props.md
//! [`theory-declares-four-things`]: ../../docs/rung-het-props.md
//! [`edge-taxonomy-is-the-theorys`]: ../../docs/rung-ct-props.md
//! [`strict-and-advisory-are-the-gate`]: ../../docs/rung-ct-props.md

use rung::ladder;
use rung_het::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

// ═════════════════════════════════════════════════════════════════════════
// 1. The theory's edge vocabulary — not Het's, and no longer EDGES.md's
// ═════════════════════════════════════════════════════════════════════════

/// **This theory's edge taxonomy** (`edge-taxonomy-is-the-theorys`).
///
/// The seven kinds `docs/EDGES.md` declared, moved to where a taxonomy belongs:
/// inside the theory that governs the items. rung-CT states that an edge type
/// *selects a pushforward* (`edge-type-selects-the-pushforward`) and
/// deliberately never enumerates the types, for the same reason Het never
/// enumerates edits (`governs-who-not-what`).
///
/// Each kind is here because it has a **lived instance** in
/// `docs/questions/` — pinned by
/// `every_declared_edge_kind_has_a_lived_instance_in_the_registry`, which is
/// what stops this list growing speculatively.
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
/// gate marker read one level up. Moving `Justification` into the `Strict` arm
/// is type-valid and turns
/// `a_strict_edge_propagates_decidably_and_an_advisory_edge_is_ruled_on` red —
/// which is what makes that test a test.
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

/// The gate each propagation is settled under — `strict-and-advisory-are-the-gate`
/// made into a lookup.
///
/// rung-CT legislates only the strict/advisory pair. `generative` → `authorial`
/// is **this theory's** reading (a spawned question is *authored* into the
/// registry by someone with standing over it, not classified), and is marked as
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
// 2. The sort — a question file, as it sits on disk
// ═════════════════════════════════════════════════════════════════════════

/// The declared status vocabulary. Four of the five have a directory;
/// `dissolved` does not, which `status_agrees_with_the_directory` has to know.
pub const STATUSES: &[&str] = &["open", "blocked", "parked", "resolved", "dissolved"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub target: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// The frontmatter `id`.
    pub id: String,
    /// The frontmatter `status`.
    pub status: String,
    /// The directory the file sits in — `open`, `blocked`, `parked`, `resolved`.
    pub dir: String,
    /// The filename without `.md`.
    pub stem: String,
    pub depends_on: Vec<Edge>,
    pub affects: Vec<Edge>,
}

/// **π for a question.**
///
/// The registry has no `author:` field, so the finest provenance available is
/// *the registry, and this question within it*. Coarse, but not vacuous: a
/// principal tagged `rung-registry` — which every registrar is — is refused as a
/// judge of every question in it, which is the correct outcome and is what
/// `p0_refuses_the_registrar_as_a_judge_of_the_registrys_own_questions` pins.
impl Provenanced for Question {
    fn provenance(&self) -> Prov {
        Prov::of(["rung-registry".to_string(), self.id.clone()])
    }
}

/// **Where a question sits** — the status directory, which is exactly what a
/// registrar holds standing over. Moving a question between directories is the
/// registry's one real edit, and the pen is minted per directory.
impl Situated for Question {
    fn container(&self) -> &str {
        &self.dir
    }
}

impl Question {
    /// Internal edge targets — the ones that name another question.
    ///
    /// A target is internal iff it matches `q<digits>`. The frontmatter format
    /// does **not** mark internal against external, so `q9-reviews` (a
    /// collective anchor for `_evidence/`) and a typo'd `q99` are
    /// indistinguishable except by this rule. Recorded as a data-format gap, not
    /// papered over.
    pub fn internal_depends_on(&self) -> Vec<(&str, &str)> {
        self.depends_on
            .iter()
            .filter(|e| is_internal_id(&e.target))
            .map(|e| (e.target.as_str(), e.kind.as_str()))
            .collect()
    }

    pub fn names_in_affects(&self, id: &str) -> bool {
        self.affects.iter().any(|e| e.target == id)
    }
}

/// `q7` is internal; `q9-reviews`, `RUNG-CT§6`, `SPEC:G2`, `_map:growth-tower`
/// are not.
pub fn is_internal_id(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('q') else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

// ═════════════════════════════════════════════════════════════════════════
// 3. The second sort — the registry, for sentences no single question can see
// ═════════════════════════════════════════════════════════════════════════

/// A decidable body takes only its model, so a sentence about a *relation
/// between* questions cannot be stated over one question. It is stated over the
/// collection instead — a second sort, and therefore a second `theory!`.
#[derive(Debug, Clone)]
pub struct Registry {
    pub questions: Vec<Question>,
}

impl Provenanced for Registry {
    fn provenance(&self) -> Prov {
        Prov::of(["rung-registry"])
    }
}

impl Situated for Registry {
    fn container(&self) -> &str {
        "docs/questions"
    }
}

impl Registry {
    pub fn by_id(&self, id: &str) -> Option<&Question> {
        self.questions.iter().find(|q| q.id == id)
    }

    /// Internal `depends_on` targets that name no question in the registry.
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

    /// **The finding.** Every inbound `depends_on` should be mirrored by an
    /// outbound `affects` on the target — that is what `affects` is *for*: "the
    /// things that rest on this item."
    ///
    /// Returns `(source, dependent, kind)` for every inbound edge the source
    /// does not acknowledge.
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
}

// ═════════════════════════════════════════════════════════════════════════
// 4. Roles — `role(φ)` for the judgmental sentences, `role(o)` for authorship
// ═════════════════════════════════════════════════════════════════════════

/// The authorial competence — `role(o)`. Who may file, move, and reword a
/// question.
#[derive(Clone, Copy)]
pub struct Registrar;
impl Role for Registrar {
    const NAME: &'static str = "registrar";
}

/// `role(is_well_posed)`. Two judgmental sentences, two roles: Het requires that
/// a role be *declared*, never that it come from a list
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

    // The status agrees with the directory the file sits in. `dissolved` has no
    // directory — a question that dissolves is deleted, not filed — so the
    // sentence would be vacuously wrong about it; there are none on disk and
    // this says so rather than pretending otherwise.
    decidable status_agrees_with_the_directory = |q: &Question|
        q.dir == q.status;

    // Every edge kind this question uses is in the theory's declared taxonomy.
    decidable edge_kinds_are_declared = |q: &Question|
        q.depends_on.iter().chain(q.affects.iter())
            .all(|e| EdgeKind::parse(&e.kind).is_some());

    // Is this a real question — one precise sentence, answerable in principle,
    // not two questions wearing one id? No predicate settles that.
    judgmental is_well_posed: Interrogator;

    // A resolved question claims a verdict. Does the verdict answer the
    // question that was asked?
    judgmental resolution_answers_the_question: Adjudicator;
});

theory!(registry for Registry {
    // Every internal `depends_on` target resolves to a question that exists.
    decidable every_dependency_resolves = |r: &Registry|
        r.dangling_dependencies().is_empty();

    decidable ids_are_unique = |r: &Registry|
        r.duplicate_ids().is_empty();

    // Every inbound edge is acknowledged by its source's `affects`.
    //
    // **This is the sentence mutation 1 targets.** Replacing the body with
    // `|_r: &Registry| true` is type-valid and stops
    // `the_real_registry_reports_its_outbound_edge_drift` reporting anything.
    decidable affects_mirrors_inbound = |r: &Registry|
        r.outbound_drift().is_empty();
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
        Prov::of(["rung-registry".to_string(), self.dependent.clone()])
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

/// The registry's pass over one edge, dispatched on the **edge type's gate**.
///
/// `strict-and-advisory-are-the-gate` as a function. The pool is in scope for
/// the whole function and is *unreachable* from the strict branch — not by
/// discipline, but because `must_reexamine::holds` takes one argument.
pub fn propagate<P: Principal>(
    e: &Exposure,
    pool: &Pool<P>,
    ruling: Verdict,
) -> Result<Propagated, QualifyError> {
    Ok(match propagation_of(e.edge) {
        Propagation::Strict => Propagated::Reexamined(propagation::must_reexamine::holds(e)),
        Propagation::Mechanical => Propagated::Mechanical(propagation::must_reexamine::holds(e)),
        Propagation::Advisory => {
            let q = pool.qualify_for::<Adjudicator>(e)?;
            Propagated::Ruled(
                propagation::survives_the_change::settle(e, q, ruling)
                    .expect("the licence was minted against this very exposure"),
            )
        }
        Propagation::Generative => Propagated::Authored {
            dependent: e.dependent.clone(),
        },
        Propagation::Inert => Propagated::Inert,
    })
}

// ═════════════════════════════════════════════════════════════════════════
// 7. The theory's edits, and the write-guard on `resolved/`
// ═════════════════════════════════════════════════════════════════════════

/// **This theory's edits** (`edit-required-not-typed`). Not Het's, not the
/// cabinet's, not the tracker's. `Dissolve` is the case with no analogue
/// anywhere else: it removes a question from the registry on the ground that it
/// was the wrong question — the answer is a diagnosis, never a resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryEdit {
    Relocate { to: &'static str },
    Dissolve { why: &'static str },
    AddEdge { target: String, kind: EdgeKind },
}

impl Applies<RegistryEdit> for Registry {
    fn territory(&self) -> &'static str {
        "docs/questions"
    }

    fn apply(&mut self, object: &'static str, edit: &RegistryEdit) -> Result<(), EnactError> {
        let has = |q: &Question, k: EdgeKind| q.depends_on.iter().any(|e| e.kind == k.name());
        let idx = self
            .questions
            .iter()
            .position(|q| q.id == object)
            .ok_or(EnactError::ObjectNotFound { object })?;

        match edit {
            RegistryEdit::Relocate { to } => {
                // **The write-guard** (`target-runs-its-own-models`). `resolved/`
                // runs its own law at its boundary: the registry README's third
                // law — a file with work still owed is in the wrong folder — is
                // enforced here as *a resolved question cites its evidence*. An
                // authorization to edit is not a licence to violate the
                // destination's law.
                if *to == "resolved" && !has(&self.questions[idx], EdgeKind::Evidence) {
                    return Err(EnactError::TargetRefused {
                        target: (*to).to_string(),
                        reason: format!("{object} cites no evidence; resolved/ is a done-pile"),
                    });
                }
                self.questions[idx].dir = (*to).to_string();
                self.questions[idx].status = (*to).to_string();
            }
            RegistryEdit::Dissolve { .. } => {
                self.questions[idx].status = "dissolved".into();
            }
            RegistryEdit::AddEdge { target, kind } => {
                self.questions[idx].affects.push(Edge {
                    target: target.clone(),
                    kind: kind.name().to_string(),
                });
            }
        }
        Ok(())
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 8. The lifecycle ladder — both new gates, on the real vocabulary
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
//   over the folder it sits in (one-pool-two-filters). The pen is minted per
//   directory, so a registrar authorized over `open/` cannot draft in
//   `resolved/`.
// - `step` is **judgmental**: it is the `dispose` position of Het's pass
//   (the-pass). A judge disjoint from the question rules; nobody rules on their
//   own question.
// - `blocked` and `parked` are **continue arms**. They are not terminals — a
//   blocked question re-enters at `Gathered` when its gate lifts, and Het places
//   no bound on that re-entry (no-bound-on-reentry).
ladder!(QuestionLifecycle {
    Open(Question)
        => #[authorial(Registrar)] Gathered(Dossier)
        => #[authorial(Registrar)] Drafted(Draft)
        => #[judgmental(Adjudicator)] {
              Resolved(Resolution)
            | Dissolved(Diagnosis)
            | Blocked -> Gathered
            | Parked  -> Gathered
        }
} impl {
    gathered = |open, pen| {
        assert_eq!(pen.role_name(), "registrar");
        let q = open.payload;
        Gathered::new(Dossier {
            sources: vec![format!("docs/questions/{}/{}.md", q.dir, q.stem)],
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

// ═════════════════════════════════════════════════════════════════════════
// 9. Reading the real registry off disk
// ═════════════════════════════════════════════════════════════════════════

fn questions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-het sits in the workspace")
        .join("docs/questions")
}

/// The same frontmatter shape `_reach.py` reads: `id`, `status`, and blocks of
/// `- {on: X, kind: Y}` / `- {target: X, kind: Y}`. Stdlib only, deliberately —
/// the registry's whole discipline is *clone and read, no service to run*, and a
/// YAML dependency here would be the first crack in it.
fn parse_question(path: &Path) -> Option<Question> {
    let text = std::fs::read_to_string(path).ok()?;
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
                let Some((k, v)) = part.split_once(':').map(|(k, v)| (k.trim(), v.trim())) else {
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

    Some(Question {
        id: scalar("id")?,
        status: scalar("status")?,
        dir: path.parent()?.file_name()?.to_str()?.to_string(),
        stem: path.file_stem()?.to_str()?.to_string(),
        depends_on: block("depends_on", "on"),
        affects: block("affects", "target"),
    })
}

/// Walk `docs/questions/**/*.md`, skipping `_`-prefixed files and `_evidence/`
/// — exactly what `_reach.py` skips, so the two agree on what a node is.
fn load_registry() -> Registry {
    fn walk(dir: &Path, out: &mut Vec<Question>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if name.starts_with('_') {
                continue;
            }
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "md")
                && let Some(q) = parse_question(&p)
            {
                out.push(q);
            }
        }
    }
    let mut questions = Vec::new();
    walk(&questions_dir(), &mut questions);
    questions.sort_by(|a, b| a.id.cmp(&b.id));
    Registry { questions }
}

// ═════════════════════════════════════════════════════════════════════════
// 10. Principals
// ═════════════════════════════════════════════════════════════════════════

pub struct Person {
    pub id: &'static str,
    pub prov: &'static [&'static str],
    pub roles: &'static [&'static str],
    pub stewards: &'static [&'static str],
}

impl Provenanced for Person {
    fn provenance(&self) -> Prov {
        Prov::of(self.prov.iter().copied())
    }
}
impl Principal for Person {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }
}
impl Steward for Person {
    fn has_standing(&self, over: &str) -> bool {
        self.stewards.contains(&over)
    }
}

/// The registrar: authors questions, stewards the folders — and is refused as a
/// judge of everything it files, which is the point.
const REGISTRAR: Person = Person {
    id: "forge",
    prov: &["rung-registry"],
    roles: &["registrar"],
    stewards: &["open", "blocked", "parked", "docs/questions"],
};

/// An outside reviewer: capable of both judgmental roles, tagged from outside
/// the registry, stewards nothing.
const OUTSIDER: Person = Person {
    id: "external-reviewer",
    prov: &["external-review"],
    roles: &["interrogator", "adjudicator"],
    stewards: &[],
};

fn pool() -> Pool<Person> {
    Pool::new(vec![
        Person {
            id: "forge",
            prov: REGISTRAR.prov,
            roles: REGISTRAR.roles,
            stewards: REGISTRAR.stewards,
        },
        Person {
            id: "external-reviewer",
            prov: OUTSIDER.prov,
            roles: OUTSIDER.roles,
            stewards: OUTSIDER.stewards,
        },
    ])
}

// ═════════════════════════════════════════════════════════════════════════
// 11. The audit — every decidable sentence, over the real registry
// ═════════════════════════════════════════════════════════════════════════

/// The registry is read, not invented. If this count drifts, every audit below
/// is silently weaker, so it is pinned.
#[test]
fn the_registry_read_from_disk_is_the_real_one() {
    let r = load_registry();
    let ids: Vec<&str> = r.questions.iter().map(|q| q.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "q1", "q10", "q11", "q2", "q3", "q4", "q5", "q6", "q7", "q8", "q9"
        ],
        "docs/questions/ holds eleven questions; an audit over zero of them proves nothing"
    );
}

#[test]
fn every_per_question_decidable_sentence_holds_over_all_eleven_questions() {
    let r = load_registry();
    let mut violations: Vec<String> = Vec::new();

    for q in &r.questions {
        for settled in [
            question::id_matches_the_filename::holds(q),
            question::status_is_declared::holds(q),
            question::status_agrees_with_the_directory::holds(q),
            question::edge_kinds_are_declared::holds(q),
        ] {
            assert!(
                !settled.consulted_outside(),
                "a decidable sentence must not report an outside call"
            );
            if let Verdict::NonConforming { reason } = settled.verdict() {
                violations.push(format!(
                    "  {:<4} {:<32} {reason}  [{}/{}.md]",
                    q.id,
                    settled.sentence(),
                    q.dir,
                    q.stem
                ));
            }
        }
    }

    println!(
        "\n  registry audit — {} questions × 4 decidable sentences",
        r.questions.len()
    );
    if violations.is_empty() {
        println!("  no violations.\n");
    } else {
        println!("{}\n", violations.join("\n"));
    }

    assert!(
        violations.is_empty(),
        "the real registry violates {} per-question sentence(s):\n{}",
        violations.len(),
        violations.join("\n")
    );
}

/// **A finding, asserted as it stands.**
///
/// `affects` is documented as *"the things that rest on this item"*, but nothing
/// maintains it: `_reach.py` builds its reverse index from `depends_on` alone,
/// so an unmirrored `affects` is invisible to every tool that reads the
/// registry. Five internal edges are unacknowledged by their source — including
/// **Q7 → Q8**, the spawn edge that `EDGES.md`'s own lived-cascade argument
/// names as one of the three responses Q7's resolution forced. The document
/// that argued for typed edges cited a cascade its own frontmatter does not
/// record.
///
/// The sentence is not weakened to make this green, and the frontmatter is not
/// quietly patched: the drift is systemic (five of five internal edges), so
/// which way to close it — maintain `affects`, or drop it for internal targets
/// and derive it — is a decision for the registry's owner, not a test fixup.
/// The exact set is pinned so that either fixing it or extending it goes red.
///
/// **Mutation 1 target.** With `affects_mirrors_inbound`'s body replaced by
/// `|_r: &Registry| true`, the assertion below flips and this test stops
/// reporting the drift.
#[test]
fn the_real_registry_reports_its_outbound_edge_drift() {
    let r = load_registry();
    let settled = registry::affects_mirrors_inbound::holds(&r);

    assert!(
        !settled.consulted_outside(),
        "an outbound-edge check is decidable; no judge is needed to read two files"
    );

    let Verdict::NonConforming { reason } = settled.verdict() else {
        panic!(
            "the drift is real and unfixed; a conforming verdict here means the \
             sentence stopped looking. `affects_mirrors_inbound` must not be weakened."
        )
    };
    assert!(reason.contains("affects_mirrors_inbound"));

    let drift = r.outbound_drift();
    println!("\n  FINDING — inbound edges no `affects` acknowledges:");
    for (src, dep, kind) in &drift {
        println!("    {src} <--{kind}-- {dep}   ({src}'s `affects` omits {dep})");
    }
    println!(
        "  {} of {} internal edges.\n",
        drift.len(),
        r.questions
            .iter()
            .map(|q| q.internal_depends_on().len())
            .sum::<usize>()
    );

    let observed: Vec<(&str, &str, &str)> = drift
        .iter()
        .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
        .collect();
    assert_eq!(
        observed,
        [
            ("q1", "q11", "justification"),
            ("q4", "q10", "related"),
            ("q7", "q8", "spawn"),
            ("q7", "q9", "premise"),
            ("q9", "q10", "premise"),
        ],
        "the drift set changed — fix the ledger of findings, do not relax the sentence"
    );
}

#[test]
fn every_internal_dependency_in_the_real_registry_resolves() {
    let r = load_registry();
    let settled = registry::every_dependency_resolves::holds(&r);
    for (from, to) in r.dangling_dependencies() {
        println!("  DANGLING  {from} depends_on {to}, which is not a question");
    }
    assert!(
        settled.verdict().is_conforming(),
        "an internal `depends_on` names a question that does not exist"
    );
    assert!(
        registry::ids_are_unique::holds(&r)
            .verdict()
            .is_conforming(),
        "two questions share an id"
    );
}

/// The lived-instance discipline `EDGES.md` stated in prose, as a test: a kind
/// stays in the vocabulary only while something in the registry uses it.
#[test]
fn every_declared_edge_kind_has_a_lived_instance_in_the_registry() {
    let r = load_registry();
    let mut used: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for q in &r.questions {
        for e in q.depends_on.iter().chain(q.affects.iter()) {
            if let Some(k) = EdgeKind::parse(&e.kind) {
                used.entry(k.name()).or_default().push(q.id.clone());
            }
        }
    }
    println!("\n  edge vocabulary — declared by the theory, each with a lived instance:");
    for k in EDGE_KINDS {
        let lived = used.get(k.name()).cloned().unwrap_or_default();
        println!(
            "    {:<14} {:<11} {:<10} {}",
            k.name(),
            gate_of(*k),
            if recurses(*k) { "recurses" } else { "leaf" },
            lived.join(" ")
        );
        assert!(
            !lived.is_empty(),
            "`{}` is declared but nothing in docs/questions/ uses it — a speculative \
             edge type, which is what the lived-instance rule forbids",
            k.name()
        );
    }
    println!();
}

// ═════════════════════════════════════════════════════════════════════════
// 12. The claim that had no test — strict against advisory
// ═════════════════════════════════════════════════════════════════════════

/// **`strict-and-advisory-are-the-gate`.**
///
/// One change — Q7 resolving, 2026-07-18 — over two of its real edges:
///
/// - `q7 --premise--> RUNG-CT§6`: its framing was *wrong* until the resolution
///   was folded in. Obligatory.
/// - `q7 --justification--> the blocking client decision`: the premise moved and the
///   decision **held**. Advisory.
///
/// Both edges leave the same node on the same day. They take different paths,
/// and the difference is the gate: the strict one is settled by a function of
/// the edge type with no pool in reach; the advisory one cannot be settled at
/// all without a `Qualified<Adjudicator>` that only the non-identity filter
/// mints.
///
/// **Mutation 2 target.** Moving `EdgeKind::Justification` into
/// `propagation_of`'s `Strict` arm is type-valid and turns this red at the
/// `Propagated::Ruled` match: the advisory edge stops consulting anyone.
#[test]
fn a_strict_edge_propagates_decidably_and_an_advisory_edge_is_ruled_on() {
    let p = pool();
    let change = "q7 resolved: transitions are Prisms, not Kleisli arrows";

    let strict = Exposure {
        edge: EdgeKind::Premise,
        source: "q7".into(),
        dependent: "RUNG-CT§6".into(),
        change: change.into(),
    };
    let advisory = Exposure {
        edge: EdgeKind::Justification,
        source: "q7".into(),
        dependent: "blocking-client-decision".into(),
        change: change.into(),
    };

    // The declared gates, before anything runs.
    assert_eq!(gate_of(EdgeKind::Premise), "decidable");
    assert_eq!(gate_of(EdgeKind::Justification), "judgmental");

    // ── the strict edge ──────────────────────────────────────────────────
    let out = propagate(&strict, &p, Verdict::Conforming).unwrap();
    let Propagated::Reexamined(settled) = out else {
        panic!("a `premise` edge must propagate decidably; got {out:?}")
    };
    assert!(
        !settled.consulted_outside(),
        "a strict lift is an obligation, not a ruling — nobody is asked"
    );
    assert!(
        settled.verdict().is_conforming(),
        "`RUNG-CT§6` rests on q7 as a premise: it must be re-examined, full stop"
    );

    // ── the advisory edge, same change ───────────────────────────────────
    //
    // The coproduct: the judge collapses `ReviewRequired + Survives`. Q7's
    // resolution confirmed "no architectural debt", so the blocking-client
    // decision survived — and that is a ruling, not a computation.
    let out = propagate(&advisory, &p, Verdict::Conforming).unwrap();
    let Propagated::Ruled(settled) = out else {
        panic!(
            "a `justification` edge must be ruled on, not computed; got {out:?}. \
             Reclassifying it strict is exactly the collapse \
             `strict-and-advisory-are-the-gate` forbids: it would report a decision \
             that was fine as broken."
        )
    };
    assert!(
        settled.consulted_outside(),
        "an advisory lift lands in a coproduct; collapsing it needs an outside"
    );
    match &settled {
        Settled::Judgmental {
            role, principal, ..
        } => {
            assert_eq!(*role, "adjudicator");
            assert_eq!(principal, "external-reviewer");
        }
        other => panic!("expected a judgmental settlement, got {other:?}"),
    }
    assert!(settled.verdict().is_conforming(), "the decision survived");

    // ── the other arm of the same coproduct ──────────────────────────────
    //
    // The advisory path is a real fork: the same edge, the same change, and a
    // judge that rules the other way. A strict edge has no such arm.
    let out = propagate(
        &advisory,
        &p,
        Verdict::NonConforming {
            reason: "the decision rested on the Kleisli framing after all".into(),
        },
    )
    .unwrap();
    let Propagated::Ruled(settled) = out else {
        panic!("still advisory")
    };
    assert!(!settled.verdict().is_conforming());
}

/// The other half of the same claim: the two paths differ in the *type*, not
/// only in the outcome. There is no term for settling the advisory edge without
/// an outside, and no parameter through which the strict one could reach for
/// one.
#[test]
fn the_two_paths_differ_in_arity_not_in_convention() {
    // `holds` is `fn(&Exposure) -> Settled`. Coercing it to that exact pointer
    // type fails to compile if a pool parameter is ever added.
    let strict: fn(&Exposure) -> Settled = propagation::must_reexamine::holds;

    // `settle` cannot be named without the token type in its signature.
    let advisory: fn(&Exposure, Qualified<Adjudicator>, Verdict) -> Result<Settled, TokenNotBound> =
        propagation::survives_the_change::settle;

    let e = Exposure {
        edge: EdgeKind::Premise,
        source: "q7".into(),
        dependent: "RUNG-CT§6".into(),
        change: "resolved".into(),
    };
    assert!(!strict(&e).consulted_outside());

    let p = pool();
    let adv = Exposure {
        edge: EdgeKind::Justification,
        ..e
    };
    let q = p.qualify_for::<Adjudicator>(&adv).unwrap();
    assert!(
        advisory(&adv, q, Verdict::Conforming)
            .unwrap()
            .consulted_outside()
    );
}

/// A licence minted against one exposure does not settle another. The registry
/// is exactly where this matters: a judge that ruled "q8 survives" has not
/// thereby ruled on q9.
#[test]
fn a_ruling_on_one_exposure_does_not_carry_to_another() {
    let p = pool();
    let one = Exposure {
        edge: EdgeKind::Justification,
        source: "q7".into(),
        dependent: "q8".into(),
        change: "resolved".into(),
    };
    let other = Exposure {
        dependent: "q9".into(),
        ..one.clone()
    };

    let q = p.qualify_for::<Adjudicator>(&one).unwrap();
    assert!(matches!(
        propagation::survives_the_change::settle(&other, q, Verdict::Conforming),
        Err(TokenNotBound { .. })
    ));
}

// ═════════════════════════════════════════════════════════════════════════
// 13. P0, on artifacts that exist
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn p0_refuses_the_registrar_as_a_judge_of_the_registrys_own_questions() {
    let r = load_registry();
    let q7 = r.by_id("q7").expect("q7 is on disk");

    let only_registrar = Pool::new(vec![Person {
        id: "forge",
        prov: REGISTRAR.prov,
        roles: &["registrar", "interrogator", "adjudicator"],
        stewards: REGISTRAR.stewards,
    }]);

    // Capable of both judgmental roles, and refused anyway: it shares
    // `rung-registry` with everything it filed.
    match only_registrar.qualify::<Interrogator>(q7).unwrap_err() {
        QualifyError::NonIdentityViolated { principal, shared } => {
            assert_eq!(principal, "forge");
            assert_eq!(shared, vec!["rung-registry".to_string()]);
        }
        other => panic!("the registrar must not judge its own registry; got {other:?}"),
    }

    // The outside reviewer does qualify — on a real file, with its real id.
    let q = pool()
        .qualify::<Interrogator>(q7)
        .expect("the external reviewer is disjoint from the registry");
    assert_eq!(q.principal_id(), "external-reviewer");
    let settled = question::is_well_posed::settle(q7, q, Verdict::Conforming)
        .expect("the licence was minted against this very question");
    assert_eq!(settled.sentence(), "is_well_posed");

    // Two judgmental sentences, two declared roles (`role-declared-not-enumerated`).
    let q = pool().qualify::<Adjudicator>(q7).unwrap();
    let settled = question::resolution_answers_the_question::settle(q7, q, Verdict::Conforming)
        .expect("minted against q7");
    match settled {
        Settled::Judgmental { role, .. } => assert_eq!(role, "adjudicator"),
        other => panic!("expected a judgmental settlement, got {other:?}"),
    }
}

/// **A stated limit, exhibited.**
///
/// `standing-conditional-gated` says standing is Het's one conditional gate:
/// containment settles it where it applies, and *otherwise a judge must rule*.
/// `Pool::authorize` implements only the first half. The second comes back as
/// `StandingIsJudgmental` and there the matter ends — there is no `settle`, no
/// role, no token for it. A registrar whose standing over `resolved/` is not
/// decidable simply cannot act, and no term in this library can change that.
#[test]
fn standing_over_a_folder_can_be_refused_with_nowhere_to_appeal() {
    let p = pool();
    // The registrar stewards open/, blocked/, parked/ — not resolved/.
    match p.authorize::<Registrar, _>(&REGISTRAR, "resolved") {
        Err(AuthorizeError::StandingIsJudgmental { principal, over }) => {
            assert_eq!(principal, "forge");
            assert_eq!(over, "resolved");
        }
        other => panic!("expected the judgmental branch; got {:?}", other.is_ok()),
    }
    // And the branch is terminal here: nothing in `rung_het` accepts a
    // `StandingIsJudgmental` and returns a pen.
    assert!(p.authorize::<Registrar, _>(&REGISTRAR, "open").is_ok());
}

// ═════════════════════════════════════════════════════════════════════════
// 14. The ladder, on a question read off disk
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn the_lifecycle_ladder_runs_the_authorial_and_judgmental_gates_in_turn() {
    let r = load_registry();
    let q1 = r.by_id("q1").expect("q1 is open").clone();
    assert_eq!(q1.container(), "open");

    let p = pool();

    // Two authorial hops. The pen is over `open/` — the folder q1 sits in — and
    // the macro-injected prologue compares them before either body runs.
    let pen = p
        .authorize::<Registrar, _>(&REGISTRAR, "open")
        .expect("the registrar stewards open/");
    let gathered = questionlifecycle::gathered(questionlifecycle::Open::new(q1), pen);
    assert_eq!(gathered.payload.sources.len(), 1);

    let pen = p.authorize::<Registrar, _>(&REGISTRAR, "open").unwrap();
    let drafted = questionlifecycle::drafted(gathered, pen);
    assert!(drafted.payload.answerable);

    // One judgmental hop. The registrar cannot mint this token; the outsider can.
    let ruling = p
        .qualify_for::<Adjudicator>(&drafted.payload)
        .expect("the external reviewer is disjoint from q1");
    assert_eq!(ruling.principal_id(), "external-reviewer");

    match questionlifecycle::step(drafted, ruling) {
        Ok(questionlifecycle::StepOutcome::Resolved(res)) => {
            assert_eq!(res.payload().id, "q1");
        }
        Ok(_) => panic!("q1 is answerable; expected a resolution"),
        Err(f) => panic!("the step failed: {}", f.error),
    }
}

#[test]
fn a_blocked_question_re_enters_at_gathered_rather_than_terminating() {
    // `blocked` and `parked` are continue arms, not terminals: the StepOutcome
    // variant carries a live `Gathered` rung. Q3 is really blocked, on a Rust
    // language feature that does not exist.
    let r = load_registry();
    let q3 = r.by_id("q3").expect("q3 is blocked").clone();
    assert_eq!(q3.status, "blocked");

    let p = pool();
    let pen = p.authorize::<Registrar, _>(&REGISTRAR, "blocked").unwrap();
    let gathered = questionlifecycle::gathered(questionlifecycle::Open::new(q3), pen);
    let pen = p.authorize::<Registrar, _>(&REGISTRAR, "blocked").unwrap();
    let drafted = questionlifecycle::drafted(gathered, pen);

    let ruling = p.qualify_for::<Adjudicator>(&drafted.payload).unwrap();
    let gathered = match questionlifecycle::step(drafted, ruling) {
        Ok(questionlifecycle::StepOutcome::Blocked(g)) => g,
        Ok(_) => panic!("q3 is blocked; expected re-entry at Gathered"),
        Err(f) => panic!("the step failed: {}", f.error),
    };
    // The rung came back live. Het places no bound on that re-entry
    // (`no-bound-on-reentry`), and neither does this ladder.
    assert_eq!(gathered.payload.question.id, "q3");
}

/// A pen for `open/` does not author in `blocked/`. Without this the pen is
/// decorative — and the registry is precisely a place where "which folder" is
/// the whole content of the edit.
#[test]
#[should_panic(expected = "this pen authorizes")]
fn a_pen_for_one_folder_does_not_author_a_question_in_another() {
    let r = load_registry();
    let q3 = r.by_id("q3").expect("q3 is in blocked/").clone();
    let p = pool();
    let elsewhere = p
        .authorize::<Registrar, _>(&REGISTRAR, "open")
        .expect("the registrar does steward open/");
    let _ = questionlifecycle::gathered(questionlifecycle::Open::new(q3), elsewhere);
}

// ═════════════════════════════════════════════════════════════════════════
// 15. The pass, and the write-guard on `resolved/`
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn resolved_runs_its_own_law_on_a_write_the_ruling_already_authorized() {
    // `target-runs-its-own-models`. The relocation is proposed by an author with
    // standing, ruled acceptable by a disjoint judge, and still refused — by the
    // destination, on its own law. Q5 has no evidence edge; `resolved/` is a
    // done-pile and will not take it.
    let mut r = load_registry();
    let p = pool();

    let pen = p
        .authorize::<Registrar, _>(&REGISTRAR, "docs/questions")
        .expect("the registrar stewards the registry");

    let proposal = Proposal::remedy(&pen, "q5", RegistryEdit::Relocate { to: "resolved" });
    let judge = p
        .qualify_for::<Adjudicator>(&proposal)
        .expect("the reviewer did not author the proposal");
    let ruling =
        dispose(&proposal, judge, Disposition::Accept).expect("minted against the proposal");
    assert!(ruling.is_affirming());

    match enact(&mut r, &ruling, &pen) {
        Err(EnactError::TargetRefused { target, reason }) => {
            assert_eq!(target, "resolved");
            assert!(reason.contains("q5"));
        }
        other => panic!("resolved/ must refuse an unevidenced question; got {other:?}"),
    }
    assert_eq!(
        r.by_id("q5").unwrap().dir,
        "open",
        "a refused write changes nothing"
    );

    // Q7 carries an `evidence` edge, so the same edit lands.
    let proposal = Proposal::remedy(&pen, "q7", RegistryEdit::Relocate { to: "resolved" });
    let judge = p.qualify_for::<Adjudicator>(&proposal).unwrap();
    let ruling = dispose(&proposal, judge, Disposition::Accept).unwrap();
    assert_eq!(enact(&mut r, &ruling, &pen).unwrap().object(), "q7");
}

#[test]
fn the_theory_exposes_its_sentences_with_their_gates() {
    // `Sen(Σ)` as data — and the first place a limit shows: `theory!` declares
    // ONE sort, so a theory over two sorts is two modules and the concatenation
    // is written by hand rather than emitted.
    assert_eq!(
        question::SENTENCES,
        &[
            ("id_matches_the_filename", "decidable"),
            ("status_is_declared", "decidable"),
            ("status_agrees_with_the_directory", "decidable"),
            ("edge_kinds_are_declared", "decidable"),
            ("is_well_posed", "judgmental"),
            ("resolution_answers_the_question", "judgmental"),
        ]
    );
    assert_eq!(
        registry::SENTENCES,
        &[
            ("every_dependency_resolves", "decidable"),
            ("ids_are_unique", "decidable"),
            ("affects_mirrors_inbound", "decidable"),
        ]
    );
    assert_eq!(
        propagation::SENTENCES,
        &[
            ("must_reexamine", "decidable"),
            ("survives_the_change", "judgmental"),
        ]
    );

    for (name, gate) in question::SENTENCES
        .iter()
        .chain(registry::SENTENCES)
        .chain(propagation::SENTENCES)
    {
        assert!(
            matches!(*gate, "decidable" | "judgmental"),
            "sentence `{name}` carries unknown gate `{gate}`"
        );
    }
}
