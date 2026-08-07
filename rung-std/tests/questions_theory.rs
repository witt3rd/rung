//! The questions theory over a **synthetic** body of questions.
//!
//! This file exists to make the library's genericity checkable rather than
//! asserted. Its carrier is an architecture-decision docket that shares nothing
//! with rung's own `questions/`:
//!
//! | | rung's questions | this docket |
//! |---|---|---|
//! | ids | `q1 … q11` | `d1 … d6` |
//! | id prefix | `q` | `d` |
//! | namespace | `rung-questions` | `atlas-decisions` |
//! | container | `questions` | `specs/decisions` |
//! | roles held by | `forge` / `external-reviewer` | `atlas-bot` / `guild-reviewer` |
//! | edges | five unacknowledged of five | three unacknowledged of five |
//! | lifecycle path | resolved, and blocked re-entry | parked re-entry |
//!
//! Nothing rung asks about its own questions can break a test here, and nothing
//! here depends on a file in this repository — the docket is frontmatter
//! strings parsed in memory. If a change to rung's questions could turn this
//! file red, the split between library and consumer is in the wrong place.

use rung_het::{Applies, Disposition, EnactError, Pool, Proposal, Verify, dispose, enact};
use rung_std::questions::*;

// ═════════════════════════════════════════════════════════════════════════
// The synthetic docket
// ═════════════════════════════════════════════════════════════════════════

const DOCKET: Scheme = Scheme {
    namespace: "atlas-decisions",
    root: "specs/decisions",
    id_prefix: "d",
};

/// Six decisions, as they would sit on disk. Between them they use all seven
/// declared edge kinds and exercise both the internal/external target rule and
/// the outbound-drift check.
const FILES: &[(&str, &str, &str)] = &[
    (
        "open",
        "d1-which-transport",
        "---\nid: d1\nstatus: open\naffects:\n  - {target: d2, kind: premise}\n---\nbody\n",
    ),
    (
        "open",
        "d2-frame-format",
        "---\nid: d2\nstatus: open\ndepends_on:\n  - {on: d1, kind: premise}\naffects:\n  - {target: d3, kind: spawn}\n---\nbody\n",
    ),
    (
        "blocked",
        "d3-backpressure-policy",
        "---\nid: d3\nstatus: blocked\ndepends_on:\n  - {on: d2, kind: spawn}\n  - {on: EXT-7, kind: gate}\n---\nbody\n",
    ),
    (
        "parked",
        "d4-schema-versioning",
        "---\nid: d4\nstatus: parked\ndepends_on:\n  - {on: d1, kind: justification}\n---\nbody\n",
    ),
    (
        "resolved",
        "d5-retry-budget",
        "---\nid: d5\nstatus: resolved\ndepends_on:\n  - {on: d3, kind: citation}\n  - {on: d9-notes, kind: evidence}\n---\nbody\n",
    ),
    (
        "open",
        "d6-operator-console",
        "---\nid: d6\nstatus: open\ndepends_on:\n  - {on: d5, kind: related}\n  - {on: ext-brief, kind: evidence}\n---\nbody\n",
    ),
];

fn docket() -> Questions {
    let parsed = FILES
        .iter()
        .map(|(dir, stem, text)| {
            Question::parse(DOCKET, text, dir, stem).expect("the frontmatter is well formed")
        })
        .collect();
    Questions::new(DOCKET, parsed)
}

// ═════════════════════════════════════════════════════════════════════════
// Principals — this docket's, not rung's
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

impl rung_het::Principal for Person {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }

    /// `authored` — the history this principal claims. `π(p)` is this
    /// **with `id()` added**, by the blanket `Provenanced` impl in `rung`:
    /// the provenance floor is not a value a principal gets to state.
    fn authored(&self) -> rung_het::Prov {
        rung_het::Prov::of(self.prov.iter().copied())
    }

    /// The oracle. The verdict is the outside's, not the caller's.
    fn rule(&self, matter: &str) -> rung_het::Response {
        rung_het::Response::Rendered(rung_het::Verdict::conforming(
            !self.dissents,
            format!("`{matter}` does not hold"),
        ))
    }
}
impl rung_het::Steward for Person {
    fn has_standing(&self, over: &str) -> bool {
        self.stewards.contains(&over)
    }
}

