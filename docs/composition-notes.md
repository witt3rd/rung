# Theory composition — what the loop needs to run itself

**Status: informative.** `rung-props.md`, `rung-ct-props.md` and
`rung-het-props.md` govern. This note records a shape the system does not yet
have, why it is wanted, and what would have to exist. It states no proposition
and imposes no obligation.

---

## 1 · The observation

On 2026-08-04 the audit-rectify pass ran end to end on a change to Het's own
doctrine. A question was posed (Q12). An outside party rendered a ruling on it.
The ruling was audited, a P0 violation was found *in the ruling itself* — it
certified its own standing — the author accepted the correction and reissued
within jurisdiction, and the substantive contribution survived and improved on
what had been proposed from inside.

That is the pass, exactly: `audit → propose → dispose`, with a `reject-remedy`
and a re-proposal in the middle. It is the loop the formalism describes, run at
full fidelity, on the formalism.

**Almost none of it passed through the machinery.**

## 2 · What did fire

Two decidable sentences, and they were not decorative. Adding Q12 turned the
pinned question-count assertion red — the theory noticed its own corpus grow
before anyone told it. And `outbound_drift` would have reported the new edge as
unmirrored had the `affects` not been added, which is why it was added.

Everything else was carried by hand.

## 3 · Why the rest could not be

**The ruling arrived out-of-band.** `propagate` qualifies an adjudicator through
the pool, and that call is where non-identity is enforced. A judge dispatched
through it cannot certify its own standing, because the pool decides
qualification and the judge never sees the decision. This ruling arrived as
prose. The theory governs dispatches it mediates; it cannot govern a principal
consulted by hand.

**The verdict is a parameter.** `propagate(e, pool, ruling: Verdict)` takes the
outcome from its caller. The theory records that an adjudicator qualified; the
ruling itself is supplied. That is the constant-arrow hazard, and closing it is
what Q12 settled.

**The lifecycle has no re-proposal arm.** `QuestionLifecycle` runs
`Open → Gathered → Drafted → { Resolved | Dissolved | Blocked → Gathered |
Parked → Gathered }`. `Drafted` *is* the proposal and `step` *is* the
disposition, so the shape is present. But a judge who rules *"this is wrong,
redo it, here is why"* has only `Blocked` or `Parked`, and neither carries a
reason or a chain of prior dispositions. The exact move that occurred has no arm.

**Two theories ran at once and only one was written down.** `QuestionLifecycle`
governs a question's *status*. The pass governs a *proposal about a subject*,
with the five-disposition vocabulary and the chain. What ran was the **pass**,
applied to a doctrine change, while the **question** sat in the questions theory
holding the topic. Q12 is a question; the disposition on it is a pass over a
proposal about a normative document. Nothing connects the two objects.

## 4 · The shape wanted

> Work proceeds in one theory. A step in it cannot be settled there. That step
> **gives rise to a question**, which is a subject of another theory and runs
> that theory's lifecycle. When the question terminates — resolved, dissolved,
> or abandoned — its terminal **returns to the originating theory as the answer
> to the step that raised it**.

Two theories, composed, with the inner run *witnessing* an outer arrow.

This is not a new categorical object. It is Level 1 of the growth tower —
arrows in **Cat**, functors between ladders — and it is what Q4 names. The
outer arrow `step-cannot-be-settled → step-settled` is witnessed by the inner
ladder reaching a terminal. The dependency opfibration
(`dependency-structure-is-an-opfibration`) is a *sibling* Level-1 structure over
the same base, not this one: it transports a change between whole subjects,
where this suspends one arrow pending another category's run.

### Where the pieces already are

- **The suspension is the residual.** A branching transition already has the
  shape `A → Σᵢ Bᵢ + A`, and `residual-summand` is the input returned
  unconsumed when the transition does not answer. *"Blocked pending a question"*
  is a residual carrying a question id, not a new construct.
- **The resumption is a backward edge.** `error-dagger-is-optional-and-unguarded`
  already permits re-entry from the residual with no progress guard, which is
  the right discipline here: a question may take any number of rounds.
