# The triage

**Status: informative.** `rung-props.md`, `rung-ct-props.md` and
`rung-het-props.md` govern. This records *why* each of the 380 propositions was
classified as it was, and it states no proposition.

The classification itself lives in `rung-doctrine/src/`, which is the source the
`*-props.md` files are rendered from. Where this note and those files disagree,
the files are right.

---

## 1 · What the classification is, and is not

A proposition's kind is **not** a claim about how important it is or how sure
anyone is of it. It is a claim about *what would settle it*:

| kind | settled by | count |
|---|---|---:|
| **decidable** | a proof — a test that fails when it is violated | **113** |
| **rationale** | nothing — it argues, or records a limit | **151** |
| **signature** | nothing — it declares vocabulary | **64** |
| **judgmental** | a principal; nothing else can | **49** |
| **owed** | decidable in principle; nothing establishes it **yet** | **3** |

Per document, and per section: **[`conformance.md`](conformance.md)**, which is
generated from the same encoding. It is not repeated here, because a count
written by hand in a document is the thing this whole scheme exists to remove —
and one written here *was* wrong by two before anybody noticed.

### The criterion, and the redo it forced

The first pass asked *"can a machine compute this from a data structure?"* and,
finding no such structure, answered **signature** 125 times. That was the wrong
question. Most of what rung guarantees is about an **implementation**, and an
implementation is checked by running something against it — not by evaluating a
closure over a value.

The right question:

> A proposition is **decidable** iff a proof exists that fails when the
> proposition is violated — and that failure has been demonstrated.

Under it the decidable fragment went from 6 to 108, drawing 64 from signature
and 40 from rationale. `G3` is the clean example: *"every rung and verdict MUST
be `!Send + !Sync`"* looked unencodable, and `rung/tests/compile_pass.rs` has
settled it by autoref specialization the whole time.

A proof takes three forms, all decidable and all checked to resolve: a named
test, `(rustc)` where violating it does not build, or a checker.

### Two things the redo revealed

