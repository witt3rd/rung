//! Canonical **principals** theory — the third building block in rung-std.
//!
//! ## What this is
//!
//! Het declares that certain operations require an outside. It does not say
//! what an outside *is*: `𝒫` is opaque (`pool-is-opaque`), and Het requires of
//! a supplier exactly four predicates and nothing further
//! (`nothing-further-required`). This module is a **supplier** — the law of who
//! may be dispatched to, for the pool that `rung`'s two gates draw from.
//!
//! `rung` already supplies the *interface*: `Provenanced` is `π`, `Role` is the
//! competence a sentence declares, `Principal::capable` is the competence
//! filter, `Steward::has_standing` is the standing predicate, and `Pool` mints
//! `Qualified` and `Authorized` against them. None of that is re-declared here.
//! What is declared here is the content Het refuses to name.
//!
//! ## This is one supplier's choice, not doctrine
//!
//! `nothing-further-required` is explicit that **kinds, substrate partitions,
//! identity fields, cost tiers, and the population itself are the supplier's**.
//! Naming any of them inside Het would internalize the outside a second way,
//! not as a sort but as a stipulated content. A supplier, by contrast, must
//! name them or it has supplied nothing.
//!
//! So [`Kind`] below is a **closed partition of four**, with required identity
//! fields and a cost tier per kind — and it is *this module's*, in the same
//! sense that the seven edge kinds of [`questions`](crate::questions) are that
//! theory's. Another supplier may partition differently and remain conformant.
//! The closure is a claim, and a claim that cannot be questioned is a
//! stipulation, which is why [`roster::kind_partition_is_adequate`] is a
//! judgmental sentence rather than a comment.
//!
//! ## Why "principals" and not "judges"
//!
//! Judgment is not all the pool does. `one-pool-two-filters`: there is one
//! pool and two filters, and the gate marker selects which qualification
//! predicate applies, not which pool is consulted. A judge is a principal
//! filtered by non-identity; an author is a principal filtered by standing.
//! They are one population under two filters — [`Filter`] is that pair as a
//! type — so a theory named `judges` would govern half its own subject matter.
//!
//! ## Belonging, never ordering — the α cut
//!
//! Capability, non-identity and standing are **belonging** predicates
//! (`three-belonging-predicates`): they decide whether a principal qualifies at
//! all. [`CostTier`] and [`Epsilon`] support **ordering** among those that
//! qualify, and ordering is HetOpt's (`ordering-is-hetopts`). Both are declared
//! here and ranked nowhere.
//!
//! That is a discipline, so it is checked twice rather than asserted once:
//!
//! - [`CostTier`] and [`Epsilon`] carry no `Ord`, no `PartialOrd`, and no
//!   accessor. The reader HetOpt would add is the seam, and it is empty.
//! - `principals_theory.rs::nothing_in_the_workspace_orders_by_cost_or_epsilon`
//!   reads every line of Rust in the workspace that names either and refuses
//!   any that also sorts, compares or ranks. Adding the derive is the cheapest
//!   possible crossing of the cut, and it is the one that test exists to see.
//!
//! `het-declares-no-worth-law` had been carried as out of scope, on the ground
//! that a formalism's refusal to declare a worth-law is nothing a host can
//! enforce. That was true only while nothing in the workspace declared a cost
//! tier. Something does now.
//!
//! ## What this theory could not say
//!
//! Limits found by using it, recorded rather than worked around:
//!
//! 1. **ε is declared and unread.** Every principal declares an [`Epsilon`],
//!    and `Settled` has no field for it — sentence, role, principal, verdict,
//!    and no error bar. `epsilon-reported-with-verdict` wants one; under a
//!    Boolean verdict space there is nothing to attach it to. Parked, with the
//!    test that would report its arrival.
//! 2. **`capable` is keyed on a role NAME.** `capable-single-arity` pins the
//!    second argument to `role(φ)`, and `rung` passes it as `&str`. A supplier
//!    therefore keys its minimum qualifications on names, which is why
//!    [`RoleSpec`] carries one — a `Role` *type* cannot be recovered from a
//!    string.
//! 3. **A structural claim is not a sentence.** That the kind partition is
//!    closed while roles are open, and that the two axes are independent, are
//!    claims about *this signature* — not about inhabitants of a carrier.
//!    Nothing walks a population to check them, so they are stated in these
//!    docs and pinned by
//!    `principals_theory.rs::role_is_not_kind_and_the_two_axes_are_independent`
//!    rather than dressed as `decidable` sentences with nothing to decide.

