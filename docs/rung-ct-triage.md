# The triage of `rung-ct-props.md`

**Status: informative.** `rung-props.md`, `rung-ct-props.md` and
`rung-het-props.md` govern. This records *why* each of the 108 propositions of
the categorical account was classified as it was, and it states no proposition.

The classification itself lives in `rung-doctrine/src/rung_ct.rs`, which is the
source `rung-ct-props.md` is rendered from. Where this note and that file
disagree, the file is right.

---

## 1 · What the classification is, and is not

A proposition's kind is **not** a claim about how important it is or how sure
anyone is of it. It is a claim about *what would settle it*:

| kind | settled by | count |
|---|---:|---|
| **signature** | nothing — it declares the correspondence | 41 |
| **rationale** | nothing — it argues, or records a limit | 41 |
| **judgmental** | a mathematician | 23 |
| **decidable** | a sentence, run over a model | 3 |

Signature and rationale carry no gate, and that is structural rather than
conventional: neither is a claim that could be satisfied, so there is nothing
for a principal to settle. `only_claims_carry_a_gate` asserts it.

## 2 · The criteria, stated before they were applied

**Signature** — declares the correspondence. *"A rung is an object."* *"The
outcome of a branching transition is a coproduct."* These constitute Σ. They can
be **badly chosen** but not **false**: nothing about a model makes "a rung is an
object" wrong, because the sentence is what fixes what a rung is taken to be.

**Judgmental** — asserts a **mathematical identification that could be wrong**.
*"The ladder is an indexed monad."* *"A transition is a Prism."* *"Opfibrations
compose."* Each of these is a bet about mathematics that a competent
category theorist could refute, and no machine can settle.

The precedent here is lived, not assumed. **Q7** (transitions are Prisms, not
Kleisli arrows), **Q9** (the dependency structure is a Grothendieck
opfibration) and **Q10** (the opfibration iterates) were each resolved by
outside expert review, and the reviews are in
`docs/questions/resolved/_evidence/`. Q7's resolution *overturned* the account
that preceded it. These 23 are the propositions of that kind — the ones where
being wrong is a live possibility with a history.

**Decidable** — settleable by running a sentence over a model. Rare here, for a
reason worth stating plainly (§4).

**Rationale** — argues, draws a consequence, or records a limit. Every
proposition opening **"The limit."** is rationale by construction: a claim that
something is *not* enforced has no satisfying model.

## 3 · Why so many are signature

41 of 108, and they cluster in §§1–3, 6–9 — the sections that say what a ladder
declaration *is*. That is the shape of a document whose job is to fix a
vocabulary: most of it introduces terms rather than asserting things about them.

The consequence is worth being explicit about. **Encoding `rung-ct-props.md`
does not make most of it machine-checkable, and was never going to.** What it
makes is a document with no hand-typed numbers, no reference that can go stale,
and an explicit record of which of its claims are bets.

## 4 · Why only three are decidable

A decidable sentence needs a **model** — a value it is evaluated over. Most of
this document's claims are about what the macro emits, and are settled by the
compiler (a non-exhaustive match does not build; a sealed constructor cannot be
called) or by `trybuild`. That is enforcement, and it is *stronger* than a
sentence, but it is not `M ⊨ φ`: there is no model in hand.

The three that are decidable all live in §11, and all three are claims about a
**dependency graph**, which is a thing `rung_std::questions` actually models:

| proposition | sentence |
|---|---|
| `declaration-names-no-foreign-object` | `every_dependency_resolves` |
| `edge-taxonomy-is-the-theorys` | `every_declared_kind_is_lived` |
| `strict-and-advisory-are-the-gate` | `must_reexamine` |

Each names the sentence carrying its body, and
`every_decidable_proposition_names_a_declared_sentence` checks those names
against what the theories declare. Without that check the marker would be a
promise someone keeps — the exact failure the encoding exists to remove,
reintroduced one level up.

## 5 · The judgments a reader should push back on first

Not every call was clean. These are the ones most likely to be wrong, offered so
that disagreement has somewhere to land:

- **`the-law`** → signature. It reads as a *law* — a constraint that could be
  violated — but it is derivable from `rungs-are-objects` and
  `transitions-are-morphisms`, so it fixes vocabulary rather than asserting
  something about a model. A reader who thinks it is a claim about every
  well-formed ladder would make it judgmental, and would have a case.
- **`elimination-is-exhaustive`** → signature. It is *enforced*, absolutely — by
  the compiler. But the proposition states what elimination out of a coproduct
  **is**, and the enforcement is a consequence.
- **`monad-laws-hold-by-construction`** → judgmental. "By construction" invites
  reading it as settled. It is a mathematical claim that the laws hold, and it
  is exactly the kind of claim Q7 showed can be wrong.
- **`exposure-is-the-backward-pass`** → signature, though it is close to
  decidable: `rung_std::questions` does compute exposure. It was left signature
  because the proposition says what blast radius *is*, not that any particular
  computation of it is right.
- **`entry-constructor-is-public`** → signature. It states a fact about
  emission, which makes it feel decidable — but there is no model of an emitted
  module to evaluate it over, only the compiler's acceptance.

## 6 · What this triage is not entitled to

**It is a proposal, not a ruling.**

`is_a_sentence_not_rationale` is judgmental for a reason, and the reason applies
here. Whoever performed this triage also authored much of the corpus, and P0
disqualifies a judge whose provenance overlaps what it judges
(`judgmental-qualifying-set`). So this classification is the output of an
**authorial** act — a proposal about the documents by someone with standing over
them — and it awaits a disjoint reader.

That is not a formality. 23 propositions are hereby marked as *claims a
mathematician could refute*, and the person marking them is not that
mathematician. The count is a bet about which parts of the account are load
bearing, and it is exactly the sort of bet the system is built to route to
someone else.