**Forty propositions I called rationale are the sharpest claims in the corpus.**
`deferral-is-not-a-verdict`, `no-bound-on-reentry`, `resumption-is-authorial`,
`decidable-cannot-consult-pool` — each is phrased as a clarification (*"X is
measured against A, never against B"*) and each has a test. Reading a
clarification as commentary was a systematic error of the first pass.

**The judgmental fragment barely moved: 50 → 49.** Only
`verdict-dagger-is-contractive` turned out to have a proof. That is the result
worth having: the propositions that need judgment were correctly identified, and
they need it because *nothing else can settle them* — not because nobody has got
round to writing a test.

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
`questions/resolved/_evidence/`. Q7's resolution *overturned* the account
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


---

## 7 · The other two documents

### `rung-props.md` — 46 signature, 24 rationale, **nothing else**

No proposition of the macro specification is decidable or judgmental, and the
reason is worth stating rather than treating as a gap.

**The guarantees are signature.** `G1`–`G16` read like claims — *"every rung and
verdict MUST be `!Send + !Sync`"* — and each names a conformance test. But what
they declare is **the signature of the emitted module**: what a `ladder!`
declaration produces. The named test checks that the macro *implements* the
declared signature, which is what a conformance test is for. Encoding one as a
`decidable` sentence would require a model of an emitted module, and there is
none — the compiler holds that, not a value.

That is stronger enforcement than a sentence, not weaker. It is simply not
`M ⊨ φ`.

**The non-guarantees are rationale, structurally.** A claim that something is
*not* enforced has no satisfying model. All fifteen of §5 are of that shape.

**`J1` and `J2` are rationale, and this is the call to push back on first.**
They are design judgments — *where should a ladder bottom out*, *what belongs in
`rung-std`* — and they read like questions an outside could rule on. They are
classified rationale because their subject is a design decision rather than an
artifact that could be handed to a judge. A reader who thinks a design decision
*is* such an artifact would make them judgmental, and the argument is available.

### `rung-het-props.md` — 27 judgmental, and that is where they live

The judgmental fragment concentrates here, as expected: this is the document
that makes mathematical identifications. *"An algebra is a functor into the
Kleisli category"*, *"the tower is a fibered category"*, *"satisfaction is a
two-player game"*, *"`enact` makes an endofunctor"* — each is a bet a competent
mathematician could refute, and Q7's ruling shows that is not hypothetical.

134 rationale is the large number, and it is honest. Het's document argues
heavily: for every structural claim there are two or three propositions saying
what it does *not* mean, why the alternative collapses, or where the limit sits.
Those are arguments, and they belong where arguments belong.

**Three decidable**, each naming a sentence `rung_std::principals` declares:

| proposition | sentence |
|---|---|
| `epsilon-declared-not-ranked` | `epsilon_is_declared` |
| `role-not-kind` | `roles_are_earned` |
| `supplier-interface` | `identity_fields_are_declared` |

Not more, deliberately. Several other propositions are *established* by tests —
`het-declares-no-worth-law` by a source scan, `ordering-is-hetopts` likewise —
but a test is not a sentence, and marking them decidable would name a body that
does not exist.

## 8 · What "decidable" does not yet mean

Naming a proof is clause one. Clause two — that the proof has been **seen to
fail** — is where the number turns:

    decidable                                     108
    with a demonstrated failure                    12   (11%)
    naming a proof nobody has watched fail         96

A test that cannot fail is not a proof. This repository has a proposition
saying so (`a-refusal-test-that-cannot-fail`) and a mutation discipline that
practises it — but the demonstration is recorded in the ledger's prose for
twelve propositions, and the other ninety-six name a test whose ability to
redden is an assumption.

That is not a claim the ninety-six are unproven. Most were probably mutated when
they were written and nobody wrote it down. It is a claim that **the record
cannot tell the difference**, which is the same thing from the outside.

There is a second, deeper gap. A proof is cited by a person who also chose which
proposition it establishes, and nothing checks the citation is **apt**. That is
`establishes_what_it_cites`, judgmental and unsettled — and it is the reason
some of those forty rationale-to-decidable moves may be the *ledger* being
generous rather than the reading being wrong. `sealing-is-the-axiom-not-a-guard`
cites a test that proves sealing; whether sealing *is the axiom* is not what the
test shows.

Which is the honest shape: 108 decidable, 12 demonstrated, 0 apt-checked.

## 9 · The limit of this reading

The rung-ct triage was made by reading each proposition. **These 272 were
classified from each proposition's leading claim** — the sentence that says what
kind of thing it is — rather than from its full body. That is a real reading and
a shallower one, and it is most likely to be wrong where a proposition opens
with a definition and then argues, or opens with an argument and then declares.

Which is a reason the section above is the right shape: the classification is a
**proposal**, and 50 propositions are now marked as claims a mathematician could
refute by someone who is not one.


---

## 10 · Non-guarantees are provable, and `owed` is not `judgmental`

Two things came out of writing `rung/tests/non_guarantees.rs`.

### A limit is proven by exercising it

I had filed all fifteen of `rung-props.md` §5 as rationale, reasoning that a
claim something is *not* enforced has no satisfying model. Wrong.

**A non-guarantee is proven by a test that exercises the gap.** `G4` is
`#[must_use]` and escapable, so the proof is a test that escapes it three ways
under `deny(unused_must_use)`. `G8` catches identical-token stalls but not
general non-progress, so the proof is a hundred rounds of guard-satisfying
motion that converges on nothing. A body may be wrong, so the proof is a ladder
whose `doubled` transition subtracts.

These fail when the system gets **stronger** — the opposite direction from every
other test in the suite, and the only place where a green run is the interesting
outcome. That matters in both directions: a stated limit nothing checks is
either quietly closed while the document goes on disclaiming it, or built upon
by someone it closes underneath.

Six §5 propositions moved to decidable this way; a seventh already had a test
nobody had cited.

**One of them taught us something.** Clippy refuses `mem::forget` on a token,
because the token implements no `Drop` — so forgetting it is identical to
dropping it. There is no destructor to skip, because a rung does nothing on the
way out. That is §5.4 stated from the other side, and it was discovered by
writing the test rather than by reading the proposition.

### `owed` is the work queue

Three propositions are decidable in principle with nothing establishing them.
Filing those as `judgmental` would hide work behind a word meaning something
else, and route it to a judge who cannot help:

| proposition | why owed |
|---|---|
| `cross-crate-provenance` | needs a fixture crate — the claim is about a boundary |
| `one-gate-unimplemented` | **unimplemented** — `#[conditional(..)]` is a refusal, not an encoding |
| `outward-conditions-remaining` | **unimplemented** — the authorial containment conjunct is left to the body |

Two of those are not waiting on anyone's judgment. They are waiting on code that
does not exist.

That distinction is the one that makes the doctrine capable of *driving*
anything. A judgmental proposition asks a principal a question. An **owed** one
tells an author what to build — and an audit that reports the queue is the
doctrine doing work rather than describing it.

The count is 3 only because `rung-props.md` was the document worked through.
`rung-het-props.md`'s 105 rationale certainly hide more, and the interesting
subset is the unimplemented one: the conditional gate, panels, HetOpt. Nothing
there is undecidable. It is unbuilt, which is a different word with a different
remedy.