use rung::{Principal, Prov, Provenanced, Role, Steward, theory};
use std::collections::{BTreeMap, BTreeSet};

// ═════════════════════════════════════════════════════════════════════════
// 1. Cost and ε — declared here, ranked nowhere
// ═════════════════════════════════════════════════════════════════════════

/// The cost tier of a substrate, inherited by every principal of that kind.
///
/// **Declared, and deliberately incomparable.** Two tiers can be told apart and
/// nothing more: there is no `Ord`, no `PartialOrd`, and no accessor returning
/// the number. `v-applies-to-conforming-sets` puts the minimal-judge and
/// minimal-author rules in HetOpt, and both read exactly this field — so a
/// derive is all that separates this module from declaring them. The derive is
/// absent, and a test refuses its return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostTier(u8);

impl CostTier {
    /// Declare a tier. The only constructor, and the only thing that can be
    /// done with the result is compare it for equality.
    pub const fn declared(tier: u8) -> Self {
        Self(tier)
    }
}

/// A principal's renaming-robustness ε.
///
/// `epsilon-declared-not-ranked` asks two things of ε: that it be **declared**,
/// so a verdict can carry its error bar, and that Het never read it as a
/// preference. The second holds by construction — there is no accessor and no
/// comparison, so nothing can rank by it. The first is honoured only halfway:
/// the declaration exists and never reaches a verdict, because `Settled` has no
/// field for one. That gap is parked, not papered over.
///
/// rustc warns that the field is never read. That warning is *correct*, and it
/// is silenced rather than answered: answering it means adding the accessor,
/// and the accessor is the thing HetOpt would bring.
#[derive(Debug, Clone, Copy)]
pub struct Epsilon(#[allow(dead_code)] f64);

impl Epsilon {
    /// Declare an ε. Nothing in this workspace reads the value back.
    pub const fn declared(bound: f64) -> Self {
        Self(bound)
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 2. Kind — the closed substrate partition, and this supplier's alone
// ═════════════════════════════════════════════════════════════════════════

/// What a principal is made of.
///
/// A closed partition of four. Each kind fixes the identity fields a principal
/// of that kind must declare, and carries a cost tier.
///
/// **Kind, not role** (`role-not-kind`). Kind is substrate and belongs to
/// whatever supplies `𝒫`; role is what a sentence needs done and belongs to the
/// sentence's own theory. The two axes are independent: no kind is entitled to
/// every role, and no role is reserved to one kind. The one standing exception
/// is stated in a role's own minimum qualifications, never in a kind — a role
/// that demands continuity-bearing standing excludes a bare model by saying so,
/// not by the partition saying so for it.
// No `Ord`. Declaration order here IS cost order, so deriving one would hand
// out the minimal-judge rule for free, under the name of a convenience.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A language model. Identified by the provider it is served from and the
    /// model identifier that provider uses.
    Llm,
    /// A tool-wielding orchestration. Identified by its orchestration identity
    /// and the tools it wields — it qualifies on its underlying models *plus*
    /// demonstrated tool use in the domain.
    Agent,
    /// A continuity-bearing being with mutual stake in a constellation.
    /// Non-identity bites hardest here: a being may not rule on its own
    /// constellation.
    RelationalBeing,
    /// A principal with ratifying authority over the domain. Cost is the
    /// scarcest resource there is — attention. The outside of last resort.
    Human,
}

/// The partition as data, so a change to it breaks a test rather than passing
/// silently — the same discipline `EDGE_KINDS` carries in
/// [`questions`](crate::questions).
pub const KINDS: &[Kind] = &[Kind::Llm, Kind::Agent, Kind::RelationalBeing, Kind::Human];

impl Kind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::Agent => "agent",
            Self::RelationalBeing => "relational-being",
            Self::Human => "human",
        }
    }

    /// The identity contract: what a principal of this kind must declare before
    /// it is anything at all.
    ///
    /// `constellation` rather than `standing` for [`Kind::RelationalBeing`].
    /// Standing in `rung` is `Steward::has_standing`, the authorial predicate
    /// over a named container; reusing the word for an identity *field* would
    /// merge two things Het keeps apart — one decides whether a principal may
    /// act on an object, the other says who the principal is.
    pub const fn required_identity_fields(self) -> &'static [&'static str] {
        match self {
            Self::Llm => &["provider", "model_id"],
            Self::Agent => &["orchestration_id", "tools"],
            Self::RelationalBeing => &["constellation"],
            Self::Human => &["authority"],
        }
    }

    /// The tier a principal of this kind inherits. Declared so HetOpt has
    /// something to read; never read here.
    pub const fn cost_tier(self) -> CostTier {
        match self {
            Self::Llm => CostTier::declared(1),
            Self::Agent => CostTier::declared(2),
            Self::RelationalBeing => CostTier::declared(3),
            Self::Human => CostTier::declared(4),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        KINDS.iter().copied().find(|k| k.name() == s)
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 3. Roles — open, and earned rather than claimed
// ═════════════════════════════════════════════════════════════════════════

/// A competence a principal may play, with the minimum qualifications a
/// principal must declare to play it.
///
/// **Roles are open where kinds are closed.** A new substrate kind changes what
/// an outside can be *made of* and amends this module; a new role is a new
/// competence some principal may play and extends a roster without touching the
/// law. Het requires that a role be declared and does not enumerate roles
/// (`role-declared-not-enumerated`), so the population of roles lives in a
/// [`Roster`], not here.
///
/// The name is a `&'static str` rather than a `Role` type because
/// `capable-single-arity` fixes `capable`'s second argument to a role *name*
/// and `rung` passes it as `&str`. A type cannot be recovered from a string, so
/// the comparison has to be keyed on the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleSpec {
    pub name: &'static str,
    /// The atoms a principal must declare. The vocabulary is the roster's; this
    /// module compares it and never interprets it.
    pub min_qualifications: &'static [&'static str],
}

