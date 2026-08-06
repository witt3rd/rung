# Handoff — driving rung's development through rung's own loop

**Status: informative.** `rung-props.md`, `rung-ct-props.md` and
`rung-het-props.md` govern. This states a goal that has not been reached, what
stands between here and it, and the order those things come in.

---

## 0 · The goal, stated so it can fail

> Every change to rung is made by its own audit-rectify pass: an audit finds a
> defect, an author proposes a fix, a judge disposes of it, an author enacts it
> — with Donald as the **last-resort** principal rather than the first.

Two words carry the weight.

**Every.** Not "a demonstration exists." The measure is whether a change can
*only* arrive this way, and whether anyone bothers to route around it.

**Last-resort.** The escalation chain must have things above him. A loop where
the human judges everything is the current arrangement with more ceremony.

The number that tracks this is in the README and reads **zero**: defects in rung
found and fixed by the loop rather than by a person.

---

## 1 · Where it stands

```
decidable          123    all satisfied — `cargo run -p rung-doctrine --bin audit`
judgmental          47    0 settled
owed                 3    2 unimplemented, 1 parked test
rationale          148
signature           59
                   ───
                   380

demonstrated failures      12 of 123   (a proof nobody watched fail is an assumption)
proofs claiming no prop   173          (mostly tooling; unfiltered on purpose)
judgment records            0
```

**What exists and works.** The doctrine is data; four documents are generated
from it and CI refuses a hand edit. The audit arm runs and attributes results
per proposition. `rung-driver` builds a `Pool` from `population.yaml`,
filters by capability, and reaches models through `rung_std::llm`. `judgments/`
has a schema whose every check is exercised against a bad record.
`rung_std::driver::Park` holds suspended runs.

**What does not exist.** A judge. An edit vocabulary over *code*. A run loop
joining the four moves. Suspension in the pass.

---

## 2 · The critical path

Ordered by what blocks what, not by size.

### 2.1 — Q14 and Q16 ruled; the carrier's implementation is the gate now

[Q14](../questions/resolved/q14-model-principal-provenance.md) asked what provenance
a model principal carries. **It is ruled** (2026-08-05): the stake-based reading
holds — `π(p) = authored(p) ∪ {id(p)}`, with `id(p)` the family identifier for
discontinuous kinds and `authored(p)` commission-local. Of the three candidate
readings:

- **per-family** disqualifies every model from nearly everything, because models
  wrote most of this corpus — refused as over-strong
- **per-invocation** satisfies non-identity always and **vacuously** — a model
  judges text it produced minutes ago, and every check in this system passes —
  refused as the constant-arrow hazard
- **per-session** makes π an orchestration fact rather than a principal fact —
  refused as incoherent

The per-invocation reading was the dangerous one: it would have shown up as 47
propositions settling smoothly and meant nothing.

**What the ruling did not supply was the carrier — and now
[Q16](../questions/resolved/q16-provenance-carrier.md) has supplied its
definition.** An outside ruling (2026-08-05) adopts the **commission
contribution record** as the carrier: `authored(p) = ⋃_{c∈S} C(f,c)`, with
`C : Family × CommissionId → P_fin(ArtifactId)`, commission boundaries harness
state, and prior commissions entering `S` only by explicit supplier decision.
The two forbidden shapes are refused (a guessed static list; a source needing
its own judgment).

**What remains open is the *implementation* of that record
([Q17](../questions/open/q17-provenance-carrier-implementation.md)).**
`population.yaml` still declares empty `authored` for every model — an
*acknowledged, temporary placeholder* (a test pins it so it cannot ship quietly
as a working configuration). The loose end is not the definition but a `C` the
pool can actually read; until it exists and is wired in, a model judge still
qualifies vacuously under the only provenance the pool can read, and **nothing
guessed is written into `population.yaml`**.

**The escalation chain's definitional wall is cleared and so is the carrier's
definitional wall; the implementation wall is not.** A human last-resort
presumes something above him, and the only candidates are models — but a model
can only be admitted once the record makes its stake real.

### 2.2 — One `dispatched` judgment record

The first thing to build after Q14. `rung-driver` already consults a pool and
reads a reply into a verdict; what is missing is writing the outcome as a record
in `judgments/` with `tier: dispatched`, carrying the judge's provenance out of
the sealed `Judgment` rather than out of a field.

That is the difference between a **receipt** and a **judgment**, and it is the
whole reason the tier exists.

Suggested first subject: one of the 22 categorical propositions in
`rung-ct-props.md`. They are self-contained, a wrong ruling is cheap, and there
is precedent — Q7, Q9 and Q10 were each settled by outside review, and **Q7's
ruling overturned the account**.

### 2.3 — An edit vocabulary over code

`DoctrineEdit` exists and edits the *doctrine*. Nothing edits the
*implementation*, which is what the primary loop rectifies.

The hard part is not the enum. It is that a code edit has no round-trip check.
For a doctrine edit, the typed edit is the specification and rendering proves
the author did exactly that and nothing else
(`an_editor_that_does_more_than_the_edit_is_caught`). For a code edit the
equivalent is: apply, then re-run the audit and confirm exactly the intended
propositions changed state. Weaker, and the weakness should be stated rather
than papered over.

