---
id: q15
status: open
depends_on:
  - {on: q4, kind: premise}
affects:
  - {target: het-pass-disposition-vocabulary, kind: gate}
---

# Q15 — Should raising a question suspend the pass, or re-enter it? *(open)*

**Status:** OPEN

**Question.** `het_pass!` disposes through a judgmental **branching**
transition, and `RaisesQuestions` is a continue arm:

```
=> #[judgmental($judge)] {
       Accept($crate::Licence<$edit>)
     | RejectDiagnosis($crate::Why)
     | RejectRemedy    -> Proposing
     | Defer           -> Proposing
     | RaisesQuestions -> Audited
   }
```

A continue arm re-enters **immediately**, carrying a live token.
[`G16`](../../rung-props.md#g16-the-residual-channel)'s residual channel is on
judgmental **forward** transitions only, so the pass has no `Suspended` and no
`resume` edge. `RaisesQuestions { question }` carries the reference and waits on
nothing.

> **Can a pass wait for a question it raised — and if so, does the residual
> channel belong on a branching transition too?**

## Why this is not a detail

Without suspension the only way for a pass to "wait" is to **re-enter and audit
again**, with the question still outstanding, finding the same violation and
proposing against it again. That is a spin.

The only way to stop a spin is a rule about when to retry — and **every such rule
is worth-shaped**. How long to wait, how often to re-audit, whether this question
is worth blocking on: those are exactly the judgments
[`het-declares-no-worth-law`](../../rung-het-props.md#het-declares-no-worth-law)
and [`ordering-is-hetopts`](../../rung-het-props.md#ordering-is-hetopts) place
outside Het.

So the absence of a residual channel on the branching transition does not leave
waiting *unimplemented*. It forces waiting to be implemented **as policy**, in a
driver, where nothing states it and no mutation reaches it. That is the pattern
this repository keeps finding — a rule that lives only in code that enforces it —
and here it would be introduced by an omission rather than by an assertion.

`rung_std::driver::Park` is deliberately incapable of it: no ordering, no
timeout, no retry rule. Which means that today, **the pass and the park cannot be
connected at all.** The mechanism for waiting exists and the pass has nothing to
hand it.

## The three shapes, and why only one is served

| where a question arises | what is left waiting | held by |
|---|---|---|
| judgmental **forward** transition | the argument, unconsumed, as `Suspended<Prev>` | `Park` |
| judgmental **branching** transition — *the pass* | nothing; the continue arm consumed the token and moved on | **nothing** |
| a `theory!` sentence | nothing consumed; a sentence borrows its model | re-consult later |

The third needs no channel: a sentence takes `&model`, so nothing is lost and
re-consulting later is free. What is lost is only the *record* that something was
waiting, which is bookkeeping.

The second is different, and that is the whole question. `Proposed` **is**
consumed — the continue arm advances the run to `Audited` rather than handing
anything back. So the pass does not merely forget that it was waiting; it has
already moved.

## Candidate answers

Stated so none is discovered late. Each is a change to doctrine, not to an
implementation, which is why this is filed rather than built.

1. **Extend the residual channel to branching judgmental transitions.** A
   `Result<StepOutcome, Suspended<Prev>>`, with `RaisesQuestions` becoming a
   suspension rather than a continue arm. Symmetric with the forward case and it
   is what would let the pass and the park meet. **Cost:** it touches the
   coproduct's shape, and
   [`elimination-is-exhaustive`](../../rung-ct-props.md#elimination-is-exhaustive)
   and [`residual-summand`](../../rung-ct-props.md#residual-summand) both have
   something to say about a second residual on the same arrow. Not obviously
   free.

2. **Rule that immediate re-entry is correct**, and that waiting is not the
   pass's job — a raised question is recorded, the pass re-audits, and the
   question's terminal is picked up by whatever audits next. Coherent, and it
   makes the pass a pure reducer. **Cost:** it concedes the spin, and pushes the
   retry rule outward to something that must then be honest about holding a worth
   law.

3. **Rule that a raised question ends the pass.** `RaisesQuestions` becomes a
   *terminal* verdict rather than a continue arm — the pass stops, the question
   runs, and a **new** pass begins when it terminates. **Cost:** the chain is
   lost across the boundary, so
   [`reproposal-carries-the-chain`](../../rung-het-props.md#reproposal-carries-the-chain)
   would not hold across a raised question, and a re-proposal after an answer
   would be indistinguishable from a fresh start. That is the same objection the
   composition note raises against the question lifecycle's missing re-proposal
   arm, one level over.

## What would count as an answer

A ruling on which of the three the pass does, with the doctrinal consequence
written out — because each one changes a different document. (1) changes
`rung-props.md` and the macro; (2) changes nothing and records a limit; (3)
changes the disposition vocabulary in `rung-het-props.md`.

An answer that says *"the driver will handle it"* is not an answer. That is
option (2) with the cost unstated.

## Why this is the tightest self-hosting test available

The questions theory is **closed under its own question-raising**: any theory's
deferral produces a question, and a question is a subject of the questions
theory — including the questions theory's own deferrals. Q11 raising Q12 is the
lived instance.

So the pass over *questions*, raising a question, has the same theory on both
sides of the composition boundary. Nothing has to be built twice, and if the
loop closes there it closes anywhere. That makes this question the shortest path
between the machinery as it stands and the loop running on itself — which is
what [`composition-notes.md`](../../composition-notes.md) is about.

## Relation to neighbours

- **[Q4](q4-composition-nested-ladders.md)** — the premise. This is Q4's outer
  arrow with a concrete shape: the outer arrow is a disposition and the inner run
  is a question lifecycle.
- **[Q13](q13-suspension-across-process-death.md)** — orthogonal. Q13 asks
  whether a suspension survives a process boundary; this asks whether the pass
  produces one at all. Answering this does not touch Q13, and Q13's answer does
  not decide this.
- **[Q5](q5-fork-join-concurrency.md)** — a disposition raising *several*
  questions at once is fork-join, and should stay there. This is one arrow
  awaiting one run.

## State

- **2026-08-04** — Filed. Found while building `rung_std::driver::Park`: the
  park was complete and mutation-verified before it became clear the pass had
  nothing to hand it. The diagnosis first offered was too narrow — *"the pass's
  judgmental transition is branching"* — and the general form is the three-shape
  table above.