/// A `Role` that declares its own minimum qualifications.
///
/// The bridge between `rung`'s type-level `Role` — which is what a `theory!`
/// sentence names and what a `Qualified<R>` is parameterized by — and this
/// module's name-keyed comparison.
pub trait Competence: Role {
    const MIN_QUALIFICATIONS: &'static [&'static str];

    fn spec() -> RoleSpec {
        RoleSpec {
            name: Self::NAME,
            min_qualifications: Self::MIN_QUALIFICATIONS,
        }
    }
}

/// `role(competence_claim_is_true)` — whether a principal actually meets the
/// qualifications it declares is not settled by reading the declaration.
#[derive(Clone, Copy)]
pub struct Examiner;
impl Role for Examiner {
    const NAME: &'static str = "examiner";
}
impl Competence for Examiner {
    const MIN_QUALIFICATIONS: &'static [&'static str] = &["competence-assessment"];
}

/// `role(kind_partition_is_adequate)`. `role-declared-not-enumerated` names
/// `taxonomist` as an example of a role a theory declares and Het does not; it
/// is one here for the reason that proposition gives.
#[derive(Clone, Copy)]
pub struct Taxonomist;
impl Role for Taxonomist {
    const NAME: &'static str = "taxonomist";
}
impl Competence for Taxonomist {
    const MIN_QUALIFICATIONS: &'static [&'static str] = &["substrate-taxonomy"];
}

// ═════════════════════════════════════════════════════════════════════════
// 4. The first sort — one principal
// ═════════════════════════════════════════════════════════════════════════