const CURATOR: Person = Person {
    id: "atlas-bot",
    prov: &["atlas-decisions"],
    roles: &["curator"],
    stewards: &["open", "blocked", "parked", "specs/decisions"],
    dissents: false,
};

fn pool() -> Pool<Person> {
    Pool::new(vec![
        Person {
            id: "atlas-bot",
            prov: CURATOR.prov,
            roles: CURATOR.roles,
            stewards: CURATOR.stewards,
            dissents: false,
        },
        Person {
            id: "guild-reviewer",
            prov: &["independent-guild"],
            roles: &["interrogator", "adjudicator"],
            stewards: &[],
            dissents: false,
        },
    ])
}

// ═════════════════════════════════════════════════════════════════════════
// The audit, over a carrier the library has never seen
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn the_docket_parses_into_six_questions_with_a_disjoint_id_space() {
    let d = docket();
    let ids: Vec<&str> = d.questions.iter().map(|q| q.id.as_str()).collect();
    assert_eq!(ids, ["d1", "d2", "d3", "d4", "d5", "d6"]);
    // The internal-target rule is the `Scheme`'s, not a literal: `EXT-7`,
    // `d9-notes` and `ext-brief` are all external here, and `d9-notes` is the
    // interesting one — it starts with the prefix and is still not an id.
    assert!(is_internal_id(DOCKET, "d3"));
    assert!(!is_internal_id(DOCKET, "d9-notes"));
    assert!(!is_internal_id(DOCKET, "EXT-7"));
    // …and rung's own convention is simply a different `Scheme`.
    assert!(!is_internal_id(DOCKET, "q7"));
    assert_eq!(d.internal_edge_count(), 5);
}

#[test]
fn every_per_question_sentence_holds_over_the_whole_docket() {
    let d = docket();
    for q in &d.questions {
        for settled in [
            question::id_matches_the_filename::holds(q),
            question::status_is_declared::holds(q),
            question::edge_kinds_are_declared::holds(q),
        ] {
            assert!(
                !settled.consulted_outside(),
                "a decidable sentence must not report an outside call"
            );
            assert!(
                settled.verdict().is_conforming(),
                "{} violates `{}`",
                q.id,
                settled.sentence()
            );
        }
    }
}

/// **The same decidable sentence, a different carrier, a different answer.**
///
/// This is the test that pairs with `questions_of_rung.rs`'s drift check. Both
/// call `questions::affects_mirrors_inbound::holds`; rung's own questions leave
/// five of five inbound edges unacknowledged, this docket leaves three of five.
/// Replacing the sentence's body with `|_: &Questions| true` turns *both* red —
/// which is what makes "one theory, two carriers" an observation rather than a
/// claim.
#[test]
fn the_docket_reports_its_own_outbound_edge_drift() {
    let d = docket();
    let settled = questions::affects_mirrors_inbound::holds(&d);

    assert!(
        !settled.consulted_outside(),
        "reading two frontmatter blocks needs no judge"
    );
    let rung_het::Verdict::NonConforming { reason } = settled.verdict() else {
        panic!(
            "the docket's drift is real; a conforming verdict means the sentence stopped looking"
        )
    };
    assert!(reason.contains("affects_mirrors_inbound"));

    let drift = d.outbound_drift();
    println!("\n  inbound edges no `affects` acknowledges:");
    for (src, dep, kind) in &drift {
        println!("    {src} <--{kind}-- {dep}");
    }
    println!(
        "  {} of {} internal edges.\n",
        drift.len(),
        d.internal_edge_count()
    );

    let observed: Vec<(&str, &str, &str)> = drift
        .iter()
        .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
        .collect();
    assert_eq!(
        observed,
        [
            ("d1", "d4", "justification"),
            ("d3", "d5", "citation"),
            ("d5", "d6", "related"),
        ]
    );
}

