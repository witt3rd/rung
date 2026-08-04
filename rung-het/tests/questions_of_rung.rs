//! **This repository governs itself.** rung's own `docs/questions/`, audited by
//! the library theory in [`rung_std::questions`].
//!
//! The four domains that came before this one were fixtures: a soul document, a
//! cabinet, an issue tracker, a review loop. Each proved something about the
//! library. None of them governed anything that exists.
//!
//! This one does. Its sort is a **question file in this repository**, its models
//! are read off disk at test time from `docs/questions/**/*.md`, and every
//! decidable sentence is evaluated against all twelve of them. If a sentence is
//! wrong about the real thing the suite says so
//! (`the_real_questions_report_their_outbound_edge_drift`) rather than being
//! relaxed until it agrees.
//!
//! ## What is *not* here any more
//!
//! The theory itself. Sorts, sentences, roles, edge taxonomy, edits and
//! lifecycle ladder now live in `rung-std` and are filled by two carriers — this
//! one and a synthetic decision docket in
//! `rung-std/tests/questions_theory.rs`. What remains below is the part that is
//! genuinely rung's: a [`Scheme`], a directory, twelve files, and the results.
//!
//! Het declares the slots (`het-declares-the-slots`); a theory fills them
//! (`theory-declares-four-things`); and a *carrier* is what a theory is
//! evaluated over. Keeping the three in three places is the point — a theory
//! with one carrier is indistinguishable from a domain model, which is exactly
//! what `governs-who-not-what` warns about one level up.
//!
//! ## What this domain could not say
//!
//! Recorded here rather than worked around, because a limit discovered by a real
//! domain is worth more than one predicted from the armchair. The four that are
//! the *theory's* limits are recorded in `rung-std`'s module header. This one is
//! the library's:
//!
//! - **The judgmental branch of `standing-conditional-gated` has no term.**
//!   `Pool::authorize` returns [`AuthorizeError::StandingIsJudgmental`] and
//!   stops. There is no way to dispatch that ruling, so a curator whose standing
//!   over `resolved/` is not settled by containment simply cannot act. Exhibited
//!   by `standing_over_a_folder_can_be_refused_with_nowhere_to_appeal`.

use rung_het::*;
use rung_std::questions::*;
use std::path::{Path, PathBuf};

// ═════════════════════════════════════════════════════════════════════════
// 1. rung's own coordinates — the whole of what this repository supplies
// ═════════════════════════════════════════════════════════════════════════

/// The three parameters `rung_std::questions` is generic over, filled in for
/// this repository. Everything else — the seven edge kinds, the five statuses,
/// the nine decidable sentences, the three roles, the ladder — is the library's.
const RUNG: Scheme = Scheme {
    namespace: "rung-questions",
    root: "docs/questions",
    id_prefix: "q",
};

fn questions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-het sits in the workspace")
        .join("docs/questions")
}

/// Walk `docs/questions/**/*.md`. `_`-prefixed files and `_evidence/` are
/// skipped by the library's loader — exactly what `_reach.py` skips, so the two
/// agree on what a node is.
fn load() -> Questions {
    Questions::load(RUNG, &questions_dir())
}

// ═════════════════════════════════════════════════════════════════════════
// 2. Principals
// ═════════════════════════════════════════════════════════════════════════

struct Person {
    id: &'static str,
    prov: &'static [&'static str],
    roles: &'static [&'static str],
    stewards: &'static [&'static str],
    /// Which way this principal rules. A field, because the verdict is now the
    /// principal's to give: a test that wants the other arm of a coproduct has
    /// to find a principal who takes it, rather than passing the verdict it
    /// wanted to `settle`.
    dissents: bool,
}

impl Principal for Person {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }

    /// `authored` — the history this principal claims. `π(p)` is this
    /// **with `id()` added**, by the blanket `Provenanced` impl in `rung`:
    /// the provenance floor is not a value a principal gets to state.
    fn authored(&self) -> Prov {
        Prov::of(self.prov.iter().copied())
    }

    /// The oracle. The verdict is the outside's, not the caller's.
    fn rule(&self, matter: &str) -> Verdict {
        Verdict::conforming(!self.dissents, format!("`{matter}` does not hold"))
    }
}
impl Steward for Person {
    fn has_standing(&self, over: &str) -> bool {
        self.stewards.contains(&over)
    }
}

