---
id: q18
status: open
depends_on:
  - {on: q13, kind: premise}
affects:
  - {target: self-hosting-run-loop, kind: gate}
  - {target: q19, kind: premise}
answerable: |
  A state-sidecar convention is well-posed iff all of a carrier's loop state
  (config, population, commission record, subjects, judgments) has one
  declared, restartable home per instance, and a second carrier gets that same
  shape rather than a new ad-hoc layout.
---

# Q18 — What is the state sidecar convention? *(open)*

**Status:** OPEN

**Question.** A theory applies to many carriers, and the audit-rectify cycle
needs state to run, restart, and leave a record. Today that state is scattered
across a repository root as a matter of habit: `population.yaml`,
`commissions.yaml`, `questions/`, `judgments/`. There is no declared convention
tying them together, and nothing that says how a *second* carrier (say, GitHub
issues, or a project portfolio) would keep its own.

> **Where does the state a self-hosting loop needs live, so that it can
> restart, log its activity, hold its configuration, name its judges and
> authors, and keep its evidence trails — per carrier?**

## Why this is new

No existing question decides this. Q14/Q16/Q17 fixed the *provenance* (what a
principal carried, its carrier, its build); Q13 asks whether a suspended run
can survive process death. Neither asks where the *whole* body of state lives.
This is a layout-and-restart question: the container that the loop's continuity
would sit in.

## What the convention must hold, per carrier

For the loop to crash-and-resume it needs, on disk and structured:

| piece | role | today |
|---|---|---|
| the **carrier** (subjects) | what gets audited | `questions/` |
| **config** (Scheme: namespace, root, id prefix) | which theory instance this is | inline in tests/binaries |
| the **population** (judges/authors, roles, providers) | who may act | `population.yaml` |
| **provenance** (commissions) | the judging gate (P0) | `commissions.yaml` |
| **judgments / evidence** (rulings, trails) | the record a cycle closed | `judgments/`, `**/_evidence/` |
| the **park** (suspended runs) | restart | in-memory only — [Q13](q13-suspension-across-process-death.md) |
| a **log** of activity | restart, audit trail | nowhere structured |

## Two axis of flexibility (deliberately not fixed here)

1. **Is the carrier colocated or external?** Either. Colocate when the carrier
   *is* a body of files this loop keeps (`questions/`, a `portfolio.jsonl`);
   point to it when it lives elsewhere (a real GitHub repository's issues). The
   convention should say "colocate by default, look up when external" — not
   force one.
2. **Is the population shared or per-carrier?** Either. A population (an org's
   maintainers, reviewers) can serve many carriers and deserves a shared home;
   a bespoke population can sit inside an instance. The configuration should be
   explicit about which, rather than it falling out of where someone happened
   to write `population.yaml` first.

A single instance's layout (one candidate — and every instance may differ,
which is what its `config.yaml` says):

```text
.het/
  <instance>/            e.g. gh-issues, rung-questions, portfolio
    config.yaml          # theory, scheme, carrier location, population shared|bespoke
    carrier/             # colocated subjects (questions/, portfolio.jsonl)
                         #   — absent when the carrier is external (GitHub issues)
    evidence/            # judgments and evidence trails
    park/                # suspended runs — restart (Q13)
    log/                 # one entry per audit -> propose -> dispose -> enact
  population.yaml        # shared population, if any (a bespoke one lives in <instance>/)
  commissions.yaml       # provenance, if shared at this level
```

## What rests on it

The **self-hosting run loop** — the part of rung's own bootstrap that would
*recover* as well as run — is gated on a place to keep its state. Without a
convention, a second carrier has nowhere to keep its population, provenance,
and record, and the loop's continuity is an accident of which directory first
held which file. This is infrastructure, not the theory: it must not hard-code
any particular carrier's shape.

## Relation to neighbours

- **[Q13](q13-suspension-across-process-death.md)** — the premise, above. The
  sidecar's `park/` can only make the loop *restart* if a suspended run can
  survive process death at all; Q13's answer bounds what "restart" here can
  mean.
- **[Q17](q17-provenance-carrier-implementation.md)** — the
  commission record Q17 built is one piece of the state; this asks where it
  (and the rest) lives. Independent: the carrier can be built anywhere, but the
  convention decides where it is *kept*.
- **[Q16](q16-provenance-carrier.md)** / **[Q14](q14-model-principal-provenance.md)**
  — fixed the provenance; this fixes the provenance's *home*.
- **[Q2](q2-cross-crate-provenance.md)** — unrelated despite the
  shared word. Q2 asks whether a *token* survives a crate boundary; this asks
  where a loop's *state* is kept.

## State

- **2026-08-05** — Filed. Raised while drawing the audit-rectify cycle for the
  README: the cycle needs per-carrier state to run and restart, and no
  convention names where it lives.
- **2026-08-05** — Corrected. Colocated/external and shared/bespoke are
  **conjunctions the convention must support for every combination**, selected
  per instance — not axes to choose between. The per-carrier `config.yaml` is
  the handhold that makes the generic driver domain-blind: given an instance's
  config it knows the governing theory, the carrier's location (colocated or
  external), and where its population lives. The carrier backends and element
  iterators to port are the archived het-rs prototype's
  (`../.archive/het-rs/src/carrier/`: folder, jsonl, file, csv, github).