#[test]
fn the_docket_has_no_dangling_dependency_and_no_duplicate_id() {
    let d = docket();
    assert!(
        questions::every_dependency_resolves::holds(&d)
            .verdict()
            .is_conforming(),
        "dangling: {:?}",
        d.dangling_dependencies()
    );
    assert!(
        questions::ids_are_unique::holds(&d)
            .verdict()
            .is_conforming()
    );
}

/// The lived-instance discipline, over a carrier chosen to exercise all seven
/// kinds. A speculative eighth kind would have no user and the sentence would
/// fail — which is the discipline, stated as a sentence rather than as prose.
#[test]
fn every_declared_edge_kind_has_a_lived_instance_in_the_docket() {
    let d = docket();
    assert!(
        questions::every_declared_kind_is_lived::holds(&d)
            .verdict()
            .is_conforming()
    );
    println!("\n  edge vocabulary — declared by the theory, each with a lived instance:");
    for (kind, users) in d.kinds_in_use() {
        println!(
            "    {:<14} {:<11} {:<10} {}",
            kind.name(),
            gate_of(kind),
            if recurses(kind) { "recurses" } else { "leaf" },
            users.join(" ")
        );
        assert!(!users.is_empty(), "`{}` has no lived instance", kind.name());
    }
    println!();
}

// ═════════════════════════════════════════════════════════════════════════
// The gate split, over the docket's own cascade
// ═════════════════════════════════════════════════════════════════════════

