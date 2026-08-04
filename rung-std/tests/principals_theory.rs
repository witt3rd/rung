//! The **principals** theory over two synthetic rosters.
//!
//! `rung-std::principals` is a supplier of `𝒫`. What it declares is the *law* of
//! who may be dispatched to; a population of principals is a **model** of that
//! law, and models are a deployment's. This file holds two of them, chosen to
//! share nothing:
//!
//! | | roster A | roster B |
//! |---|---|---|
//! | namespace | `atlas-bench` | `orchard-council` |
//! | roles | `category-theorist`, `structural-auditor` | `pruner`, `grafter` |
//! | ids | `nine-b`, `mirabel`, `swarm`, `chorus`, `deep-prover` | `hollis`, `sap-reader` |
//! | kinds in use | all four | two |
//! | qualification atoms | reasoning, rule-following, category-theory | shears, graft, patience |
//!
//! Only two role names are shared, and they are shared because they are the
//! *library's own*: `examiner` and `taxonomist` are `role(φ)` for this theory's
//! two judgmental sentences, which Het requires be declared and does not
//! enumerate (`role-declared-not-enumerated`). Everything else in the two
//! columns is a deployment's, and
//! [`the_library_names_no_role_or_principal_of_either_roster`] checks that no
//! part of it leaked into `rung-std`.

use rung_std::principals::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rung::{Principal, Prov, Provenanced, Response, Role, Settled, Steward, Verdict};
use rung_het::{Pool, QualifyError};

// ═════════════════════════════════════════════════════════════════════════
// Roster A — a benchmarking bench
// ═════════════════════════════════════════════════════════════════════════

const CATEGORY_THEORIST: RoleSpec = RoleSpec {
    name: "category-theorist",
    min_qualifications: &["strong-reasoning", "category-theory"],
};
const STRUCTURAL_AUDITOR: RoleSpec = RoleSpec {
    name: "structural-auditor",
    min_qualifications: &["rule-following"],
};

/// A principal, with no standing anywhere and a placeholder ε. Standing and ε
/// are added by the two combinators below where a case turns on them, so that a
/// roster reads as the population it is rather than as a wall of arguments.
fn decl(
    id: &str,
    kind: Kind,
    identity: &[(&str, &str)],
    quals: &[&str],
    plays: &[RoleSpec],
    prov: &[&str],
) -> PrincipalDecl {
    PrincipalDecl {
        id: id.to_string(),
        kind,
        identity: identity
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect::<BTreeMap<_, _>>(),
        qualifications: quals
            .iter()
            .map(|q| (*q).to_string())
            .collect::<BTreeSet<_>>(),
        plays: plays.to_vec(),
        provenance: prov
            .iter()
            .map(|p| (*p).to_string())
            .collect::<BTreeSet<_>>(),
        stewards: BTreeSet::new(),
        epsilon: Some(Epsilon::declared(0.1)),
    }
}

fn stewarding(mut p: PrincipalDecl, containers: &[&str]) -> PrincipalDecl {
    p.stewards = containers.iter().map(|c| (*c).to_string()).collect();
    p
}

fn with_epsilon(mut p: PrincipalDecl, bound: f64) -> PrincipalDecl {
    p.epsilon = Some(Epsilon::declared(bound));
    p
}

