//! `rung-het-props.md`, encoded.
//!
//! **Generated once** from `docs/rung-het-props.md` by `docs/_migrate.py`, and the
//! source of truth from then on. The markdown is rendered from this; where the
//! two disagree, this is right and the markdown is stale.
//!
//! Every proposition arrives as [`Kind::Rationale`], which is not a claim that
//! they are all arguments — it is the absence of a claim. Markdown does not
//! record what kind a proposition is, so the migration does not invent one. The
//! triage into signature, decidable and judgmental is a reading, done
//! deliberately, and it is the work this encoding exists to make possible.

use crate::{Doctrine, Element, Kind, Prop};

/// The doctrine `docs/rung-het-props.md` is rendered from.
pub fn doctrine() -> Doctrine {
    Doctrine {
        file: "rung-het-props.md".into(),
        elements: vec![
        Element::Verbatim(r#"# Het — The Formalism

**Status: normative.** This document is Het. It is self-contained: it
depends on no other document, cites no artifact, and records no history.
Every claim is stated once, in one place, and referred to elsewhere by
number.

The numbering is a tree. A proposition `n.m` is a remark on `n`; `n.mm`
is a remark on `n.m`. Interior propositions are the conjunction of their
children. Leaves are single checkable claims.

**Scope.** Propositions [1](#one-relation)–[7](#satisfaction-is-a-game)
and [9](#composition-is-closed)–[11](#theory-declares-four-things)
specify Het. Proposition [8](#het-settles-hetopt-orders) specifies the
cut between Het and HetOpt, and states of HetOpt only what the cut
requires. Proposition [12](#no-bound-on-reentry) states the limit Het
does not close.

**This document is generated.** Its source is `rung-doctrine/src/rung_het.rs`,
and it is written by `cargo run -p rung-doctrine --bin render`. Editing it here
does not change what it says; the next render restores this text. Where the two
differ, the encoding is right and this file is stale — CI checks exactly that.

**Numbers are derived, not authored.** A proposition's identity is its slug;
its place in the tree is its declared parent; its order is declaration order.
The decimal number and every reference to it are computed at render time and
appear nowhere in the source, so inserting, removing or reparenting a
proposition cannot break a reference and cannot leave a number stale — there
is no number to leave.

---

## 1 · The relation

"#.into()),
        Element::Prop(Prop {
            slug: "one-relation".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"There is one relation:

$$M \models_\Sigma \varphi$$

A model $M$ satisfies sentence $\varphi$ under signature $\Sigma$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "institution-quadruple".into(),
            parent: Some("one-relation".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The ambient structure is an institution — a quadruple
$(\mathbf{Sign}, \mathsf{Sen}, \mathsf{Mod}, \models)$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "sign-category".into(),
            parent: Some("institution-quadruple".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"$\mathbf{Sign}$ is a category. Its objects are signatures; its
morphisms are signature morphisms.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "sen-functor".into(),
            parent: Some("institution-quadruple".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"$\mathsf{Sen} : \mathbf{Sign} \to \mathbf{Set}$ assigns to each
signature its sentences.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "mod-functor".into(),
            parent: Some("institution-quadruple".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"$\mathsf{Mod} : \mathbf{Sign}^{\text{op}} \to \mathbf{Cat}$
assigns to each signature its algebras.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "satisfaction-typing".into(),
            parent: Some("institution-quadruple".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"$\models_\Sigma \;\subseteq\; \lvert\mathsf{Mod}(\Sigma)\rvert \times \mathsf{Sen}(\Sigma)$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "satisfaction-condition".into(),
            parent: Some("one-relation".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The institution's single axiom is the satisfaction condition:
truth is invariant under change of notation.

$$M \models_{\Sigma'} \mathsf{Sen}(\sigma)(\varphi) \iff \mathsf{Mod}(\sigma)(M) \models_\Sigma \varphi$$

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "signature-declares".into(),
            parent: Some("one-relation".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A signature declares sorts, operation symbols with arities, gate
markers, and the laws the theory declares.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "extension-is-in-models".into(),
            parent: Some("signature-declares".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The signature layer is standard. Het's entire extension is in
$\models$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "no-layer-above-sigma".into(),
            parent: Some("signature-declares".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"There is no layer above $\Sigma$. There is $\Sigma$, there is
$M$, and there is one gate-dispatched $\models$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "rest-is-bookkeeping".into(),
            parent: Some("one-relation".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Every other structure named in this document — the gates, the
pool, the tower, the game — is bookkeeping around {#one-relation}.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 2 · The gate

"#.into()),
        Element::Prop(Prop {
            slug: "gate-marker-required".into(),
            parent: None,
            kind: Kind::Decidable { proof: "rung-het/tests/gate_law.rs::every_sentence_carries_a_gate_from_the_declared_vocabulary".into() },
            numbering: None,
            prose: r#"Every sentence and every operation carries a **gate marker**, which
fixes how its satisfaction is computed.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "four-gates".into(),
            parent: Some("gate-marker-required".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/gate_law.rs::every_sentence_carries_a_gate_from_the_declared_vocabulary".into() },
            numbering: None,
            prose: r#"The marker is one of exactly four.

| gate | satisfaction mechanism |
|---|---|
| `decidable` | $M \models \varphi$ is machine-checked. Standard equational logic. |
| `judgmental` | $M \models \varphi$ dispatches to a **judge** — an inhabitant of the principal pool $\mathcal{P}$. The judge's verdict *is* the satisfaction outcome. |
| `authorial` | The operation *transforms* the subject rather than classifying it, or produces new content about it. It dispatches to an **author**, also from $\mathcal{P}$, holding standing over the subject. |
| `conditional` | Whether satisfaction is decidable depends on the specific algebra. The condition is classified one level up ({#conditional-names-classifier}). |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "no-other-gate-value".into(),
            parent: Some("four-gates".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"No other value is well-formed.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "unmarked-not-wellformed".into(),
            parent: Some("gate-marker-required".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"An operation without a gate marker is not a well-formed
declaration.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgmental-declares-role".into(),
            parent: Some("gate-marker-required".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A judgmental operation declares the **competence role** required
to discharge it.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "role-not-kind".into(),
            parent: Some("judgmental-declares-role".into()),
            kind: Kind::Decidable { proof: "rung-std/tests/principals_theory.rs::role_is_not_kind_and_the_two_axes_are_independent".into() },
            numbering: None,
            prose: r#"A role, not a kind. Kind is what a principal is made of, and
belongs to whatever supplies $\mathcal{P}$ ({#nothing-further-required}). Role is what the
sentence needs done, and only the sentence's own theory knows that.

"#.into(),
            mechanism: r#"Two axes, and a supplier that declares both is what makes their independence visible. `rung-std::principals::Kind` is substrate — the supplier's, closed, with identity fields and a tier; `Role` is what a sentence needs done and is `rung`'s type. The cited test plays one role across all four kinds and shows a kind entitled to no role it has not earned. The one apparent exception — a competence that excludes a bare model — is stated in that role's own minimum qualifications and never in the partition, which is the asymmetry this proposition names."#.into(),
        }),
        Element::Prop(Prop {
            slug: "role-declared-pointwise".into(),
            parent: Some("judgmental-declares-role".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The declaration is pointwise. There is no global map from
sentences to competences, and none is needed: the pointwise declaration
is what lets $\models$ resolve a judge.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "authorial-declares-standing".into(),
            parent: Some("gate-marker-required".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads".into() },
            numbering: None,
            prose: r#"An authorial operation declares a **standing predicate**.

"#.into(),
            mechanism: r#"G14. `#[authorial]` with no role is a `compile_error!` — the qualifying set is a conjunction and a marker naming no role can witness only its right half — and the pen that IS emitted carries the container standing was measured over. The macro then injects `must_hold_standing_over(&src.payload, &pen)` ahead of the body, so the declared predicate is consulted whether or not the body mentions it: the cited ladder's body never does. Stubbing the prologue to a no-op turns it red. This is what makes a marked transition's source payload have to be `Situated` — without a container there is nothing standing could be over."#.into(),
        }),
        Element::Prop(Prop {
            slug: "conditional-names-classifier".into(),
            parent: Some("gate-marker-required".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A conditional operation names a **classifying sentence**.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "classifier-not-judgmental".into(),
            parent: Some("conditional-names-classifier".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The classifying sentence is not itself judgmental. A judgmental
classifier reopens the regress {#tower-floor} closes.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "conditional-partitions-fiber".into(),
            parent: Some("conditional-names-classifier".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"A conditional gate partitions the fiber $\mathsf{Mod}(\Sigma)$:

$$\mathsf{Mod}_{\mathsf{dec}}(\Sigma, \varphi) \quad\text{and}\quad \mathsf{Mod}_{\mathsf{jud}}(\Sigma, \varphi)$$

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "classifier-one-level-up".into(),
            parent: Some("conditional-names-classifier".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"For every conditional sentence $\varphi$ of $\Sigma$ there exists
a classifying sentence in the theory one level up,

$$\mathsf{Decidable}_\Sigma(\varphi) \in \mathsf{Sen}(\Sigma^\uparrow)$$

such that

$$M \in \mathsf{Mod}_{\mathsf{dec}}(\Sigma, \varphi) \iff M \models_{\Sigma^\uparrow} \mathsf{Decidable}_\Sigma(\varphi)$$

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "decidability-expressible-internally".into(),
            parent: Some("conditional-names-classifier".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The predicate *"$\varphi$ is decidable in this algebra"* is
therefore expressible inside the ambient institution. The two sub-classes
are ordinary sub-fibers defined by satisfaction of a higher sentence.
Re-indexing transports that higher sentence, and fiber-wise uniformity is
restored.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 3 · The pool

"#.into()),
        Element::Prop(Prop {
            slug: "pool-is-parameter".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"$\mathcal{P}$ is a **parameter of the satisfaction relation**, not a
sort of the signature.

> The theory declares *what* must be judged; $\models$ determines *how* —
> mechanically or by delegation.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "pool-not-a-sort".into(),
            parent: Some("pool-is-parameter".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$\mathcal{P}$ does not appear as a sort in any signature.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "internalizing-outside-collapses".into(),
            parent: Some("pool-not-a-sort".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"A signature that declares $\mathcal{P}$ as a sort has
internalized the outside. The ontological separation collapses and
non-identity becomes unenforceable: if the judge is an element of the
algebra, what judges the judge?

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "pool-is-opaque".into(),
            parent: Some("pool-is-parameter".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$\mathcal{P}$ is **opaque**. Het never names a principal
substrate, never enumerates kinds, and never inspects an inhabitant.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "supplier-interface".into(),
            parent: Some("pool-is-opaque".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"Het requires only that whatever supplies $\mathcal{P}$ exposes
four predicates.

| predicate | arity | gate | what $\models$ needs it for |
|---|---|---|---|
| $\mathsf{capable}$ | $\mathcal{P} \times \mathsf{Role} \to \mathsf{Bool}$ | decidable | competence filter — can this principal play the role the sentence declares ({#judgmental-declares-role})? |
| $\pi$ | $X \to \mathsf{Prov}$, for $X$ a principal or an subject | decidable | provenance tags; both filters read it |
| $\mathsf{standing}$ | $\mathcal{P} \times S \to \mathsf{Bool}$ | conditional | authorial filter ({#authorial-qualifying-set}); classified one level up |
| $\varepsilon$ | $\mathcal{P} \to {#epsilon-reported-with-verdict}) |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "interface-by-signature-inspection".into(),
            parent: Some("pool-is-opaque".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A theory that supplies $\mathcal{P}$ declares all four at these
arities. Conformance is signature inspection — decidable, and requiring
no edge machinery beyond reading the declaration.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "nothing-further-required".into(),
            parent: Some("pool-is-opaque".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Het requires nothing further of a supplier. Kinds, substrate
partitions, identity fields, cost tiers, and the population itself are
the supplier's. Naming any of them here would internalize the outside a
second way — not as a sort, but as a stipulated content.

"#.into(),
            mechanism: r#"The division is now observable from both sides. `rung::Principal` asks for `capable` and `id`, `Provenanced` for `π`, `Steward` for standing — and nothing anywhere in `rung` names a kind, a substrate partition, an identity field, a cost tier or a population. `rung-std::principals` names all five, because a supplier that named none of them would have supplied nothing. The cited test binds the interface at its declared arities and shows the licence that comes back out carrying an id, a provenance and a role — the kind, its required fields and its tier stay on the supplier's side of the line. What is NOT enforced: that a future `rung` stays incurious. Nothing structurally prevents the library growing a `Kind`; this row records that it has not."#.into(),
        }),
        Element::Prop(Prop {
            slug: "capable-single-arity".into(),
            parent: Some("pool-is-opaque".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$\mathsf{capable}$ is used at exactly one arity,
$\mathcal{P} \times \mathsf{Role}$, everywhere in Het. Its second
argument is $\mathsf{role}(\varphi)$ or $\mathsf{role}(o)$ — the role the
*sentence* or *operation* declares — never the sentence or subject itself.
A supplier of $\mathcal{P}$ cannot be asked to inspect Het's sentences; it
does not have them.

"#.into(),
            mechanism: r#"`Principal::capable(&self, role_name: &str)` — one arity, and the second argument is a NAME. A supplier keys its qualification table on that name (`rung-std::principals::RoleSpec`), because a `Role` type cannot be recovered from a string; that is the shape this proposition forces, met rather than worked around. The cited test passes a *sentence* name where a role name goes and gets `false`: a principal does not have the theory's sentences and cannot be asked to inspect them. rung proves the arity, not that any supplier's table is right."#.into(),
        }),
        Element::Prop(Prop {
            slug: "principal-provenance-floor".into(),
            parent: Some("pool-is-opaque".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$\pi(p) \supseteq \{\mathsf{id}(p)\}$ — a principal's provenance
contains its own identity. A supplier may declare a principal with no history
whatever; it may not declare one with **no provenance**.

$\pi(p) = \emptyset$ is disjoint from everything, so such a principal survives
{#judgmental-qualifying-set} against every argument in the signature: a
**universal judge**, admitted to rule on work it authored, with the filter
running and passing. That is the exact shape in which non-identity becomes
decorative, and it is the mirror of the vacuity already refused on the argument
side — $\pi(a) = \emptyset$ makes disjointness hold trivially and is refused
before the filter runs.

The floor is a **derivation condition, not a check**. A supplier states what a
principal has authored; $\pi$ is that with the identity adjoined, and the
supplier has no term for the result. Refusing an empty $\pi(p)$ at the point of
use would be one uncalled code path away from vacuous — the failure
{#non-identity-by-construction} exists to foreclose — whereas a value the
language will not produce cannot be reached by any path at all. *Conformance:
`rung`'s `Principal` declares `authored` and never `provenance`, the sole route
being `impl<P: Principal> Provenanced for P { authored().with(id()) }`;
`rung/tests/provenance_floor.rs`, whose third case is a `trybuild` **E0119** —
a hand-written `Provenanced` impl for a principal is refused by coherence.*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "commission-record-is-the-carrier".into(),
            parent: Some("principal-provenance-floor".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"**The carrier of `authored(p)` (Q16).** For a discontinuous
kind — a model, an agent — the supplier does not enumerate `authored(p)` as a
declaration: that would be a growing, hand-maintained second source of truth.
Instead `authored(p)` is **derived by lookup** from a **commission contribution
record**:

$$\mathsf{authored}(p) = \bigcup_{c \in S} C(f, c)$$

where $f$ is the principal's family (Q14's family identifier), $S$ is the
**active commission set** — the current commission plus any prior commissions
the supplier explicitly carried forward — and $C(f,c)$ is the finite set of
artifacts family $f$ produced under commission $c$. A principal that names a
family carries no `authored` list of its own; the pool reads the record at
qualification time. A principal without a family (a continuous kind, e.g. a
person) keeps `authored` as its own genuine, declared record.

"#.into(),
            mechanism: r#"Conformance: the unified model's
`rung_std::principals::PrincipalDecl::family` and `CommissionLog` in `rung-driver`;
`Configured::authored` derives from the record when a family is present.
`commissions.yaml` is the record rung's own population reads."#.into(),
        }),
        Element::Prop(Prop {
            slug: "commission-authored-is-lookup".into(),
            parent: Some("principal-provenance-floor".into()),
            kind: Kind::Decidable { proof: "rung-driver/tests/commission.rs::authored_is_the_union_of_a_familys_active_commissions".into() },
            numbering: None,
            prose: r#"`authored(f)` is the union, over the active commissions, of
`C(f,c)` — an artifact in two active commissions is counted once, and the
result is order-stable because it is the content of a set, not a list the
supplier typed.

"#.into(),
            mechanism: r#"Exercised by
`commission-authored-is-lookup`'s proof: a family with `x`,`y` in one active
commission and `y`,`z` in another derives `{x,y,z}`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "commission-record-not-total".into(),
            parent: Some("principal-provenance-floor".into()),
            kind: Kind::Decidable { proof: "rung-driver/tests/commission.rs::closed_non_carried_commissions_stay_open".into() },
            numbering: None,
            prose: r#"**Not total.** An artifact in a commission that is closed
and **not** carried forward is not in `authored(f)` — it falls out of the
active set and re-opens to later, disjoint instances of the same family. Only a
supplier's explicit carry-forward brings a prior commission's artifacts back
into $S$.

"#.into(),
            mechanism: r#"Exercised by `commission-record-not-total`'s proof:
a closed, non-carried commission's artifact is absent from `authored`, and
returns only when the commission is explicitly carried forward."#.into(),
        }),
        Element::Prop(Prop {
            slug: "commission-new-commission-empty".into(),
            parent: Some("principal-provenance-floor".into()),
            kind: Kind::Decidable { proof: "rung-driver/tests/commission.rs::a_new_commission_starts_empty".into() },
            numbering: None,
            prose: r#"A newly opened commission is empty for every family: no
artifact is retroactively claimed. `authored(f)` for an untouched family is the
empty set, which is the honest "nothing recorded yet" state rather than the
refused per-invocation vacuity.

"#.into(),
            mechanism: r#"Exercised by `commission-new-commission-empty`'s
proof: an active commission with no contributions attributes nothing."#.into(),
        }),
        Element::Prop(Prop {
            slug: "commission-non-vacuous".into(),
            parent: Some("principal-provenance-floor".into()),
            kind: Kind::Decidable { proof: "rung-driver/tests/commission.rs::a_family_cannot_judge_what_it_produced_but_can_judge_elsewhere".into() },
            numbering: None,
            prose: r#"**Non-vacuous.** Inside an open commission, a family cannot
judge an artifact its family produced under that commission: it is in
$C(f,c)$ for an active commission, hence in `authored(f)`, hence in
$\pi(p)$, so {#disjointness-against-argument} refuses it end to end. Judging
an artifact of a different family is untouched.

"#.into(),
            mechanism: r#"Exercised by `commission-non-vacuous`'s proof: a
family that recorded a contribution is refused for its own artifact and
qualifies for another family's — reached through the pool, derived solely from
the record."#.into(),
        }),
        Element::Prop(Prop {
            slug: "commission-derived-not-declared".into(),
            parent: Some("principal-provenance-floor".into()),
            kind: Kind::Decidable { proof: "rung-driver/tests/commission.rs::family_principals_declare_no_standing_authored".into() },
            numbering: None,
            prose: r#"A principal that names a family **declares no `authored`
list** — its stake is derived from the record, not typed. The declaration
carries only the family, which is stable, and the record carries everything
that changes.

"#.into(),
            mechanism: r#"Exercised by `commission-derived-not-declared`'s
proof: every family principal in a sample population has `family` set and an
empty `authored` field."#.into(),
        }),
        Element::Prop(Prop {
            slug: "commission-no-dual-source".into(),
            parent: Some("principal-provenance-floor".into()),
            kind: Kind::Decidable { proof: "rung-driver/tests/commission.rs::family_plus_authored_is_a_fault".into() },
            numbering: None,
            prose: r#"A principal that declares **both** a `family` and a static
`authored` is refused as ill-formed: `authored` is derived for a family, so a
second hand-maintained copy is the exact two-sources-of-truth the carrier
exists to remove.

"#.into(),
            mechanism: r#"Exercised by `commission-no-dual-source`'s proof: the
population's `check` reports `FamilyWithAuthored` for such a principal."#.into(),
        }),
        Element::Prop(Prop {
            slug: "commission-record-roundtrips".into(),
            parent: Some("principal-provenance-floor".into()),
            kind: Kind::Decidable { proof: "rung-driver/tests/commission.rs::the_record_round_trips_through_yaml".into() },
            numbering: None,
            prose: r#"The commission record is data in a file, and it round-trips
through YAML: a record a driver reads and the one it re-serializes cannot
drift.

"#.into(),
            mechanism: r#"Exercised by `commission-record-roundtrips`'s proof:
serializing then re-parsing a record yields an equal value."#.into(),
        }),
        Element::Prop(Prop {
            slug: "three-belonging-predicates".into(),
            parent: Some("pool-is-parameter".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Three of the four are **belonging predicates**: capability,
non-identity, and standing. They decide whether a principal qualifies at
all. All three are Het's.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "ordering-is-hetopts".into(),
            parent: Some("three-belonging-predicates".into()),
            kind: Kind::Decidable { proof: "rung-std/tests/principals_theory.rs::the_qualifying_set_is_not_ordered_by_cost".into() },
            numbering: None,
            prose: r#"$\varepsilon$ and cost tier support **ordering** among those
that qualify. Ordering is HetOpt's ({#het-settles-hetopt-orders}).

"#.into(),
            mechanism: r#"Cost tier is declared — per substrate kind, in `rung-std::principals::Kind::cost_tier` — and ordered nowhere. The cited test is the direct observation: roster A is laid out so the qualifying set opens with the costliest substrate and closes with the cheapest, and `Pool::qualify_for` picks the human over the model. Under the minimal-judge rule the order inverts. Deriving `Ord` on `CostTier` and sorting the set by it in `qualifying_set` is type-valid and turns the test red at the kind sequence. This row was `out-of-scope` while nothing in the workspace declared a tier; a supplier now does, and ordering it is a thing a host can refuse to do."#.into(),
        }),
        Element::Prop(Prop {
            slug: "epsilon-declared-not-ranked".into(),
            parent: Some("three-belonging-predicates".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"Het requires $\varepsilon$ be declared so the verdict can carry
its error bar. Het never reads it as a preference.

"#.into(),
            mechanism: r#"HALF HOLDS, HALF IS A GAP. *Never ranked*: `rung-std::principals` declares an `Epsilon` per principal and no accessor and no comparison exist for one, so nothing can read it as a preference; `principals_theory.rs::nothing_in_the_workspace_orders_by_cost_or_epsilon` enforces that across every source file. *Declared so the verdict can carry its error bar*: it cannot. `Settled::Judgmental` carries sentence, role, principal and verdict, and there is no field for an error bar — so the ε a supplier already declares stops at the supplier. This is a **different** gap from {#epsilon-reported-with-verdict}, which asks whether a judge's confidence is expressible at all; this one asks whether the ε that IS declared reaches the caller. Deleting the `#[ignore]` reports it."#.into(),
        }),
        Element::Prop(Prop {
            slug: "one-pool-two-filters".into(),
            parent: Some("pool-is-parameter".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one".into() },
            numbering: None,
            prose: r#"There is **one pool and two filters**. The gate marker selects
which qualification predicate applies, not which pool is consulted.
Distinct pools are not licensed.

### The judgmental filter — non-identity

"#.into(),
            mechanism: r#"G14, and this is the row G14 exists for. One `Pool` mints both tokens; the gate marker on a `ladder!` transition selects which filter runs, not which pool is consulted. `#[judgmental(R)]` emits `Qualified<R>` and runs capability + disjointness; `#[authorial(R)]` emits `Authorized<'_, R>` and runs capability + standing. The cited test drives the same three principals through both filters over one subject and asserts they DISAGREE. Dropping the capability conjunct from `Pool::authorize` turns it red."#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgmental-qualifying-set".into(),
            parent: Some("pool-is-parameter".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A judgmental sentence dispatches to a judge drawn from its
qualifying set:

$$\mathcal{P}_{\text{judg}}(\varphi, a) = \{\, p \in \mathcal{P} : \mathsf{capable}(p, \mathsf{role}(\varphi)) \wedge \pi(p) \cap \pi(a) = \emptyset \,\}$$

"#.into(),
            mechanism: r#"Both conjuncts are implemented and both are tested — competence by `gate_law.rs::competence_is_filtered_before_provenance_matters`, disjointness by `::p0_refuses_a_judge_who_authored_the_material`. What is parked is the set's own **edge**. `Pool::qualify_for` refuses a model with `π(a) = ∅`, because every candidate would then pass disjointness vacuously; the mirror on the *principal's* side is unguarded, so a principal declaring `π(p) = ∅` is disjoint from everything and is a universal judge admitted by construction. Het as written admits it. Whether that is a hole or the honest consequence of the definition is a change to **this proposition**, which is why the cited test presumes an answer and is parked rather than run: the engine invented the model-side guard on its own judgment once, and inventing its mirror unasked would be the same overreach twice."#.into(),
        }),
        Element::Prop(Prop {
            slug: "disjointness-against-argument".into(),
            parent: Some("judgmental-qualifying-set".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/token_binding.rs::settle_refuses_a_token_minted_against_a_different_model".into() },
            numbering: None,
            prose: r#"Disjointness is measured against **the argument the operation is
applied to**, not against the model in general.

"#.into(),
            mechanism: r#"G13. Disjointness is measured against the argument, and the token now remembers WHICH argument, so spending it elsewhere is a refusal rather than an unobservable mistake. `dispose` admits a token only against the **proposal**; `settle` only against the **model**. Until the binding landed this proposition was satisfied only by the caller passing the right reference — `qualify_for` was a pure alias for `qualify` and nothing downstream could tell the two apart."#.into(),
        }),
        Element::Prop(Prop {
            slug: "argument-governs".into(),
            parent: Some("judgmental-qualifying-set".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/token_binding.rs::dispose_refuses_a_token_minted_against_the_model".into() },
            numbering: None,
            prose: r#"Where the argument is the subject under audit, $\pi(a) = \pi(M)$
and the two readings coincide. Where the argument is a Proposal, its
provenance is its author's ({#proposal-provenance-is-authors}) and the author need not be the model.
The argument governs.

"#.into(),
            mechanism: r#"G13, at the point where the two readings come apart. A judge that authored a Proposal is disjoint from the MODEL by construction, so a model-relative mint would admit it to rule on its own work; the cited test performs exactly that laundering, with a token minted honestly against the model, and `dispose` refuses it. `Pool::qualify` is now the `audit` reading of `qualify_for`, where π(a) = π(M) — one filter, and which name the caller used is a comment rather than the check."#.into(),
        }),
        Element::Prop(Prop {
            slug: "non-identity-before-dispatch".into(),
            parent: Some("judgmental-qualifying-set".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Non-identity is enforced before any judgmental dispatch. It is
decidable — disjointness of finite provenance-tag sets — and belongs to
the decidable fragment.

"#.into(),
            mechanism: r#"The filter is set operations over declared predicates ({#conformance-half-needs-no-judge}), and it runs **before** dispatch because dispatch has no other door: a judgmental transition called without a token is E0061, and the only mint is `Pool::qualify_for`, which refuses before it returns. The cited `trybuild` case is that refusal with its message committed. rung enforces *that the token was constructed*, never that the body computed the set correctly — SPEC §5, transition-body correctness."#.into(),
        }),
        Element::Prop(Prop {
            slug: "non-identity-by-construction".into(),
            parent: Some("judgmental-qualifying-set".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/token_binding.rs::dispose_refuses_a_token_minted_against_the_model".into() },
            numbering: None,
            prose: r#"Non-identity is discharged by the **construction of the qualifying
token**, not by a check inside a dispatching body. The token witnesses a
**pair** — the principal, and the argument it was measured against — and the
operation that consumes it admits it only for that argument.

A token recording only the principal is unforgeable but **unbound**. It proves
someone passed the filter, not that they passed it against *this* argument, so
it can be earned against one argument and spent on another — which is the act
{#disjointness-against-argument} forbids. Sealing the constructor closes
fabrication; it does not close transfer.

"#.into(),
            mechanism: r#"G12 + G13. The token witnesses the **pair** this proposition names: `Qualified<R>` records the principal AND `π(a)`, the argument disjointness was measured against, and `Qualified::admit` is the one gate that spends it. The seal closes *fabrication* — there is no public constructor, `Pool::qualify_for` is the only mint. The binding closes *transfer* — a licence earned against one argument is refused anywhere else, as `TokenNotBound` from `dispose` and `settle`, and as the macro-injected prologue on a `#[judgmental(R)]` transition, which a body can no more skip than it can skip G8's `must_progress`. Deleting the `admit` call turns the cited test red. NOT enforced: the *returned* value. `π(f(a)) ∩ π(a) = ∅` is a body property and inherits SPEC §5."#.into(),
        }),
        Element::Prop(Prop {
            slug: "non-identity-not-deferrable".into(),
            parent: Some("judgmental-qualifying-set".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Non-identity is not deferrable to valuation. It is a belonging
predicate, not a preference. A system that dispatches without it is
self-certifying, which is the failure this formalism exists to refuse.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "no-preference-among-judges".into(),
            parent: Some("judgmental-qualifying-set".into()),
            kind: Kind::Decidable { proof: "rung-std/tests/principals_theory.rs::every_member_of_the_qualifying_set_is_a_well_formed_dispatch".into() },
            numbering: None,
            prose: r#"Het dispatches to *a* qualifying judge. It does not tier, cost,
or prefer among qualifying judges. Any of them yields a well-formed
verdict, reported with its own $\varepsilon$.

### The authorial filter — standing

"#.into(),
            mechanism: r#"The set is now **exposed as a set**, and that is what moves this row. `Pool::qualify_for` still walks the pool and returns the first survivor — candidates skipped for failing a *conjunct*, never for being ranked below another (`gate_law.rs::qualification_walks_the_pool_and_takes_any_survivor`) — but a single-survivor API could only ever IMPLY that any other survivor would have done. `rung-std::principals::qualifying_set` returns all of them, and the cited test takes each of the four in turn, mints a licence against the very same argument and settles the very same sentence: four well-formed dispatches, one per member. Truncating the set to its first member is type-valid and turns the test red at the count. The UNARGUED residue is gone with it — pool position cannot constitute an ordering over a value that carries every member."#.into(),
        }),
        Element::Prop(Prop {
            slug: "authorial-qualifying-set".into(),
            parent: Some("pool-is-parameter".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one".into() },
            numbering: None,
            prose: r#"An authorial operation dispatches to an author drawn from its
qualifying set:

$$\mathcal{P}_{\text{auth}}(o, M) = \{\, p \in \mathcal{P} : \mathsf{capable}(p, \mathsf{role}(o)) \wedge \mathsf{standing}(p, M) \,\}$$

"#.into(),
            mechanism: r#"G14. `Pool::authorize::<R>` is the only mint for `Authorized` and checks BOTH conjuncts — `capable(p, role(o))` then `standing(p, M)`. Standing alone mints nothing: the cited test hands it a steward of the container who is capable of nothing and requires `AuthorizeError::NotCapable`. NOT enforced: the outcome condition of {#admissibility-subcategories}, `π(f(a)) ⊆ π(p)`, which is a body property and inherits SPEC §5."#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgment-refuses-authorship-requires".into(),
            parent: Some("authorial-qualifying-set".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one".into() },
            numbering: None,
            prose: r#"Judgment classifies; authorship transforms. Both require an
outside, in opposite directions.

> **Judgment refuses the audited party. Authorship requires standing over it.**

"#.into(),
            mechanism: r#"G12 + G14 together, which is the only way this proposition can be shown: it is a claim about two filters, so one filter cannot witness it. The cited test asserts both directions over one subject — a principal that PASSES the judgmental filter (capable, provenance-disjoint) is refused a pen, and the principal that HOLDS the pen is refused as a judge of the very subject it stewards. An authorial gate built as the judgmental gate with its token renamed passes every other gate test and fails this one."#.into(),
        }),
        Element::Prop(Prop {
            slug: "provenance-overlap-is-the-point".into(),
            parent: Some("authorial-qualifying-set".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one".into() },
            numbering: None,
            prose: r#"Non-identity excludes exactly the arrows authorship needs: the
author of a candidate *is* the party under audit, and enacting a remedy
means revising one's own text. Provenance overlap is the point, not the
defect.

"#.into(),
            mechanism: r#"G12 + G14, read as the reason the two filters must disagree. The cited test's subject is authored by the principal that stewards its container, so the overlap that disqualifies the curator as a judge is the same fact that makes it the author. Weakening either second conjunct — disjointness in `qualify_for`, standing in `authorize` — turns the test red, because the two assertions are about the same principal and the same subject."#.into(),
        }),
        Element::Prop(Prop {
            slug: "standing-conditional-gated".into(),
            parent: Some("authorial-qualifying-set".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one".into() },
            numbering: None,
            prose: r#"Standing is conditional-gated. It is **decidable** when
provenance containment settles it, $\pi(\text{outcome}) \subseteq \pi(p)$,
and **judgmental** otherwise.

"#.into(),
            mechanism: r#"`Pool::classify_standing` + `AuthorizeError::StandingIsJudgmental`. What is enforced is the REFUSAL TO GUESS: where containment does not settle standing, `authorize` returns the judgmental branch as an error rather than minting a pen, and the cited test requires that variant by name. NOT enforced, and not closable here: the branch itself. Closing it needs a judge, terminating at depth one ({#standing-terminates-at-depth-one}) and disjoint from the AUTHOR ({#standing-judge-disjoint-from-author}); rung has no term for that dispatch and inventing a ruling would be worse than surfacing the gap."#.into(),
        }),
        Element::Prop(Prop {
            slug: "standing-terminates-at-depth-one".into(),
            parent: Some("authorial-qualifying-set".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Standing-judgment terminates at depth one. The standing-judge's
own qualification is plain non-identity, decidable by
provenance-disjointness.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "standing-judge-disjoint-from-author".into(),
            parent: Some("authorial-qualifying-set".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"That disjointness is relative to the **author**, not to the
audited subject. The judge ruling *"does this principal have standing over
that subject?"* must not be that principal.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "two-escalation-triggers".into(),
            parent: Some("authorial-qualifying-set".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Two escalation triggers exist and are not the same.

| trigger | level | reason |
|---|---|---|
| standing is judgmental in this model | **Het** | qualification itself needs a ruling |
| the minimal author cannot close it | **HetOpt** | worth-ordering says escalate |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "standing-escalation-precedes-valuation".into(),
            parent: Some("authorial-qualifying-set".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Standing-escalation happens before any valuation is applied.

"#.into(),
            mechanism: r#""#.into(),
        }),


        Element::Prop(Prop {
            slug: "judgment-presupposes-the-standard".into(),
            parent: Some("pool-is-parameter".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"**A judge cannot evaluate whether a subject meets a standard
without access to the criteria that constitute it.** Judgment of conformity is
impossible without the standard's defining basis — the act of judging
presupposes the very measure against which the subject is to be held.

A judgmental dispatch therefore carries the **standard to the judge** (in the
matter or the prompt), or its ruling is vacuous: it asserts conformity without
the basis of the standard, which is the constant-arrow hazard one level in — a
verdict that never consulted the thing it was supposed to measure against.

"#.into(),
            mechanism: r#"The driver's `Prompt` is where the standard travels:
`WellPosedAdjudicate` embeds the four-cut well-posedness doctrine into the
instruction, and `WELL_POSED_STANDARD` is the single source of the measure the
judge holds. The differential is observable: `consult.rs` consults the same
real model with and without the standard — with a paraphrase it deferred or
affirmed; with the standard it refused q18/q19 on the authentic cut ("a work
item, not a question whose answer the structure determines"). A judge
consulted without the standard is being asked to judge conformity it has no
basis to assert."#.into(),
        }),
Element::Verbatim(r#"---

## 4 · The verdict

"#.into()),
        Element::Prop(Prop {
            slug: "verdict-space-with-metric".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"Satisfaction is quantitative. Every theory declares a **verdict
space** carrying a **metric** $d$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "judges-are-stochastic".into(),
            parent: Some("verdict-space-with-metric".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Judges are stochastic. Verdicts carry confidence, distributional
information, and sensitivity to surface features such as naming.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "boolean-breaks-satisfaction".into(),
            parent: Some("judges-are-stochastic".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Under Boolean satisfaction the satisfaction condition ({#satisfaction-condition})
breaks: renaming a sort changes the verdict.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "typical-verdict-spaces".into(),
            parent: Some("verdict-space-with-metric".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Typical verdict spaces are $[0,1]$, a probability simplex
$\Delta^n$, or a strategy lattice.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "satisfaction-condition-relaxed".into(),
            parent: Some("verdict-space-with-metric".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The satisfaction condition is relaxed from strict equivalence to
a **distance bound**:

$$d\!\left(M \models_{\Sigma'} \mathsf{Sen}(\sigma)(\varphi),\;\; \mathsf{Mod}(\sigma)(M) \models_\Sigma \varphi\right) \le \varepsilon$$

where $\varepsilon$ bounds acceptable naming-induced drift.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "drift-within-tolerance".into(),
            parent: Some("satisfaction-condition-relaxed".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A judge whose confidence shifts from 0.92 to 0.81 under renaming
is within tolerance if $\varepsilon = 0.15$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "metric-carried-by-verdict-space".into(),
            parent: Some("verdict-space-with-metric".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$d$ is carried by the verdict space the theory declares, not
bolted on. Without $d$ there is nothing for $\varepsilon$ to bound, and
satisfaction falls back to Boolean.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "metric-measures-not-ranks".into(),
            parent: Some("verdict-space-with-metric".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$d$ **measures**. It is symmetric. It states how far two verdicts
lie apart under renaming, and nothing about which is better.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "order-as-preference-is-hetopts".into(),
            parent: Some("metric-measures-not-ranks".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Reading an order on the verdict space as preference is
valuation, and belongs to HetOpt ({#het-settles-hetopt-orders}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "epsilon-reported-with-verdict".into(),
            parent: Some("verdict-space-with-metric".into()),
            kind: Kind::Owed { why: "the test exists and is #[ignore]d: `Settled` does not yet carry an error bar, so nothing runs".into() },
            numbering: None,
            prose: r#"$\varepsilon$ is reported alongside the verdict — an honest error
bar.

"#.into(),
            mechanism: r#"GAP — `Verdict` is Boolean (`Conforming | NonConforming`). No metric, no epsilon, so the satisfaction condition does not survive renaming ({#boolean-breaks-satisfaction}). The cited test is the gap as an assertion: two judges settle the same sentence with the same polarity, one barely persuaded and one certain, and the two verdicts are the same object. Deleting the `#[ignore]` reports whether an error bar has reached the caller."#.into(),
        }),
        Element::Prop(Prop {
            slug: "translation-invariance-is-candidates-burden".into(),
            parent: Some("verdict-space-with-metric".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Translation-invariance is the **candidate's** burden. A candidate
that adopts obscure naming bears the cost of the judge's drift. The
Proponent must name its structures clearly enough that its strategy
survives renaming ({#satisfaction-is-a-game}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 5 · The semantics

"#.into()),
        Element::Prop(Prop {
            slug: "algebra-is-kleisli-functor".into(),
            parent: None,
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"An algebra is a functor into the Kleisli category of the principal
monad:

$$M : T \to \mathbf{Kl}(\mathcal{P})$$

| gate | interpretation |
|---|---|
| `decidable` | an ordinary pure morphism — an actual function on the carrier; factors through $\eta$ |
| `judgmental` | a morphism in $\mathbf{Kl}_{\text{judg}}(\mathcal{P})$ — a computation that may consult the outside |
| `authorial` | a morphism in $\mathbf{Kl}_{\text{auth}}(\mathcal{P})$ — an enactment by a principal with standing. Never pure. |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "not-a-set-functor".into(),
            parent: Some("algebra-is-kleisli-functor".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"An algebra cannot be a functor into $\mathbf{Set}$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "set-functor-decides-everything".into(),
            parent: Some("not-a-set-functor".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A functor to $\mathbf{Set}$ assigns every operation — including
judgmental ones — to an actual function, that is, to a decision
procedure.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "set-functor-violates-refusal".into(),
            parent: Some("not-a-set-functor".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Such an algebra would *decide* the judgmental operations,
computing the very judgments the gate marker says no closed system can
discharge on itself. That is {#non-identity-not-deferrable} violated in the semantic dimension.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "monad-reading".into(),
            parent: Some("algebra-is-kleisli-functor".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$\mathcal{P}(X)$ is *"an $X$, possibly obtained by a call on a
principal."*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "unit-is-no-outside".into(),
            parent: Some("monad-reading".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The unit $\eta : X \to \mathcal{P}(X)$ is *"no outside needed"*;
decidable data embeds.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgmental-is-kleisli-arrow".into(),
            parent: Some("monad-reading".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/panel.rs::a_judgmental_arrow_returns_a_set_and_not_a_value".into() },
            numbering: None,
            prose: r#"A judgmental operation is a Kleisli arrow
$A \to \mathcal{P}(B)$.

"#.into(),
            mechanism: r#"`A → 𝒫(B)` is a claim about **shape**, and the shape is exhibited directly: one argument, two qualifying judges, two different and equally well-formed Dispositions. Were `dispose` an `A → B` the second call could not disagree. The non-determinism is the outside itself — {#no-preference-among-judges} forbids Het from ranking the two. A *blocking* outside call works today; `rung-std`'s `LlmCall` ladder puts one on the arrow. Q8 constrains **how** the call is made, not whether the arrow is Kleisli."#.into(),
        }),
        Element::Prop(Prop {
            slug: "monad-is-what-outside-adds".into(),
            parent: Some("monad-reading".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The monad is exactly *what the trip through the outside adds
that the algebra could not generate alone.*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "kleisli-composition-interleaves".into(),
            parent: Some("monad-reading".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Composing pure morphisms with judgmental ones is Kleisli
composition. This is why the fragments interleave without collapsing.

### Provenance

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgmental-arrow-shape".into(),
            parent: Some("monad-reading".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_failed_type".into() },
            numbering: None,
            prose: r#"A judgmental operation has the shape

$$A \longrightarrow \mathcal{P}\Big(\textstyle\sum_i B_i \;+\; A\Big)$$

The monad is the outside call. The sum is the verdict space. The final
summand is the **residual** — the argument returned unconsumed when the
outside does not answer.

"#.into(),
            mechanism: r#"The `+ A` residual is `Failed<Prev> { token, error }` — the unconsumed argument handed back. rung-CT names it the Prism's residual ([residual-is-the-optics-residual](rung-ct-props.md#residual-is-the-optics-residual)) and is why the error structure is not a Kleisli arrow; the monad `P` layers on the forward pass, which rung-CT explicitly permits ([effects-layer-on-the-forward-pass](rung-ct-props.md#effects-layer-on-the-forward-pass))."#.into(),
        }),
        Element::Prop(Prop {
            slug: "provenance-structure".into(),
            parent: Some("algebra-is-kleisli-functor".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The base category carries a **provenance structure**: every
object $X$ is equipped with a provenance map

$$\pi_X : X \to \mathsf{Prov}$$

to a discrete category of provenance tags.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "morphisms-preserve-provenance".into(),
            parent: Some("provenance-structure".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"Morphisms preserve or strictly externalize provenance.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "monad-is-provenance-strict".into(),
            parent: Some("provenance-structure".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"$\mathcal{P}$ is **provenance-strict**:

$$\pi_{\mathcal{P}X} \circ \eta_X = \pi_X, \qquad \pi_{\mathcal{P}X} \circ \mu_X = \pi_{\mathcal{P}^2X}$$

$\eta$ never invents a new author; $\mu$ propagates the outermost author.

### Admissibility

"#.into(),
            mechanism: r#"`carry` is the natural home for provenance: a product factor preserved across every arrow, immutable by G5. It does not carry a *principal's* provenance, which lives outside the ladder."#.into(),
        }),
        Element::Prop(Prop {
            slug: "constant-arrow-hazard".into(),
            parent: Some("algebra-is-kleisli-functor".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624".into() },
            numbering: None,
            prose: r#"Nothing in the plain Kleisli construction prevents $M$ from
sending a judgmental operation to a **constant** arrow
$c_j : a \mapsto \eta(j)$ whose value $j$ is drawn from $M$'s own carrier.
The selection rule never fires; self-reference has been hard-coded into
the interpretation.

"#.into(),
            mechanism: r#"G2 sealed construction. A judgmental arrow cannot be interpreted by a constant drawn from the algebra's own carrier, because no mid-ladder rung is constructible outside its module."#.into(),
        }),
        Element::Prop(Prop {
            slug: "admissibility-subcategories".into(),
            parent: Some("constant-arrow-hazard".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Judgmental and authorial arrows therefore inhabit their
respective admissibility sub-categories:

$$\mathbf{Kl}_{\text{judg}}(\mathcal{P}) = \{\, f : \pi(f(a)) \cap \pi(a) = \emptyset \,\} \qquad \text{(the outside)}$$

$$\mathbf{Kl}_{\text{auth}}(\mathcal{P}) = \{\, f : \pi(f(a)) \subseteq \pi(p) \ \wedge\ \mathsf{standing}(p, a) \,\} \qquad \text{(the steward)}$$

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgment-provenance-is-the-judges".into(),
            parent: Some("constant-arrow-hazard".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/gate_law.rs::a_settled_receipt_carries_the_judges_provenance".into() },
            numbering: None,
            prose: r#"A judgmental arrow's outcome carries its **judge's** provenance:
$\pi(f(a)) \subseteq \pi(p)$ for the principal $p$ drawn from
{#judgmental-qualifying-set}. The judgmental mirror of
{#proposal-provenance-is-authors}: a Proposal's provenance is its
author's, and a ruling's is its judge's.

Without it {#admissibility-subcategories} states a condition on $f(a)$
that nothing in the interpretation obliges $f$ to meet. {#constant-arrow-hazard}
is precisely the arrow that meets every *dispatch* condition and no outcome
condition: the selection rule fires honestly and the value still comes from
$M$'s own carrier. A dispatch discipline cannot refuse it, because the dispatch
was not what was wrong.

**Output admissibility is then a theorem, not a further check.** With
{#judgmental-qualifying-set} enforced at the mint and this enforced where
the outcome is spent:

$$\pi(f(a)) \subseteq \pi(p) \ \wedge\ \pi(p) \cap \pi(a) = \emptyset \implies \pi(f(a)) \cap \pi(a) = \emptyset$$

which is {#admissibility-subcategories}'s judgmental clause. An
implementation that also asserted the disjointness would be asserting a
conclusion whose premises it already enforces — a third guarantee in
appearance and none in substance. *Conformance: `theory!`'s `settle` refuses
$\pi(f(a)) \not\subseteq \pi(p)$ with `SettleError::OutcomeNotFromJudge`, and
`ladder!` injects `must_derive_from_judge` as an epilogue on a forward
judgmental transition ({#g15-outcome-provenance});
`gate_markers.rs::{a_judgmental_arrow_may_not_return_the_provenance_it_judged,
the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render}`,
`gate_law.rs::a_judgment_rendered_by_another_principal_is_refused`.*

The outcome is unforgeable because it is **sealed**: `Judgment` has no
constructor outside `rung` and `Principal::judgment` is the only mint, so the
provenance an outcome carries is not a value its producer chose. A verdict
handed in as a parameter — which is what `settle` took before this proposition
— is exactly the case the seal removes.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "authorial-admissibility-stronger".into(),
            parent: Some("constant-arrow-hazard".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Authorial admissibility is **stronger, not weaker** — not
"anything goes," but "only the principal who holds stewardship may enact
on it." Where judgmental demands disjointness, authorial demands
containment plus standing.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "one-monad".into(),
            parent: Some("constant-arrow-hazard".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Both are sub-categories of the **same** $\mathbf{Kl}(\mathcal{P})$.
Distinct monads would mean distinct principal pools, which {#one-pool-two-filters} does not
license.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "gate-relative-admissibility-licensed".into(),
            parent: Some("constant-arrow-hazard".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Admissibility is gate-relative, and this is licensed.
Decidability is already fiber-relative and classified one level up
({#classifier-one-level-up}); gate-relative admissibility is the same pattern applied to
provenance instead of decidability. The institution's uniformity lives in
*one $\models$, gate-dispatched* — not in having one admissibility
predicate.

### Gate-faithfulness

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "gate-faithful".into(),
            parent: Some("algebra-is-kleisli-functor".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"An algebra is **gate-faithful** when every `decidable` operation
factors through $\eta$, every `judgmental` operation is a
judgmentally-admissible Kleisli arrow, and every `authorial` operation is
an authorially-admissible Kleisli arrow.

"#.into(),
            mechanism: r#"Q11 (gate-faithfulness, open), with **one** blocker rather than two. Blocker (1) — the returned value — has CLOSED, and it closed by derivation rather than by an epilogue guard on the condition itself. R2 obliges a judgmental outcome to carry its judge's provenance ({#judgment-provenance-is-the-judges}): `Judgment` is sealed, `Principal::judgment` is the only mint and calls the oracle `Principal::rule`, and π(f(a)) ⊆ π(p) is asserted where a `Judgment` is spent — by `theory!`'s `settle` and by G15's injected epilogue. With G13 already enforcing π(p) ∩ π(a) = ∅, {#admissibility-subcategories}'s judgmental clause is a THEOREM of two enforced facts, so nothing calls `Prov::overlaps` on the way out and nothing should. What is left of blocker (1) is narrower and is recorded as such: the authorial outward conjunct and branching judgmental arms ([5.621](rung-props.md#outward-conditions-remaining)). Blocker (2) STANDS and is why this row does not move: `#[conditional(..)]` is a parse-time refusal, gate-faithfulness quantifies over EVERY operation, and an algebra with a conditional operation therefore cannot state this proposition here at all. The cited test is that blocker made runnable — it asks the macro to accept a conditional marker, and deleting its `#[ignore]` reports whether it does. Purity was a third blocker and is CLOSED on received advisory input: η is 𝒫's unit, so "factors through η" IS 𝒫-purity and never claimed absolute purity; that a decidable body may read a clock is {#purity-not-secured}, a limit already stated. Argued with its falsifiers at `questions/q11-gate-faithfulness.md`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "mod-only-gate-faithful".into(),
            parent: Some("gate-faithful".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$\mathsf{Mod}(\Sigma)$ consists **only** of gate-faithful
algebras.

"#.into(),
            mechanism: r#"Follows {#gate-faithful}, and parks on the same remaining blocker: `Mod(Σ)` can consist only of gate-faithful algebras once gate-faithfulness is checkable, and it is not checkable for an algebra with a conditional operation, because such an algebra cannot be declared. The outward half that used to park this row has closed — a `theory!` declaration can no longer settle a judgmental sentence with a verdict its judge never gave ({#judgment-provenance-is-the-judges}) — which narrows the row without moving it."#.into(),
        }),
        Element::Prop(Prop {
            slug: "refusal-at-model-category".into(),
            parent: Some("gate-faithful".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A gate-faithful algebra cannot launder a judgmental operation
into a decidable one, and cannot dispatch judgment to itself. The refusal
is enforced at the level of the model category, not as a post-hoc
selection rule.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "condition-propagates-by-reindexing".into(),
            parent: Some("gate-faithful".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Because provenance re-indexes along signature morphisms, the
condition propagates through the fibration. Re-indexing cannot invent a
common author that did not already exist.

### Subjects

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "subject-defined".into(),
            parent: Some("algebra-is-kleisli-functor".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"An **subject** is an inhabitant of a carrier set $M(S)$ — a
specific datum, an element sitting in the algebra's interpretation of a
sort.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "decidable-runs-pure".into(),
            parent: Some("subject-defined".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A decidable operation on an subject runs as a pure morphism: its
result is computed inside the algebra, with no outside.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgmental-runs-kleisli".into(),
            parent: Some("subject-defined".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A judgmental operation on an subject runs as a Kleisli morphism:
it emits an outside call, and the outcome is obtained only when the
outside answers.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "self-governing-not-self-closing".into(),
            parent: Some("subject-defined".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624".into() },
            numbering: None,
            prose: r#"An subject is therefore **self-governing** — its own algebra runs
its decidable audit — but **not self-closing**: its judgmental
dispositions require the monad's outside.

"#.into(),
            mechanism: r#"G2 sealed construction. This proposition *is* rung's founding refusal: an attempt to fold a live verdict into the next state was rejected by the sealed constructor — [the law](rung-ct-props.md#the-law). The algebra runs its own decidable step; it cannot construct the state that holds a judgmental outcome."#.into(),
        }),
        Element::Prop(Prop {
            slug: "autopoiesis-made-precise".into(),
            parent: Some("subject-defined".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"That is autopoiesis without self-loop degeneracy, made precise.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 6 · The tower

"#.into()),
        Element::Prop(Prop {
            slug: "fractal-property".into(),
            parent: None,
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"An algebra whose carrier contains subjects that themselves carry
signature declarations **becomes a theory at the next level**, with its
own fiber of algebras below.

"#.into(),
            mechanism: r#"The composite Grothendieck opfibration ([opfibrations-compose](rung-ct-props.md#opfibrations-compose)), resolved by Q10 (`questions/`). The correspondence is proved and no hierarchy is built — which leaves the property itself needing a run, and the cited test is one: the pass composed with itself at a container boundary, where the destination's own law is what refuses a write the source's judge already authorized ({#target-runs-its-own-models})."#.into(),
        }),
        Element::Prop(Prop {
            slug: "tower-is-a-fibration".into(),
            parent: Some("fractal-property".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The tower is a **fibered category** — the Grothendieck
construction over the category of theories.

| level | role in fibration |
|---|---|
| theory $T$ | object in the base category $\mathbf{B}$ |
| $\mathsf{Mod}(T)$ | fiber over $T$ — the category of $T$-algebras |
| $\sigma : T_1 \to T_2$ | base morphism — a signature morphism |
| $\mathsf{Mod}(\sigma)$ | re-indexing — restricts $T_2$-algebra views to $T_1$ |
| $\models_T$ | fiber-wise relation: algebra × sentence → verdict |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "same-relation-every-level".into(),
            parent: Some("tower-is-a-fibration".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The satisfaction relation is the same at every level. What
changes is which theory's $\models$ is invoked and which principal pool
is available.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "kleisli-iterates".into(),
            parent: Some("tower-is-a-fibration".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The Kleisli construction iterates: the same algebra becomes the
theory whose satisfaction relation tests algebras one level below.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "tower-semantic-every-level".into(),
            parent: Some("tower-is-a-fibration".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The tower is **semantic at every level**. The fibration carries
the Kleisli structure through re-indexing, and gate-faithfulness is
preserved by signature morphisms.

### Two kinds of pointing

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "two-directions-two-bases".into(),
            parent: Some("tower-is-a-fibration".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Two directions run over different bases and must not be conflated.
**Conformance** runs from a model to its theory and re-indexes
contravariantly — the tower of {#tower-is-a-fibration}.
**Propagation** runs from a revised subject to whatever depends on it and
transports covariantly ({#target-runs-its-own-models}). Het declares
that propagation occurs; the taxonomy of dependency is the theory's, not
Het's ({#governs-who-not-what}).

"#.into(),
            mechanism: r#"Conformance is Het's fibration (Mod: Sign^op → Cat, contravariant). Propagation is rung-CT's opfibration, pushforward and opcartesian ([conformance-and-propagation-run-over-different-bases](rung-ct-props.md#conformance-and-propagation-run-over-different-bases)). Different bases at adjacent levels — not opposite orientations of one tower. The cited test is where the two are visible at once and are not conflated: the docket's sentences are run *per question* — conformance, each model against its own theory — while drift is reported *along outbound edges*, from a revised question to whatever depended on it. One suite, two directions, and the edge set is the theory's rather than Het's ({#governs-who-not-what})."#.into(),
        }),
        Element::Prop(Prop {
            slug: "two-kinds-of-pointing".into(),
            parent: Some("fractal-property".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Two distinct relations both look like "pointing," and run in
opposite directions.

| | direction | what it is |
|---|---|---|
| **conformance declaration** | up (concrete → abstract) | a *model* declares the theory it interprets. This is what a checker walks. |
| **signature morphism** | down (abstract → concrete) | the arrow selecting the structure a theory's algebras must carry — the semantic map whose existence the declaration asserts. |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "pointings-are-duals".into(),
            parent: Some("two-kinds-of-pointing".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The two are duals of one edge. The up-pointing declaration is
what the satisfaction-checker walks to find the theory to test against;
the down-pointing morphism is the truth-condition the declaration claims
to satisfy.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "declaration-on-models-only".into(),
            parent: Some("two-kinds-of-pointing".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A conformance declaration is carried by **models only**. A
theory does not carry one.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "model-without-theory-is-empty".into(),
            parent: Some("two-kinds-of-pointing".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A model with no declared theory is a set of records with no law
to be measured against. There is nothing for $\models$ to evaluate.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "declaration-is-not-a-morphism".into(),
            parent: Some("two-kinds-of-pointing".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A conformance declaration is not a signature morphism and cannot
serve as one. Theory-to-theory morphisms are the arrows of
$\mathbf{Sign}$ ({#sign-category}) and are constitutive of the institution.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "three-relations-not-conflated".into(),
            parent: Some("two-kinds-of-pointing".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Three relations must not be conflated: a population interprets a
law ({#declaration-on-models-only}); a theory supplies $\mathcal{P}$ ({#supplier-interface}, checked by signature
inspection); a theory extends another (a morphism in $\mathbf{Sign}$).

### The gate law

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "gate-law".into(),
            parent: Some("fractal-property".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"Gate markers may be preserved or increased along morphisms —
`decidable` → `decidable` or `judgmental`; `judgmental` → `judgmental`.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "no-laundering-along-morphisms".into(),
            parent: Some("gate-law".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"No morphism may launder a judgmental predicate into a decidable
one. This is {#non-identity-not-deferrable} at the morphism level.

### Termination

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "tower-floor".into(),
            parent: Some("fractal-property".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The tower terminates on a **decidable well-formedness predicate**
$W$ on signatures.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "wellformedness-clauses".into(),
            parent: Some("tower-floor".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"$W(\Sigma)$ holds when: $\Sigma$ declares at least one sort and
at least one operation; every operation carries a gate marker (2, {#unmarked-not-wellformed});
every judgmental operation declares a competence role ({#judgmental-declares-role}); every
authorial operation declares a standing predicate ({#authorial-declares-standing}); every
conditional operation names a classifying sentence ({#conditional-names-classifier}); and, if
$\Sigma$ supplies $\mathcal{P}$, it declares the four predicates of {#supplier-interface}
at their stated arities.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "clauses-decidable-by-inspection".into(),
            parent: Some("tower-floor".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Each clause is decidable by inductive inspection of the
declaration. $W$ invokes no judge.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "floor-not-gate-marked".into(),
            parent: Some("tower-floor".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$W$ is the floor the regress terminates on. It is not
gate-marked and is not itself a Het theory; asking it to be one would ask
the floor to stand on itself.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "w-checks-declaration-not-adequacy".into(),
            parent: Some("tower-floor".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$W$ checks **declaration, never adequacy**. It never asserts
that any concrete principal satisfies its own predicates, nor that the
pool is non-empty.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "adequacy-defined".into(),
            parent: Some("fractal-property".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::the_pool_propagates_a_deferral_and_mints_no_licence".into() },
            numbering: None,
            prose: r#"Adequacy lives one level below, inside the theories that actually
invoke judges. For a judgmental sentence $\varphi$ of a theory $T$:

$$\mathsf{Adequate}_T(\varphi) \equiv \text{“a qualifying non-identical judge for } \varphi \text{ exists and returns a verdict”}$$

"#.into(),
            mechanism: r#"Adequacy is a CONJUNCTION — a qualifying judge exists AND returns a verdict — and the engine now has a term for each conjunct failing separately. An empty qualifying set is `QualifyError::NotCapable` / `NonIdentityViolated` / `PoolExhausted`; a judge that exists and has not answered is `QualifyError::JudgeDeferred`, which is documented as NOT a filter failure. Before the deferral there was one outcome for both and the second conjunct was unrepresentable, so the definition could not be wrong about anything. The cited test settles both halves against one argument: the deferring pool mints no licence, and an answering pool does. Collapsing `JudgeDeferred` into `PoolExhausted` is type-valid and reddens it at the `Err(other)` arm."#.into(),
        }),
        Element::Prop(Prop {
            slug: "adequacy-is-judgmental".into(),
            parent: Some("adequacy-defined".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"That sentence is itself **judgmental**, discharged by an outside
call exactly when an algebra of $T$ attempts to interpret $\varphi$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "adequacy-failure-is-not-a-w-defect".into(),
            parent: Some("adequacy-defined".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Failure of adequacy is an ordinary judgmental failure at the
level where the judge is required. It is not a defect in $W$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "adequacy-asks-for-a-judge".into(),
            parent: Some("adequacy-defined".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Adequacy asks for *a* qualifying judge, not the minimal one.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "adequacy-local-not-global".into(),
            parent: Some("adequacy-defined".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Adequacy is **local, not global**. There is no infinite regress
and no global fixed-point proof.

### Self-grounding

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "adequacy-failure-returns-residual".into(),
            parent: Some("adequacy-defined".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::recovers_from_the_failed_error_path".into() },
            numbering: None,
            prose: r#"Adequacy failure returns the residual
({#judgmental-arrow-shape}). The argument is not consumed, and
re-enters.

"#.into(),
            mechanism: r#"G9 error-path recovery, `Failed(R) => R`, explicitly unguarded — a re-entry after an unanswered call may reuse the argument. G4 additionally forbids silently dropping the returned residual. A judgmental FORWARD transition now carries the same residual too ([G16](rung-props.md#g16-the-residual-channel)); the children below are that case."#.into(),
        }),
        Element::Prop(Prop {
            slug: "suspension-is-the-residual".into(),
            parent: Some("adequacy-failure-returns-residual".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::a_judgmental_forward_transition_returns_the_argument_unconsumed".into() },
            numbering: None,
            prose: r#"A judge that **exists and has not answered** is one of the two ways
{#adequacy-defined} fails, and it is the interesting one. Adequacy is a
conjunction — a qualifying judge exists *and* returns a verdict — so a judge
that raises a matter instead of ruling leaves adequacy **undischarged**, which
{#adequacy-failure-returns-residual} already disposes of: the argument
comes back unconsumed and re-enters.

**A suspension is therefore not a new summand.** It is the final `+ A` of
{#judgmental-arrow-shape}, read as *"awaiting a matter this dispatch
raised"* rather than *"nobody answered"*. Nothing in the arrow's shape changes;
what changes is that the residual now carries **what** is being awaited.

"#.into(),
            mechanism: r#"G16, and the whole of the claim is that it adds nothing. A judgmental forward transition returns `Result<Next, Suspended<Prev>>`, and the cited test coerces the emitted `fn` to that exact pointer type and then reads the token back out to find the very argument it passed in — unconsumed, as {#adequacy-failure-returns-residual} requires. What is new is the channel, not the summand: before it a forward judgmental transition returned its target rung and a theory whose principal could not answer yet had no term for saying so. Emitting `#to` instead of the `Result` is type-valid at the macro and turns the test red at the coercion."#.into(),
        }),
        Element::Prop(Prop {
            slug: "raised-reference-is-opaque".into(),
            parent: Some("suspension-is-the-residual".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::the_raised_reference_is_carried_and_never_interpreted".into() },
            numbering: None,
            prose: r#"The identity of the raised matter is **opaque** to Het, for the
reason {#pool-is-opaque} gives. Het never inspects an inhabitant of
$\mathcal{P}$, and what an inhabitant raised is on the same side of that line:
an issue number, a lifecycle subject, a filename — all of them the supplying
theory's, none of them Het's. Het transports the reference from the principal
that raised it to the position that resumes on it, and has no predicate over it
whatever ({#nothing-further-required}).

"#.into(),
            mechanism: r#"{#pool-is-opaque} reaches the raised matter. `rung::Raised` carries two strings the crate never reads — no ordering, no well-formedness, no roster of live references — and the cited test raises `¶ anything at all §` and gets it back unchanged. `Terminated::of` is the one derived constructor, which is what keeps opacity from becoming laxity: evidence is built FROM a `Raised`, so it cannot name a reference nobody raised. Adding any predicate over the reference to `rung` is type-valid and makes the cited case a refusal instead of a round trip."#.into(),
        }),
        Element::Prop(Prop {
            slug: "deferral-is-not-a-verdict".into(),
            parent: Some("suspension-is-the-residual".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::a_deferral_is_not_a_judgment".into() },
            numbering: None,
            prose: r#"A deferral **is not a verdict**, and no operation converts one into
one. A judge that raised a matter has said nothing about the sentence, so a
verdict attributed to it would name a judge that did not rule — which is
{#constant-arrow-hazard} in the one disguise the dispatch discipline
cannot see through, because here the judge is real and was honestly selected
and it is the *answer* that does not exist.

"#.into(),
            mechanism: r#"The R2 seal, on the side where the judge is real and the verdict is not. `Principal::judgment` is the only mint for a `Judgment` and it calls `rule`; when `rule` defers there is no verdict, and the sealed `Consulted` says so rather than manufacturing one. There is no `From<Raised> for Judgment` and no `unwrap_or`. The mutation is the direct one: making the deferring branch of `Principal::judgment` build a `Judgment` anyway is type-valid — any verdict at all will do, which is exactly the point — and turns the cited test red at its `Rendered` arm."#.into(),
        }),
        Element::Prop(Prop {
            slug: "no-preference-after-a-deferral".into(),
            parent: Some("suspension-is-the-residual".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::the_pool_propagates_a_deferral_and_mints_no_licence".into() },
            numbering: None,
            prose: r#"A theory MAY dispatch again, and the pool does not do it for it.
Walking on to the next qualifying judge because the first raised a matter is a
**preference among qualifying judges**, which {#no-preference-among-judges}
forbids Het to have; whether it is worth doing is a worth-question and belongs
to HetOpt ({#het-settles-hetopt-orders}).

"#.into(),
            mechanism: r#"The pool reports what the principal it selected said, and does not walk on. `Pool::consult` and `Pool::qualify_for` return `QualifyError::JudgeDeferred` carrying the reference; the cited test also shows a pool whose member answers is unaffected, so the deferral is a distinct outcome and not a new way for the FILTER to fail. Looping to the next survivor is type-valid and is a preference among qualifying judges, which {#no-preference-among-judges} forbids; it turns the cited test red at the `JudgeDeferred` arm as soon as the pool holds a second, answering principal."#.into(),
        }),
        Element::Prop(Prop {
            slug: "resumption-is-authorial".into(),
            parent: Some("adequacy-failure-returns-residual".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::a_suspension_resumes_through_the_authorial_edge".into() },
            numbering: None,
            prose: r#"Resuming a suspended dispatch is **authorial**, not judgmental.

The residual re-enters at the position that produced it, which means the
suspended object is *written back into* the run. That is a transformation of the
subject, and {#judgment-refuses-authorship-requires} settles which filter
governs a transformation: judgment classifies and refuses the audited party,
authorship transforms and requires standing over it. So resumption dispatches
through {#authorial-qualifying-set} — capability **and** standing — exactly
as `enact` does ({#enact-makes-an-endofunctor}).

**The judge that ruled on the raised matter cannot be the one that resumes.**
It qualified by being provenance-disjoint from the subject
({#judgmental-qualifying-set}), and that disjointness is precisely what
denies it standing ({#provenance-overlap-is-the-point}). The two roles are
held by two principals, and that is the shape rather than an inconvenience.

"#.into(),
            mechanism: r#"G16, and it is FORCED rather than chosen. Reviving a suspended run constructs a rung, which [G2](rung-props.md#g2-sealed-construction) seals from outside the module — so the resume edge is emitted inside it, and an edge inside the seal that anyone may call is the seal with a door in it. The marker is therefore mandatory: a resume edge with no `#[authorial(R)]` is a `compile_error!` (`suspension.rs::a_resume_edge_without_an_authorial_marker_is_refused`), and calling one without its pen is E0061. The cited test drives the round trip and coerces the emitted `fn` to its exact pointer type. Deleting the injected `must_hold_standing_over` from the resume path is type-valid and reddens `::resume_refuses_a_pen_over_another_container`, where the body never mentions the pen at all."#.into(),
        }),
        Element::Prop(Prop {
            slug: "resumption-needs-a-terminal".into(),
            parent: Some("resumption-is-authorial".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::resume_refuses_evidence_from_another_raised_matter".into() },
            numbering: None,
            prose: r#"Resumption is gated on the raised matter having reached a
**terminal**, and on that terminal being the one **this** dispatch awaits. What
counts as a terminal is the supplying theory's ({#nothing-further-required}),
exactly as the reference is.

This is not a promise of termination. {#no-bound-on-reentry} is a stated
limit and this does not close it: a raised matter that never terminates yields
no evidence, and the outer arrow stays suspended — **visibly**, which is what
{#stated-as-limit-not-closed} asks for and all it asks for.

"#.into(),
            mechanism: r#"`must_answer_the_raised`, injected. A `Terminated` is derived from the `Raised` it is about, which closes fabrication; the guard closes TRANSFER, exactly as `must_be_bound_to` does for a licence and `must_hold_standing_over` for a pen. The cited test resumes with evidence about `q-99` and is refused. It asserts nothing about termination and must not: {#no-bound-on-reentry} stands, and a matter that never terminates yields no evidence and leaves the arrow suspended, visibly."#.into(),
        }),
        Element::Prop(Prop {
            slug: "resumption-is-unguarded".into(),
            parent: Some("resumption-is-authorial".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::the_same_suspension_resumes_twice_with_no_progress_guard".into() },
            numbering: None,
            prose: r#"Re-entry through a resumption is **unguarded**. A raised matter may
take any number of rounds, and a host that bounded them would have declared the
bound Het declines to declare — {#guarded-reentry-is-eviction} names that
an eviction rule under another name, whatever else it is called.

"#.into(),
            mechanism: r#"The ABSENCE, pinned. The macro injects no `must_progress` on a resume edge, and the cited test suspends and resumes the same run twice with a payload that does not change — which is the normal case, not a stall: the argument was never consumed and the raised matter took another round. Injecting `must_progress` there is type-valid and reddens the test on the FIRST round, which is what makes this row an enforcement rather than an observation that nothing happened. A guard would be the bound Het declines to declare ({#guarded-reentry-is-eviction})."#.into(),
        }),
        Element::Prop(Prop {
            slug: "self-grounding-is-a-pair".into(),
            parent: Some("fractal-property".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Self-grounding is a property of a **pair**, never of one member
alone.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "het-self-grounding-condition".into(),
            parent: Some("self-grounding-is-a-pair".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Het is self-grounding when its own signature satisfies $W$, and
$W$ is decidable.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "neither-stands-on-itself".into(),
            parent: Some("self-grounding-is-a-pair".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Neither member stands on itself: the signature is grounded by
satisfying a predicate that is not gate-marked, and the predicate is
grounded by being an ordinary shape-check.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "first-question-is-hets-own-signature".into(),
            parent: Some("self-grounding-is-a-pair".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The first question is therefore not whether some domain conforms
to Het, but whether Het's own signature satisfies $W$. Answering it
**demonstrates** self-grounding rather than asserting it.

### Signature-claims are not sentences

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "signature-claims-are-w-clauses".into(),
            parent: Some("fractal-property".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A theory's claims about **its own signature** — that a type is
closed, that two axes are orthogonal, that the theory declares no
population — are clauses of $W$, not sentences.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "sentence-needs-an-inhabitant".into(),
            parent: Some("signature-claims-are-w-clauses".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A sentence is evaluated as $M \models \varphi$ against
inhabitants of a carrier. A signature-claim has no such inhabitant to
test, and walking a population cannot check it.

"#.into(),
            mechanism: r#"A signature-claim has no carrier inhabitant to test. Nothing for a host to run."#.into(),
        }),
        Element::Prop(Prop {
            slug: "empty-equation-is-a-misfiling".into(),
            parent: Some("signature-claims-are-w-clauses".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Such a claim carries no equation because there is nothing for
$\models$ to compute. **A decidable sentence with no equation is a
mis-filing, not an omission** — the emptiness is the diagnostic.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 7 · The game

"#.into()),
        Element::Prop(Prop {
            slug: "satisfaction-is-a-game".into(),
            parent: None,
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Satisfaction is a two-player game. A sentence is satisfied iff the
**Proponent** has a winning strategy.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "proponent-and-opponent".into(),
            parent: Some("satisfaction-is-a-game".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The Proponent is the candidate algebra, asserting
$M \models \varphi$. The **Opponent** is the environment, which may query
an oracle — the judge.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "decidable-games-are-bounded".into(),
            parent: Some("proponent-and-opponent".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Decidable predicates are games with finite, mechanizable winning
strategies: the tree is bounded and the strategy is a decision procedure.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgmental-games-have-an-oracle".into(),
            parent: Some("proponent-and-opponent".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Judgmental predicates are games where the Opponent has oracle
access: the tree may be unbounded, and the strategy involves querying the
oracle at specific nodes.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "game-resolves-disagreement".into(),
            parent: Some("proponent-and-opponent".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Static satisfaction cannot say who is right when judge and
candidate disagree. The game can: the Proponent may contest, and the
contest is itself a move ({#proposal-vocabulary}).

### The pass

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "the-pass".into(),
            parent: Some("satisfaction-is-a-game".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/pass_ladder.rs::the_pass_runs_end_to_end_as_a_ladder".into() },
            numbering: None,
            prose: r#"The audit-rectify pass is the game in operation — a chain of
principals, each acting on what the previous one produced. The gate says
*how* each move is settled; the table says *by whom*, and relative to
whose authorship.

| game move | operation | gate | acts | result |
|---|---|---|---|---|
| a violation is found | `audit` | decidable, or judgmental per $\varphi$ | nobody, or a judge disjoint from $M$ | Verdict |
| the Proponent answers | `propose` | **authorial** | an author with standing over $x$ | Proposal |
| the Opponent rules | `dispose` | judgmental | a judge disjoint from **the Proposal** | Disposition |
| the Proponent applies it | `enact` | authorial | an author with standing over $x$ | the revised subject |

"#.into(),
            mechanism: r#"One `ladder!` declaration, and it is now written: `het_pass!` expands to the spine `Governed => Audited => Proposing => #[authorial(Author)] Proposed => #[judgmental(Judge)] { .. }`. The table's `gate` column is a **marker** and its `acts` column is a **parameter type**, so which principal may move is settled by rustc rather than by a driver keeping to a convention: `propose` without a pen is E0061 and `dispose` without a licence is E0061, each with its message committed as a `trybuild` snapshot. Retargeting the judgmental marker at the author's role — one token, type-valid, the library still compiles — turns the cited test red on `expected Qualified<Editor>, found Qualified<Reader>`. rung proves each move was made by one who qualified, not that the move was wise (SPEC §5). What is NOT in the declaration is `enact`: see {#a-cycle-through-an-authorial-act-cannot-close}."#.into(),
        }),
        Element::Prop(Prop {
            slug: "propose-is-authorial".into(),
            parent: Some("the-pass".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"`propose` is **authorial**. Answering a verdict is the
Proponent's move, and producing content about an subject is authorship,
which requires standing over it ({#authorial-qualifying-set}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "judgmental-propose-swaps-roles".into(),
            parent: Some("the-pass".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A judgmental gate on `propose` would dispatch under the
disjointness filter ({#judgmental-qualifying-set}), that is, to the Opponent's side — making the
Opponent play the Proponent's move.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "difficulty-is-not-an-outside".into(),
            parent: Some("the-pass".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"That the remedy is sometimes mechanically determined and
sometimes requires assessment is a statement about the author's
difficulty, not about whether an outside is needed. Authorship is
required either way.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "proposal-provenance-is-authors".into(),
            parent: Some("the-pass".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A Proposal's provenance is its **author's**:
$\pi(\mathsf{propose}(x, v)) \subseteq \pi(p)$ for the authoring
principal $p$. Without this, {#judgmental-qualifying-set} cannot be evaluated at `dispose`.

### The Proposal vocabulary

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "proposal-vocabulary".into(),
            parent: Some("satisfaction-is-a-game".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/acceptance.rs::an_author_may_dispute_a_verdict_without_first_authoring_a_remedy".into() },
            numbering: None,
            prose: r#"A Proposal is one of exactly two.

| | means | licenses |
|---|---|---|
| `remedy` | *"the verdict stands; here is the fix"* | `enact` on acceptance |
| `dispute` | *"the verdict is wrong; the subject stands as authored"* | nothing to enact |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "dispute-is-still-judged".into(),
            parent: Some("proposal-vocabulary".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A `dispute` is still judged. The Opponent rules on it exactly as
on a `remedy`; an author does not overturn a verdict by asserting it.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "dispute-is-the-only-contest".into(),
            parent: Some("proposal-vocabulary".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"`dispute` is the only path to contest a verdict. `propose` is
defined only on a non-conforming verdict, so without it an author who
believed the audit wrong would have to author a remedy for a diagnosis
they dispute, in order to obtain a vehicle for disputing it.

### The Disposition vocabulary

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "remedy-carries-an-edit".into(),
            parent: Some("proposal-vocabulary".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/pass_ladder.rs::the_pass_runs_end_to_end_as_a_ladder".into() },
            numbering: None,
            prose: r#"A `remedy` carries an **edit** — what would be done to the subject.
The edits are the theory's, not Het's ({#edit-required-not-typed}); Het requires only that a
remedy name one, and that `enact` apply it.

"#.into(),
            mechanism: r#"The edit is the rung payload's type, supplied by the theory, and the requirement is now a *variant shape*: an author answers through `Answer<E>`, whose `Remedy(E)` has nowhere to put the absence of an edit. A theory that let a remedy carry none would make `remedy` and `dispute` indistinguishable, and there is no term for it. Dropping the edit in `Proposal::from_chain` — type-valid, `Remedy` rewritten to `Dispute` — reddens the cited test at `rounds: left 1, right 2` (the judge has nothing to reject, so the loop it exists to exercise never runs) and `acceptance.rs::the_pass_runs_end_to_end_as_a_chain_of_principals` with it. The boundary itself is pinned by `acceptance.rs::an_author_may_dispute_a_verdict_without_first_authoring_a_remedy`: a dispute's `edit()` is `None`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "disposition-vocabulary".into(),
            parent: Some("satisfaction-is-a-game".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/acceptance.rs::the_disposition_vocabulary_is_exactly_the_five_that_survive_the_gate".into() },
            numbering: None,
            prose: r#"A Disposition is one of exactly five.

| | terminal | affirming | who acts next |
|---|---|---|---|
| `accept` | ✓ | ✓ | the author enacts |
| `reject-diagnosis` | ✓ | ✗ | nobody — the audit was wrong; the subject stands |
| `reject-remedy` | ✗ | ✗ | the author re-proposes, carrying the reason |
| `defer` | ✗ | ✗ | a prerequisite is required first |
| `raises-questions` | ✗ | ✗ | the auditor clarifies; the subject re-enters |

"#.into(),
            mechanism: r#"G6 exhaustive outcomes. `StepOutcome` is an enum, so every match site must handle all five; adding a disposition breaks every call site at compile time. The cited test pins the vocabulary itself — the five, in order, each with its terminal and affirming flags — so that the two that Het's gate boundary excludes (`accept-with-mod`, `reject-with-alternative`) cannot return without the assertion changing."#.into(),
        }),
        Element::Prop(Prop {
            slug: "disposition-is-a-ruling".into(),
            parent: Some("disposition-vocabulary".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624".into() },
            numbering: None,
            prose: r#"A Disposition is a **ruling, not a revision**. Something must
apply it, and that something is an author with standing ({#authorial-qualifying-set}).

"#.into(),
            mechanism: r#"G2. `dispose` returns a verdict; only the separately-declared authorial arrow produces the revised object. A ruling cannot construct what it rules on."#.into(),
        }),
        Element::Prop(Prop {
            slug: "no-amending-disposition".into(),
            parent: Some("disposition-vocabulary".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/pass_ladder.rs::a_chain_cannot_be_read_for_an_edit".into() },
            numbering: None,
            prose: r#"No Disposition amends a Proposal. A judge that amends is
*transforming*, not classifying; and being provenance-disjoint from the
subject ({#judgmental-qualifying-set}), it cannot hold standing over a modification it has just
authored ({#authorial-qualifying-set}). Any amending variant would require one principal to
satisfy two opposite conditions on one subject.

"#.into(),
            mechanism: r#"G2 plus G10, and the second half is what the pass added. A judge's arrow has no constructor for the authored object — but a continue arm's target rung is built INLINE by `step`, i.e. by the judge, so the pass's re-entry rung is the one place an amendment could have arrived. Its payload is therefore `Chain`: a concrete, non-generic record of an id, a container, a count and prose, with no edit and no type parameter one could hide in. The cited `trybuild` case pins the E0599 that reading an edit off it produces; giving `Chain` an `edit` accessor — type-valid, the library still compiles — turns it red on a diff."#.into(),
        }),
        Element::Prop(Prop {
            slug: "reason-is-not-an-edit".into(),
            parent: Some("disposition-vocabulary".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/acceptance.rs::reject_remedy_is_non_terminal_and_the_reason_reaches_the_author".into() },
            numbering: None,
            prose: r#"`reject-remedy` may carry a **reason**, which is advisory prose
and **not an edit**. Stating why a remedy fails is classification;
supplying the replacement is authorship. The author re-proposes with the
reason in hand.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "reproposal-carries-the-chain".into(),
            parent: Some("disposition-vocabulary".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/pass_ladder.rs::reject_remedy_re_enters_with_no_progress_guard".into() },
            numbering: None,
            prose: r#"A re-proposal carries the chain of prior dispositions and their
reasons. Without them an author can cycle indefinitely on the same
objection, and nothing downstream could detect it.

### Enactment

"#.into(),
            mechanism: r#"The chain rides in the rung payload and there is no other route to a re-proposal: the pass's authorial transition builds its Proposal from the `Chain` the continue arm handed back, so an author cannot drop it by omission. The cited test rejects the identical remedy five times and reads all five reasons off the sixth chain. Deleting the push in `Chain::reentered` — type-valid — turns it red at `left: 0, right: 5`, and `acceptance.rs::reject_remedy_is_non_terminal_and_the_reason_reaches_the_author` with it. Without the chain an author can cycle on one objection and nothing downstream can tell. NOTE: this is exactly what would make a G8 progress guard vacuous — a strictly growing chain never compares equal — which is why re-entry must not use a guarded edge ({#guarded-reentry-is-eviction})."#.into(),
        }),
        Element::Prop(Prop {
            slug: "enact-makes-an-endofunctor".into(),
            parent: Some("satisfaction-is-a-game".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"`enact` is what makes the pass an **endofunctor** rather than a
one-way funnel into a verdict.

"#.into(),
            mechanism: r#"The loop closes, and it closes by COMPOSITION rather than inside the declaration — which is the honest reading and is now recorded as a non-guarantee ({#a-cycle-through-an-authorial-act-cannot-close}). `ladder!` declares a linear spine with backward continue arms, and a continue arm's target is built inline by `step`, so an `Accept -> Governed` arm would have the judge apply the edit ({#no-amending-disposition}). `Accept` is therefore terminal and carries a `Licence`; `enact` is a separate authorial arrow consuming that licence and a pen, and what comes out is audited again. The cited test closes the loop that way: the relocated specimen lands in the fieldbook and the fieldbook's own decidable sentence is run over the result. STILL `expressible`, and the reason is not shyness — no single `ladder!` declaration is an endofunctor, and saying otherwise would be a claim no mutation could falsify. Declaring the composite is Q4, open. rung enforces that the edit ran, not that it was right (SPEC §5), and the edit itself is the theory's ({#edit-required-not-typed})."#.into(),
        }),
        Element::Prop(Prop {
            slug: "licence-is-not-guarantee".into(),
            parent: Some("enact-makes-an-endofunctor".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/acceptance.rs::the_pass_runs_end_to_end_as_a_chain_of_principals".into() },
            numbering: None,
            prose: r#"A terminal-and-affirming Disposition licenses `enact`; it does
not guarantee the edit lands.

"#.into(),
            mechanism: r#"A `Licence<E>` is now a type, minted only from an affirming `Ruling` and consumed by `enact` — so the pass's `Accept` arm carries PERMISSION rather than a revised subject. Permission is all it is: `enact` still checks the pen against `Applies::territory` and hands the domain's own refusal back untouched. Making `enact` swallow `Applies::apply`'s error — type-valid, `world.apply(..)?` to `let _ = world.apply(..)` — turns the cited test red where it requires the fieldbook to refuse a write the cabinet's judge already accepted. The two failure points are {#enact-has-two-failure-points}."#.into(),
        }),
        Element::Prop(Prop {
            slug: "target-runs-its-own-models".into(),
            parent: Some("enact-makes-an-endofunctor".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/questions_of_rung.rs::resolved_runs_its_own_law_on_a_write_the_ruling_already_authorized".into() },
            numbering: None,
            prose: r#"Where the revised subject enters another governed container,
**that container's own $\models$ runs** — the pass composed with itself
under {#fractal-property} — and may refuse it.

"#.into(),
            mechanism: r#"The write-guard exists and fires. `enact` checks the pen against `Applies::territory` and hands `EnactError::TargetRefused` back untouched, so a destination may decline a write its own judge already authorized: in the cited test the relocation is accepted by a qualified judge, refused by the fieldbook for want of a locality, and the source container is left unchanged. The target's law is the **theory's** — the library cannot know what admits a specimen — so rung secures the seam and the standing, not the law. `second_domain.rs::a_pen_for_one_territory_does_not_authorize_another` pins the standing half. What stays with Q4 is expressing the composite as a ladder inside a ladder; the boundary itself no longer waits on it."#.into(),
        }),
        Element::Prop(Prop {
            slug: "enact-has-two-failure-points".into(),
            parent: Some("enact-makes-an-endofunctor".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"An authorization to edit is not a licence to violate the
target's law. `enact` has three failure points: the Disposition may
withhold it, the target may refuse it, and — the one that survives both
refusals — the remedy may land yet not be observably in effect
({#enact-verify}).

### Panels

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "enact-verify".into(),
            parent: Some("enact-has-two-failure-points".into()),
            kind: Kind::Decidable { proof: "rung-driver/tests/rectify_questions.rs::the_seam_runs_one_audit_rectify_cycle_over_rungs_own_questions".into() },
            numbering: None,
            prose: r#"An authorization to edit is not a report of success. After a
remedy is enacted, an **observer** — not the author — must be able to read
back, from the world's state, that the edit is observably in effect; otherwise
a claimed change that did not happen (\`X is now Y\` when the state is \`W\`)
is taken on the author's word. A world that cannot confirm an edit **fails
closed**: success is not claimed.

"#.into(),
            mechanism: r#"Exercised by \`rung-driver/tests/rectify_questions.rs\`'s
seam: after enact the world confirms the enacted edge, and an impostor edit — a
different target, a claimed-but-unapplied change — is refused."#.into(),
        }),
        Element::Prop(Prop {
            slug: "panels".into(),
            parent: Some("satisfaction-is-a-game".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/panel.rs::a_panel_is_the_pass_with_more_than_one_judge".into() },
            numbering: None,
            prose: r#"Panels are $\models$ with more than one judge — the game with an
enlarged oracle-move set. They are not a separate construction.

"#.into(),
            mechanism: r#"A panel is `⊨` with more than one judge, and the proposition says it is **not a separate construction** — so the encoding must not add one. It does not: a seat is a pool of one principal, each seat mints its own licence against the very same argument, and the cited test convenes three of them with nothing `rung-het` does not already export. The combination rule is the theory's, exactly as its edits are ({#edit-required-not-typed}); putting a `panel()` primitive in the library would legislate a rule Het does not have. What stays with Q5 is running the seats **at the same time** — latency, which is HetOpt's ([cut-at-valuation](rung-het-props.md#cut-at-valuation)), not Het's."#.into(),
        }),
        Element::Prop(Prop {
            slug: "panels-cannot-weaken-the-opponent".into(),
            parent: Some("panels".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/panel.rs::a_panel_cannot_weaken_the_opponent".into() },
            numbering: None,
            prose: r#"A Proponent winning strategy in the original game remains
winning in the composite; additional oracle answers can only strengthen
the Opponent.

"#.into(),
            mechanism: r#"The observable form of the claim: the same Proponent move, the same first oracle answer, plus two more — and the seat that played in the original game answers identically in the composite. Added answers may take affirmation away and never grant it, so the Proponent's winning set under the panel is contained in its winning set against any single seat. rung proves the rulings were reached through qualified licences, not that unanimity is the right combination rule ({#panels})."#.into(),
        }),
        Element::Verbatim(r#"---

## 8 · The cut

"#.into()),
        Element::Prop(Prop {
            slug: "het-settles-hetopt-orders".into(),
            parent: None,
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**Het settles belonging. HetOpt orders what belongs.**

$$\textbf{Het} = \text{judgmental institution} + \text{gate-marked } \models + \text{metric verdict space}$$

$$\textbf{HetOpt} = \textbf{Het} + V$$

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "metric-and-preference-same-furniture".into(),
            parent: Some("het-settles-hetopt-orders".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Metric and preference are the same categorical furniture read two
ways. A metric space *is* a category enriched over
$([0,\infty], \ge, +)$, and quantale-enrichment is the general form. They
are not the same **role**.

| | what it does | where it lives |
|---|---|---|
| $d$ — verdict metric | symmetric; how far two verdicts lie apart under renaming | **Het** |
| $V$ — worth-law | orders a conforming set by preference | **HetOpt** |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "cut-at-valuation".into(),
            parent: Some("het-settles-hetopt-orders".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The cut is drawn at **valuation itself**, not at any one
application of it.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "het-declares-no-worth-law".into(),
            parent: Some("cut-at-valuation".into()),
            kind: Kind::Decidable { proof: "rung-std/tests/principals_theory.rs::nothing_in_the_workspace_orders_by_cost_or_epsilon".into() },
            numbering: None,
            prose: r#"A Het theory declares no worth-law $V$, and does not declare the
minimal-judge rule.

"#.into(),
            mechanism: r#"**The α cut, given teeth.** This row and {#ordering-is-hetopts} were `out-of-scope` by default and never inspected — correctly, while nothing in the workspace declared a cost tier or an ε for a worth-law to be built out of. `rung-std::principals` declares both, so the refusal is now a property a run can check: the cited test reads every line of Rust in all four crates that names a cost tier or an ε and fails on any that also sorts, compares, ranks or takes an extremum. It reads attribute lines above a hit as well, so `#[derive(.., Ord)]` on `CostTier` is caught although the derive names no cost of its own — that derive is the cheapest possible crossing of the cut and it is the mutation this test exists to see. `CostTier` and `Epsilon` independently carry no `Ord`, no `PartialOrd` and no accessor, so the minimal-judge rule of {#v-applies-to-conforming-sets} has neither a comparison nor a value to read."#.into(),
        }),
        Element::Prop(Prop {
            slug: "v-applies-to-conforming-sets".into(),
            parent: Some("cut-at-valuation".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$V$ applies wherever Het has produced a conforming set.

| Het produces | HetOpt orders it by | yielding |
|---|---|---|
| the qualifying judges for a sentence | cost tier, then $\varepsilon$ | the **minimal-judge rule** |
| the qualifying authors for an operation | cost tier | the **minimal-author rule** |
| the conforming algebras of a theory | the declared worth-law | ranked candidates |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "valuation-instantiated-twice".into(),
            parent: Some("cut-at-valuation".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"One piece of machinery, two levels — {#fractal-property} applied to valuation.
Judge selection and candidate ranking are not two features but one:
*conformance, then valuation*, instantiated twice.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "filter-then-optimize".into(),
            parent: Some("het-settles-hetopt-orders".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The cut lands here because the order is filter first, then
optimize. Non-identity is enforced at the model-category level as an
admissibility restriction on Kleisli arrows ({#admissibility-subcategories}); the minimal-judge
rule optimizes only among arrows that have already survived that filter.
Het is the filter; HetOpt is the optimization.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "cut-lands-no-later".into(),
            parent: Some("filter-then-optimize".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"It lands no later: non-identity cannot move to HetOpt ({#non-identity-not-deferrable}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "cut-lands-no-earlier".into(),
            parent: Some("filter-then-optimize".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"It lands no earlier: Het has no $V$ anywhere, HetOpt has $V$
everywhere. Keeping a valuation in Het for judges while withholding one
for candidates would leave *"why judges and not candidates?"* with no
answer beyond stipulation.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "hetopt-is-a-theory-extension".into(),
            parent: Some("het-settles-hetopt-orders".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"HetOpt is a theory extension in the ordinary sense.
$\mathbf{Sign}_{\textbf{HetOpt}}$ extends $\mathbf{Sign}_{\textbf{Het}}$
with the declaration of $V$, and
$\textbf{Het} \hookrightarrow \textbf{HetOpt}$ carries Het-algebras into
the HetOpt fiber by re-indexing.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "enrichment-base-is-the-metric".into(),
            parent: Some("hetopt-is-a-theory-extension".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"In HetOpt the enrichment base $V$ **is** the metric $d$, and the
fibers become $V$-enriched. In Het the verdict space carries $d$ alone.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 9 · Composition

"#.into()),
        Element::Prop(Prop {
            slug: "composition-is-closed".into(),
            parent: None,
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"When two theories are combined, their principal pools combine, and
the composite is again a judgmental institution.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "composite-monad".into(),
            parent: Some("composition-is-closed".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"$\mathcal{P}_{1+2} = \mathcal{P}_1 + \mathcal{P}_2$, provenance
preserved componentwise.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "non-identity-extends-to-composite".into(),
            parent: Some("composite-monad".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The non-identity restriction extends to the composite Kleisli
category.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "composite-qualifying-set".into(),
            parent: Some("composite-monad".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The qualifying set of the composite is the union of the
component qualifying sets, each still filtered by non-identity.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "composite-kinds".into(),
            parent: Some("composition-is-closed".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Kinds form the disjoint union $K_1 \sqcup K_2$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "adequacy-composes".into(),
            parent: Some("composition-is-closed".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The composite qualifying set is non-empty whenever either
component's was. Adequacy composes.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "theory-combination-closed".into(),
            parent: Some("composition-is-closed".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Theory combination is closed.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 10 · Evaluation

"#.into()),
        Element::Prop(Prop {
            slug: "models-defined-by-dispatch".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"$\models$ is defined by dispatch on the gate marker:

$$
M \models \varphi \;=\;
\begin{cases}
\mathsf{check}(M, \varphi) & \varphi\text{ decidable} \\[2pt]
\mathsf{dispatch}(\varphi, a, \mathcal{P}_{\text{judg}}) & \varphi\text{ judgmental} \\[2pt]
\mathsf{dispatch}(\varphi, a, \mathcal{P}_{\text{auth}}) & \varphi\text{ authorial} \\[2pt]
M \models_{\Sigma^\uparrow} \mathsf{Decidable}_\Sigma(\varphi) \;?\; \mathsf{check} : \mathsf{dispatch} & \varphi\text{ conditional}
\end{cases}
$$

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "run-over-every-sentence".into(),
            parent: Some("models-defined-by-dispatch".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$\models$ is run over every $\varphi \in \mathsf{Sen}(\Sigma)$
against $M$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "dispatch-is-two-operations".into(),
            parent: Some("models-defined-by-dispatch".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/gate_law.rs::competence_is_filtered_before_provenance_matters".into() },
            numbering: None,
            prose: r#"Dispatch is two operations, and the first is decidable:

$$\text{qualifying} = \{\, p \in \mathcal{P} : \mathsf{capable}(p, \mathsf{role}(\varphi)) \wedge \pi(p) \cap \pi(a) = \emptyset \,\}$$

$$\mathsf{dispatch} = \text{any member of } \text{qualifying}$$

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "dispatch-argument-is-the-argument".into(),
            parent: Some("dispatch-is-two-operations".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"$a$ is **the argument the operation is applied to** ({#disjointness-against-argument}) —
the subject at `audit`, the Proposal at `dispose`. Reading $\pi(M)$ in its
place is the error {#argument-governs} excludes.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "conformance-half-needs-no-judge".into(),
            parent: Some("dispatch-is-two-operations".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Both conjuncts read only the four predicates of
{#supplier-interface}. The conformance half requires no judge to
test: it is set operations over declared predicates.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "any-is-specified-argmin-is-the-seam".into(),
            parent: Some("dispatch-is-two-operations".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Returning *any* qualifying judge is not a decision deferred; it
is what Het specifies ({#no-preference-among-judges}). The minimal-judge rule replaces
*any* with *argmin*, and that substitution is the seam where HetOpt lands
({#v-applies-to-conforming-sets}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 11 · The surface

"#.into()),
        Element::Prop(Prop {
            slug: "theory-declares-four-things".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A theory written *in* Het declares four things and nothing else: its
sorts, its edits, its sentences with their gates, and a role for each
judgmental sentence.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "het-declares-the-slots".into(),
            parent: Some("theory-declares-four-things".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Het declares the **slots**. The theory fills them. This is the
division that runs through the whole document: Het says what must be
declared and under what condition it is settled; it never says what the
content is.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "role-declared-not-enumerated".into(),
            parent: Some("het-declares-the-slots".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Het requires that a judgmental sentence declare a role ({#role-not-kind}). It
does not enumerate roles. `taxonomist`, `triager`, `chord-reader` are the
theory's.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "edit-required-not-typed".into(),
            parent: Some("het-declares-the-slots".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Het requires that a `remedy` carry an **edit** ({#remedy-carries-an-edit}) and that
`enact` apply one ({#enact-makes-an-endofunctor}). It does not enumerate edits. Whether the domain's
edits are `amend | remove | relocate`, or `fix | won't-fix | duplicate |
reprioritize`, is the theory's.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "verdict-space-required-not-fixed".into(),
            parent: Some("het-declares-the-slots".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Het requires a verdict space carrying a metric ({#judges-are-stochastic}). It does not
say what the space is.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "interface-required-not-populated".into(),
            parent: Some("het-declares-the-slots".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Het requires that a supplier of $\mathcal{P}$ expose four
predicates ({#supplier-interface}). It does not say what a principal is made of ({#interface-by-signature-inspection}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "enact-generic-over-edit".into(),
            parent: Some("theory-declares-four-things".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Consequently `enact` is **generic over the theory's edit type**. Het
cannot apply an edit it did not name. The theory supplies the application;
Het governs only who may perform it ({#one-pool-two-filters}) and whether the result is admitted
({#target-runs-its-own-models}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "governs-who-not-what".into(),
            parent: Some("enact-generic-over-edit".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"This is not a limitation worked around. Het governs *who may act,
and under what condition*. What the act **is** belongs to the domain, and a
formalism that enumerated edits would be legislating domains it does not
know.

### The decidable fragment

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "decidable-is-a-total-predicate".into(),
            parent: Some("theory-declares-four-things".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A decidable sentence is **any total predicate of the host language
on the model**. Het names no logical fragment.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "two-signatures-not-two-fragments".into(),
            parent: Some("decidable-is-a-total-predicate".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen".into() },
            numbering: None,
            prose: r#"The two gates are not two fragments. They are two **signatures**,
and the host language's type system separates them:

| gate | the sentence's form |
|---|---|
| `decidable` | $M \to \mathsf{Bool}$ — the model alone |
| `judgmental` | $M \times \mathsf{Qualified}\langle \mathsf{role}(\varphi) \rangle \to \mathsf{Verdict}$ |

"#.into(),
            mechanism: r#"`ladder!` gate markers, now three signatures rather than two. Unmarked emits `fn t(prev)`; `#[judgmental(R)]` emits `fn t(prev, q: Qualified<R>)`; `#[authorial(R)]` emits `fn t(prev, pen: Authorized<'_, R>)`. The gates differ in the ARITY and the TYPE of the emitted transition, so a pen cannot be passed where a licence is asked for or the reverse, and the host's type system separates all three with no knowledge of Het."#.into(),
        }),
        Element::Prop(Prop {
            slug: "decidable-cannot-consult-pool".into(),
            parent: Some("decidable-is-a-total-predicate".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::a_qualified_token_cannot_be_constructed_outside_the_pool".into() },
            numbering: None,
            prose: r#"A decidable sentence therefore *cannot* consult $\mathcal{P}$: no
parameter admits a principal, and the qualifying token has no constructor
outside {#judgmental-qualifying-set}. The prohibition is not a rule
the author is asked to respect; it is a term that cannot be written.

"#.into(),
            mechanism: r#"G2. The qualifying token has no constructor reachable from a decidable body, so the prohibition is a term that cannot be written rather than a rule an author is asked to respect. An unmarked transition has no parameter a token could enter through, and `Qualified` is sealed: constructing one outside `Pool::qualify` is E0451."#.into(),
        }),
        Element::Prop(Prop {
            slug: "mismarking-is-not-a-false-claim".into(),
            parent: Some("decidable-is-a-total-predicate".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::calling_a_judgmental_transition_without_a_token_is_e0061".into() },
            numbering: None,
            prose: r#"Mis-marking is likewise not a claim that could be false. Marking a
sentence `decidable` gives it the decidable signature. A body needing an
outside will not typecheck in that position.

"#.into(),
            mechanism: r#"rustc. Marking a transition judgmental gives it the judgmental signature; calling it as though it were decidable is E0061, not a promise someone broke."#.into(),
        }),
        Element::Prop(Prop {
            slug: "signature-replaces-fragment-membership".into(),
            parent: Some("decidable-is-a-total-predicate".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::calling_a_judgmental_transition_without_a_token_is_e0061".into() },
            numbering: None,
            prose: r#"This replaces fragment-membership as the mechanism of
gate-honesty. A chosen fragment is a constraint someone must check; a
signature is checked by the host language's compiler, which does not know
Het exists and cannot be persuaded.

"#.into(),
            mechanism: r#"rustc. The refusal is an arity error from a compiler that does not know Het exists and cannot be persuaded — which is the whole claim of this proposition."#.into(),
        }),
        Element::Prop(Prop {
            slug: "two-properties-not-secured".into(),
            parent: Some("theory-declares-four-things".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Two properties the signature does **not** secure. Both are stated
as limits.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "termination-not-secured".into(),
            parent: Some("two-properties-not-secured".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**Termination.** A host language admitting non-termination admits
a `decidable` sentence that does not terminate. Het does not check this. The
type proves the sentence was *evaluated as* a machine check, not that the
check *halts*.

"#.into(),
            mechanism: r#"Matches SPEC §5 exactly — 'liveness beyond the guard'. Het and rung state the same limit independently."#.into(),
        }),
        Element::Prop(Prop {
            slug: "purity-not-secured".into(),
            parent: Some("two-properties-not-secured".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**Purity.** The decidable signature excludes $\mathcal{P}$. It does
not exclude the world: a predicate on the model may still reach a network, a
clock, a file. "Consults no outside" is exact about **Het's** outside — the
principal pool — and silent about every other.

"#.into(),
            mechanism: r#"rung has no effect system; a decidable body may still reach the world. Het already states this as a limit rather than a guarantee."#.into(),
        }),
        Element::Prop(Prop {
            slug: "neither-limit-closed-here".into(),
            parent: Some("two-properties-not-secured".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Neither is closed here. Closing {#termination-not-secured} requires a total language;
closing {#purity-not-secured} requires an effect system. Het requires neither, and a Het
built on a host that supplies them inherits the guarantee for free.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## Vocabulary

Terms not listed here are not part of the formalism. An encoding that
introduces one has drifted.

**The `in rung` column is the dictionary** between this document's vocabulary
and the host's. Plain = exists today. *Italic* = agreed, not yet built. `—` =
no surface counterpart, because the term is mathematics of the institution and
nothing in a host answers to it.

### The institution

| term | symbol | in rung | meaning | prop |
|---|---|---|---|---|
| **signature** | $\Sigma$ | `ladder!` decl | a theory declaration: sorts, operation symbols with arities, gate markers, and the laws the theory declares | [1.3](#signature-declares) |
| **signature category** | $\mathbf{Sign}$ | — | the category of signatures; objects are theories, morphisms are signature morphisms | [1.11](#sign-category) |
| **sentence** | $\varphi$ | *`theory!` sentence* | an element of $\mathsf{Sen}(\Sigma)$; a claim over the signature, carrying a gate marker | [1.12](#sen-functor), [2](#gate-marker-required) |
| **sentence functor** | $\mathsf{Sen}$ | — | $\mathbf{Sign} \to \mathbf{Set}$ | [1.12](#sen-functor) |
| **algebra**, **model** | $M$ | — | an interpretation of a signature; here a functor $T \to \mathbf{Kl}(\mathcal{P})$ | [1.13](#mod-functor), [5](#algebra-is-kleisli-functor) |
| **model functor** | $\mathsf{Mod}$ | — | $\mathbf{Sign}^{\text{op}} \to \mathbf{Cat}$ | [1.13](#mod-functor) |
| **satisfaction relation** | $\models$ | — | the mechanism testing an algebra against a sentence; the locus of the entire extension | [1](#one-relation), [1.31](#extension-is-in-models) |
| **satisfaction condition** | | — | truth is invariant under change of notation; the institution's only axiom | [1.2](#satisfaction-condition), [4.3](#satisfaction-condition-relaxed) |
| **signature morphism** | $\sigma$ | — | a structure-preserving map of signatures; translates sentences forward and algebras backward | [1.11](#sign-category), [6.24](#declaration-is-not-a-morphism) |
| **re-indexing** | $\mathsf{Mod}(\sigma)$ | — | transport of algebras along a signature morphism | [6.1](#tower-is-a-fibration) |
| **sort** | $S$ | rung payload type | a type declared by the signature, interpreted as a carrier $M(S)$ | [1.3](#signature-declares) |
| **subject** | $x : M(S)$ | payload | an inhabitant of a carrier — a specific datum under judgment | [5.6](#subject-defined) |
| **conformance declaration** | | — | the up-pointing edge on a model: "this population interprets that law" | [6.2](#two-kinds-of-pointing), [6.22](#declaration-on-models-only) |

### Judgment

| term | symbol | in rung | meaning | prop |
|---|---|---|---|---|
| **gate marker** | | *`#[…]` on a rung* | the annotation fixing a sentence's or operation's satisfaction mechanism | [2](#gate-marker-required) |
| **decidable** | | *unmarked transition* | satisfaction is machine-checked by standard equational logic | [2.1](#four-gates) |
| **judgmental** | | *`#[judgmental(R)]`* | satisfaction dispatches to a judge; the verdict *is* the outcome | [2.1](#four-gates) |
| **authorial** | | *`#[authorial]`* | the operation transforms rather than classifies; dispatches to an author | [2.1](#four-gates), [3.6](#authorial-qualifying-set) |
| **conditional** | | *`#[conditional(φ)]`* | decidability depends on the algebra; classified one level up | [2.1](#four-gates), [2.5](#conditional-names-classifier) |
| **competence role** | $\mathsf{Role}$ | `Role` | what a judgmental sentence needs done; declared pointwise by the sentence | [2.3](#judgmental-declares-role) |
| **principal pool** | $\mathcal{P}$ | `Pool` (supplied) | the pool dispatched to by non-decidable gates. **A parameter of $\models$, never a sort** | [3](#pool-is-parameter) |
| **judge** | | `Qualified<R>` | a principal filtered by capability and non-identity; renders a verdict | [3.5](#judgmental-qualifying-set) |
| **author** | | `Authorized` | a principal filtered by capability and standing; enacts a ruling | [3.6](#authorial-qualifying-set) |
| **standing** | | — | an author holds stewardship of what it enacts on. Conditional-gated | [3.6](#authorial-qualifying-set), [3.63](#standing-conditional-gated) |
| **non-identity** | | — | a judge must not be the author of what it judges. Decidable; enforced before dispatch | [3.5](#judgmental-qualifying-set), [3.53](#non-identity-before-dispatch) |
| **belonging predicate** | | — | a predicate deciding whether a principal qualifies at all: capability, non-identity, standing | [3.3](#three-belonging-predicates) |
| **qualifying set** | | — | the principals surviving the gate's belonging predicates. Het's output | [3.5](#judgmental-qualifying-set), [3.6](#authorial-qualifying-set), [10.2](#dispatch-is-two-operations) |
| **kind** | $K_i$ | — | a partition of $\mathcal{P}$ by substrate. The supplier's, not Het's | [3.23](#nothing-further-required), [9.2](#composite-kinds) |
| **cost tier** | | — | ordering on principals by resource consumption. **HetOpt** | [3.31](#ordering-is-hetopts), [8.22](#v-applies-to-conforming-sets) |
| **minimal-judge rule** | | — | select the cheapest qualifying judge, breaking ties by lowest $\varepsilon$. **HetOpt** | [8.22](#v-applies-to-conforming-sets) |
| **minimal-author rule** | | — | select the cheapest principal with standing, escalating when it cannot close. **HetOpt** | [3.66](#two-escalation-triggers), [8.22](#v-applies-to-conforming-sets) |
| **renaming-robustness** | $\varepsilon$ | — | tolerated verdict drift under signature morphisms. Reported in Het; a criterion in HetOpt | [3.32](#epsilon-declared-not-ranked), [4.6](#epsilon-reported-with-verdict) |
| **adequacy** | | — | that *a* qualifying non-identical judge exists and returns a verdict. Judgmental, discharged where invoked | [6.5](#adequacy-defined) |
| **gate law** | | — | gate markers may be preserved or increased along morphisms, never laundered downward | [6.3](#gate-law) |

### Semantics

| term | symbol | in rung | meaning | prop |
|---|---|---|---|---|
| **Kleisli category** | $\mathbf{Kl}(\mathcal{P})$ | — | where algebras land; judgmental and authorial operations are Kleisli arrows, decidable ones factor through $\eta$ | [5](#algebra-is-kleisli-functor) |
| **admissibility sub-categories** | $\mathbf{Kl}_{\text{judg}}$, $\mathbf{Kl}_{\text{auth}}$ | — | gate-selected restrictions: provenance-disjoint versus containment-plus-standing | [5.41](#admissibility-subcategories) |
| **provenance** | $\pi_X$ | `Provenanced` | a map to provenance tags, carried by every object; strict under $\eta$ and $\mu$ | [5.3](#provenance-structure), [5.32](#monad-is-provenance-strict) |
| **gate-faithful** | | *emitted signature* | an algebra whose decidable operations are pure, judgmental ones judgmentally-admissible, authorial ones authorially-admissible | [5.5](#gate-faithful) |
| **fibration** | | — | the Grothendieck construction over the category of theories | [6.1](#tower-is-a-fibration) |
| **fractal property** | | — | an algebra carrying its own signature declaration becomes a theory at the next level | [6](#fractal-property) |
| **well-formedness predicate** | $W$ | — | the decidable shape-check on signatures on which the tower terminates | [6.4](#tower-floor) |

### Verdicts, worth, and the two formalisms

| term | symbol | in rung | meaning | prop |
|---|---|---|---|---|
| **verdict** | | `Verdict` | a judge's answer; the satisfaction outcome for a judgmental sentence | [2.1](#four-gates) |
| **verdict space** | | — | the space verdicts inhabit — $[0,1]$, a simplex $\Delta^n$, a strategy lattice | [4](#verdict-space-with-metric), [4.2](#typical-verdict-spaces) |
| **metric** | $d$ | — | distance on the verdict space. **Measures** drift; symmetric | [4](#verdict-space-with-metric), [4.5](#metric-measures-not-ranks) |
| **worth-law**, **valuation** | $V$ | — | a quantale whose order **ranks** a conforming set. **HetOpt only** | [8](#het-settles-hetopt-orders), [8.22](#v-applies-to-conforming-sets) |
| **belonging**, **conformance** | $\chi$ | — | the belonging predicate: what a candidate must satisfy to be a conforming algebra | [8](#het-settles-hetopt-orders) |
| **Het** | | — | judgmental institution + gate-marked $\models$ + metric verdict space. Settles belonging | [8](#het-settles-hetopt-orders) |
| **HetOpt** | | — | Het + $V$. Orders what belongs — qualifying judges and conforming candidates alike | [8](#het-settles-hetopt-orders) |

### The game

| term | in rung | meaning | prop |
|---|---|---|---|
| **Proponent** | — | the candidate algebra, asserting $M \models \varphi$ | [7.1](#proponent-and-opponent) |
| **Opponent** | — | the environment; may query the judge as oracle | [7.1](#proponent-and-opponent) |
| **winning strategy** | — | what satisfaction amounts to: the Proponent has one | [7](#satisfaction-is-a-game) |
| **audit** | transition | a violation is found; produces a Verdict | [7.2](#the-pass) |
| **propose** | transition | the Proponent answers; authorial; produces a Proposal | [7.2](#the-pass), [7.21](#propose-is-authorial) |
| **dispose** | branching transition | the Opponent rules; judgmental; produces a Disposition | [7.2](#the-pass) |
| **enact** | transition | the Proponent applies a terminal-and-affirming Disposition; produces the revised subject | [7.2](#the-pass), [7.5](#enact-makes-an-endofunctor) |
| **panel** | — | $\models$ with more than one judge; the game with an enlarged oracle-move set | [7.6](#panels) |

---

## 12 · The limit

"#.into()),
        Element::Prop(Prop {
            slug: "no-bound-on-reentry".into(),
            parent: None,
            kind: Kind::Decidable { proof: "rung-het/tests/pass_ladder.rs::reject_remedy_re_enters_with_no_progress_guard".into() },
            numbering: None,
            prose: r#"Het places **no bound on re-entry**.

"#.into(),
            mechanism: r#"A continue arm loops with no host-imposed bound, and the pass now runs that loop: the cited test drives five identical rounds — the same edit answered by the same reason — and nothing panics, nothing evicts, and the subject is still in the loop at attempt six. Choosing a guarded edge instead would supply a bound Het declines to declare ({#guarded-reentry-is-eviction}); so would giving up quietly after three tries, which is the mutation that proves the test can fail — `assert!(chain.attempt() <= 3)` in the re-entry arm reddens it on the fourth round. `acceptance.rs::het_places_no_bound_on_re_entry` additionally pins `Disposition::REENTRY_BOUND` as `None`. Either answer would be a worth-law smuggled in under another name ({#cut-at-valuation})."#.into(),
        }),
        Element::Prop(Prop {
            slug: "reentry-never-terminates".into(),
            parent: Some("no-bound-on-reentry".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"If no acceptable remedy exists, `reject-remedy` re-enters
forever ({#disposition-vocabulary}) and the subject never leaves the loop.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "answers-are-worth-shaped".into(),
            parent: Some("no-bound-on-reentry".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Het cannot close this. The available answers — evict the subject,
bound the attempts, or accept non-conformance as declared debt — are all
worth-shaped, and {#het-declares-no-worth-law} forbids a Het theory from declaring a worth-law.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "bound-belongs-to-hetopt".into(),
            parent: Some("no-bound-on-reentry".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"This is the one state that belonging alone produces and cannot
exit. The bound belongs to HetOpt.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "stated-as-limit-not-closed".into(),
            parent: Some("no-bound-on-reentry".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"It is stated as a limit rather than closed by an eviction rule,
which would be a worth-law under another name.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "guarded-reentry-is-eviction".into(),
            parent: Some("no-bound-on-reentry".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn".into() },
            numbering: None,
            prose: r#"A host that injects a termination guard on re-entry has declared
the bound Het declines to declare. Re-entry is an **unguarded** return to
the authoring position; a guarded one is an eviction rule under another
name.
"#.into(),
            mechanism: r#"G10 continue arms — 'no recover function, no guard, no source'. Re-entry must be `RejectRemedy -> Proposing`, never `RejectRemedy => Proposing`: the recoverable-verdict form injects G8's `must_progress`, which panics on no progress and is therefore an eviction rule ({#answers-are-worth-shaped}). CONSTRAINT: a continue arm's target rung is built inline by `dispose`, i.e. by the judge, so that rung's payload must be classification-only ({#no-amending-disposition}). The resume edge is the SECOND unguarded re-entry and is pinned separately by {#resumption-is-unguarded}: injecting `must_progress` there is type-valid and reddens the double-resume test on the first round."#.into(),
        }),
        ],
    }
}
