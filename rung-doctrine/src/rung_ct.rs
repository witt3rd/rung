//! The categorical account, encoded.
//!
//! **Generated once** from `docs/rung-ct-props.md` by `docs/_migrate.py`, and the
//! source of truth from then on. The markdown is rendered from this; where the
//! two disagree, this is right and the markdown is stale.
//!
//! Every proposition arrives as [`Kind::Rationale`], which is not a claim that
//! they are all arguments — it is the absence of a claim. Markdown does not
//! record what kind a proposition is, so the migration does not invent one. The
//! triage into signature, decidable and judgmental is a reading, done
//! deliberately, and it is the work this encoding exists to make possible.

use crate::{Doctrine, Element, Kind, Prop};

/// The categorical account of what a `ladder` declaration is.
pub fn doctrine() -> Doctrine {
    Doctrine {
        file: "rung-ct-props.md".into(),
        elements: vec![
        Element::Verbatim(r#"# rung-CT — The category rung declares

**Status: normative.** This document states what a `ladder` declaration *is*.
It records no history, cites no artifact, and names no reviewer. Every claim is
stated once, in one place, and referred to elsewhere by number.

**One exception, deliberate.** The appendix is a record of claims this
account once made and has since withdrawn. It is not part of the numbered
tree and states nothing this document asserts — it exists because a
superseded claim that leaves no trace comes back.

The numbering is a tree. A proposition `n.m` is a remark on `n`; `n.mm` is a
remark on `n.m`. Interior propositions are the conjunction of their children.
Leaves are single checkable claims.

**This document is generated.** Its source is
`rung-doctrine/src/rung_ct.rs`, and it is written by
`cargo run -p rung-doctrine --bin render`. Editing it here does not change what
it says; the next render restores this text. Where the two differ, the encoding
is right and this file is stale — CI checks exactly that.

**Numbers are derived, not authored.** A proposition's identity is its slug;
its place in the tree is its declared parent; its order is declaration order.
The decimal number and every reference to it are computed at render time and
appear nowhere in the source, so inserting, removing or reparenting a
proposition cannot break a reference and cannot leave a number stale — there is
no number to leave.

**Two documents, one slug space.** A reference whose target names
`rung-het-props.md` points into Het's formalism. Where a claim here
touches one there, it links rather than restates.

**G-numbers.** `G1`–`G14` name the guarantees of the ladder specification. This
document says what each guarantee *means* categorically; it never restates what
the guarantee requires. The specification is the normative statement of the
guarantee; this document is the normative statement of the category.

---

## 1 · The category

"#.into()),
        Element::Prop(Prop {
            slug: "ladder-declares-a-category".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A `ladder` declaration is a **presentation of a free category**. It
declares the objects and the generating morphisms; the category is everything
those generate and nothing else.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "rungs-are-objects".into(),
            parent: Some("ladder-declares-a-category".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A **rung is an object**. An object is inert — data at rest, a point. It
has no verbs.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "transitions-are-morphisms".into(),
            parent: Some("ladder-declares-a-category".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A **declared transition is a generating morphism**. Every *doing* lives
on an arrow.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "the-law".into(),
            parent: Some("ladder-declares-a-category".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"**The law: a verb can live only on a morphism, never inside an object.**
Compute, judge, call an outside, touch the world — each is a verb, and each
belongs in a transition body, never in the construction of a state.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "verb-in-object-position-refused".into(),
            parent: Some("the-law".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Constructing a state *to hold the result of an action that has not
been performed* asks for a morphism in object-position. No such thing exists in
a category, and the request is refused.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "sealing-is-the-axiom-not-a-guard".into(),
            parent: Some("the-law".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624".into() },
            numbering: None,
            prose: r#"Sealed construction (`G2`) is therefore not merely a fabrication
guard. It is the enforcement of what a category *is*: a state is reached only by
traversing an arrow.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "law-is-the-second-axis-of-one-refusal".into(),
            parent: Some("the-law".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The law is one axis of a refusal Het states on another: an algebra runs
its own decidable step and cannot construct the state that holds a judgmental
outcome ({#self-governing-not-self-closing}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "category-is-freely-generated".into(),
            parent: Some("ladder-declares-a-category".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The category is **freely generated** by the declared arrows. Between two
objects there is no morphism but the composites of declared arrows; a path
that skips a rung does not exist to be taken.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "freeness-enforced-only-with-bodies".into(),
            parent: Some("category-is-freely-generated".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**The limit.** Enforcement of freeness exists only where the
declaration supplies transition bodies. Bodies expand inside the seal boundary,
so every constructor but the entry's can be made module-private. A **type-only
declaration publishes every constructor**, and is freely generated by convention
only — external code can mint any object of the category directly, and no
diagnostic fires.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "entry-constructor-is-public".into(),
            parent: Some("freeness-enforced-only-with-bodies".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The entry object's constructor is public even with bodies present. This
is not a breach: it is the morphism that starts a run, and a free category with
no way in presents nothing.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "module-boundary-is-the-limit".into(),
            parent: Some("freeness-enforced-only-with-bodies".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Sealing is a module boundary and inherits that boundary's reach. Code
inside the generated module can mint any object; freeness is enforced against
the outside, not against the module's own contents.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "well-typed-program-is-a-functor".into(),
            parent: Some("ladder-declares-a-category".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A well-typed program is a **functor** from the declared category into
the host's types — each object to a type, each generating morphism to a
function — and the host's type checker enforces that it respects composition.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "composition-consumes".into(),
            parent: Some("ladder-declares-a-category".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::using_a_rung_after_a_transition_consumed_it_is_e0382".into() },
            numbering: None,
            prose: r#"Composition is **linear** (`G1`): the intermediate object is consumed.
Composition is not sequencing; it is resource consumption.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "intermediate-survives-only-as-a-record".into(),
            parent: Some("composition-consumes".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"After composition the intermediate object no longer exists as a value.
It survives only as an entry in an accumulated trace ({#trace-is-a-writer-monad}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 2 · Branching is a coproduct

"#.into()),
        Element::Prop(Prop {
            slug: "branching-is-a-coproduct".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The outcome of a branching transition is a **coproduct**. Its injections
construct; its universal property is elimination by exhaustive case analysis.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "shape-of-the-branching-transition".into(),
            parent: Some("branching-is-a-coproduct".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A branching transition out of object $A$ has the shape

$$A \longrightarrow \textstyle\sum_i B_i \;+\; A$$

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "injections-point-into-the-coproduct".into(),
            parent: Some("shape-of-the-branching-transition".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"Each summand **injects into** the coproduct. Elimination is the unique
morphism *out*, determined by one morphism per injection. The two directions are
not interchangeable: injections point in, elimination points out.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "coproduct-is-heterogeneous".into(),
            parent: Some("branching-is-a-coproduct".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The coproduct is **heterogeneous**. Its summands are not all of one
kind, and the distinction is structural, not cosmetic.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "verdict-summand".into(),
            parent: Some("coproduct-is-heterogeneous".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A **verdict summand** carries a verdict object — an object that is not
a rung and has no outgoing generating morphism. It is where a run stops, or
where it hands off to a backward edge.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "continue-summand-carries-an-object".into(),
            parent: Some("coproduct-is-heterogeneous".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A **continue summand** carries an object *of the category itself* — a
live rung, not a verdict. Nothing has left the category.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "residual-summand".into(),
            parent: Some("coproduct-is-heterogeneous".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::a_judgmental_forward_transition_returns_the_argument_unconsumed".into() },
            numbering: None,
            prose: r#"The **residual summand** is the final `+ A`: the input object returned
unconsumed when the transition does not answer. It is the same residual Het's
judgmental arrow carries
({#judgmental-arrow-shape}).

"#.into(),
            mechanism: r#"The `+ A` is emitted. A judgmental forward transition returns `Result<Next, Suspended<Prev>>` and the `Suspended` carries the INPUT OBJECT unconsumed, which is what this proposition says the summand is — the cited test reads the very argument back out of it. This row was `out-of-scope` while the residual existed only as `Failed`'s error string, which carries no object the caller handed in and no identity for what went unanswered. Emitting `#to` instead of the `Result` is type-valid at the macro and turns the cited test red at its `fn`-pointer coercion."#.into(),
        }),
        Element::Prop(Prop {
            slug: "elimination-is-exhaustive".into(),
            parent: Some("branching-is-a-coproduct".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::a_match_missing_a_step_outcome_summand_is_e0004".into() },
            numbering: None,
            prose: r#"Elimination is **exhaustive** (`G6`): a morphism out of the coproduct
must be defined on every injection. Exhaustiveness is not a lint; it is the
universal property.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "adding-a-summand-breaks-every-eliminator".into(),
            parent: Some("elimination-is-exhaustive".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Adding a summand therefore breaks every elimination site at compile
time. That breakage is the compile-time gate, and it is the whole reason a
vocabulary can be *closed*.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "closed-vocabularies-rest-on-this".into(),
            parent: Some("elimination-is-exhaustive".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Every "exactly $n$" claim of a declared vocabulary — Het's four gates
({#four-gates}), its two Proposals
({#proposal-vocabulary}), its five Dispositions
({#disposition-vocabulary}) — is enforceable
exactly because it is a coproduct, and enforced by
{#adding-a-summand-breaks-every-eliminator}.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "continue-arm-is-an-ordinary-generating-morphism".into(),
            parent: Some("branching-is-a-coproduct".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A **continue arm** is an ordinary generating morphism whose *selection*
is deferred to the coproduct. The morphism into its target rung is taken on the
forward pass; which summand was taken is what the eliminator learns.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "continue-arm-needs-no-backward-edge".into(),
            parent: Some("continue-arm-is-an-ordinary-generating-morphism".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A continue arm needs **no backward edge**. It never leaves the
category, so there is nothing to return from — and no round trip for a
contraction to shrink ({#the-dagger-is-partial-and-contractive}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "continue-arm-has-no-verdict-object".into(),
            parent: Some("continue-arm-is-an-ordinary-generating-morphism".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn".into() },
            numbering: None,
            prose: r#"A continue arm therefore emits **no verdict object** and no adjoint
(`G10`). A declaration that demanded one for a continue arm would be demanding
an inverse for a morphism that was never inverted.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "two-arms-two-readings".into(),
            parent: Some("continue-arm-is-an-ordinary-generating-morphism".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A recoverable verdict and a continue arm to the same target are
different constructions, not two spellings of one. The first leaves the
category and returns under a guard; the second never leaves.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 3 · Carry is a product factor

"#.into()),
        Element::Prop(Prop {
            slug: "carry-is-a-product-factor".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"Where a ladder declares a `carry`, every object is a **product** of a
payload factor and the carry factor.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "projection-onto-carry".into(),
            parent: Some("carry-is-a-product-factor".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_carry_accessor_exists".into() },
            numbering: None,
            prose: r#"The second projection $\pi_2$ is the only access to the carry, and it is
read-only (`G5`). No morphism of the category mutates it in place.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "carry-factor-is-unrestricted".into(),
            parent: Some("carry-is-a-product-factor".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The two factors obey different disciplines. The payload factor is
consumed by the arrow that acts on it; the carry factor is unrestricted. This is
a cartesian product sitting inside an otherwise affine category.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "carry-is-copied-per-object".into(),
            parent: Some("carry-is-a-product-factor".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The carry is **copied into each object by value**. Every object holds
its own carry field; the successor's carry is written when the successor is
built. It is not structurally shared, and no object holds a reference to
another's.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "copying-is-what-makes-it-cartesian".into(),
            parent: Some("carry-is-copied-per-object".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The copy *is* the cartesian diagonal $\Delta : C \to C \times C$. A
factor that could not be duplicated would not be cartesian, and duplication is
exactly what {#carry-factor-is-unrestricted} licenses.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "carry-is-a-comonadic-context".into(),
            parent: Some("carry-is-a-product-factor".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The carry reads as a **graded comonadic context**: the grade is the
object's position in the ladder, and the counit is $\pi_2$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "constancy-is-not-enforced".into(),
            parent: Some("carry-is-a-comonadic-context".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**The limit.** Coassociativity — that the carry is the *same* at every
object — is not enforced. Immutability is per-object; the arrow that builds the
successor supplies the successor's carry and may supply a different value. The
comonadic reading is available to a ladder whose bodies preserve the carry, and
is not a guarantee of the construction.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 4 · The ladder is an indexed monad

"#.into()),
        Element::Prop(Prop {
            slug: "ladder-is-an-indexed-monad".into(),
            parent: None,
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The ladder as a whole is an **indexed monad**. A value of $M\,i\,j\,A$ is a
computation that starts at object $i$, ends at object $j$, and yields $A$.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "index-alignment-is-composition".into(),
            parent: Some("ladder-is-an-indexed-monad".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"`bind` composes $M\,i\,j\,A$ with $A \to M\,j\,k\,B$ only when the inner
index matches. Index alignment is the free category's composition, read as a
monad.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "unrepresentable-paths".into(),
            parent: Some("ladder-is-an-indexed-monad".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A path not generated by the declared arrows has no inhabitant. It is not
forbidden by a check; it is **unrepresentable** — the consequence of
{#category-is-freely-generated}.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "monad-laws-hold-by-construction".into(),
            parent: Some("ladder-is-an-indexed-monad".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The monad laws hold by construction. They require no separate proof
because the only compositions that exist are those the declaration generates.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "indexed-monad-is-a-reading".into(),
            parent: Some("ladder-is-an-indexed-monad".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"No $M$ is emitted. The index discipline is carried entirely by the
ordinary types of the generated functions; the indexed monad is a *reading* of
those types, not an artifact alongside them.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 5 · The trace is a writer monad

"#.into()),
        Element::Prop(Prop {
            slug: "trace-is-a-writer-monad".into(),
            parent: None,
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"An accumulated record of which arrows ran is a **trace**, and it is the
output of a **writer monad**.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "trace-is-a-free-monoid".into(),
            parent: Some("trace-is-a-writer-monad".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"A trace is the **free monoid** on trace entries: an empty trace and
concatenation, associative, with the empty trace as unit. Nothing about a trace
depends on what an entry says.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "graded-writer".into(),
            parent: Some("trace-is-a-writer-monad".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Composed with {#ladder-is-an-indexed-monad}, the full type is a
**graded writer**: the grade tracks the pair of objects, the writer accumulates
the trace.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "trace-is-the-proof-term".into(),
            parent: Some("trace-is-a-writer-monad".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The trace is the **proof term** of
{#types-are-propositions} — the explicit sequence of inference steps that a
reached object asserts abstractly.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "trace-is-not-emitted".into(),
            parent: Some("trace-is-a-writer-monad".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**The limit.** No trace is emitted by the construction. The writer
correspondence describes the structure a trace has *wherever a caller
accumulates one*; it is not a claim that one is accumulated.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "trace-is-not-authorship-provenance".into(),
            parent: Some("trace-is-a-writer-monad".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A trace is **not provenance**. A trace records which arrows ran.
Provenance is a map to authorship tags
({#provenance-structure}), and it is disjointness
of *those* that decides whether a judge may rule
({#judgmental-qualifying-set}). The two are
unrelated structures, and neither substitutes for the other.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 6 · A transition is a Prism

"#.into()),
        Element::Prop(Prop {
            slug: "transition-is-a-prism".into(),
            parent: None,
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"A branching transition is a **Prism** — a dependent optic — presented as a
forward/backward pair.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "match-is-the-forward-pass".into(),
            parent: Some("transition-is-a-prism".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The forward pass (*match*) is exactly
{#shape-of-the-branching-transition}.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "build-is-the-backward-pass".into(),
            parent: Some("transition-is-a-prism".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The backward pass (*build*) is the declared backward edges
({#the-dagger-is-partial-and-contractive}). The recover edges *are* the
Prism's build pass.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "residual-is-the-optics-residual".into(),
            parent: Some("transition-is-a-prism".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The residual summand ({#residual-summand}) is the optic's
residual. Returning the input on failure is not an oddity of the encoding; it is
what the shape requires.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "not-a-monad".into(),
            parent: Some("transition-is-a-prism".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The structure is **not a monad**. Compose $f : A \to B + A$ with
$g : B \to C + B$: a failing $g$ hands back $B$, while the composite's domain is
$A$. No `bind` routes $B \to A$; only an explicit backward edge can.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "effects-layer-on-the-forward-pass".into(),
            parent: Some("transition-is-a-prism".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Effects layer on the **forward pass**, never on the backward one. An
effectful transition is a strong monad $T$ on match:

$$A \longrightarrow T\Big(\textstyle\sum_i B_i \;+\; A\Big)$$

This is the same shape Het gives a judgmental operation
({#algebra-is-kleisli-functor}), with $T$ the
principal monad.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "strength-carries-linearity".into(),
            parent: Some("effects-layer-on-the-forward-pass".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Tensorial strength $A \otimes T(B) \to T(A \otimes B)$ is what proves
linearity survives suspension. A monad without strength cannot carry a linear
token across a suspension point.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "error-and-effect-are-orthogonal".into(),
            parent: Some("effects-layer-on-the-forward-pass".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Error and effect are **orthogonal gadgets**: error is the optic's
backward pass, effect is a monad on its forward pass. They require no
distributive law, and neither subsumes the other.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "generative-body-is-a-kernel".into(),
            parent: Some("effects-layer-on-the-forward-pass".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"A generative transition body is a **Markov kernel** — an affine
probability monad on the forward pass. It is an instance of
{#effects-layer-on-the-forward-pass}, not a further construction.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 7 · The dagger is partial and contractive

"#.into()),
        Element::Prop(Prop {
            slug: "the-dagger-is-partial-and-contractive".into(),
            parent: None,
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The category's dagger is **partial**: an adjoint $f^\dagger$ exists only
where a backward edge declares one.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "three-shapes-of-loop-back".into(),
            parent: Some("the-dagger-is-partial-and-contractive".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::recover_guard_is_auto_injected".into() },
            numbering: None,
            prose: r#"There are **three shapes of loop-back, and only two are daggers.**

| shape | leaves the category? | adjoint | guarded |
|---|---|---|---|
| recoverable verdict | yes — into a verdict object | required (`G7`) | yes (`G8`) |
| residual | yes — into the failure summand | optional (`G9`) | no |
| continue arm | **no** | none (`G10`) | n/a |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "only-a-departure-can-be-a-return".into(),
            parent: Some("three-shapes-of-loop-back".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A continue arm is not a dagger because it never departs
({#continue-arm-needs-no-backward-edge}). An adjoint is a return, and
nothing that stayed can return.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "verdict-dagger-is-mandatory".into(),
            parent: Some("the-dagger-is-partial-and-contractive".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::a_recoverable_verdict_without_a_recover_edge_is_refused".into() },
            numbering: None,
            prose: r#"The dagger is **total on recoverable verdicts** (`G7`): each has an
adjoint and each adjoint has a verdict. The pairing is checked, not trusted.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "verdict-dagger-is-contractive".into(),
            parent: Some("the-dagger-is-partial-and-contractive".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::recover_guard_is_auto_injected".into() },
            numbering: None,
            prose: r#"The verdict dagger is **contractive, not involutive**. The round trip
forward-then-back is required to *decrease* (`G8`), so
$f^\dagger \circ f \neq \mathrm{id}$ by construction.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "contraction-is-on-the-payload".into(),
            parent: Some("verdict-dagger-is-contractive".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The contraction is measured **on the payload factor**, not on the
object. The guard compares the payload the verdict carried in with the payload
the recovered object carries out. Objects are never compared: each is a distinct
sealed value, so an inequality on objects would be vacuous and an equality
impossible.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "well-foundedness-over-symmetry".into(),
            parent: Some("verdict-dagger-is-contractive".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"This trades the dagger's **symmetry** for **well-foundedness**. A
recovery that may return the value it received is a stall loop with a type; a
recovery required to decrease terminates.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "contraction-is-a-runtime-guard".into(),
            parent: Some("verdict-dagger-is-contractive".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**The limit.** The contraction is a runtime guard. It aborts on a
non-decreasing step; it does not prove decrease, and it does not prove general
forward progress.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "error-dagger-is-optional-and-unguarded".into(),
            parent: Some("the-dagger-is-partial-and-contractive".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::recovers_from_the_failed_error_path".into() },
            numbering: None,
            prose: r#"The residual's adjoint is **optional and unguarded** (`G9`). Optional,
because a caller may handle a returned residual itself; unguarded, because a
re-entry after an unanswered call may legitimately reuse the argument — the same
licence Het grants a returned residual
({#adequacy-failure-returns-residual}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "resume-edge-is-the-residual-dagger".into(),
            parent: Some("error-dagger-is-optional-and-unguarded".into()),
            kind: Kind::Decidable { proof: "rung/tests/suspension.rs::a_suspension_resumes_through_the_authorial_edge".into() },
            numbering: None,
            prose: r#"A **resume edge** is that adjoint, declared. It consumes the residual
of a judgmental forward transition — the argument returned unconsumed together
with the reference to what the outside raised
({#suspension-is-the-residual}) — and returns the
object to the position that produced it.

It inherits both halves of {#error-dagger-is-optional-and-unguarded}
unchanged. **Optional:** a caller may hold the residual and never resume, and a
suspended run that no one revives is a blocked arrow, not a broken one.
**Unguarded:** no contraction is required, because a raised matter may take any
number of rounds and a guard would be an eviction rule
({#resumption-is-unguarded}).

What it does *not* inherit is freedom of who may take it. The verdict and
residual daggers are morphisms of the same category as the forward pass; this
one **writes back into** the subject, so it dispatches through the authorial
filter ({#resumption-is-authorial}). That is a
condition on the *principal*, not on the arrow, and so does not disturb
{#three-shapes-of-loop-back}: there are still three shapes of loop-back,
and this is the second one with its adjoint written down.

"#.into(),
            mechanism: r#"[G16](rung-props.md#g16-the-residual-channel). The residual's adjoint, declared: `resume { revive: #[authorial(R)] Suspended(Rung) => Rung }`. It inherits both halves of {#error-dagger-is-optional-and-unguarded} — OPTIONAL, because a driver may hold a `Suspended` and never resume, and UNGUARDED, which `suspension.rs::the_same_suspension_resumes_twice_with_no_progress_guard` pins by resuming an unchanged payload twice. What it does not inherit is freedom of WHO may take it, and that is a condition on the principal rather than on the arrow, so {#three-shapes-of-loop-back} is undisturbed: still three shapes, with the second one's adjoint now written down."#.into(),
        }),
        Element::Prop(Prop {
            slug: "terminal-verdicts-have-no-adjoint".into(),
            parent: Some("the-dagger-is-partial-and-contractive".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A terminal verdict has **no adjoint**. It is an absorbing object, and
declaring a backward edge from one is rejected.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "dagger-laws-are-not-verified".into(),
            parent: Some("the-dagger-is-partial-and-contractive".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**The limit.** Neither $f^{\dagger\dagger} = f$ nor
$(g \circ f)^\dagger = f^\dagger \circ g^\dagger$ is verified. For the verdict
dagger the involution is not merely unverified but **deliberately broken** by
{#verdict-dagger-is-contractive}.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 8 · The substrate is affine

"#.into()),
        Element::Prop(Prop {
            slug: "substrate-is-affine".into(),
            parent: None,
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"The host substrate implements **affine logic** — linear logic without the
requirement to use exactly once — and that is what the category's linearity
rests on.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "linear-logic-dictionary".into(),
            parent: Some("substrate-is-affine".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The correspondence:

| linear logic | substrate |
|---|---|
| resource $A$ | a value of type $A$ |
| $A \multimap B$ | a function taking and returning ownership |
| $!A$ | a shared reference, or a freely copyable type |
| $A \otimes B$ | a product |
| $A \oplus B$ | a coproduct ({#branching-is-a-coproduct}) |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "at-most-once-not-exactly-once".into(),
            parent: Some("substrate-is-affine".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The substrate gives **at most once**, not exactly once. An object cannot
be used twice; it *can* be dropped.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "must-use-is-the-affine-approximation".into(),
            parent: Some("at-most-once-not-exactly-once".into()),
            kind: Kind::Decidable { proof: "rung/tests/spec_refusals.rs::dropping_a_verdict_under_deny_must_use_is_an_error".into() },
            numbering: None,
            prose: r#"`G4` closes the second half by lint rather than by type, and it covers
every carrier of a live token — objects, verdict objects, the branching
outcome, and the residual — not the objects alone.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "lint-is-escapable".into(),
            parent: Some("at-most-once-not-exactly-once".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**The limit.** A lint is escapable: a token may be leaked, bound to a
discard, or buried in a dropped container. The close is partial.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "one-token-one-thread".into(),
            parent: Some("substrate-is-affine".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync".into() },
            numbering: None,
            prose: r#"Linearity is a claim about a **unique consumer**, and a shared reference
crossing a thread boundary would supply a second. `G3` forbids it: no object of
the category may be shared or sent, so one token cannot be driven by two
threads.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "move-semantics-alone-are-insufficient".into(),
            parent: Some("one-token-one-thread".into()),
            kind: Kind::Decidable { proof: "rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync".into() },
            numbering: None,
            prose: r#"Move semantics alone give one consumer for an *owned* value and say
nothing about shared references. `G3` is what makes the linearity claim hold of
the whole object, not merely of its owned form.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "true-linearity-needs-the-language".into(),
            parent: Some("substrate-is-affine".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"**The limit.** Exactly-once cannot be closed above the language. A
linear substrate would make it exact; an affine one admits the approximation
{#must-use-is-the-affine-approximation} states.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 9 · Types are propositions

"#.into()),
        Element::Prop(Prop {
            slug: "types-are-propositions".into(),
            parent: None,
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Under Curry–Howard an object is a **proposition** and a path through the
category is a **proof** of it.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "object-asserts-its-history".into(),
            parent: Some("types-are-propositions".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"Holding an object asserts that every arrow on some path into it ran. The
assertion is discharged by the type, not by an accompanying check.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "residual-is-a-conjunction".into(),
            parent: Some("types-are-propositions".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The residual is a **conjunction**: *the step did not answer* **and**
*the argument is preserved*. Both conjuncts are carried by the one object.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "terminal-payload-is-the-witness".into(),
            parent: Some("types-are-propositions".into()),
            kind: Kind::Decidable { proof: "rung/tests/end_to_end.rs::drives_to_convergence".into() },
            numbering: None,
            prose: r#"A terminal verdict may carry a payload (`G11`). That payload is the
**witness** the terminal proposition asserts the existence of, returned through
the verdict rather than around it.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "proof-is-of-traversal-not-correctness".into(),
            parent: Some("types-are-propositions".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The proof is of **traversal, not correctness**. A path proves the arrows
ran in a legal order. It says nothing about whether any body computed the right
thing.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 10 · The verification boundary

"#.into()),
        Element::Prop(Prop {
            slug: "verification-boundary".into(),
            parent: None,
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The construction verifies the **shape of the category**. It never
verifies the **content of a morphism**.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "guarantees-carry-categorical-content".into(),
            parent: Some("verification-boundary".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Each guarantee has a categorical reading, and the reading is what this
document adds to it:

| guarantee | what it means categorically |
|---|---|
| `G1` | composition consumes the intermediate object ({#composition-consumes}) |
| `G2` | the category is freely generated ({#category-is-freely-generated}), within {#freeness-enforced-only-with-bodies} |
| `G3` | one token has one consumer; sharing would supply a second ({#one-token-one-thread}) |
| `G4` | the affine approximation of exactly-once ({#must-use-is-the-affine-approximation}) |
| `G5` | the carry projection is read-only ({#projection-onto-carry}) |
| `G6` | the coproduct's universal property ({#elimination-is-exhaustive}) |
| `G7` | the dagger is total on recoverable verdicts ({#verdict-dagger-is-mandatory}) |
| `G8` | the backward pass is contractive, on payloads ({#contraction-is-on-the-payload}) |
| `G9` | the residual's optional, unguarded adjoint ({#error-dagger-is-optional-and-unguarded}) |
| `G10` | an object summand of the coproduct ({#continue-arm-is-an-ordinary-generating-morphism}) |
| `G11` | the witness a terminal proposition carries ({#terminal-payload-is-the-witness}) |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "what-is-not-verified".into(),
            parent: Some("verification-boundary".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"What is **not** verified, and why:

| property | why not |
|---|---|
| functoriality of the program | a claim about what a body does, not about its type ({#proof-is-of-traversal-not-correctness}) |
| the dagger laws | no equational reasoning in the host — and involution is deliberately broken ({#dagger-laws-are-not-verified}) |
| the monad laws | nothing to verify; they hold by construction ({#monad-laws-hold-by-construction}) |
| carry coassociativity | not a property of the construction ({#constancy-is-not-enforced}) |
| the writer's trace | nothing is emitted to verify ({#trace-is-not-emitted}) |
| freeness of a type-only declaration | no bodies, so no seal to enforce it ({#freeness-enforced-only-with-bodies}) |
| recovery well-foundedness | a runtime guard, not a proof ({#contraction-is-a-runtime-guard}) |
| exactly-once consumption | a lint, and escapable ({#lint-is-escapable}) |
| gate admissibility of a marked arrow | the guarantees constrain the domain; the sub-category condition is on the codomain ({#gate-guarantees-constrain-the-domain-not-the-arrow}) |

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "gate-guarantees-constrain-the-domain-not-the-arrow".into(),
            parent: Some("what-is-not-verified".into()),
            kind: Kind::Decidable { proof: "rung/tests/gate_markers.rs::judgmental_transition_takes_a_qualified_token".into() },
            numbering: None,
            prose: r#"The gate guarantees are the sharpest instance of this boundary, and
the reason they have no row in
{#guarantees-carry-categorical-content}. `G12`–`G14` of the specification
put a capability token in a marked transition's **domain** and refuse it unless
the filter minted it against this very subject — $\mathsf{Qualified}$ for the
judgmental marker, $\mathsf{Authorized}$ for the authorial one. That is a
constraint on *who may traverse the arrow*.

Het's admissibility sub-categories
({#admissibility-subcategories}) are defined
instead by a condition on $f(a)$ — what the arrow **returns** — and the two
gates take opposite conditions there: disjointness from the argument for
$\mathbf{Kl}_{\text{judg}}$, provenance containment plus standing for
$\mathbf{Kl}_{\text{auth}}$. Both restrict the *same* Kleisli category
({#one-monad}), so this is one boundary crossed
twice and not two separate gaps.

Membership of either sub-category is therefore a property of the body, which is
{#proof-is-of-traversal-not-correctness} exactly. Guarantees on the domain
do not compose into a guarantee on the codomain, and adding a third such
guarantee did not change that — which is the whole content of
[Q11](../questions/q11-gate-faithfulness.md)'s answer.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "boundary-is-typestate-not-verification".into(),
            parent: Some("verification-boundary".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The boundary between the two tables is the boundary between
**typestate** and **formal verification**. Every row of
{#what-is-not-verified} is on the far side of it, and none is closed by
adding a check to the construction.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 11 · The dependency structure is an opfibration

"#.into()),
        Element::Prop(Prop {
            slug: "dependency-structure-is-an-opfibration".into(),
            parent: None,
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"**Governed subjects** and the declared dependency morphisms between them
form a **free base category**; subjects-in-states over it form a **Grothendieck
opfibration** $p : E \to B$. A governed subject is one that carries its own
signature, and so is a theory at the next level
({#fractal-property}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "fibre-is-a-ladder".into(),
            parent: Some("dependency-structure-is-an-opfibration".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The **fibre over a subject is exactly that subject's ladder** — the
category of {#ladder-declares-a-category}. Each object of the base contains a
category; that is the fractal property read in this setting.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "declaration-names-no-foreign-object".into(),
            parent: Some("fibre-is-a-ladder".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"**A declaration names no object of another fibre.** An edge whose
domain lies in a different fibre is not a morphism of *this* free category — it
is a morphism of the total space $E$, crossing fibres. It is a different
categorical object, and a declaration that admitted one would no longer present
a free category ({#category-is-freely-generated}). Cross-fibre structure
enters a declaration only through composition operators, never as a declared
arrow.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "typed-edge-is-an-opcartesian-lift".into(),
            parent: Some("dependency-structure-is-an-opfibration".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"A typed edge is an **opcartesian lift**. A state change at the source
transports *forward* along the edge by a pushforward functor between fibres.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "orientation-is-load-bearing".into(),
            parent: Some("typed-edge-is-an-opcartesian-lift".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The orientation is load-bearing. Transport runs with the direction of
information flow, which is what makes the structure an **op**fibration rather
than a fibration; reversing it changes which lifts exist.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "edge-type-selects-the-pushforward".into(),
            parent: Some("typed-edge-is-an-opcartesian-lift".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The **edge type selects the pushforward**. A strict edge lifts to an
obligation; an advisory edge lifts to a coproduct; a generative edge lifts to
the dependent's existence; a mechanical edge lifts to a state update with no
outside.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "edge-taxonomy-is-the-theorys".into(),
            parent: Some("edge-type-selects-the-pushforward".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/questions_of_rung.rs::every_declared_edge_kind_has_a_lived_instance_on_disk".into() },
            numbering: None,
            prose: r#"**The taxonomy is the governing theory's**, declared by it exactly as
an edit vocabulary is ({#edit-required-not-typed}).
This document states that an edge type selects a pushforward; it never
enumerates the types, for the reason
{#governs-who-not-what} gives.

"#.into(),
            mechanism: r#"The edge vocabulary is declared by the governing theory, not by the formalism — neither `rung` nor `rung-het` has an edge type, and `EdgeKind` lives in `rung-std::questions`, the theory that governs bodies of questions, exactly where an edit vocabulary sits (`edit-required-not-typed`). Moving the theory out of a test and into a library sharpened the row without changing its verdict: the taxonomy is now demonstrably neither the formalism's NOR one carrier's, because two carriers with disjoint id spaces and disjoint edge sets fill the same seven kinds — rung's `questions/` and a synthetic decision docket. What the cited test pins is the **lived-instance** discipline, now a decidable sentence of the theory (`every_declared_kind_is_lived`) rather than prose: a kind stays in the vocabulary only while some question in the set under audit uses it, and deleting the sentence turns a test red in BOTH carriers. STILL NOT enforced, and the reason is unchanged: what would have to fail is a crate BELOW the theory naming an edge type, and no test can fail for code that was never written. The location is a choice this theory makes; the test protects the discipline, not the choice."#.into(),
        }),
        Element::Prop(Prop {
            slug: "strict-and-advisory-are-the-gate".into(),
            parent: Some("edge-type-selects-the-pushforward".into()),
            kind: Kind::Decidable { proof: "rung-het/tests/questions_of_rung.rs::a_strict_edge_propagates_decidably_and_an_advisory_edge_is_ruled_on".into() },
            numbering: None,
            prose: r#"The load-bearing split among edge types — obligatory against advisory,
*this breaks* against *check whether this breaks* — is the **gate marker**
({#four-gates}) at this level. A strict edge
propagates decidably; an advisory edge requires a ruling. It is not a second
taxonomy laid over the gates; it is the gates, read one level up.

"#.into(),
            mechanism: r#"G12 + G2, read at the dependency level. `premise` routes to a `decidable` sentence whose `holds` takes only the model — there is no parameter a pool could enter through — and `justification` routes to a `judgmental` one whose `settle` consumes a `Qualified<Adjudicator>` that only `Pool::qualify_for` mints. The two lifts therefore differ in ARITY, not in convention, and the cited test runs both over the one real cascade (Q7's resolution) that forced typed edges. Reclassifying `justification` as strict is type-valid and turns the test red twice — at the declared gate, and again at the `Propagated::Ruled` match, where the advisory edge is found to have consulted nobody. That mutation is what establishes the row."#.into(),
        }),
        Element::Prop(Prop {
            slug: "advisory-lift-lands-in-a-coproduct".into(),
            parent: Some("dependency-structure-is-an-opfibration".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"An **advisory lift does not break functoriality.** It does not land in
the target's objects; it lands in a **coproduct** — *review required* plus
*survives* — which the base delivers rigidly. Collapsing that coproduct is the
target's own work, done by its own transitions.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "same-coproduct-at-both-levels".into(),
            parent: Some("advisory-lift-lands-in-a-coproduct".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"This is the same coproduct as
{#branching-is-a-coproduct}, one level up. The structure that makes a
branching transition honest is the structure that keeps the dependency level
functorial; the two levels share machinery rather than resemble each other.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "vertical-morphisms-preserve-agency".into(),
            parent: Some("advisory-lift-lands-in-a-coproduct".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Because every fibre is a free category, the target retains its own
**vertical** morphisms. Lifts evaluate against the target's *current* state, so
functoriality holds over the total space rather than in spite of it.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "edges-are-dependent-optics".into(),
            parent: Some("dependency-structure-is-an-opfibration".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"A dependency edge is a **dependent optic**: covariant forward transport
of a state change, contravariant backward query of exposure. The backward pass's
type depends on the state transported forward, which is what makes it dependent.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "exposure-is-the-backward-pass".into(),
            parent: Some("edges-are-dependent-optics".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"**Blast radius is the backward pass, not a count.** Querying backward
along the composite optic returns a *typed exposure* — how many obligations of
which kind — and a count of reachable subjects is its Boolean shadow.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "opfibrations-compose".into(),
            parent: Some("dependency-structure-is-an-opfibration".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Opfibrations **compose**. A map from governed subjects to the theories
that govern them is itself an opfibration, and its composite with $p$ is a
single opfibration over the whole tower.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "iteration-not-a-second-level".into(),
            parent: Some("opfibrations-compose".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Composing is an **iteration of one level, not the arrival of a second**.
Opfibrations are 1-cells and compose as such. A genuine second level needs a
2-cell *between* fibrations — a remapping of the structure itself — which
nesting does not supply.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "transport-is-scale-invariant".into(),
            parent: Some("opfibrations-compose".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Because optics compose, obligation-transport is **scale-invariant**. A
traversal of the backward pass need not know whether an edge crosses a domain
boundary; the same pass runs at every scale.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "horizontal-and-vertical-coincide".into(),
            parent: Some("opfibrations-compose".into()),
            kind: Kind::Judgmental { role: "category-theorist".into(), ruling: None },
            numbering: None,
            prose: r#"Under the Grothendieck construction the hierarchy flattens: a
sibling-to-sibling edge and a domain-to-parent edge are both generating
morphisms of one composite base and lift identically. That, precisely, is the
content of *"the structure is fractal."*

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "conformance-and-propagation-run-over-different-bases".into(),
            parent: Some("dependency-structure-is-an-opfibration".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Propagation along this opfibration is **not** the inverse of
conformance. Conformance runs from a model to its theory and re-indexes
contravariantly; propagation runs from a revised subject to its dependents and
transports covariantly. They are adjacent levels over different bases, not two
orientations of one tower
({#two-directions-two-bases}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 12 · The mathematics is the implementation, not the surface

"#.into()),
        Element::Prop(Prop {
            slug: "mathematics-is-the-implementation-not-the-surface".into(),
            parent: None,
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The surface syntax names **objects and transitions**. It does not name
free categories, coproducts, indexed monads, daggers, or optics, and it must
not.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "surface-is-the-programmers-model".into(),
            parent: Some("mathematics-is-the-implementation-not-the-surface".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A declaration is written in the vocabulary of the domain being
modelled. The mathematics of this document is what the *construction* is
obliged to, not what the author is obliged to write.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "same-move-as-the-substrate".into(),
            parent: Some("mathematics-is-the-implementation-not-the-surface".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"This is the substrate's own move, one level up: an author writes
ordinary bindings and the checker enforces the affine discipline
({#substrate-is-affine}). Here an author writes rungs and transitions, and
the construction enforces the category.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "hiding-is-not-optional".into(),
            parent: Some("mathematics-is-the-implementation-not-the-surface".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The hiding is **not a convenience**. A surface that required the
mathematics would make the mathematics the language, and the enforcement would
then rest on the author restating it correctly — which is the failure the
construction exists to remove.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "correspondence-is-falsifiable".into(),
            parent: Some("mathematics-is-the-implementation-not-the-surface".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The correspondence is **falsifiable in the construction, not in the
prose**. Every claim here either names a guarantee that a conformance test
protects, or is marked a limit. A claim that is neither has no standing.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## Appendix · Withdrawn claims

*Not a proposition. No slug, no number, no place in the tree.* These are claims
earlier revisions of this account made and which do not survive into it. Each
was checked against `rung-macro/src/lib.rs`, `rung-props.md`, or the test suite. A
withdrawn claim that leaves no trace comes back, so it is recorded here rather
than deleted.

| withdrawn claim | what is true | evidence |
|---|---|---|
| "the compiler refuses **any** out-of-category construction" | only with an inline `impl` block; a type-only declaration publishes every constructor | `rung-macro/src/lib.rs` — `ctor_vis` |
| two example ladders, `Designed/Claimed/Active` and `Spec/Active` | one running example; the earlier two were mutually incompatible **and both ungrammatical** — the verdict block is mandatory | `rung-props.md` §1 grammar |
| a forward transition is named `design: Designed → Claimed` | named after its **target**, lowercased — so `claimed`, or `active`. The branching transition is always `step` | `rung-props.md` §1; `rung-macro/src/lib.rs` |
| the carry is "structurally shared (duplicated by reference)" | copied **by value** into each object's own `carry` field | `rung-macro/src/lib.rs` — `carry: Carry` field + ctor init |
| the carry satisfies comonad coassociativity | not enforced — a body supplies the successor's carry and may change it, and the running example's `iterate` decrements `budget` | `rung/tests/end_to_end.rs` |
| `f† ∘ f ≠ id` (on tokens) | the guard compares **payloads**, never tokens; a token comparison would be vacuous | `rung-macro/src/lib.rs` — `must_progress(&__before, &__after.payload)` |
| G4 covers "every rung and verdict type" | also `StepOutcome` and `Failed` | `rung-props.md` G4; `rung-macro/src/lib.rs` |
| the coproduct diagram's injection arrows | injections point **into** the coproduct; elimination is the unique morphism out. The ASCII diagram had them reversed, and the diagram is dropped rather than redrawn | — |
| the verification table omitted G3, G10, G11 | all three are in [10.1](#guarantees-carry-categorical-content); G3 has a real categorical reading — one token cannot be driven by two threads | `rung-props.md` G3/G10/G11 |
| "resolves on the reviews" stated as a claim inside the theory | an epistemic-status claim about a document, not a claim about the category; it has no place in a normative account and is dropped | — |
"#.into()),
        ],
    }
}
