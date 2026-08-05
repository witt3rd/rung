# The triage — what kind of thing each proposition is

**Status: informative.** `rung-props.md`, `rung-ct-props.md` and
`rung-het-props.md` govern. This records the criteria by which each of the 380
propositions was classified, and the calls most likely to be wrong. It states no
proposition.

The classification lives in `rung-doctrine/src/`, which is the source those
documents are rendered from. Where this note and that source disagree, the
source is right. Counts are in [`conformance.md`](conformance.md), which is
generated — they are not repeated here, because a count written by hand in a
document is the thing this whole scheme exists to remove, and one written here
*was* wrong by two before anybody noticed.

---

## 1 · The criterion

A proposition's kind is not how important it is or how sure anyone is. It is
**what would settle it** — which is also who it is dispatched to.

| kind | settled by |
|---|---|
| **decidable** | a proof: a test that fails when the proposition is violated |
| **judgmental** | a principal, provenance-**disjoint** from what it judges |
| **owed** | an author with **standing** — decidable in principle, nothing establishes it yet |
| **signature** | nothing; it declares vocabulary |
| **rationale** | nothing; it argues, or records a limit |

The two middle rows route to **structurally exclusive** principals: judgment
requires `π(p) ∩ π(a) = ∅`, authorship requires `standing(p, M)`. Opposite
conjuncts of one filter, so no principal is both for the same thing. The
classification is therefore not a taxonomy someone invented — it falls out of
who is permitted to act.

Signature and rationale carry no gate, and that is structural rather than
conventional: neither is a claim that could be satisfied, so there is nothing to
settle. `only_claims_carry_a_gate` asserts it.

## 2 · Decidable means a proof, not a computation

The question is **not** *"can a machine compute this from a data structure?"*
Asking that produced 125 `signature` classifications and was wrong: most of what
rung guarantees is about an **implementation**, and an implementation is checked
by running something against it.

> A proposition is decidable iff a proof exists that fails when the proposition
> is violated — and that failure has been demonstrated.

`G3` is the clean case. *"Every rung and verdict MUST be `!Send + !Sync`"* looks
unencodable, and `rung/tests/compile_pass.rs` has settled it by autoref
specialization the whole time.

A proof takes several forms, all of them runnable and all checked to resolve: a
named test, a `trybuild` case with a committed `.stderr`, an autoref
specialization, a driven run, a source scan.

**What is not a proof.** `(rustc)` — *"the compiler enforces this"* — was
carried for seven propositions and established nothing: nothing failed when they
were violated, because nothing tried. All seven turned out writable. An
`#[ignore]`d test is likewise not a proof, for the same reason. Both are refused
by tests.

### A limit is proven by exercising it

The non-guarantees ([§5](rung-props.md#non-guarantees)) say what the macro does
*not* enforce, which looks unprovable — a claim that something is not enforced
has no satisfying model. The proof is a test that **exercises the gap**: escape
`#[must_use]` three ways under `deny(unused_must_use)`; run a hundred
guard-satisfying rounds that converge on nothing.

These fail when the system gets **stronger**, and that matters both ways: a
specification that understates its guarantees is as wrong as one that overstates
them, and a limit someone builds on can close underneath them silently.

## 3 · `owed` is "nobody yet"; `judgmental` is "nobody ever"

A status field can say *not implemented*. What it cannot say is that the two go
to different people with different powers.

`one-gate-unimplemented` is not waiting on a mathematician. It is waiting on
`#[conditional(..)]`, which does not exist. Filing it judgmental would spend the
scarcest resource in the system — a qualified outside — on work no judge can do.

So: **a judgmental proposition asks a principal a question; an owed one tells an
author what to build.** A test prints the queue.

## 4 · Where each kind concentrates, and why

**`rung-props.md` has no judgmental propositions at all.** It describes an
artifact, and artifacts can be tested. Its guarantees read like claims and each
names a conformance test, but what they declare is the *signature of the emitted
module* — the test checks that the macro implements it, which is what a
conformance test is for.

**`rung-het-props.md` and `rung-ct-props.md` hold the judgmental fragment.**
Both make mathematical identifications — *"an algebra is a functor into the
Kleisli category"*, *"the tower is a fibered category"* — and each is a bet a
competent mathematician could refute. The precedent is lived: **Q7's ruling
overturned the account that preceded it.**

**Rationale is the largest kind in Het**, and that is honest rather than a
failure of nerve. Het argues heavily; for every structural claim there are two
or three propositions saying what it does *not* mean, why the alternative
collapses, or where the limit sits.

**A caution about the rationale count.** Forty propositions were first filed
rationale and are the sharpest claims in the corpus —
`deferral-is-not-a-verdict`, `no-bound-on-reentry`, `resumption-is-authorial`,
`decidable-cannot-consult-pool`. Each is phrased as a clarification (*"X is
measured against A, never against B"*) and each has a test. Reading a
clarification as commentary was a systematic error, and it may not be fully
corrected.

## 5 · The calls to push back on first

Not every call was clean. These are the ones most likely wrong:

- **`the-law`** → signature. It reads as a *law* — a constraint that could be
  violated — but it is derivable from `rungs-are-objects` and
  `transitions-are-morphisms`, so it fixes vocabulary. A reader who thinks it
  claims something about every well-formed ladder would make it judgmental.
- **`monad-laws-hold-by-construction`** → judgmental. "By construction" invites
  reading it as settled. It is a mathematical claim, and exactly the kind Q7
  showed can be wrong.
- **`J1` and `J2`** → rationale. Design judgments — *where should a ladder bottom
  out*, *what belongs in `rung-std`* — classified rationale because their
  subject is a decision rather than an artifact a judge could be handed. A
  reader who thinks a design decision *is* such an artifact would make them
  judgmental.
- **`compile-fail-asserts-only-non-compilation`** → rationale, and probably
  decidable. It is a checkable fact about rustdoc: a `compile_fail,E0999`
  doctest passes, and E0999 does not exist. Nobody has written the test, and it
  is not filed `owed` because nobody has claimed one is coming.
- **`exposure-is-the-backward-pass`** → signature, though `rung_std::questions`
  does compute exposure. Left signature because the proposition says what blast
  radius *is*, not that any computation of it is right.

## 6 · Two things the classification does not establish

**That a proof was ever watched fail.** Naming a proof is one clause; the
demonstration is the other, and it is recorded in prose for a small minority.
Most were probably mutated when written and nobody wrote it down — but the
record cannot tell the difference, which from outside is the same thing.

**That a proof is *apt*.** A proof is cited by someone who also chose which
proposition it establishes, and nothing checks the citation. That is
`establishes_what_it_cites`, judgmental and unsettled — and it is why some
classifications may be the *record* being generous rather than the reading being
right. `sealing-is-the-axiom-not-a-guard` cites a test that proves sealing;
whether sealing *is the axiom* is not what the test shows.

## 7 · This is a proposal, not a ruling

`is_a_claim_not_rationale` is judgmental for a reason, and the reason applies
here. Whoever performed this triage also authored much of the corpus, and P0
disqualifies a judge whose provenance overlaps what it judges.

So the classification is the output of an **authorial** act awaiting a disjoint
reader. That is not a formality: propositions are hereby marked as *claims a
mathematician could refute*, by someone who is not one.

The classification was also made from each proposition's **leading claim** for
two of the three documents, rather than from its full body — a real reading and
a shallower one, most likely wrong where a proposition opens with a definition
and then argues, or opens with an argument and then declares.