/// Deliberately **not** in cost order. `mirabel` is a human — the scarcest
/// substrate this partition declares — and stands first; `deep-prover` is a
/// language model and stands last. A minimal-judge rule would reverse them, and
/// that is what makes the α cut observable here rather than merely asserted.
fn roster_a() -> Roster {
    Roster {
        namespace: "atlas-bench",
        roles: vec![
            CATEGORY_THEORIST,
            STRUCTURAL_AUDITOR,
            Examiner::spec(),
            Taxonomist::spec(),
        ],
        principals: vec![
            decl(
                "nine-b",
                Kind::Llm,
                &[("provider", "hexcorp"), ("model_id", "nine-b")],
                &["rule-following"],
                &[STRUCTURAL_AUDITOR],
                &["hexcorp"],
            ),
            stewarding(
                with_epsilon(
                    decl(
                        "mirabel",
                        Kind::Human,
                        &[("authority", "maintainer")],
                        &[
                            "strong-reasoning",
                            "category-theory",
                            "competence-assessment",
                            "substrate-taxonomy",
                        ],
                        &[CATEGORY_THEORIST, Examiner::spec(), Taxonomist::spec()],
                        &["mirabel"],
                    ),
                    0.02,
                ),
                &["specs/atlas"],
            ),
            decl(
                "swarm",
                Kind::Agent,
                &[("orchestration_id", "sw-1"), ("tools", "evals,fetch")],
                &["rule-following", "competence-assessment"],
                &[STRUCTURAL_AUDITOR, Examiner::spec()],
                &["swarm-works"],
            ),
            with_epsilon(
                decl(
                    "chorus",
                    Kind::RelationalBeing,
                    &[("constellation", "atlas-circle")],
                    &["mutual-stake", "competence-assessment"],
                    &[Examiner::spec()],
                    &["atlas-circle"],
                ),
                0.2,
            ),
            decl(
                "deep-prover",
                Kind::Llm,
                &[("provider", "orbital"), ("model_id", "dp-2")],
                &[
                    "strong-reasoning",
                    "category-theory",
                    "competence-assessment",
                    "substrate-taxonomy",
                ],
                &[CATEGORY_THEORIST, Examiner::spec(), Taxonomist::spec()],
                &["orbital"],
            ),
        ],
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Roster B — a different population, a different vocabulary
// ═════════════════════════════════════════════════════════════════════════

const PRUNER: RoleSpec = RoleSpec {
    name: "pruner",
    min_qualifications: &["shears-competence"],
};
const GRAFTER: RoleSpec = RoleSpec {
    name: "grafter",
    min_qualifications: &["graft-competence", "patience"],
};

fn roster_b() -> Roster {
    Roster {
        namespace: "orchard-council",
        roles: vec![PRUNER, GRAFTER, Examiner::spec(), Taxonomist::spec()],
        principals: vec![
            stewarding(
                decl(
                    "hollis",
                    Kind::Human,
                    &[("authority", "orchard-steward")],
                    &[
                        "shears-competence",
                        "graft-competence",
                        "patience",
                        "competence-assessment",
                        "substrate-taxonomy",
                    ],
                    &[PRUNER, GRAFTER, Examiner::spec(), Taxonomist::spec()],
                    &["hollis"],
                ),
                &["orchard/north"],
            ),
            decl(
                "sap-reader",
                Kind::Llm,
                &[("provider", "greenline"), ("model_id", "sr-1")],
                &["shears-competence", "competence-assessment"],
                &[PRUNER, Examiner::spec()],
                &["greenline"],
            ),
        ],
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Sen(Σ)
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn the_theory_exposes_its_sentences_with_their_gates() {
    assert_eq!(
        principal::SENTENCES,
        &[
            ("identity_fields_are_declared", "decidable"),
            ("roles_are_earned", "decidable"),
            ("cost_is_declared", "decidable"),
            ("epsilon_is_declared", "decidable"),
            ("competence_claim_is_true", "judgmental"),
        ]
    );
    assert_eq!(
        roster::SENTENCES,
        &[
            ("ids_are_unique", "decidable"),
            ("every_played_role_is_declared", "decidable"),
            ("every_declared_role_is_played", "decidable"),
            ("kind_partition_is_adequate", "judgmental"),
        ]
    );
    assert_eq!(sentences().len(), 9);
    for (name, gate) in sentences() {
        assert!(
            matches!(gate, "decidable" | "judgmental"),
            "sentence `{name}` carries unknown gate `{gate}`"
        );
    }
}

/// One theory, two carriers. Every decidable sentence of both sorts holds over
/// both rosters, and none of them consults an outside.
#[test]
fn every_decidable_sentence_holds_over_both_rosters() {
    for r in [roster_a(), roster_b()] {
        for p in &r.principals {
            for settled in [
                principal::identity_fields_are_declared::holds(p),
                principal::roles_are_earned::holds(p),
                principal::cost_is_declared::holds(p),
                principal::epsilon_is_declared::holds(p),
            ] {
                assert!(
                    !settled.consulted_outside(),
                    "a decidable sentence must not report an outside call"
                );
                assert!(
                    settled.verdict().is_conforming(),
                    "{} violates `{}` in {}",
                    p.id,
                    settled.sentence(),
                    r.namespace
                );
            }
        }
        for settled in [
            roster::ids_are_unique::holds(&r),
            roster::every_played_role_is_declared::holds(&r),
            roster::every_declared_role_is_played::holds(&r),
        ] {
            assert!(!settled.consulted_outside());
            assert!(
                settled.verdict().is_conforming(),
                "{} violates `{}`",
                r.namespace,
                settled.sentence()
            );
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Capability — the comparison is mechanical, or it is a claim
// ═════════════════════════════════════════════════════════════════════════

/// **The claim the theory has to make true.** Capability is decidable by
/// structural inspection: the role declares its minimum qualifications, the
/// principal declares what it is, and the comparison is a subset test.
///
/// A principal that *claims* a role it has not earned is refused by `capable`
/// itself, not merely reported by a sentence — which is the difference between
/// a declaration that is checked and one that is believed.
#[test]
fn capability_is_a_mechanical_comparison_and_a_claimed_role_is_not_an_earned_one() {
    let r = roster_a();
    let nine_b = r.by_id("nine-b").expect("nine-b is on the bench");

    assert!(nine_b.capable("structural-auditor"));
    assert!(!nine_b.capable("category-theorist"));

    // The pretender plays `category-theorist` and declares none of its
    // qualifications. The role is claimed; it is not earned.
    let pretender = decl(
        "pretender",
        Kind::Llm,
        &[("provider", "hexcorp"), ("model_id", "p-0")],
        &["rule-following"],
        &[CATEGORY_THEORIST],
        &["hexcorp"],
    );
    assert_eq!(
        pretender.unearned_roles(),
        vec!["category-theorist"],
        "the comparison is a subset test over declared atoms"
    );
    assert!(
        !pretender.capable("category-theorist"),
        "`capable` runs the comparison; it does not read the claim"
    );
    assert!(
        !principal::roles_are_earned::holds(&pretender)
            .verdict()
            .is_conforming()
    );

    // And the refusal reaches the pool: the left conjunct of both filters.
    let only_pretender = Pool::new(vec![pretender]);
    let subject = r.by_id("mirabel").unwrap();
    assert!(matches!(
        only_pretender.qualify_for::<CategoryTheoristProbe>(subject),
        Err(QualifyError::NotCapable { .. })
    ));
}

/// A role name this test declares so it can ask the pool for a competence the
/// *roster* declares. `role(φ)` is a theory's; a probe is a caller's.
#[derive(Clone, Copy)]
struct CategoryTheoristProbe;
impl Role for CategoryTheoristProbe {
    const NAME: &'static str = "category-theorist";
}

/// **`role-not-kind`.** Kind is what a principal is made of; role is what the
/// sentence needs done. The two axes are independent: `examiner` is played
/// across all four kinds in roster A, and no kind is entitled to every role —
/// `nine-b` is a language model and is not an examiner.
#[test]
fn role_is_not_kind_and_the_two_axes_are_independent() {
    let r = roster_a();
    let examiner_kinds: BTreeSet<&str> = r
        .principals
        .iter()
        .filter(|p| p.capable(Examiner::NAME))
        .map(|p| p.kind.name())
        .collect();
    assert_eq!(
        examiner_kinds,
        BTreeSet::from(["llm", "agent", "relational-being", "human"]),
        "one role across the whole substrate partition"
    );
    assert!(!r.by_id("nine-b").unwrap().capable(Examiner::NAME));

    // …and the same role name in roster B is played by two kinds that share no
    // qualification atom with roster A's.
    let b = roster_b();
    let pruner_kinds: BTreeSet<&str> = b
        .principals
        .iter()
        .filter(|p| p.capable("pruner"))
        .map(|p| p.kind.name())
        .collect();
    assert_eq!(pruner_kinds, BTreeSet::from(["llm", "human"]));
}

/// Each kind fixes the identity fields a principal of that kind must declare.
/// The partition is closed and the field list is the supplier's; a principal
/// missing one of its kind's fields is not well-formed.
#[test]
fn a_kind_fixes_its_identity_fields_and_a_principal_missing_one_is_refused() {
    assert_eq!(KINDS.len(), 4);
    assert_eq!(
        Kind::Llm.required_identity_fields(),
        &["provider", "model_id"]
    );
    assert_eq!(
        Kind::Agent.required_identity_fields(),
        &["orchestration_id", "tools"]
    );
    assert_eq!(
        Kind::RelationalBeing.required_identity_fields(),
        &["constellation"]
    );
    assert_eq!(Kind::Human.required_identity_fields(), &["authority"]);

    let nameless = decl(
        "nameless",
        Kind::Llm,
        &[("provider", "hexcorp")],
        &["rule-following"],
        &[STRUCTURAL_AUDITOR],
        &["hexcorp"],
    );
    assert_eq!(nameless.missing_identity_fields(), vec!["model_id"]);
    assert!(
        !principal::identity_fields_are_declared::holds(&nameless)
            .verdict()
            .is_conforming()
    );
}

// ═════════════════════════════════════════════════════════════════════════
// One pool, two filters
// ═════════════════════════════════════════════════════════════════════════

/// `one-pool-two-filters` as a function of the roster. The same population,
/// filtered two ways: the judgmental filter refuses provenance overlap, the
/// authorial filter demands standing over a named container. Neither consults a
/// second pool.
#[test]
fn one_pool_two_filters_over_the_same_roster() {
    let r = roster_a();
    let subject = r.by_id("nine-b").expect("nine-b is on the bench");

    let judges = qualifying_set::<Examiner>(&r, &Filter::NonIdentical(subject));
    let judge_ids: Vec<&str> = judges.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(judge_ids, ["mirabel", "swarm", "chorus", "deep-prover"]);

    let authors = qualifying_set::<Examiner>(&r, &Filter::Standing("specs/atlas"));
    let author_ids: Vec<&str> = authors.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(author_ids, ["mirabel"]);

    // Same pool. The gate marker chose the predicate, not the population.
    assert_eq!(r.principals.len(), 5);
    assert!(r.by_id("mirabel").unwrap().has_standing("specs/atlas"));
    assert!(!r.by_id("deep-prover").unwrap().has_standing("specs/atlas"));
}

/// **`no-preference-among-judges`, shown rather than assumed.**
///
/// The qualifying set here has four members. Each of them, on its own, mints a
/// licence against the very same argument and settles the very same sentence —
/// so *any* of them is a well-formed dispatch, which is what the proposition
/// says and what a single-survivor API could only imply.
///
/// Truncating [`qualifying_set`] to its first member is type-valid and turns
/// this red at the fourth settlement.
#[test]
fn every_member_of_the_qualifying_set_is_a_well_formed_dispatch() {
    let r = roster_a();
    let claimant = r.by_id("nine-b").expect("nine-b is on the bench");
    let set = qualifying_set::<Examiner>(&r, &Filter::NonIdentical(claimant));
    assert_eq!(set.len(), 4, "the set is a set, not a choice already made");

    let mut who = Vec::new();
    for member in &set {
        // A pool of one: the member is the only survivor, so the licence is
        // demonstrably that member's.
        let seat = Pool::new(vec![(*member).clone()]);
        let (q, judgment) = seat
            .consult::<Examiner>(claimant, "competence_claim_is_true")
            .expect("this member is in the qualifying set");
        let settled = principal::competence_claim_is_true::settle(claimant, q, judgment)
            .expect("the licence and the judgment are this member's");
        match settled {
            Settled::Judgmental {
                sentence,
                role,
                principal,
                judgment,
            } => {
                assert_eq!(sentence, "competence_claim_is_true");
                assert_eq!(role, "examiner");
                assert!(judgment.verdict().is_conforming());
                assert_eq!(
                    judgment.judge_id(),
                    principal,
                    "the receipt names the judge that spoke, because the verdict \
                     came sealed with its provenance rather than as a parameter"
                );
                who.push(principal);
            }
            other => panic!("a judgmental sentence must report its outside; got {other:?}"),
        }
    }
    assert_eq!(who, ["mirabel", "swarm", "chorus", "deep-prover"]);
}

/// **The α cut, part one: the set is not ordered by cost.**
///
/// `ordering-is-hetopts`. Cost tier is declared per kind, and roster A is laid
/// out so the qualifying set opens with the costliest substrate and closes with
/// the cheapest. If anything in this theory ranked, the order would invert.
///
/// Deriving `Ord` on [`CostTier`] and sorting the set by it in
/// [`qualifying_set`] is type-valid and turns this red.
#[test]
fn the_qualifying_set_is_not_ordered_by_cost() {
    let r = roster_a();
    let claimant = r.by_id("nine-b").expect("nine-b is on the bench");
    let set = qualifying_set::<Examiner>(&r, &Filter::NonIdentical(claimant));

    let kinds: Vec<&str> = set.iter().map(|p| p.kind.name()).collect();
    assert_eq!(kinds, ["human", "agent", "relational-being", "llm"]);
    assert_eq!(set[0].cost_tier(), Kind::Human.cost_tier());
    assert_eq!(set[3].cost_tier(), Kind::Llm.cost_tier());
    assert_ne!(Kind::Human.cost_tier(), Kind::Llm.cost_tier());

    // `Pool::qualify_for` takes the first survivor in declaration order, and
    // declaration order is not cost order: the human is picked over the model.
    let picked = r
        .pool()
        .qualify_for::<Examiner>(claimant)
        .expect("four principals qualify");
    assert_eq!(picked.principal_id(), "mirabel");
}

/// **The α cut, part two, and the first time it has teeth.**
///
/// `het-declares-no-worth-law`: a Het theory declares no worth-law and does not
/// declare the minimal-judge rule. Until cost tier existed anywhere in this
/// workspace the proposition had nothing to bite on. It does now — so this test
/// reads every line of Rust in the workspace that mentions a cost tier or an ε
/// and refuses any that also orders, compares, sorts, or ranks.
///
/// The scan covers attribute lines above a hit as well, so adding `Ord` to
/// [`CostTier`]'s derive is caught even though the derive line names no cost of
/// its own. That is the cheapest possible mutation and it is the one this test
/// exists to see.
#[test]
fn nothing_in_the_workspace_orders_by_cost_or_epsilon() {
    // Written as separate tables so that neither line carries both halves.
    const DECLARED: &[&str] = &["cost_tier", "CostTier", "epsilon", "Epsilon"];
    const ORDERS: &[&str] = &[
        " < ", " > ", "<=", ">=", "sort", ".min(", "::min", ".max(", "::max", "cmp", "Ord", "rank",
        "cheap", "argmin", "prefer",
    ];

    let mut offences: Vec<String> = Vec::new();
    let mut sighted = 0usize;

    for file in workspace_sources() {
        let text = std::fs::read_to_string(&file).expect("a source file is readable");
        let lines: Vec<&str> = text.split('\n').collect();
        for (i, raw) in lines.iter().enumerate() {
            let code = strip_comment(raw);
            if !DECLARED.iter().any(|d| code.contains(d)) {
                continue;
            }
            sighted += 1;
            let mut ctx = String::new();
            if i > 0 && lines[i - 1].trim_start().starts_with("#[") {
                ctx.push_str(strip_comment(lines[i - 1]));
                ctx.push('\n');
            }
            ctx.push_str(code);
            for order in ORDERS {
                if ctx.contains(order) {
                    offences.push(format!(
                        "{}:{}: `{}` next to a declared cost or ε — {}",
                        file.display(),
                        i + 1,
                        order.trim(),
                        code.trim()
                    ));
                }
            }
        }
    }

    assert!(
        sighted >= 10,
        "the scan saw only {sighted} declarations; a scan that sees nothing \
         cannot refuse anything"
    );
    assert!(
        offences.is_empty(),
        "the α cut is drawn at valuation itself. Cost tier and ε are declared \
         here and ranked nowhere; the minimal-judge and minimal-author rules \
         read exactly these fields and are HetOpt's:\n  {}",
        offences.join("\n  ")
    );
}

/// The other half of the same cut: what is declared *is* declared. A cut that
/// held because nothing was ever written down would be vacuous.
#[test]
fn cost_is_declared_per_kind_and_epsilon_per_principal() {
    let tiers: Vec<CostTier> = KINDS.iter().map(|k| k.cost_tier()).collect();
    for (a, x) in tiers.iter().enumerate() {
        for y in &tiers[a + 1..] {
            assert_ne!(x, y, "each kind declares its own tier");
        }
    }
    for r in [roster_a(), roster_b()] {
        for p in &r.principals {
            assert_eq!(p.cost_tier(), p.kind.cost_tier());
            // ε is declared. It is also unread: `Epsilon` exposes no accessor
            // and no comparison, so the only fact available about one is that
            // it is there. The reader HetOpt would add is the seam, and it is
            // empty.
            assert!(p.epsilon.is_some(), "every principal declares an ε");
            assert!(
                principal::epsilon_is_declared::holds(p)
                    .verdict()
                    .is_conforming()
            );
        }
    }
}

/// …and the ε sentence can fail, which is what makes it a sentence rather than
/// a restatement of the type. Declaring ε is the half of
/// `epsilon-declared-not-ranked` this theory can honour; carrying it out to the
/// verdict is the half it cannot.
#[test]
fn a_principal_that_declares_no_epsilon_is_not_well_formed() {
    let mut p = roster_a().by_id("nine-b").expect("on the bench").clone();
    p.epsilon = None;
    assert!(
        !principal::epsilon_is_declared::holds(&p)
            .verdict()
            .is_conforming()
    );
}

// ═════════════════════════════════════════════════════════════════════════
// P0, over a theory whose subject matter is the outside itself
// ═════════════════════════════════════════════════════════════════════════

/// A principal is itself an object under some theory, and may be examined. The
/// non-identity filter is what keeps that from collapsing: nobody rules on the
/// truth of their own competence claim.
#[test]
fn p0_refuses_a_principal_as_the_examiner_of_its_own_competence_claim() {
    let r = roster_a();
    let mirabel = r.by_id("mirabel").expect("mirabel is on the bench");

    let herself = Pool::new(vec![mirabel.clone()]);
    match herself.qualify_for::<Examiner>(mirabel).unwrap_err() {
        QualifyError::NonIdentityViolated { principal, shared } => {
            assert_eq!(principal, "mirabel");
            assert_eq!(shared, vec!["mirabel".to_string()]);
        }
        other => panic!("nobody examines their own claim; got {other:?}"),
    }

    let q = r
        .pool()
        .qualify_for::<Examiner>(mirabel)
        .expect("three others are disjoint from mirabel");
    assert_ne!(q.principal_id(), "mirabel");
}

/// A principal that rules the other way, wrapping a declared one.
///
/// `PrincipalDecl` is a *declaration* — a record of who a principal is, with no
/// channel to an actual outside — so its `rule` affirms whatever it is asked,
/// which is this supplier's honest limit and is recorded as such in
/// `rung_std::principals`. Reaching the other arm of a coproduct therefore
/// means locating a principal who takes it, and that is the point: under R2 a
/// test cannot arrange a verdict, only choose whom to ask.
struct Dissenting(PrincipalDecl);

impl Principal for Dissenting {
    fn capable(&self, role_name: &str) -> bool {
        self.0.capable(role_name)
    }
    fn id(&self) -> &str {
        self.0.id()
    }
    fn authored(&self) -> Prov {
        self.0.authored()
    }
    fn rule(&self, matter: &str) -> Response {
        Response::Rendered(Verdict::NonConforming {
            reason: format!("`{matter}` does not hold, and I am the one asked"),
        })
    }
}

/// The kind partition is a *claim*, and a claim that cannot be questioned is a
/// stipulation. Whether four kinds are the right partition is settled by an
/// outside with taxonomic competence — never computed from the enum.
#[test]
fn the_kind_partition_is_ruled_on_by_an_outside_and_not_computed() {
    let r = roster_a();
    let outside = Pool::new(vec![Dissenting(
        roster_b()
            .by_id("hollis")
            .expect("hollis keeps the orchard")
            .clone(),
    )]);
    let (q, judgment) = outside
        .consult::<Taxonomist>(&r, "kind_partition_is_adequate")
        .expect("the orchard shares no provenance with the bench");
    let settled = roster::kind_partition_is_adequate::settle(&r, q, judgment)
        .expect("the licence and the judgment are hollis's");
    assert!(settled.consulted_outside());
    assert!(
        !settled.verdict().is_conforming(),
        "an outside that fits none of the four falsifies the partition — and it \
         is the outside that says so. This test can no longer arrange the \
         answer it wants: it has to find a principal who gives it"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// The gap left open
// ═════════════════════════════════════════════════════════════════════════

/// **ε is declared and never reaches the verdict.**
///
/// `epsilon-declared-not-ranked` asks two things: that ε be declared so the
/// verdict can carry its error bar, and that Het never read it as a preference.
/// The second half holds — nothing here ranks by it, and
/// [`nothing_in_the_workspace_orders_by_cost_or_epsilon`] enforces that. The
/// first half does not: `Settled` has four fields and none of them is an error
/// bar, so two principals whose declared ε differ by an order of magnitude
/// return settlements that are the same object.
///
/// This is a different gap from the one `rung-het` parks on
/// `epsilon-reported-with-verdict`. That one asks whether a *judge's confidence*
/// is expressible at all. This one asks whether the ε **the supplier already
/// declares** reaches the caller — and the answer is that there is no field for
/// it to reach.
///
/// **Ignored, deliberately.** Nothing below is broken; ε is declared and unread
/// by construction, and this theory says so in its own docs.
#[test]
#[ignore = "GAP: `Epsilon` is declared per principal and nothing reads it. \
            `Settled::Judgmental` carries sentence, role, principal and a \
            sealed `Judgment` — there is no field for an error bar, so the ε a \
            supplier declares cannot reach the caller. Closing this needs a verdict \
            space carrying a metric (rung-het-props.md#verdict-space-with-metric) \
            and an ε on `Settled` sourced from the principal that rendered the \
            verdict. Unpark by deleting this attribute once `Settled` carries \
            the declared ε; the two settlements below must then differ by it."]
fn a_verdict_carries_the_declared_epsilon_of_the_principal_that_rendered_it() {
    let r = roster_a();
    let claimant = r.by_id("nine-b").expect("nine-b is on the bench");

    // ε 0.02 against ε 0.2 — an order of magnitude apart, by declaration.
    let settle_by = |id: &str| {
        let seat = Pool::new(vec![r.by_id(id).expect("on the bench").clone()]);
        let (q, judgment) = seat
            .consult::<Examiner>(claimant, "competence_claim_is_true")
            .expect("disjoint");
        principal::competence_claim_is_true::settle(claimant, q, judgment)
            .expect("minted against this claimant")
    };

    assert_ne!(
        settle_by("mirabel").verdict(),
        settle_by("chorus").verdict(),
        "the ε a principal declares is the error bar its verdict is reported \
         with. Two verdicts of the same polarity, from principals whose \
         declared ε differ by an order of magnitude, are not one settlement"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// Genericity, and the interface
// ═════════════════════════════════════════════════════════════════════════

/// **`nothing-further-required`, from the supplier's side.**
///
/// Het requires four predicates of a supplier and nothing further: kinds,
/// substrate partitions, identity fields, cost tiers and the population are the
/// supplier's. All five of those are declared in `rung-std::principals` and
/// none of them crosses into `rung` — the pool qualifies a principal reading
/// only `capable` and `π`, and the licence it mints carries an id, a
/// provenance and a role.
///
/// `capable-single-arity` falls out of the same shape: `capable`'s second
/// argument is a role *name*. A sentence name is not a role, and passing one
/// gets the answer that fact deserves.
#[test]
fn nothing_further_than_the_declared_interface_crosses_into_rung() {
    // The interface, at exactly the arities Het names.
    let capable: fn(&PrincipalDecl, &str) -> bool = <PrincipalDecl as Principal>::capable;
    let pi: fn(&PrincipalDecl) -> Prov = <PrincipalDecl as Provenanced>::provenance;
    let standing: fn(&PrincipalDecl, &str) -> bool = <PrincipalDecl as Steward>::has_standing;

    let r = roster_a();
    let mirabel = r.by_id("mirabel").expect("mirabel is on the bench");

    assert!(capable(mirabel, Examiner::NAME));
    assert!(
        !capable(mirabel, "competence_claim_is_true"),
        "a supplier is asked which ROLE it can play, never which SENTENCE it \
         can settle — it does not have this theory's sentences"
    );
    assert_eq!(pi(mirabel), Prov::of(["mirabel"]));
    assert!(standing(mirabel, "specs/atlas"));

    // The kind, its identity fields, its cost tier and the roster are all
    // declared here and none of them appears in the licence.
    let q = r
        .pool()
        .qualify_for::<Examiner>(r.by_id("nine-b").unwrap())
        .expect("four qualify");
    assert_eq!(q.principal_id(), "mirabel");
    assert_eq!(q.role_name(), "examiner");
    assert_eq!(format!("{q:?}"), "Qualified<examiner>(mirabel)");
}

/// **Genericity, checked rather than claimed.** The theory is shared; the
/// roster is a deployment's. If a role name, a qualification atom or a
/// principal id from either roster had leaked into the library, this fails.
#[test]
fn the_library_names_no_role_or_principal_of_either_roster() {
    // `examiner` and `taxonomist` are absent from this list on purpose: they
    // are the library's own `role(φ)`, declared by the theory for its two
    // judgmental sentences.
    const DEPLOYMENT_ONLY: &[&str] = &[
        "category-theorist",
        "structural-auditor",
        "pruner",
        "grafter",
        "nine-b",
        "mirabel",
        "swarm",
        "chorus",
        "deep-prover",
        "hollis",
        "sap-reader",
        "atlas-bench",
        "orchard-council",
        "strong-reasoning",
        "rule-following",
        "shears-competence",
        "graft-competence",
    ];

    let root = workspace_root();
    let mut leaks = Vec::new();
    for crate_dir in ["rung", "rung-het", "rung-std", "rung-macro"] {
        for file in sources_under(&root.join(crate_dir).join("src")) {
            let text = std::fs::read_to_string(&file).expect("a source file is readable");
            for name in DEPLOYMENT_ONLY {
                if text.contains(name) {
                    leaks.push(format!("{}: names `{name}`", file.display()));
                }
            }
        }
    }
    assert!(
        leaks.is_empty(),
        "the population is a model of the theory, and a library that knew one \
         roster would be a deployment wearing a library's name:\n  {}",
        leaks.join("\n  ")
    );
}

// ═════════════════════════════════════════════════════════════════════════
// Source-reading helpers
// ═════════════════════════════════════════════════════════════════════════

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-std sits in the workspace")
        .to_path_buf()
}

fn workspace_sources() -> Vec<PathBuf> {
    let root = workspace_root();
    let mut out = Vec::new();
    for crate_dir in ["rung", "rung-het", "rung-std", "rung-macro"] {
        out.extend(sources_under(&root.join(crate_dir)));
    }
    out.sort();
    out
}

fn sources_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            out.extend(sources_under(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out
}

/// Everything from the first `//` onward is prose. A cut drawn against prose
/// would forbid *describing* the cut, which is the opposite of the intent.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}