/// The curator: authors questions, stewards the folders — and is refused as a
/// judge of everything it files, which is the point.
const CURATOR: Person = Person {
    id: "forge",
    prov: &["rung-questions"],
    roles: &["curator"],
    stewards: &["open", "blocked", "parked", "docs/questions"],
    dissents: false,
};

fn pool() -> Pool<Person> {
    Pool::new(vec![
        Person {
            id: "forge",
            prov: CURATOR.prov,
            roles: CURATOR.roles,
            stewards: CURATOR.stewards,
            dissents: false,
        },
        // An outside reviewer: capable of both judgmental roles, tagged from
        // outside, stewards nothing.
        Person {
            id: "external-reviewer",
            prov: &["external-review"],
            roles: &["interrogator", "adjudicator"],
            stewards: &[],
            dissents: false,
        },
    ])
}

// ═════════════════════════════════════════════════════════════════════════
// 3. The audit — every decidable sentence, over the real files
// ═════════════════════════════════════════════════════════════════════════

/// The questions are read, not invented. If this count drifts, every audit
/// below is silently weaker, so it is pinned.
#[test]
fn the_twelve_questions_are_read_from_disk() {
    let r = load();
    let ids: Vec<&str> = r.questions.iter().map(|q| q.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "q1", "q10", "q11", "q12", "q2", "q3", "q4", "q5", "q6", "q7", "q8", "q9"
        ],
        "docs/questions/ holds twelve questions; an audit over zero of them proves nothing"
    );
}

#[test]
fn every_per_question_decidable_sentence_holds_over_all_twelve_questions() {
    let r = load();
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
        "\n  audit — {} questions × 4 decidable sentences",
        r.questions.len()
    );
    if violations.is_empty() {
        println!("  no violations.\n");
    } else {
        println!("{}\n", violations.join("\n"));
    }

    assert!(
        violations.is_empty(),
        "the real questions violate {} per-question sentence(s):\n{}",
        violations.len(),
        violations.join("\n")
    );
}

/// **A result, asserted as it stands.**
///
/// `affects` is documented as *"the things that rest on this item"*, but nothing
/// maintains it: `_reach.py` builds its reverse index from `depends_on` alone,
/// so an unmirrored `affects` is invisible to every tool that reads these files.
/// Five internal edges are unacknowledged by their source — including
/// **Q7 → Q8**, the spawn edge that the retired `EDGES.md`'s own lived-cascade
/// argument names as one of the three responses Q7's resolution forced. The
/// document that argued for typed edges cited a cascade its own frontmatter does
/// not record.
///
/// The sentence is not weakened to make this green, and the frontmatter is not
/// quietly patched: the drift is systemic (five of five internal edges), so
/// which way to close it — maintain `affects`, or drop it for internal targets
/// and derive it — is a decision for the owner, not a test fixup. The exact set
/// is pinned so that either fixing it or extending it goes red.
///
/// **Mutation 1 target, consumer half.** With `affects_mirrors_inbound`'s body
/// replaced by `|_: &Questions| true` in `rung-std`, the assertion below flips —
/// and so does `questions_theory.rs::the_docket_reports_its_own_outbound_edge_drift`,
/// over a carrier that shares no datum with this one. One sentence, two
/// carriers, two red tests.
#[test]
fn the_real_questions_report_their_outbound_edge_drift() {
    let r = load();
    let settled = questions::affects_mirrors_inbound::holds(&r);

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
    println!("\n  DRIFT — inbound edges no `affects` acknowledges:");
    for (src, dep, kind) in &drift {
        println!("    {src} <--{kind}-- {dep}   ({src}'s `affects` omits {dep})");
    }
    println!(
        "  {} of {} internal edges.\n",
        drift.len(),
        r.internal_edge_count()
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
        "the drift set changed — fix the ledger of results, do not relax the sentence"
    );
}

