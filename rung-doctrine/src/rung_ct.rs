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
        file: "rung-ct-props.md",
        elements: vec![
            Element::Verbatim(
                r#"# rung-CT — The category rung declares

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

**Numbers are derived, not authored.** A proposition's identity is the slug in
the anchor above it; its place in the tree is `data-parent`; its order is
document order. The decimal number and every reference to it are generated from
those three. Inserting, removing, or reparenting a proposition therefore cannot
break a reference — run `./_props.py fmt` and the numbering follows.
`./_props.py check` fails on a duplicate slug, a dangling parent or reference, a
proposition out of order with its parent, or a number that has gone stale.

**Two documents, one slug space.** A reference whose target names
`rung-het-props.md` points into Het's formalism. Where a claim here
touches one there, it links rather than restates.

**G-numbers.** `G1`–`G14` name the guarantees of the ladder specification. This
document says what each guarantee *means* categorically; it never restates what
the guarantee requires. The specification is the normative statement of the
guarantee; this document is the normative statement of the category.

---

## 1 · The category

"#,
            ),
            Element::Prop(Prop {
                slug: "ladder-declares-a-category",
                parent: None,
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A `ladder` declaration is a **presentation of a free category**. It
declares the objects and the generating morphisms; the category is everything
those generate and nothing else.

"#,
            }),
            Element::Prop(Prop {
                slug: "rungs-are-objects",
                parent: Some("ladder-declares-a-category"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A **rung is an object**. An object is inert — data at rest, a point. It
has no verbs.

"#,
            }),
            Element::Prop(Prop {
                slug: "transitions-are-morphisms",
                parent: Some("ladder-declares-a-category"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A **declared transition is a generating morphism**. Every *doing* lives
on an arrow.

"#,
            }),
            Element::Prop(Prop {
                slug: "the-law",
                parent: Some("ladder-declares-a-category"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"**The law: a verb can live only on a morphism, never inside an object.**
Compute, judge, call an outside, touch the world — each is a verb, and each
belongs in a transition body, never in the construction of a state.

"#,
            }),
            Element::Prop(Prop {
                slug: "verb-in-object-position-refused",
                parent: Some("the-law"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"Constructing a state *to hold the result of an action that has not
been performed* asks for a morphism in object-position. No such thing exists in
a category, and the request is refused.

"#,
            }),
            Element::Prop(Prop {
                slug: "sealing-is-the-axiom-not-a-guard",
                parent: Some("the-law"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"Sealed construction (`G2`) is therefore not merely a fabrication
guard. It is the enforcement of what a category *is*: a state is reached only by
traversing an arrow.

"#,
            }),
            Element::Prop(Prop {
                slug: "law-is-the-second-axis-of-one-refusal",
                parent: Some("the-law"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"The law is one axis of a refusal Het states on another: an algebra runs
its own decidable step and cannot construct the state that holds a judgmental
outcome ({#self-governing-not-self-closing}).

"#,
            }),
            Element::Prop(Prop {
                slug: "category-is-freely-generated",
                parent: Some("ladder-declares-a-category"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The category is **freely generated** by the declared arrows. Between two
objects there is no morphism but the composites of declared arrows; a path
that skips a rung does not exist to be taken.

"#,
            }),
            Element::Prop(Prop {
                slug: "freeness-enforced-only-with-bodies",
                parent: Some("category-is-freely-generated"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"**The limit.** Enforcement of freeness exists only where the
declaration supplies transition bodies. Bodies expand inside the seal boundary,
so every constructor but the entry's can be made module-private. A **type-only
declaration publishes every constructor**, and is freely generated by convention
only — external code can mint any object of the category directly, and no
diagnostic fires.

"#,
            }),
            Element::Prop(Prop {
                slug: "entry-constructor-is-public",
                parent: Some("freeness-enforced-only-with-bodies"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The entry object's constructor is public even with bodies present. This
is not a breach: it is the morphism that starts a run, and a free category with
no way in presents nothing.

"#,
            }),
            Element::Prop(Prop {
                slug: "module-boundary-is-the-limit",
                parent: Some("freeness-enforced-only-with-bodies"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"Sealing is a module boundary and inherits that boundary's reach. Code
inside the generated module can mint any object; freeness is enforced against
the outside, not against the module's own contents.

"#,
            }),
            Element::Prop(Prop {
                slug: "well-typed-program-is-a-functor",
                parent: Some("ladder-declares-a-category"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A well-typed program is a **functor** from the declared category into
the host's types — each object to a type, each generating morphism to a
function — and the host's type checker enforces that it respects composition.

"#,
            }),
            Element::Prop(Prop {
                slug: "composition-consumes",
                parent: Some("ladder-declares-a-category"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"Composition is **linear** (`G1`): the intermediate object is consumed.
Composition is not sequencing; it is resource consumption.

"#,
            }),
            Element::Prop(Prop {
                slug: "intermediate-survives-only-as-a-record",
                parent: Some("composition-consumes"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"After composition the intermediate object no longer exists as a value.
It survives only as an entry in an accumulated trace ({#trace-is-a-writer-monad}).

"#,
            }),
            Element::Verbatim(
                r#"---

## 2 · Branching is a coproduct

"#,
            ),
            Element::Prop(Prop {
                slug: "branching-is-a-coproduct",
                parent: None,
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The outcome of a branching transition is a **coproduct**. Its injections
construct; its universal property is elimination by exhaustive case analysis.

"#,
            }),
            Element::Prop(Prop {
                slug: "shape-of-the-branching-transition",
                parent: Some("branching-is-a-coproduct"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A branching transition out of object $A$ has the shape

$$A \longrightarrow \textstyle\sum_i B_i \;+\; A$$

"#,
            }),
            Element::Prop(Prop {
                slug: "injections-point-into-the-coproduct",
                parent: Some("shape-of-the-branching-transition"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"Each summand **injects into** the coproduct. Elimination is the unique
morphism *out*, determined by one morphism per injection. The two directions are
not interchangeable: injections point in, elimination points out.

"#,
            }),
            Element::Prop(Prop {
                slug: "coproduct-is-heterogeneous",
                parent: Some("branching-is-a-coproduct"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The coproduct is **heterogeneous**. Its summands are not all of one
kind, and the distinction is structural, not cosmetic.

"#,
            }),
            Element::Prop(Prop {
                slug: "verdict-summand",
                parent: Some("coproduct-is-heterogeneous"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A **verdict summand** carries a verdict object — an object that is not
a rung and has no outgoing generating morphism. It is where a run stops, or
where it hands off to a backward edge.

"#,
            }),
            Element::Prop(Prop {
                slug: "continue-summand-carries-an-object",
                parent: Some("coproduct-is-heterogeneous"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A **continue summand** carries an object *of the category itself* — a
live rung, not a verdict. Nothing has left the category.

"#,
            }),
            Element::Prop(Prop {
                slug: "residual-summand",
                parent: Some("coproduct-is-heterogeneous"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The **residual summand** is the final `+ A`: the input object returned
unconsumed when the transition does not answer. It is the same residual Het's
judgmental arrow carries
({#judgmental-arrow-shape}).

"#,
            }),
            Element::Prop(Prop {
                slug: "elimination-is-exhaustive",
                parent: Some("branching-is-a-coproduct"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"Elimination is **exhaustive** (`G6`): a morphism out of the coproduct
must be defined on every injection. Exhaustiveness is not a lint; it is the
universal property.

"#,
            }),
            Element::Prop(Prop {
                slug: "adding-a-summand-breaks-every-eliminator",
                parent: Some("elimination-is-exhaustive"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"Adding a summand therefore breaks every elimination site at compile
time. That breakage is the compile-time gate, and it is the whole reason a
vocabulary can be *closed*.

"#,
            }),
            Element::Prop(Prop {
                slug: "closed-vocabularies-rest-on-this",
                parent: Some("elimination-is-exhaustive"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"Every "exactly $n$" claim of a declared vocabulary — Het's four gates
({#four-gates}), its two Proposals
({#proposal-vocabulary}), its five Dispositions
({#disposition-vocabulary}) — is enforceable
exactly because it is a coproduct, and enforced by
{#adding-a-summand-breaks-every-eliminator}.

"#,
            }),
            Element::Prop(Prop {
                slug: "continue-arm-is-an-ordinary-generating-morphism",
                parent: Some("branching-is-a-coproduct"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A **continue arm** is an ordinary generating morphism whose *selection*
is deferred to the coproduct. The morphism into its target rung is taken on the
forward pass; which summand was taken is what the eliminator learns.

"#,
            }),
            Element::Prop(Prop {
                slug: "continue-arm-needs-no-backward-edge",
                parent: Some("continue-arm-is-an-ordinary-generating-morphism"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"A continue arm needs **no backward edge**. It never leaves the
category, so there is nothing to return from — and no round trip for a
contraction to shrink ({#the-dagger-is-partial-and-contractive}).

"#,
            }),
            Element::Prop(Prop {
                slug: "continue-arm-has-no-verdict-object",
                parent: Some("continue-arm-is-an-ordinary-generating-morphism"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"A continue arm therefore emits **no verdict object** and no adjoint
(`G10`). A declaration that demanded one for a continue arm would be demanding
an inverse for a morphism that was never inverted.

"#,
            }),
            Element::Prop(Prop {
                slug: "two-arms-two-readings",
                parent: Some("continue-arm-is-an-ordinary-generating-morphism"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"A recoverable verdict and a continue arm to the same target are
different constructions, not two spellings of one. The first leaves the
category and returns under a guard; the second never leaves.

"#,
            }),
            Element::Verbatim(
                r#"---

## 3 · Carry is a product factor

"#,
            ),
            Element::Prop(Prop {
                slug: "carry-is-a-product-factor",
                parent: None,
                kind: Kind::Signature,
                numbering: None,
                prose: r#"Where a ladder declares a `carry`, every object is a **product** of a
payload factor and the carry factor.

"#,
            }),
            Element::Prop(Prop {
                slug: "projection-onto-carry",
                parent: Some("carry-is-a-product-factor"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The second projection $\pi_2$ is the only access to the carry, and it is
read-only (`G5`). No morphism of the category mutates it in place.

"#,
            }),
            Element::Prop(Prop {
                slug: "carry-factor-is-unrestricted",
                parent: Some("carry-is-a-product-factor"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The two factors obey different disciplines. The payload factor is
consumed by the arrow that acts on it; the carry factor is unrestricted. This is
a cartesian product sitting inside an otherwise affine category.

"#,
            }),
            Element::Prop(Prop {
                slug: "carry-is-copied-per-object",
                parent: Some("carry-is-a-product-factor"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The carry is **copied into each object by value**. Every object holds
its own carry field; the successor's carry is written when the successor is
built. It is not structurally shared, and no object holds a reference to
another's.

"#,
            }),
            Element::Prop(Prop {
                slug: "copying-is-what-makes-it-cartesian",
                parent: Some("carry-is-copied-per-object"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"The copy *is* the cartesian diagonal $\Delta : C \to C \times C$. A
factor that could not be duplicated would not be cartesian, and duplication is
exactly what {#carry-factor-is-unrestricted} licenses.

"#,
            }),
            Element::Prop(Prop {
                slug: "carry-is-a-comonadic-context",
                parent: Some("carry-is-a-product-factor"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"The carry reads as a **graded comonadic context**: the grade is the
object's position in the ladder, and the counit is $\pi_2$.

"#,
            }),
            Element::Prop(Prop {
                slug: "constancy-is-not-enforced",
                parent: Some("carry-is-a-comonadic-context"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"**The limit.** Coassociativity — that the carry is the *same* at every
object — is not enforced. Immutability is per-object; the arrow that builds the
successor supplies the successor's carry and may supply a different value. The
comonadic reading is available to a ladder whose bodies preserve the carry, and
is not a guarantee of the construction.

"#,
            }),
            Element::Verbatim(
                r#"---

## 4 · The ladder is an indexed monad

"#,
            ),
            Element::Prop(Prop {
                slug: "ladder-is-an-indexed-monad",
                parent: None,
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"The ladder as a whole is an **indexed monad**. A value of $M\,i\,j\,A$ is a
computation that starts at object $i$, ends at object $j$, and yields $A$.

"#,
            }),
            Element::Prop(Prop {
                slug: "index-alignment-is-composition",
                parent: Some("ladder-is-an-indexed-monad"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"`bind` composes $M\,i\,j\,A$ with $A \to M\,j\,k\,B$ only when the inner
index matches. Index alignment is the free category's composition, read as a
monad.

"#,
            }),
            Element::Prop(Prop {
                slug: "unrepresentable-paths",
                parent: Some("ladder-is-an-indexed-monad"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A path not generated by the declared arrows has no inhabitant. It is not
forbidden by a check; it is **unrepresentable** — the consequence of
{#category-is-freely-generated}.

"#,
            }),
            Element::Prop(Prop {
                slug: "monad-laws-hold-by-construction",
                parent: Some("ladder-is-an-indexed-monad"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"The monad laws hold by construction. They require no separate proof
because the only compositions that exist are those the declaration generates.

"#,
            }),
            Element::Prop(Prop {
                slug: "indexed-monad-is-a-reading",
                parent: Some("ladder-is-an-indexed-monad"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"No $M$ is emitted. The index discipline is carried entirely by the
ordinary types of the generated functions; the indexed monad is a *reading* of
those types, not an artifact alongside them.

"#,
            }),
            Element::Verbatim(
                r#"---

## 5 · The trace is a writer monad

"#,
            ),
            Element::Prop(Prop {
                slug: "trace-is-a-writer-monad",
                parent: None,
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"An accumulated record of which arrows ran is a **trace**, and it is the
output of a **writer monad**.

"#,
            }),
            Element::Prop(Prop {
                slug: "trace-is-a-free-monoid",
                parent: Some("trace-is-a-writer-monad"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"A trace is the **free monoid** on trace entries: an empty trace and
concatenation, associative, with the empty trace as unit. Nothing about a trace
depends on what an entry says.

"#,
            }),
            Element::Prop(Prop {
                slug: "graded-writer",
                parent: Some("trace-is-a-writer-monad"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"Composed with {#ladder-is-an-indexed-monad}, the full type is a
**graded writer**: the grade tracks the pair of objects, the writer accumulates
the trace.

"#,
            }),
            Element::Prop(Prop {
                slug: "trace-is-the-proof-term",
                parent: Some("trace-is-a-writer-monad"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"The trace is the **proof term** of
{#types-are-propositions} — the explicit sequence of inference steps that a
reached object asserts abstractly.

"#,
            }),
            Element::Prop(Prop {
                slug: "trace-is-not-emitted",
                parent: Some("trace-is-a-writer-monad"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"**The limit.** No trace is emitted by the construction. The writer
correspondence describes the structure a trace has *wherever a caller
accumulates one*; it is not a claim that one is accumulated.

"#,
            }),
            Element::Prop(Prop {
                slug: "trace-is-not-authorship-provenance",
                parent: Some("trace-is-a-writer-monad"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"A trace is **not provenance**. A trace records which arrows ran.
Provenance is a map to authorship tags
({#provenance-structure}), and it is disjointness
of *those* that decides whether a judge may rule
({#judgmental-qualifying-set}). The two are
unrelated structures, and neither substitutes for the other.

"#,
            }),
            Element::Verbatim(
                r#"---

## 6 · A transition is a Prism

"#,
            ),
            Element::Prop(Prop {
                slug: "transition-is-a-prism",
                parent: None,
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"A branching transition is a **Prism** — a dependent optic — presented as a
forward/backward pair.

"#,
            }),
            Element::Prop(Prop {
                slug: "match-is-the-forward-pass",
                parent: Some("transition-is-a-prism"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The forward pass (*match*) is exactly
{#shape-of-the-branching-transition}.

"#,
            }),
            Element::Prop(Prop {
                slug: "build-is-the-backward-pass",
                parent: Some("transition-is-a-prism"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The backward pass (*build*) is the declared backward edges
({#the-dagger-is-partial-and-contractive}). The recover edges *are* the
Prism's build pass.

"#,
            }),
            Element::Prop(Prop {
                slug: "residual-is-the-optics-residual",
                parent: Some("transition-is-a-prism"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The residual summand ({#residual-summand}) is the optic's
residual. Returning the input on failure is not an oddity of the encoding; it is
what the shape requires.

"#,
            }),
            Element::Prop(Prop {
                slug: "not-a-monad",
                parent: Some("transition-is-a-prism"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"The structure is **not a monad**. Compose $f : A \to B + A$ with
$g : B \to C + B$: a failing $g$ hands back $B$, while the composite's domain is
$A$. No `bind` routes $B \to A$; only an explicit backward edge can.

"#,
            }),
            Element::Prop(Prop {
                slug: "effects-layer-on-the-forward-pass",
                parent: Some("transition-is-a-prism"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"Effects layer on the **forward pass**, never on the backward one. An
effectful transition is a strong monad $T$ on match:

$$A \longrightarrow T\Big(\textstyle\sum_i B_i \;+\; A\Big)$$

This is the same shape Het gives a judgmental operation
({#algebra-is-kleisli-functor}), with $T$ the
principal monad.

"#,
            }),
            Element::Prop(Prop {
                slug: "strength-carries-linearity",
                parent: Some("effects-layer-on-the-forward-pass"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"Tensorial strength $A \otimes T(B) \to T(A \otimes B)$ is what proves
linearity survives suspension. A monad without strength cannot carry a linear
token across a suspension point.

"#,
            }),
            Element::Prop(Prop {
                slug: "error-and-effect-are-orthogonal",
                parent: Some("effects-layer-on-the-forward-pass"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"Error and effect are **orthogonal gadgets**: error is the optic's
backward pass, effect is a monad on its forward pass. They require no
distributive law, and neither subsumes the other.

"#,
            }),
            Element::Prop(Prop {
                slug: "generative-body-is-a-kernel",
                parent: Some("effects-layer-on-the-forward-pass"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"A generative transition body is a **Markov kernel** — an affine
probability monad on the forward pass. It is an instance of
{#effects-layer-on-the-forward-pass}, not a further construction.

"#,
            }),
            Element::Verbatim(
                r#"---

## 7 · The dagger is partial and contractive

"#,
            ),
            Element::Prop(Prop {
                slug: "the-dagger-is-partial-and-contractive",
                parent: None,
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"The category's dagger is **partial**: an adjoint $f^\dagger$ exists only
where a backward edge declares one.

"#,
            }),
            Element::Prop(Prop {
                slug: "three-shapes-of-loop-back",
                parent: Some("the-dagger-is-partial-and-contractive"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"There are **three shapes of loop-back, and only two are daggers.**

| shape | leaves the category? | adjoint | guarded |
|---|---|---|---|
| recoverable verdict | yes — into a verdict object | required (`G7`) | yes (`G8`) |
| residual | yes — into the failure summand | optional (`G9`) | no |
| continue arm | **no** | none (`G10`) | n/a |

"#,
            }),
            Element::Prop(Prop {
                slug: "only-a-departure-can-be-a-return",
                parent: Some("three-shapes-of-loop-back"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"A continue arm is not a dagger because it never departs
({#continue-arm-needs-no-backward-edge}). An adjoint is a return, and
nothing that stayed can return.

"#,
            }),
            Element::Prop(Prop {
                slug: "verdict-dagger-is-mandatory",
                parent: Some("the-dagger-is-partial-and-contractive"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The dagger is **total on recoverable verdicts** (`G7`): each has an
adjoint and each adjoint has a verdict. The pairing is checked, not trusted.

"#,
            }),
            Element::Prop(Prop {
                slug: "verdict-dagger-is-contractive",
                parent: Some("the-dagger-is-partial-and-contractive"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"The verdict dagger is **contractive, not involutive**. The round trip
forward-then-back is required to *decrease* (`G8`), so
$f^\dagger \circ f \neq \mathrm{id}$ by construction.

"#,
            }),
            Element::Prop(Prop {
                slug: "contraction-is-on-the-payload",
                parent: Some("verdict-dagger-is-contractive"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The contraction is measured **on the payload factor**, not on the
object. The guard compares the payload the verdict carried in with the payload
the recovered object carries out. Objects are never compared: each is a distinct
sealed value, so an inequality on objects would be vacuous and an equality
impossible.

"#,
            }),
            Element::Prop(Prop {
                slug: "well-foundedness-over-symmetry",
                parent: Some("verdict-dagger-is-contractive"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"This trades the dagger's **symmetry** for **well-foundedness**. A
recovery that may return the value it received is a stall loop with a type; a
recovery required to decrease terminates.

"#,
            }),
            Element::Prop(Prop {
                slug: "contraction-is-a-runtime-guard",
                parent: Some("verdict-dagger-is-contractive"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"**The limit.** The contraction is a runtime guard. It aborts on a
non-decreasing step; it does not prove decrease, and it does not prove general
forward progress.

"#,
            }),
            Element::Prop(Prop {
                slug: "error-dagger-is-optional-and-unguarded",
                parent: Some("the-dagger-is-partial-and-contractive"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The residual's adjoint is **optional and unguarded** (`G9`). Optional,
because a caller may handle a returned residual itself; unguarded, because a
re-entry after an unanswered call may legitimately reuse the argument — the same
licence Het grants a returned residual
({#adequacy-failure-returns-residual}).

"#,
            }),
            Element::Prop(Prop {
                slug: "resume-edge-is-the-residual-dagger",
                parent: Some("error-dagger-is-optional-and-unguarded"),
                kind: Kind::Signature,
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

"#,
            }),
            Element::Prop(Prop {
                slug: "terminal-verdicts-have-no-adjoint",
                parent: Some("the-dagger-is-partial-and-contractive"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A terminal verdict has **no adjoint**. It is an absorbing object, and
declaring a backward edge from one is rejected.

"#,
            }),
            Element::Prop(Prop {
                slug: "dagger-laws-are-not-verified",
                parent: Some("the-dagger-is-partial-and-contractive"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"**The limit.** Neither $f^{\dagger\dagger} = f$ nor
$(g \circ f)^\dagger = f^\dagger \circ g^\dagger$ is verified. For the verdict
dagger the involution is not merely unverified but **deliberately broken** by
{#verdict-dagger-is-contractive}.

"#,
            }),
            Element::Verbatim(
                r#"---

## 8 · The substrate is affine

"#,
            ),
            Element::Prop(Prop {
                slug: "substrate-is-affine",
                parent: None,
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"The host substrate implements **affine logic** — linear logic without the
requirement to use exactly once — and that is what the category's linearity
rests on.

"#,
            }),
            Element::Prop(Prop {
                slug: "linear-logic-dictionary",
                parent: Some("substrate-is-affine"),
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

"#,
            }),
            Element::Prop(Prop {
                slug: "at-most-once-not-exactly-once",
                parent: Some("substrate-is-affine"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The substrate gives **at most once**, not exactly once. An object cannot
be used twice; it *can* be dropped.

"#,
            }),
            Element::Prop(Prop {
                slug: "must-use-is-the-affine-approximation",
                parent: Some("at-most-once-not-exactly-once"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"`G4` closes the second half by lint rather than by type, and it covers
every carrier of a live token — objects, verdict objects, the branching
outcome, and the residual — not the objects alone.

"#,
            }),
            Element::Prop(Prop {
                slug: "lint-is-escapable",
                parent: Some("at-most-once-not-exactly-once"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"**The limit.** A lint is escapable: a token may be leaked, bound to a
discard, or buried in a dropped container. The close is partial.

"#,
            }),
            Element::Prop(Prop {
                slug: "one-token-one-thread",
                parent: Some("substrate-is-affine"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"Linearity is a claim about a **unique consumer**, and a shared reference
crossing a thread boundary would supply a second. `G3` forbids it: no object of
the category may be shared or sent, so one token cannot be driven by two
threads.

"#,
            }),
            Element::Prop(Prop {
                slug: "move-semantics-alone-are-insufficient",
                parent: Some("one-token-one-thread"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"Move semantics alone give one consumer for an *owned* value and say
nothing about shared references. `G3` is what makes the linearity claim hold of
the whole object, not merely of its owned form.

"#,
            }),
            Element::Prop(Prop {
                slug: "true-linearity-needs-the-language",
                parent: Some("substrate-is-affine"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"**The limit.** Exactly-once cannot be closed above the language. A
linear substrate would make it exact; an affine one admits the approximation
{#must-use-is-the-affine-approximation} states.

"#,
            }),
            Element::Verbatim(
                r#"---

## 9 · Types are propositions

"#,
            ),
            Element::Prop(Prop {
                slug: "types-are-propositions",
                parent: None,
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"Under Curry–Howard an object is a **proposition** and a path through the
category is a **proof** of it.

"#,
            }),
            Element::Prop(Prop {
                slug: "object-asserts-its-history",
                parent: Some("types-are-propositions"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"Holding an object asserts that every arrow on some path into it ran. The
assertion is discharged by the type, not by an accompanying check.

"#,
            }),
            Element::Prop(Prop {
                slug: "residual-is-a-conjunction",
                parent: Some("types-are-propositions"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The residual is a **conjunction**: *the step did not answer* **and**
*the argument is preserved*. Both conjuncts are carried by the one object.

"#,
            }),
            Element::Prop(Prop {
                slug: "terminal-payload-is-the-witness",
                parent: Some("types-are-propositions"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"A terminal verdict may carry a payload (`G11`). That payload is the
**witness** the terminal proposition asserts the existence of, returned through
the verdict rather than around it.

"#,
            }),
            Element::Prop(Prop {
                slug: "proof-is-of-traversal-not-correctness",
                parent: Some("types-are-propositions"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"The proof is of **traversal, not correctness**. A path proves the arrows
ran in a legal order. It says nothing about whether any body computed the right
thing.

"#,
            }),
            Element::Verbatim(
                r#"---

## 10 · The verification boundary

"#,
            ),
            Element::Prop(Prop {
                slug: "verification-boundary",
                parent: None,
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"The construction verifies the **shape of the category**. It never
verifies the **content of a morphism**.

"#,
            }),
            Element::Prop(Prop {
                slug: "guarantees-carry-categorical-content",
                parent: Some("verification-boundary"),
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

"#,
            }),
            Element::Prop(Prop {
                slug: "what-is-not-verified",
                parent: Some("verification-boundary"),
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

"#,
            }),
            Element::Prop(Prop {
                slug: "gate-guarantees-constrain-the-domain-not-the-arrow",
                parent: Some("what-is-not-verified"),
                kind: Kind::Rationale,
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
[Q11](questions/open/q11-gate-faithfulness.md)'s answer.

"#,
            }),
            Element::Prop(Prop {
                slug: "boundary-is-typestate-not-verification",
                parent: Some("verification-boundary"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"The boundary between the two tables is the boundary between
**typestate** and **formal verification**. Every row of
{#what-is-not-verified} is on the far side of it, and none is closed by
adding a check to the construction.

"#,
            }),
            Element::Verbatim(
                r#"---

## 11 · The dependency structure is an opfibration

"#,
            ),
            Element::Prop(Prop {
                slug: "dependency-structure-is-an-opfibration",
                parent: None,
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"**Governed subjects** and the declared dependency morphisms between them
form a **free base category**; subjects-in-states over it form a **Grothendieck
opfibration** $p : E \to B$. A governed subject is one that carries its own
signature, and so is a theory at the next level
({#fractal-property}).

"#,
            }),
            Element::Prop(Prop {
                slug: "fibre-is-a-ladder",
                parent: Some("dependency-structure-is-an-opfibration"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The **fibre over a subject is exactly that subject's ladder** — the
category of {#ladder-declares-a-category}. Each object of the base contains a
category; that is the fractal property read in this setting.

"#,
            }),
            Element::Prop(Prop {
                slug: "declaration-names-no-foreign-object",
                parent: Some("fibre-is-a-ladder"),
                kind: Kind::Decidable {
                    sentence: "every_dependency_resolves",
                },
                numbering: None,
                prose: r#"**A declaration names no object of another fibre.** An edge whose
domain lies in a different fibre is not a morphism of *this* free category — it
is a morphism of the total space $E$, crossing fibres. It is a different
categorical object, and a declaration that admitted one would no longer present
a free category ({#category-is-freely-generated}). Cross-fibre structure
enters a declaration only through composition operators, never as a declared
arrow.

"#,
            }),
            Element::Prop(Prop {
                slug: "typed-edge-is-an-opcartesian-lift",
                parent: Some("dependency-structure-is-an-opfibration"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"A typed edge is an **opcartesian lift**. A state change at the source
transports *forward* along the edge by a pushforward functor between fibres.

"#,
            }),
            Element::Prop(Prop {
                slug: "orientation-is-load-bearing",
                parent: Some("typed-edge-is-an-opcartesian-lift"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"The orientation is load-bearing. Transport runs with the direction of
information flow, which is what makes the structure an **op**fibration rather
than a fibration; reversing it changes which lifts exist.

"#,
            }),
            Element::Prop(Prop {
                slug: "edge-type-selects-the-pushforward",
                parent: Some("typed-edge-is-an-opcartesian-lift"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"The **edge type selects the pushforward**. A strict edge lifts to an
obligation; an advisory edge lifts to a coproduct; a generative edge lifts to
the dependent's existence; a mechanical edge lifts to a state update with no
outside.

"#,
            }),
            Element::Prop(Prop {
                slug: "edge-taxonomy-is-the-theorys",
                parent: Some("edge-type-selects-the-pushforward"),
                kind: Kind::Decidable {
                    sentence: "every_declared_kind_is_lived",
                },
                numbering: None,
                prose: r#"**The taxonomy is the governing theory's**, declared by it exactly as
an edit vocabulary is ({#edit-required-not-typed}).
This document states that an edge type selects a pushforward; it never
enumerates the types, for the reason
{#governs-who-not-what} gives.

"#,
            }),
            Element::Prop(Prop {
                slug: "strict-and-advisory-are-the-gate",
                parent: Some("edge-type-selects-the-pushforward"),
                kind: Kind::Decidable {
                    sentence: "must_reexamine",
                },
                numbering: None,
                prose: r#"The load-bearing split among edge types — obligatory against advisory,
*this breaks* against *check whether this breaks* — is the **gate marker**
({#four-gates}) at this level. A strict edge
propagates decidably; an advisory edge requires a ruling. It is not a second
taxonomy laid over the gates; it is the gates, read one level up.

"#,
            }),
            Element::Prop(Prop {
                slug: "advisory-lift-lands-in-a-coproduct",
                parent: Some("dependency-structure-is-an-opfibration"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"An **advisory lift does not break functoriality.** It does not land in
the target's objects; it lands in a **coproduct** — *review required* plus
*survives* — which the base delivers rigidly. Collapsing that coproduct is the
target's own work, done by its own transitions.

"#,
            }),
            Element::Prop(Prop {
                slug: "same-coproduct-at-both-levels",
                parent: Some("advisory-lift-lands-in-a-coproduct"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"This is the same coproduct as
{#branching-is-a-coproduct}, one level up. The structure that makes a
branching transition honest is the structure that keeps the dependency level
functorial; the two levels share machinery rather than resemble each other.

"#,
            }),
            Element::Prop(Prop {
                slug: "vertical-morphisms-preserve-agency",
                parent: Some("advisory-lift-lands-in-a-coproduct"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"Because every fibre is a free category, the target retains its own
**vertical** morphisms. Lifts evaluate against the target's *current* state, so
functoriality holds over the total space rather than in spite of it.

"#,
            }),
            Element::Prop(Prop {
                slug: "edges-are-dependent-optics",
                parent: Some("dependency-structure-is-an-opfibration"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"A dependency edge is a **dependent optic**: covariant forward transport
of a state change, contravariant backward query of exposure. The backward pass's
type depends on the state transported forward, which is what makes it dependent.

"#,
            }),
            Element::Prop(Prop {
                slug: "exposure-is-the-backward-pass",
                parent: Some("edges-are-dependent-optics"),
                kind: Kind::Signature,
                numbering: None,
                prose: r#"**Blast radius is the backward pass, not a count.** Querying backward
along the composite optic returns a *typed exposure* — how many obligations of
which kind — and a count of reachable subjects is its Boolean shadow.

"#,
            }),
            Element::Prop(Prop {
                slug: "opfibrations-compose",
                parent: Some("dependency-structure-is-an-opfibration"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"Opfibrations **compose**. A map from governed subjects to the theories
that govern them is itself an opfibration, and its composite with $p$ is a
single opfibration over the whole tower.

"#,
            }),
            Element::Prop(Prop {
                slug: "iteration-not-a-second-level",
                parent: Some("opfibrations-compose"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"Composing is an **iteration of one level, not the arrival of a second**.
Opfibrations are 1-cells and compose as such. A genuine second level needs a
2-cell *between* fibrations — a remapping of the structure itself — which
nesting does not supply.

"#,
            }),
            Element::Prop(Prop {
                slug: "transport-is-scale-invariant",
                parent: Some("opfibrations-compose"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"Because optics compose, obligation-transport is **scale-invariant**. A
traversal of the backward pass need not know whether an edge crosses a domain
boundary; the same pass runs at every scale.

"#,
            }),
            Element::Prop(Prop {
                slug: "horizontal-and-vertical-coincide",
                parent: Some("opfibrations-compose"),
                kind: Kind::Judgmental {
                    role: "category-theorist",
                },
                numbering: None,
                prose: r#"Under the Grothendieck construction the hierarchy flattens: a
sibling-to-sibling edge and a domain-to-parent edge are both generating
morphisms of one composite base and lift identically. That, precisely, is the
content of *"the structure is fractal."*

"#,
            }),
            Element::Prop(Prop {
                slug: "conformance-and-propagation-run-over-different-bases",
                parent: Some("dependency-structure-is-an-opfibration"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"Propagation along this opfibration is **not** the inverse of
conformance. Conformance runs from a model to its theory and re-indexes
contravariantly; propagation runs from a revised subject to its dependents and
transports covariantly. They are adjacent levels over different bases, not two
orientations of one tower
({#two-directions-two-bases}).

"#,
            }),
            Element::Verbatim(
                r#"---

## 12 · The mathematics is the implementation, not the surface

"#,
            ),
            Element::Prop(Prop {
                slug: "mathematics-is-the-implementation-not-the-surface",
                parent: None,
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"The surface syntax names **objects and transitions**. It does not name
free categories, coproducts, indexed monads, daggers, or optics, and it must
not.

"#,
            }),
            Element::Prop(Prop {
                slug: "surface-is-the-programmers-model",
                parent: Some("mathematics-is-the-implementation-not-the-surface"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"A declaration is written in the vocabulary of the domain being
modelled. The mathematics of this document is what the *construction* is
obliged to, not what the author is obliged to write.

"#,
            }),
            Element::Prop(Prop {
                slug: "same-move-as-the-substrate",
                parent: Some("mathematics-is-the-implementation-not-the-surface"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"This is the substrate's own move, one level up: an author writes
ordinary bindings and the checker enforces the affine discipline
({#substrate-is-affine}). Here an author writes rungs and transitions, and
the construction enforces the category.

"#,
            }),
            Element::Prop(Prop {
                slug: "hiding-is-not-optional",
                parent: Some("mathematics-is-the-implementation-not-the-surface"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"The hiding is **not a convenience**. A surface that required the
mathematics would make the mathematics the language, and the enforcement would
then rest on the author restating it correctly — which is the failure the
construction exists to remove.

"#,
            }),
            Element::Prop(Prop {
                slug: "correspondence-is-falsifiable",
                parent: Some("mathematics-is-the-implementation-not-the-surface"),
                kind: Kind::Rationale,
                numbering: None,
                prose: r#"The correspondence is **falsifiable in the construction, not in the
prose**. Every claim here either names a guarantee that a conformance test
protects, or is marked a limit. A claim that is neither has no standing.

"#,
            }),
            Element::Verbatim(
                r#"---

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
"#,
            ),
        ],
    }
}