### 2.4 — Q15, or the pass cannot wait

[Q15](../questions/open/q15-does-the-pass-suspend.md): `het_pass!` disposes
through a **branching** transition, and G16's residual channel is on *forward*
transitions only. So `RaisesQuestions -> Audited` re-enters immediately and the
pass cannot wait for a question it raised.

Consequence, and it is the reason this matters: without suspension the only way
to wait is to re-audit and find the same violation again — a spin. Every rule
that stops a spin is worth-shaped. So the missing channel does not leave waiting
*unimplemented*; it forces waiting to be implemented **as policy**, in a driver,
where nothing states it and no mutation reaches it.

`Park` is deliberately incapable of holding such a policy, which is why the pass
and the park cannot currently be connected.

### 2.5 — The run loop

Only after the above. Four moves, each requiring a principal the previous move
disqualified. `rung-driver` is the place; it is domain-blind by construction and
must stay that way.

---

## 3 · The first milestone worth aiming at

Not the full loop. **One cycle, end to end, on the smallest real subject.**

1. `audit` reports a judgmental proposition as unsettled
2. a model judge, admissible under Q14's ruling, is dispatched
3. its ruling is written as `tier: dispatched` in `judgments/`
4. if **non-conforming**, an author with standing over `rung-doctrine/src`
   proposes a `DoctrineEdit`
5. a *different* judge disposes
6. the author enacts; `render` regenerates; the round-trip proves the edit was
   the edit

Every step but 2 and 3 exists. That is the shortest path to the README's number
becoming one.

---

## 4 · Standing rules a successor should not relax

These were learned expensively.

- **A test that cannot fail is not a proof.** Run the mutation, watch it redden,
  restore byte-for-byte. `(rustc)` was carried as a proof for seven propositions
  and established nothing.
- **A proof must run.** An `#[ignore]`d test cannot fail. This was got wrong
  once *one commit after* explicitly avoiding it.
- **Numbers in prose are derived or pinned.** A hand-written table drifted by
  two, in the note explaining why numbers should not be hand-written.
- **One source of truth.** `conformance.md` and the doctrine held the same fact
  for a day and diverged in six places.
- **Report unfiltered.** The 173 unclaimed proofs include tooling tests that
  will never cite a proposition. Excluding them by name would be the quiet
  narrowing that makes a queue look shorter than it is.
- **Do not fabricate evidence.** `judgments/` is empty because nobody has ruled.
  A plausible example record would be the sharpest available version of the
  mistake this whole collection exists to prevent.

---

## 5 · The failure mode, named

It recurred roughly six times in two days and a successor will meet it:

> **Building the structure and mistaking it for the thing.**

Instances: a census of 380 "suspended runs" constructed by hand and parked,
which no dispatch would ever have produced; the doctrine encoded as a theory
*about markdown* rather than about the implementation; a `!is_serializable()`
stub returning literal `false`; a cross-crate fixture whose two "contrasting"
paths were the same call.

The tell is identical every time: **a lot of shape, and nothing having actually
passed through it.** The check is to ask what would be different if the
machinery were not there.

---

## 6 · Open questions

| | | status |
|---|---|---|
| Q14 | what provenance a model principal carries | **resolved** — definition ruled |
| Q16 | what carrier makes `authored(p)` a derived fact | **resolved** — commission contribution record; implementation open (Q17) |
| Q17 | implement the commission contribution record | open — the implementation gate |
| Q15 | does the pass suspend, or re-enter | open |
| Q11 | gate-faithfulness; `#[conditional]` unencoded | open |
| Q13 | suspension across process death | open |
| Q1, Q4, Q5, Q6, Q8 | body correctness, composition, fork-join, genericity, async | open |
| Q2 | cross-crate provenance | parked — *the non-guarantee is now proven* |
| Q3 | true linearity | blocked on the language |

Q2 is worth a look: `rung-fixture` now proves the non-guarantee by exercising
it, so when a sub-crate-per-ladder closes the gap the fixture stops compiling —
which is the signal to retire the proposition rather than a regression.

---

## 7 · What would falsify the goal

Stated so it can be abandoned honestly rather than quietly.

- **No carrier can be built that keeps non-identity real.** Q14 and Q16 have
  ruled on the definition; if the commission contribution record Q17 builds
  cannot be made decidable, non-vacuous and non-total at once, then models
  cannot judge this corpus, the escalation chain has nothing above the human,
  and "last-resort" is unreachable. The loop still runs for the decidable
  fragment; the judgmental one stays declared and unsettled indefinitely.
- **A code-edit round trip cannot be made to bite.** Then `enact` over the
  implementation is unverifiable, and an author agent is a code generator with
  extra ceremony.
- **Nobody routes through it.** If changes keep arriving as direct commits
  because the loop is slower, the goal has failed on cost rather than on
  mechanism — and cost is HetOpt's, which does not exist.
