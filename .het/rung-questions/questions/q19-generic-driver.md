---
id: q19
status: open
filing: ill-posed
depends_on:
  - {on: q18, kind: premise}
affects:
  - {target: self-hosting-run-loop, kind: gate}
ill_posed: |
  As posed this is a design decision and work item, not a determinate qu
  estion: it specifies what to build or decide rather than naming a fact
   the structure determines. The real judge refused it repeatedly on the
   authentic (and, here, the unique) cut, and authorial repair attempts 
  were likewise refused. Filed Mode B: tracked as a decision/work item, 
  not claimed as a well-posed question.
---


# Q19 — The generic driver: a theory-blind engine every theory runs on *(open)*

**Status:** OPEN

**Question.** The operational machinery of the loop — the audit-rectify pass,
the discipline of judgment and authoring, questions and their resolutions,
bookkeeping, suspend/resume, and how each principal *kind* (LLM, agent,
relational being, human) is reached — is not something a theory author should
ever write. It belongs to **one generic driver**: a library and CLI that can be
pointed at a governing theory and applied to a carrier instance.

> **What is the single generic driver, and how does a theory instantiate it?**

## Why this deserves its own question

Q18 asks *where the state lives* (the `.het/` config + carrier convention). This
asks *what drives it* — the engine that reads that config and runs the loop,
theory-blind. Coupled but distinct: the driver is the reusable machinery; the
config says which theory, which carrier, which population a given run uses. And
this question carries an explicit end-state: **after it, there are no more
hand-rolled drivers or driver fragments.** Every test and every piece of
self-hosting runs through the generic driver.

## What the driver must own (no theory author ever sees this)

- the **audit-rectify loop** — audit, propose, dispose, enact, and the gates
  between them;
- the **discipline of judgment and authoring** — disjoint judges, standing
  authors, who may act on what;
- **questions and their resolutions** — raising, deferring, resuming;
- **bookkeeping** — the dispatched-judgment record, the activity log, the
  evidence trails;
- **suspend/resume** — the park and the continuity it holds;
- **principal interaction** — reaching an LLM, an agent, a relational being, or
  a human through one dispatch abstraction;
- **carrier walking** — moving over colocated and external carriers via
  element iterators, from config, not constants.

A theory fills the slots (sorts, edits, sentences, roles, the pass ladder); the
driver supplies everything else.

## What already exists — the driver fragments, and what each proves

**The hard parts are proven, in tests, scattered.** This is not greenfield. The
basic driver is already present across four fixtures plus the supporting
machinery; Q19 is their composition, not their invention.

| fragment | where | what it proves |
|---|---|---|
| the same pass over two domains | `rung-het/tests/acceptance.rs`, `rung-het/tests/second_domain.rs` | the loop **is generic**: the cabinet and issue triage (`Fix \| WontFix \| Duplicate \| Reprioritize`) run through one unchanged `rung-het`. If the library had to learn `WontFix`, it was never generic. |
| the park / suspend-resume | `rung-std/tests/driver.rs` | the residual channel composed: many suspended runs held at once, evidence arriving out of order, each released when (and only when) its matter terminates. |
| the pass **as a `ladder!`** | `rung-het/tests/pass_ladder.rs` | the audit-rectify spine declared as a typed ladder (a gate on every arrow) instead of hand-rolled free functions. |
| the full-cycle seam | `rung-driver/tests/rectify_questions.rs` | audit -> propose -> dispose -> enact over rung's own `questions/` through the pool — the join that was missing, proved with a deterministic local judge. |

Supporting machinery already in crates: `het_pass!`, `dispose`, `enact`,
`Proposal`, `Disposition`, `Applies` (`rung-het`); `Park` (`rung-std::driver`);
`ModelOracle`, `agent`, `tools` (`rung-std`); population -> pool (`rung-driver`).

## The composition gap

What exists is powerful but **scattered as test fixtures**. The sharpest
illustration of the gap is `rectify_questions` — it *is* a runnable driver
instance, but a hardcoded one: questions theory, rung's own `questions/`, fixed
Scheme, a `Holding` judge, no config, no bookkeeping written to `judgments/`.
That is exactly the special case the generic driver must make impossible.

**Missing pieces to build:** the carrier layer (folder/jsonl/file/csv/github
backends + element iterators, port from `../.archive/het-rs/src/carrier/`);
instance `config.yaml` (theory, scheme, carrier, population); the composed,
runnable engine with a CLI; bookkeeping (dispatched judgments, activity log,
persistent park per [Q13](q13-suspension-across-process-death.md)); one dispatch
abstraction across all `Kind`s; and the per-theory-crate instantiation that
replaces every hand-rolled fragment.

## The instantiation pattern (the point)

A new theory is a **new Rust crate** that imports `rung*` and uses the macros
(`ladder!`, `theory!`, `het_pass!`). It is *not* data the driver interprets; it
is Rust that **constructs and runs an instance of the generic driver** over its
carrier(s):

```text
theory crate (rung*, macros) --instantiates--> generic driver --reads--> config.yaml
                                                      --drives--> carrier, population
```

## Acceptance: no hand-rolled driver survives

Q19 is done only when the following all run through the generic driver and the
hand-rolled fragments are gone:

- the multi-domain pass tests (`acceptance.rs`, `second_domain.rs`);
- the park-composition test (`driver.rs`);
- the typed pass (`pass_ladder.rs`);
- the full-cycle seam and the `rectify_questions` binary — replaced by the
  questions-theory crate instantiating the driver over
  `.het/rung-questions/config.yaml`;
- rung's own self-hosting bootstrap.

And rung's bootstrap becomes the **first instance of the pattern**: the
questions theory instantiates the driver pointed at its config and folder
carrier, proving the shape the same way users will.

## What rests on it

The **self-hosting run loop** goes generic through this driver. A theory author
must never re-implement the loop, grow a special case, or hand-roll a driver
fragment — anything that would make Q19's acceptance false is the bug.

## Relation to neighbours

- **[Q18](q18-het-state-sidecar-convention.md)** — the premise, above. Q18
  decides where state and config live; the driver reads them. The driver
  architecture is constrained by Q18's config/carrier convention.
- **[Q15](q15-does-the-pass-suspend.md)** — the driver's suspend/resume story
  leans on whether the pass can wait; the driver composes what Q15 decides.
- **[Q13](q13-suspension-across-process-death.md)** — a driver that restarts
  needs a park that survives death; the driver's continuity is bounded by Q13.
- **[Q4](q4-composition-nested-ladders.md)** — cycles compose; the driver is
  where the composed loop lives, one level above a single pass.

## State

- **2026-08-05** — Filed. Raised while deciding how to structure the driver
  work. Expanded to capture the full picture: the driver's machinery already
  exists, proven across four test fixtures, but is scattered rather than
  composed; the end-state is that **no hand-rolled driver or fragment
  survives** — every test and all of rung's self-hosting run through one
  theory-blind engine. [Q18](q18-het-state-sidecar-convention.md) is the
  premise: this driver reads that convention's config and drives its carriers.