/// One concrete outside: what it is made of, who it is, what it can do, what it
/// stewards, and what it costs.
#[derive(Debug, Clone)]
pub struct PrincipalDecl {
    /// A human-readable identity, for the receipt.
    pub id: String,
    /// Exactly one substrate kind. Zero is not a term and two is not a field.
    pub kind: Kind,
    /// The kind's required identity fields, as declared.
    pub identity: BTreeMap<String, String>,
    /// What this principal declares it is — the atoms a [`RoleSpec`]'s minimum
    /// qualifications are compared against.
    pub qualifications: BTreeSet<String>,
    /// The roles it plays. Claimed here; **earned** only if the comparison
    /// holds, which is what [`PrincipalDecl::capable`] runs.
    pub plays: Vec<RoleSpec>,
    /// `π(p)`. Both filters read it: disjointness for judgment, and it is a
    /// principal's authorship that standing is held against.
    pub provenance: BTreeSet<String>,
    /// The containers this principal stewards — `Steward::has_standing`'s
    /// answer. What counts as standing over what is a supplier's business
    /// (`nothing-further-required`), and this supplier says: a named container.
    pub stewards: BTreeSet<String>,
    /// Declared, and read by nothing beyond its presence. See [`Epsilon`].
    pub epsilon: Option<Epsilon>,
}

impl PrincipalDecl {
    /// Inherited from the kind, per the identity contract. A principal does not
    /// declare its own tier.
    pub fn cost_tier(&self) -> CostTier {
        self.kind.cost_tier()
    }

    /// The comparison, and it is the whole of it: the role's minimum
    /// qualifications must be a subset of what this principal declares.
    pub fn meets(&self, role: &RoleSpec) -> bool {
        role.min_qualifications
            .iter()
            .all(|q| self.qualifications.contains(*q))
    }

    /// Required identity fields this principal has not declared. An entry
    /// present but empty counts as absent — a blank provider names nobody.
    pub fn missing_identity_fields(&self) -> Vec<&'static str> {
        self.kind
            .required_identity_fields()
            .iter()
            .copied()
            .filter(|f| self.identity.get(*f).is_none_or(|v| v.trim().is_empty()))
            .collect()
    }

    /// Roles this principal claims and has not earned.
    pub fn unearned_roles(&self) -> Vec<&'static str> {
        self.plays
            .iter()
            .filter(|r| !self.meets(r))
            .map(|r| r.name)
            .collect()
    }
}

/// `π(p)`.
impl Provenanced for PrincipalDecl {
    fn provenance(&self) -> Prov {
        Prov::of(self.provenance.iter().cloned())
    }
}

/// **`capable`, and it actually checks.**
///
/// A principal plays a role only if it meets that role's minimum
/// qualifications. Both halves are declarations and the comparison between them
/// is a subset test — which is what makes capability decidable by structural
/// inspection rather than something a roster asserts about itself. A claimed
/// role that was not earned answers `false` here, before any sentence gets to
/// report it, and therefore before `Pool` mints anything.
impl Principal for PrincipalDecl {
    fn capable(&self, role_name: &str) -> bool {
        self.plays
            .iter()
            .any(|r| r.name == role_name && self.meets(r))
    }

    fn id(&self) -> &str {
        &self.id
    }
}