/// **`strict-and-advisory-are-the-gate`, second carrier.**
///
/// D1 settles, and two of its real edges carry the change: `d1 --premise--> d2`
/// (obligatory) and `d1 --justification--> d4` (advisory). Same change, same
/// day, two paths — and the difference is the gate, not a convention.
///
/// Moving `EdgeKind::Justification` into `propagation_of`'s `Strict` arm is
/// type-valid and turns this red at the `Propagated::Ruled` match.
#[test]
fn a_strict_edge_propagates_decidably_and_an_advisory_edge_is_ruled_on() {
    let p = pool();
    let change = "d1 resolved: the transport is framed, not streamed";

    let strict = Exposure {
        scheme: DOCKET,
        edge: EdgeKind::Premise,
        source: "d1".into(),
        dependent: "d2".into(),
        change: change.into(),
    };
    let advisory = Exposure {
        edge: EdgeKind::Justification,
        dependent: "d4".into(),
        ..strict.clone()
    };

    assert_eq!(gate_of(EdgeKind::Premise), "decidable");
    assert_eq!(gate_of(EdgeKind::Justification), "judgmental");

    let out = propagate(&strict, &p).unwrap();
    let Propagated::Reexamined(settled) = out else {
        panic!("a `premise` edge must propagate decidably; got {out:?}")
    };
    assert!(
        !settled.consulted_outside(),
        "a strict lift is an obligation, not a ruling — nobody is asked"
    );
    assert!(settled.verdict().is_conforming());

    let out = propagate(&advisory, &p).unwrap();
    let Propagated::Ruled(settled) = out else {
        panic!("a `justification` edge must be ruled on, not computed; got {out:?}")
    };
    assert!(settled.consulted_outside());
    match &settled {
        rung_het::Settled::Judgmental {
            role, principal, ..
        } => {
            assert_eq!(*role, "adjudicator");
            assert_eq!(principal, "guild-reviewer");
        }
        other => panic!("expected a judgmental settlement, got {other:?}"),
    }

    // The other arm of the same coproduct — a strict edge has no such arm.
    // The other arm is reached by locating a judge who takes it, not by
    // handing one to `settle`.
    let dissenter = Pool::new(vec![Person {
        id: "second-reader",
        prov: &["another-docket"],
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

/// The other half of the same claim: the two paths differ in the *type*, not
/// only in the outcome. There is no term for settling the advisory edge without
/// an outside, and no parameter through which the strict one could reach for
/// one.
#[test]
fn the_two_paths_differ_in_arity_not_in_convention() {
    // `holds` is `fn(&Exposure) -> Settled`. Coercing it to that exact pointer
    // type fails to compile if a pool parameter is ever added.
    let strict: fn(&Exposure) -> rung_het::Settled = propagation::must_reexamine::holds;

    // `settle` cannot be named without the token type in its signature.
    let advisory: fn(
        &Exposure,
        rung_het::Qualified<Adjudicator>,
        rung_het::Judgment,
    ) -> Result<rung_het::Settled, rung_het::SettleError> =
        propagation::survives_the_change::settle;

    let e = Exposure {
        scheme: DOCKET,
        edge: EdgeKind::Premise,
        source: "d1".into(),
        dependent: "d2".into(),
        change: "resolved".into(),
    };
    assert!(!strict(&e).consulted_outside());

    let p = pool();
    let adv = Exposure {
        edge: EdgeKind::Justification,
        ..e
    };
    let (q, judgment) = p
        .consult::<Adjudicator>(&adv, "survives_the_change")
        .unwrap();
    assert!(advisory(&adv, q, judgment).unwrap().consulted_outside());
}

/// A licence minted against one exposure does not settle another. A body of
/// questions is exactly where this matters: a judge that ruled "d2 survives"
/// has not thereby ruled on d4.
#[test]
fn a_ruling_on_one_exposure_does_not_carry_to_another() {
    let p = pool();
    let one = Exposure {
        scheme: DOCKET,
        edge: EdgeKind::Justification,
        source: "d1".into(),
        dependent: "d2".into(),
        change: "resolved".into(),
    };
    let other = Exposure {
        dependent: "d4".into(),
        ..one.clone()
    };

    let (q, judgment) = p
        .consult::<Adjudicator>(&one, "survives_the_change")
        .unwrap();
    assert!(matches!(
        propagation::survives_the_change::settle(&other, q, judgment),
        Err(rung_het::SettleError::TokenNotBound(_))
    ));
}

// ═════════════════════════════════════════════════════════════════════════
// P0, the write-guard, and the ladder — on the docket
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn p0_refuses_the_curator_as_a_judge_of_the_questions_it_filed() {
    let d = docket();
    let d3 = d.by_id("d3").expect("d3 is on the docket");

    let only_curator = Pool::new(vec![Person {
        id: "atlas-bot",
        prov: CURATOR.prov,
        roles: &["curator", "interrogator", "adjudicator"],
        stewards: CURATOR.stewards,
        dissents: false,
    }]);

    match only_curator.qualify::<Interrogator>(d3).unwrap_err() {
        rung_het::QualifyError::NonIdentityViolated { principal, shared } => {
            assert_eq!(principal, "atlas-bot");
            assert_eq!(shared, vec!["atlas-decisions".to_string()]);
        }
        other => panic!("the curator must not judge what it filed; got {other:?}"),
    }

    let (q, judgment) = pool()
        .consult::<Interrogator>(d3, "is_well_posed")
        .expect("the guild reviewer is disjoint from the docket");
    assert_eq!(q.principal_id(), "guild-reviewer");
    let settled = question::is_well_posed::settle(d3, q, judgment).expect("minted against d3");
    assert_eq!(settled.sentence(), "is_well_posed");
}

#[test]
fn the_done_pile_runs_its_own_law_on_a_write_the_ruling_already_authorized() {
    let mut d = docket();
    let p = pool();

    let pen = p
        .authorize::<Curator, _>(&CURATOR, "specs/decisions")
        .expect("the curator stewards the docket");

    // D4 cites no evidence; the done-pile will not take it, even though a
    // disjoint judge accepted the move (`target-runs-its-own-models`).
    let proposal = Proposal::remedy(&pen, "d4", QuestionEdit::Relocate { to: "resolved" });
    let judge = p.qualify_for::<Adjudicator>(&proposal).unwrap();
    let ruling = dispose(&proposal, judge, Disposition::Accept).unwrap();
    assert!(ruling.is_affirming());

    match enact(&mut d, &ruling, &pen) {
        Err(EnactError::TargetRefused { target, reason }) => {
            assert_eq!(target, "resolved");
            assert!(reason.contains("d4"));
        }
        other => panic!("the done-pile must refuse an unevidenced question; got {other:?}"),
    }
    assert_eq!(
        d.by_id("d4").unwrap().dir,
        "parked",
        "a refused write changes nothing"
    );

    // D6 carries an `evidence` edge, so the same edit lands.
    let proposal = Proposal::remedy(&pen, "d6", QuestionEdit::Relocate { to: "resolved" });
    let judge = p.qualify_for::<Adjudicator>(&proposal).unwrap();
    let ruling = dispose(&proposal, judge, Disposition::Accept).unwrap();
    assert_eq!(enact(&mut d, &ruling, &pen).unwrap().object(), "d6");
    assert_eq!(d.by_id("d6").unwrap().status, "resolved");
}

/// A different lifecycle path from the one rung's own questions take: D4 is
/// parked, so the judge returns it to `Gathered` rather than resolving it, and
/// the pen has to be minted over `parked/` rather than `open/`.
#[test]
fn a_parked_question_re_enters_at_gathered_rather_than_terminating() {
    let d = docket();
    let d4 = d.by_id("d4").expect("d4 is parked").clone();
    let p = pool();

    let pen = pen_over(&p, &CURATOR, "parked").expect("the curator stewards parked/");
    let gathered = questionlifecycle::gathered(questionlifecycle::Open::new(d4), pen);
    assert_eq!(
        gathered.payload.sources,
        vec!["specs/decisions/d4-schema-versioning.md".to_string()],
        "the source path is built from the Scheme, not from a literal"
    );

    let pen = pen_over(&p, &CURATOR, "parked").unwrap();
    let drafted = questionlifecycle::drafted(gathered, pen);

    let ruling = p
        .qualify_for::<Adjudicator>(&drafted.payload)
        .expect("the guild reviewer is disjoint from d4");
    match questionlifecycle::step(drafted, ruling) {
        Ok(questionlifecycle::StepOutcome::Parked(g)) => {
            assert_eq!(g.payload.question.id, "d4");
        }
        Ok(_) => panic!("d4 is parked; expected re-entry at Gathered"),
        Err(f) => panic!("the step failed: {}", f.error),
    }
}

/// A pen for one folder does not author in another — the docket's folders are
/// not rung's, and the check is still the pen's.
#[test]
#[should_panic(expected = "this pen authorizes")]
fn a_pen_for_one_folder_does_not_author_a_question_in_another() {
    let d = docket();
    let d3 = d.by_id("d3").expect("d3 is in blocked/").clone();
    let p = pool();
    let elsewhere = pen_over(&p, &CURATOR, "open").expect("the curator does steward open/");
    let _ = questionlifecycle::gathered(questionlifecycle::Open::new(d3), elsewhere);
}

#[test]
fn the_theory_exposes_its_sentences_with_their_gates() {
    assert_eq!(
        question::SENTENCES,
        &[
            ("id_matches_the_filename", "decidable"),
            ("status_is_declared", "decidable"),
            ("edge_kinds_are_declared", "decidable"),
            ("answerable_is_declared", "decidable"),
            ("ill_posed_filings_name_their_condition", "decidable"),
            ("is_well_posed", "judgmental"),
            ("resolution_answers_the_question", "judgmental"),
        ]
    );
    assert_eq!(
        questions::SENTENCES,
        &[
            ("every_dependency_resolves", "decidable"),
            ("ids_are_unique", "decidable"),
            ("every_declared_kind_is_lived", "decidable"),
            ("affects_mirrors_inbound", "decidable"),
            ("gate_edges_are_acyclic", "decidable"),
        ]
    );
    assert_eq!(
        propagation::SENTENCES,
        &[
            ("must_reexamine", "decidable"),
            ("survives_the_change", "judgmental"),
        ]
    );
    // `Sen(Σ)` for the theory is a hand-written concatenation, because
    // `theory!` declares one sort per invocation.
    assert_eq!(sentences().len(), 14);
    for (name, gate) in sentences() {
        assert!(
            matches!(gate, "decidable" | "judgmental"),
            "sentence `{name}` carries unknown gate `{gate}`"
        );
    }
}
/// **Well-posedness begins with the cold, decidable first cut**: a question
/// must declare what would count as an answer (`answerable:`) — the structural
/// anchor of existence/adequacy, and the one cut the audit can check without
/// an outside.
#[test]
fn the_cold_first_cut_is_declaring_an_answer() {
    let wp = Question::parse(
        DOCKET,
        "---\nid: w1\nstatus: open\nanswerable: |\n  a single determinate fact, reached by the structure: whether the driver is theory-blind.\n---\nbody\n",
        "open",
        "w1",
    )
    .expect("parses");
    assert!(wp.declares_resolution());
    let settled = question::answerable_is_declared::holds(&wp);
    assert!(
        !settled.consulted_outside(),
        "the first cut reads the declaration only"
    );
    assert!(settled.verdict().is_conforming());

    // A question that defaults to Mode A but never declares its answer is not
    // a member yet — it *claims* well-posedness and owes the anchor; the first
    // cut refuses it cold (this is the intake gate).
    let bare = Question::parse(
        DOCKET,
        "---\nid: w2\nstatus: open\n---\nbody\n",
        "open",
        "w2",
    )
    .expect("parses");
    assert!(bare.filing.is_well_posed(), "no filing declared -> Mode A");
    assert!(!bare.declares_resolution());
    assert!(
        !question::answerable_is_declared::holds(&bare)
            .verdict()
            .is_conforming()
    );
}

/// **Mode B is the escape hatch, and it is honest.** A question filed
/// `ill-posed` makes no well-posedness claim — `answerable` absent on purpose,
/// the ill-posed condition named — so the first cut passes vacuously (there is
/// no well-posedness asserted to fail), and the escape hatch itself is checked:
/// an ill-posed filing must actually name its condition.
#[test]
fn mode_b_claims_nothing_and_names_its_condition() {
    let mode_b = Question::parse(
        DOCKET,
        "---\nid: w3\nstatus: open\nfiling: ill-posed\nill_posed: |\n  this is a decision between two designs, not a determinate fact.\n---\nbody\n",
        "open",
        "w3",
    )
    .expect("parses");
    assert!(mode_b.filing.is_ill_posed());
    assert!(
        !mode_b.declares_resolution(),
        "Mode B carries no answerable"
    );
    assert!(
        question::answerable_is_declared::holds(&mode_b)
            .verdict()
            .is_conforming(),
        "Mode B claims nothing, so the first cut is vacuous — not a failure"
    );
    assert!(mode_b.names_its_ill_posed_condition());
    assert!(
        question::ill_posed_filings_name_their_condition::holds(&mode_b)
            .verdict()
            .is_conforming()
    );

    // ...and the escape hatch is not a silent opt-out: an ill-posed filing
    // that fails to name its condition is caught cold.
    let silent = Question::parse(
        DOCKET,
        "---\nid: w4\nstatus: open\nfiling: ill-posed\n---\nbody\n",
        "open",
        "w4",
    )
    .expect("parses");
    assert!(!silent.names_its_ill_posed_condition());
    assert!(
        !question::ill_posed_filings_name_their_condition::holds(&silent)
            .verdict()
            .is_conforming()
    );
}

/// **The authorial remedy set is conditioned on the judgment**
/// (`remedy-presupposes-the-judgment`). A question ruled ill-posed is re-filed
/// Mode B — `Refile` is the licensed remedy, and `AddEdge` is *not in* the set:
/// mirroring a structural edge cannot repair ill-posedness, so it is not a
/// remedy the judgment licenses.
#[test]
fn the_remedies_are_conditioned_on_the_judgment() {
    let d = docket();
    let ill = d.remedies_for(&JudgmentClass::IllPosed);
    assert!(
        ill.iter().any(|e| matches!(
            e,
            QuestionEdit::Refile {
                to: Filing::IllPosed,
                ..
            }
        )),
        "the ill-posed judgment licenses the Mode B re-file"
    );
    assert!(
        !ill.iter()
            .any(|e| matches!(e, QuestionEdit::AddEdge { .. })),
        "AddEdge is not a remedy for ill-posedness"
    );

    // a real option set, not a single forced move: repair (Rewrite) is the
    // primary remedy; demotion (Refile → Mode B) the fallback
    assert!(
        matches!(ill.first(), Some(QuestionEdit::Rewrite { .. })),
        "repair is the primary remedy an ill-posed judgment licenses"
    );
    assert!(
        ill.iter().any(|e| matches!(
            e,
            QuestionEdit::Refile {
                to: Filing::IllPosed,
                ..
            }
        )),
        "demotion to Mode B is the fallback remedy"
    );

    // a well-posed judgment licenses no remedy — there is nothing to fix
    assert!(d.remedies_for(&JudgmentClass::WellPosed).is_empty());
}

/// A `Rewrite` repairs the question into conformity: it stays Mode A with a
/// sharpened `answerable:` (ill_posed cleared), and the observer reads that back.
#[test]
fn rewriting_repairs_to_conformity_and_verifies() {
    let mut d = docket();
    let id = &d.questions[0].id.clone();
    let edit = QuestionEdit::Rewrite {
        answerable: "one determinate fact, unique and authentic".into(),
    };
    d.apply(id, &edit).expect("the world admits the repair");
    assert!(d.confirms(&edit, id), "the repair is observably in effect");
    let q = d.questions.iter().find(|x| &x.id == id).unwrap();
    assert_eq!(q.filing, Filing::WellPosed, "repair keeps Mode A");
    assert_eq!(
        q.answerable.as_deref(),
        Some("one determinate fact, unique and authentic")
    );
    assert_eq!(
        q.ill_posed, None,
        "a repaired question names no ill-posed condition"
    );
}

/// Re-filing applies and verifies: the filing flips to Mode B, `answerable` is
/// dropped (absent on purpose), and the condition is named — the observer reads
/// the post-state back, not the author's word.
#[test]
fn refiling_to_mode_b_applies_and_verifies() {
    let mut d = docket();
    let id = &d.questions[0].id.clone();
    let edit = QuestionEdit::Refile {
        to: Filing::IllPosed,
        condition: Some("this is a decision, not a determinate question".into()),
    };
    d.apply(id, &edit).expect("the world admits the re-file");
    assert!(!d.confirms(
        &QuestionEdit::AddEdge {
            target: "nope".into(),
            kind: EdgeKind::Premise,
        },
        id
    ));
    assert!(d.confirms(&edit, id), "the re-file is observably in effect");
    let q = d.questions.iter().find(|x| &x.id == id).unwrap();
    assert_eq!(q.filing, Filing::IllPosed);
    assert_eq!(q.answerable, None, "Mode B carries no answerable by design");
    assert_eq!(
        q.ill_posed.as_deref(),
        Some("this is a decision, not a determinate question")
    );
}

/// The other three cuts (unique, stable, authentic) are judgmental — but they
/// are judged **against the declared criterion**, not in a vacuum. `is_well_posed`
/// is ruled by an `Interrogator`; this pins that the sentence is a judgment
/// (the theory treats "the answer is found, not made" as the standard) and
/// that its shape is settled as such.
#[test]
fn the_deep_cuts_are_judgmental() {
    assert!(
        question::SENTENCES
            .iter()
            .any(|(n, g)| *n == "is_well_posed" && *g == "judgmental")
    );
}

// ═════════════════════════════════════════════════════════════════════════
// Gate acyclicity — a deadlock is not a slow answer
// ═════════════════════════════════════════════════════════════════════════

/// Build a docket from inline frontmatter, so a shape can be stated directly.
fn shaped(files: &[(&str, &str, &str)]) -> Questions {
    let parsed = files
        .iter()
        .map(|(dir, stem, text)| {
            Question::parse(DOCKET, text, dir, stem).expect("the frontmatter is well formed")
        })
        .collect();
    Questions::new(DOCKET, parsed)
}

#[test]
fn a_gate_cycle_is_a_deadlock_and_the_sentence_refuses_it() {
    // d1 blocked by d2, d2 blocked by d1. Neither can move, and no ruling
    // changes that — it is not a slow answer, it is a deadlock.
    let qs = shaped(&[
        (
            "blocked",
            "d1-a",
            "---\nid: d1\nstatus: blocked\ndepends_on:\n  - {on: d2, kind: gate}\n---\nx\n",
        ),
        (
            "blocked",
            "d2-b",
            "---\nid: d2\nstatus: blocked\ndepends_on:\n  - {on: d1, kind: gate}\n---\nx\n",
        ),
    ]);

    let cycles = qs.gate_cycles();
    assert_eq!(
        cycles.len(),
        1,
        "one cycle, reported once, not once per entry"
    );
    let members: std::collections::BTreeSet<&str> = cycles[0].iter().map(String::as_str).collect();
    assert_eq!(members, ["d1", "d2"].into_iter().collect());

    assert!(
        matches!(
            questions::gate_edges_are_acyclic::holds(&qs).verdict(),
            rung::Verdict::NonConforming { .. }
        ),
        "a gate cycle must not be conforming"
    );
}

#[test]
fn nesting_is_not_a_cycle_a_premise_up_and_a_gate_down() {
    // The shape this repository's own Q11 and Q12 have. Answering d1 raised d2;
    // d2's framing rests on d1 (premise, upward); d1 waits on d2's answer
    // (gate, downward). Two edges, opposite directions, different kinds.
    //
    // A naive any-edge acyclicity check calls this a loop and is wrong: it is
    // what healthy nesting looks like, and flagging it would make the sentence
    // fire on every question that ever raised another.
    let qs = shaped(&[
        (
            "open",
            "d1-parent",
            "---\nid: d1\nstatus: open\ndepends_on:\n  - {on: d2, kind: gate}\naffects:\n  - {target: d2, kind: premise}\n---\nx\n",
        ),
        (
            "open",
            "d2-raised-while-answering-d1",
            "---\nid: d2\nstatus: open\ndepends_on:\n  - {on: d1, kind: premise}\naffects:\n  - {target: d1, kind: gate}\n---\nx\n",
        ),
    ]);

    assert!(
        qs.gate_cycles().is_empty(),
        "premise-up + gate-down is nesting, not a deadlock; got {:?}",
        qs.gate_cycles()
    );
    assert!(matches!(
        questions::gate_edges_are_acyclic::holds(&qs).verdict(),
        rung::Verdict::Conforming
    ));
}

#[test]
fn the_docket_has_no_gate_cycle() {
    assert!(matches!(
        questions::gate_edges_are_acyclic::holds(&docket()).verdict(),
        rung::Verdict::Conforming
    ));
}