#[test]
fn every_internal_dependency_in_the_real_files_resolves() {
    let r = load();
    for (from, to) in r.dangling_dependencies() {
        println!("  DANGLING  {from} depends_on {to}, which is not a question");
    }
    assert!(
        questions::every_dependency_resolves::holds(&r)
            .verdict()
            .is_conforming(),
        "an internal `depends_on` names a question that does not exist"
    );
    assert!(
        questions::ids_are_unique::holds(&r)
            .verdict()
            .is_conforming(),
        "two questions share an id"
    );
}

/// The lived-instance discipline as a sentence of the theory, evaluated over the
/// real files: a kind stays in the vocabulary only while something on disk uses
/// it.
#[test]
fn every_declared_edge_kind_has_a_lived_instance_on_disk() {
    let r = load();
    println!("\n  edge vocabulary — declared by the theory, each with a lived instance:");
    for (kind, users) in r.kinds_in_use() {
        println!(
            "    {:<14} {:<11} {:<10} {}",
            kind.name(),
            gate_of(kind),
            if recurses(kind) { "recurses" } else { "leaf" },
            users.join(" ")
        );
        assert!(
            !users.is_empty(),
            "`{}` is declared but nothing in docs/questions/ uses it — a speculative \
             edge type, which is what the lived-instance rule forbids",
            kind.name()
        );
    }
    println!();
    assert!(
        questions::every_declared_kind_is_lived::holds(&r)
            .verdict()
            .is_conforming()
    );
}

// ═════════════════════════════════════════════════════════════════════════
// 4. The claim that had no test — strict against advisory, on Q7's cascade
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
/// **Mutation 2 target.** Moving `EdgeKind::Justification` into `rung-std`'s
/// `propagation_of` `Strict` arm is type-valid and turns this red at the
/// `Propagated::Ruled` match: the advisory edge stops consulting anyone.
#[test]
fn a_strict_edge_propagates_decidably_and_an_advisory_edge_is_ruled_on() {
    let p = pool();
    let change = "q7 resolved: transitions are Prisms, not Kleisli arrows";

    let strict = Exposure {
        scheme: RUNG,
        edge: EdgeKind::Premise,
        source: "q7".into(),
        dependent: "RUNG-CT§6".into(),
        change: change.into(),
    };
    let advisory = Exposure {
        edge: EdgeKind::Justification,
        dependent: "blocking-client-decision".into(),
        ..strict.clone()
    };

    // The declared gates, before anything runs.
    assert_eq!(gate_of(EdgeKind::Premise), "decidable");
    assert_eq!(gate_of(EdgeKind::Justification), "judgmental");

    // ── the strict edge ──────────────────────────────────────────────────
    let out = propagate(&strict, &p).unwrap();
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
    let out = propagate(&advisory, &p).unwrap();
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
    // The other arm is reached by finding a judge who takes it, not by handing
    // one to `settle`. That is the whole of R2 in one call site: the fork is
    // real because the *outside* differs, not because the caller said so.
    let dissenter = Pool::new(vec![Person {
        id: "second-reader",
        prov: &["another-shop"],
        roles: &["adjudicator"],
        stewards: &[],
        dissents: true,
    }]);
    let out = propagate(&advisory, &dissenter).unwrap();
    let Propagated::Ruled(settled) = out else {
        panic!("still advisory")
    };
    assert!(!settled.verdict().is_conforming());
}