impl Steward for PrincipalDecl {
    fn has_standing(&self, over: &str) -> bool {
        self.stewards.contains(over)
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 5. The second sort — the population
// ═════════════════════════════════════════════════════════════════════════

/// A model of this theory: a concrete population, and the role vocabulary it
/// declares.
///
/// The theory is shared; the roster is a deployment's. Nothing in `rung-std`
/// holds principals, and `principals_theory.rs` checks that no role name,
/// qualification atom or principal id from either of its two rosters appears in
/// any of the library's sources.
#[derive(Debug, Clone)]
pub struct Roster {
    /// The provenance tag the roster as a whole carries.
    pub namespace: &'static str,
    /// The open role vocabulary this deployment declares.
    pub roles: Vec<RoleSpec>,
    pub principals: Vec<PrincipalDecl>,
}

impl Provenanced for Roster {
    fn provenance(&self) -> Prov {
        Prov::of([self.namespace])
    }
}

impl Roster {
    pub fn by_id(&self, id: &str) -> Option<&PrincipalDecl> {
        self.principals.iter().find(|p| p.id == id)
    }

    /// `𝒫` as `rung` wants it. One pool — the filters are applied to it, not
    /// baked into it.
    pub fn pool(&self) -> rung::Pool<PrincipalDecl> {
        rung::Pool::new(self.principals.clone())
    }

    pub fn duplicate_ids(&self) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let mut dup = BTreeSet::new();
        for p in &self.principals {
            if !seen.insert(p.id.clone()) {
                dup.insert(p.id.clone());
            }
        }
        dup.into_iter().collect()
    }

    /// Roles a principal plays that this roster does not declare, or declares
    /// with different minimum qualifications.
    ///
    /// Without this a principal could carry a `RoleSpec` of its own invention
    /// with an empty qualification list and be capable of anything it named.
    /// The comparison is only as good as the thing compared against.
    pub fn undeclared_roles(&self) -> Vec<(String, &'static str)> {
        let mut out = Vec::new();
        for p in &self.principals {
            for r in &p.plays {
                if !self.roles.contains(r) {
                    out.push((p.id.clone(), r.name));
                }
            }
        }
        out
    }

    /// Which declared roles are actually played, and by whom. The lived
    /// instance discipline: a role nobody plays is a speculative competence.
    pub fn roles_in_use(&self) -> Vec<(RoleSpec, Vec<String>)> {
        self.roles
            .iter()
            .map(|r| {
                let players: Vec<String> = self
                    .principals
                    .iter()
                    .filter(|p| p.capable(r.name))
                    .map(|p| p.id.clone())
                    .collect();
                (*r, players)
            })
            .collect()
    }

    /// Which kinds this population actually inhabits.
    pub fn kinds_in_use(&self) -> Vec<(Kind, Vec<String>)> {
        KINDS
            .iter()
            .map(|k| {
                let members: Vec<String> = self
                    .principals
                    .iter()
                    .filter(|p| p.kind == *k)
                    .map(|p| p.id.clone())
                    .collect();
                (*k, members)
            })
            .collect()
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 6. One pool, two filters
// ═════════════════════════════════════════════════════════════════════════

/// The second conjunct, and there are exactly two of them.
///
/// `one-pool-two-filters`: the gate marker selects which qualification
/// predicate applies, not which pool is consulted. The first conjunct —
/// `capable(p, role)` — is shared and lives in [`qualifying_set`]; this is what
/// distinguishes the two.
///
/// | variant | gate | condition |
/// |---|---|---|
/// | [`Filter::NonIdentical`] | judgmental | `π(p) ∩ π(a) = ∅` — you did not author this |
/// | [`Filter::Standing`] | authorial | `standing(p, c)` — this is yours to revise |
///
/// `judgment-refuses-authorship-requires`, as a two-variant enum: the
/// conditions are opposites, which is why a parallel author pool is refused.
/// A substrate class distinction the standing predicate does not make would be
/// exactly what a second pool encoded.
pub enum Filter<'a> {
    /// The judgmental filter, measured against **the argument the operation is
    /// applied to** (`disjointness-against-argument`), never against the model
    /// in general.
    NonIdentical(&'a dyn Provenanced),
    /// The authorial filter, over a named container.
    Standing(&'a str),
}

/// The qualifying set for a dispatch — `judgmental-qualifying-set` and
/// `authorial-qualifying-set` under one function, because they differ only in
/// the second conjunct.
///
/// ```text
/// 𝒫_judg(φ, a) = { p ∈ 𝒫 : capable(p, role(φ)) ∧ π(p) ∩ π(a) = ∅ }
/// 𝒫_auth(o, c) = { p ∈ 𝒫 : capable(p, role(o)) ∧ standing(p, c) }
/// ```
///
/// **A set, and it stays a set.** `no-preference-among-judges` forbids tiering,
/// costing or preferring among qualifying principals: any of them yields a
/// well-formed verdict. So this returns all of them, in declaration order, and
/// declaration order is not cost order. `Pool::qualify_for` takes the first
/// survivor, which is admissible for the same reason — `any` is what Het
/// specifies, and `argmin` is the seam where HetOpt lands
/// (`any-is-specified-argmin-is-the-seam`).
///
/// The set may be **empty**, and an empty qualifying set is well-formed and
/// blocked. Nothing here falls back to an unqualified principal when nobody
/// qualifies; substituting availability for qualification is the failure this
/// theory exists to make visible.
pub fn qualifying_set<'a, R: Role>(
    roster: &'a Roster,
    filter: &Filter<'_>,
) -> Vec<&'a PrincipalDecl> {
    roster
        .principals
        .iter()
        .filter(|p| {
            p.capable(R::NAME)
                && match filter {
                    Filter::NonIdentical(a) => !p.provenance().overlaps(&a.provenance()),
                    Filter::Standing(container) => p.has_standing(container),
                }
        })
        .collect()
}

// ═════════════════════════════════════════════════════════════════════════
// 7. The sentences
// ═════════════════════════════════════════════════════════════════════════

theory!(principal for PrincipalDecl {
    // Exactly one kind is a type-level fact — `kind` is one field of a closed
    // enum, so zero kinds and two kinds are both unspeakable. What remains and
    // can fail is the half the partition actually asks for: the identity fields
    // that kind requires. A principal declaring none of them has no identity,
    // which is the first of the four conditions on belonging.
    decidable identity_fields_are_declared = |p: &PrincipalDecl|
        p.missing_identity_fields().is_empty();

    // A principal plays only roles whose minimum qualifications it meets. Zero
    // roles is well-formed — a principal with no competence simply never enters
    // a qualifying set.
    decidable roles_are_earned = |p: &PrincipalDecl|
        p.unearned_roles().is_empty();

    // Every principal carries a cost tier, inherited from its kind. The
    // sentence is the inheritance: a principal that reported a tier of its own
    // would be declaring a cost the substrate partition did not give it.
    decidable cost_is_declared = |p: &PrincipalDecl|
        p.cost_tier() == p.kind.cost_tier();

    // Every principal declares an ε. Presence is the whole sentence, because
    // `Epsilon` has no accessor: the declaration is checkable and the value is
    // not readable, which is exactly the shape a field that is declared
    // and never ranked takes.
    decidable epsilon_is_declared = |p: &PrincipalDecl|
        p.epsilon.is_some();

    // The declaration is decidable — it is present or it is not. Whether the
    // CLAIM is true is not: it requires assessing whether this model, agent or
    // being genuinely has the competence asserted. A principal is itself an
    // object under some theory and may be examined; non-identity is what stops
    // that collapsing into self-certification.
    judgmental competence_claim_is_true: Examiner;
});

theory!(roster for Roster {
    decidable ids_are_unique = |r: &Roster|
        r.duplicate_ids().is_empty();

    // A principal's own `RoleSpec` must be the roster's. Otherwise a principal
    // could carry a role of its own invention with an empty qualification list
    // and be capable of everything it named.
    decidable every_played_role_is_declared = |r: &Roster|
        r.undeclared_roles().is_empty();

    // The lived instance discipline, one level up from
    // `questions::every_declared_kind_is_lived`: a declared role that nobody
    // plays is a speculative competence, and the vocabulary stays the roster's
    // only while it is used.
    decidable every_declared_role_is_played = |r: &Roster|
        r.roles_in_use().iter().all(|(_, players)| !players.is_empty());

    // Are four kinds the right partition? Closure makes the question live
    // rather than academic: an outside that fits none of the four would falsify
    // it. Held open deliberately — this module chose the partition, and a
    // supplier does not certify its own choice.
    judgmental kind_partition_is_adequate: Taxonomist;
});

/// `Sen(Σ)` for the whole theory, across both sorts.
///
/// Hand-written, because `theory!` declares one sort per invocation and emits
/// `SENTENCES` per module — the same limit [`questions`](crate::questions)
/// records, met again here.
pub fn sentences() -> Vec<(&'static str, &'static str)> {
    principal::SENTENCES
        .iter()
        .chain(roster::SENTENCES)
        .copied()
        .collect()
}