- **The edge kind exists.** The questions theory declares `spawn` — *the
  dependent exists only because of this* — and classifies it `Generative`,
  gated authorial. That is exactly the outer-to-inner relation.
- **The tower is the justification.** `fractal-property`: an algebra whose
  carrier holds subjects that carry their own signatures becomes a theory at the
  next level. A question is a subject of the questions theory *and* the thing an
  outer step awaits.

## 5 · What must exist

Ordered by what blocks what.

1. **The outside must supply the verdict.** Until a ruling is *obtained* rather
   than *recorded*, composition would only propagate an unchecked value across a
   theory boundary — the constant arrow, one level up. Settled by Q12; being
   built.

2. **The human and the outside expert must be principals.** This is the one that
   makes the difference between a demonstration and a system. Today they are
   channels *around* the machinery: consulted by hand, their answers pasted in.
   They must be inhabitants of the pool with declared capability and provenance,
   dispatched to through `qualify_for`, returning verdicts through the sealed
   envelope. Then a ruling that certifies its own standing is not a mistake to
   catch in review — it is a term that cannot be constructed.

3. **The lifecycle needs a re-proposal arm.** A `RejectDraft → Drafted` continue
   arm carrying a chain of prior dispositions and their reasons, so a redraft is
   a *re-proposal* and not a fresh start, and so `reproposal-carries-the-chain`
   holds at this level as it does in the pass.

4. **A suspension outcome, and a resumption edge.** An outer transition needs a
   summand meaning *"raised a question, awaiting its terminal"*, carrying the
   inner run's identity; and an edge back that consumes an inner terminal and
   resumes the outer arrow. The residual carries the first; the unguarded
   backward edge carries the second.

5. **Identity across the boundary.** The inner subject must know what raised it
   and the outer must know what it awaits. Provenance is the obvious carrier,
   and after the provenance floor
   (`π(p) ⊇ {id(p)}`) it is non-empty by construction on the principal side.

6. **The gate law must hold across the boundary.** `no-laundering-along-morphisms`:
   an outer step that is judgmental must not become decidable by being answered
   by an inner run that never consulted anyone. The composition has to preserve
   the marker, or it becomes the most elegant available way to launder a gate.

7. **Termination is not promised, and should not be faked.** The inner run may
   never terminate — `no-bound-on-reentry` is a stated limit, and it surfaces
   here as an outer arrow blocked forever. Any bound is worth-shaped and belongs
   to HetOpt. The composition should make the block *visible*, not resolve it.

## 6 · What this does not need

Not a second level of the tower. `iteration-not-a-second-level`: opfibrations
compose as 1-cells, and stacking a theory on a theory stacks 1-categorical
bricks. A genuine Level 2 needs a 2-cell *between* fibrations, which nesting does
not introduce. If a design for this composition reaches for natural
transformations, that is a signal it has misidentified what it is building.

## 7 · Where it sits against the open questions

- **Q4** (composition / nested ladders) — this *is* Q4, with a concrete
  motivating instance rather than a hypothetical one. The instance is worth
  recording on it: the inner run is a question lifecycle and the outer is the
  pass over doctrine.
- **Q11** (gate-faithfulness) — item 6 above is Q11's gate law meeting a new
  boundary. Composition is a place a gate could be laundered that does not exist
  today.
- **Q12** — item 1 is Q12's ruling, and composition is a reason it mattered
  beyond the single case.
- **Q5** (fork-join) — a step raising *several* questions at once is fork-join,
  not this. Keep them apart: this is one arrow awaiting one run.

## 8 · The honest summary

The loop ran at full fidelity and the machinery watched two decidable sentences
go by. That is not a failure of the theory — every gap above is either a stated
limit or a filed question. It is a statement about where the boundary currently
sits: **the system governs what passes through it, and the interesting work
passed around it.**

The single change that moves that boundary most is item 2. Everything else is
mechanism; that one is about who is inside.