// ═════════════════════════════════════════════════════════════════════════
// 5. P0 and standing, on artifacts that exist
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn p0_refuses_the_curator_as_a_judge_of_this_repositorys_own_questions() {
    let r = load();
    let q7 = r.by_id("q7").expect("q7 is on disk");

    let only_curator = Pool::new(vec![Person {
        id: "forge",
        prov: CURATOR.prov,
        roles: &["curator", "interrogator", "adjudicator"],
        stewards: CURATOR.stewards,
        dissents: false,
    }]);

    // Capable of both judgmental roles, and refused anyway: it shares
    // `rung-questions` with everything it filed.
    match only_curator.qualify::<Interrogator>(q7).unwrap_err() {
        QualifyError::NonIdentityViolated { principal, shared } => {
            assert_eq!(principal, "forge");
            assert_eq!(shared, vec!["rung-questions".to_string()]);
        }
        other => panic!("the curator must not judge what it filed; got {other:?}"),
    }

    // The outside reviewer does qualify — on a real file, with its real id.
    let (q, judgment) = pool()
        .consult::<Interrogator>(q7, "is_well_posed")
        .expect("the external reviewer is disjoint");
    assert_eq!(q.principal_id(), "external-reviewer");
    let settled = question::is_well_posed::settle(q7, q, judgment)
        .expect("the licence was minted against this very question");
    assert_eq!(settled.sentence(), "is_well_posed");

    // Two judgmental sentences, two declared roles (`role-declared-not-enumerated`).
    let (q, judgment) = pool()
        .consult::<Adjudicator>(q7, "resolution_answers_the_question")
        .unwrap();
    let settled = question::resolution_answers_the_question::settle(q7, q, judgment)
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
/// role, no token for it. A curator whose standing over `resolved/` is not
/// decidable simply cannot act, and no term in this library can change that.
#[test]
fn standing_over_a_folder_can_be_refused_with_nowhere_to_appeal() {
    let p = pool();
    // The curator stewards open/, blocked/, parked/ — not resolved/.
    match p.authorize::<Curator, _>(&CURATOR, "resolved") {
        Err(AuthorizeError::StandingIsJudgmental { principal, over }) => {
            assert_eq!(principal, "forge");
            assert_eq!(over, "resolved");
        }
        other => panic!("expected the judgmental branch; got {:?}", other.is_ok()),
    }
    // And the branch is terminal here: nothing in `rung_het` accepts a
    // `StandingIsJudgmental` and returns a pen.
    assert!(p.authorize::<Curator, _>(&CURATOR, "open").is_ok());
}

// ═════════════════════════════════════════════════════════════════════════
// 6. The ladder and the pass, on questions read off disk
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn the_lifecycle_ladder_runs_the_authorial_and_judgmental_gates_in_turn() {
    let r = load();
    let q1 = r.by_id("q1").expect("q1 is open").clone();
    assert_eq!(q1.container(), "open");

    let p = pool();

    // Two authorial hops. The pen is over `open/` — the folder q1 sits in — and
    // the macro-injected prologue compares them before either body runs.
    let pen = pen_over(&p, &CURATOR, "open").expect("the curator stewards open/");
    let gathered = questionlifecycle::gathered(questionlifecycle::Open::new(q1), pen);
    assert_eq!(
        gathered.payload.sources,
        vec!["docs/questions/open/q1-transition-body-correctness.md".to_string()]
    );

    let pen = pen_over(&p, &CURATOR, "open").unwrap();
    let drafted = questionlifecycle::drafted(gathered, pen);
    assert!(drafted.payload.answerable);

    // One judgmental hop. The curator cannot mint this token; the outsider can.
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
    let r = load();
    let q3 = r.by_id("q3").expect("q3 is blocked").clone();
    assert_eq!(q3.status, "blocked");

    let p = pool();
    let pen = pen_over(&p, &CURATOR, "blocked").unwrap();
    let gathered = questionlifecycle::gathered(questionlifecycle::Open::new(q3), pen);
    let pen = pen_over(&p, &CURATOR, "blocked").unwrap();
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

#[test]
fn resolved_runs_its_own_law_on_a_write_the_ruling_already_authorized() {
    // `target-runs-its-own-models`. The relocation is proposed by an author with
    // standing, ruled acceptable by a disjoint judge, and still refused — by the
    // destination, on its own law. Q5 has no evidence edge; `resolved/` is a
    // done-pile and will not take it.
    let mut r = load();
    let p = pool();

    let pen = pen_over(&p, &CURATOR, "docs/questions").expect("the curator stewards the tree");

    let proposal = Proposal::remedy(&pen, "q5", QuestionEdit::Relocate { to: "resolved" });
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
    let proposal = Proposal::remedy(&pen, "q7", QuestionEdit::Relocate { to: "resolved" });
    let judge = p.qualify_for::<Adjudicator>(&proposal).unwrap();
    let ruling = dispose(&proposal, judge, Disposition::Accept).unwrap();
    assert_eq!(enact(&mut r, &ruling, &pen).unwrap().object(), "q7");
}
