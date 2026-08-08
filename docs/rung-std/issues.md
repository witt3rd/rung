# issues — a theory of work items

**Status: informative, not normative.** This document introduces the `issues`
theory — the canonical theory over a body of **work items** (tasks, defects,
requests) — from the ground up. It is a reading companion, not the law; the
normative statements live in [`rung-het-props.md`](../rung-het-props.md) and
the authoritative code in [`rung-std/src/issues.rs`](../../rung-std/src/issues.rs).

`issues` is the natural counterpart to [`questions`](questions.md): where the
questions theory governs *questions* (claims whose answer the structure finds),
`issues` governs *work* (tasks whose completion someone enacts). And it is the
answer to the question the questions theory keeps raising: when a question fails
the **authentic cut** — *"this is not a determinate question; it is a decision or
a piece of work"* — the correct destination is an **issues instance** of the
same project. That hand-off between two theories' carriers is the
[cross-theory rectification](../theory-of-theories-in-context.md) at the heart of
the theory of theories in context.

---

## 0 · The need, in one paragraph

A body of work items — a bug list, a task board, an issue queue — is not a pile
of prose: it is a set with a law. Who may file an issue, what counts as a
*well-scoped* piece of work (one bounded task with a reachable definition of
"done"), how an issue moves from *open* to *resolved* or *closed*, and who is
allowed to do each. Untended, such a set decays: issues become vague wishes,
"done" means whatever the closer meant, and triage is guesswork. `issues` is a
complete Het theory over exactly such a set: it names the sorts, declares the
gate-marked sentences that judge them, provides the lifecycle and the edits,
and fills the roles. It knows no particular body of issues; every deployment
supplies its own ids, files and statuses as parameters.

## 1 · What an issue is

An **issue** is a **work item**: a bounded, well-scoped task. It is deliberately
*not* a question. A question asks something the structure can answer; an issue
asks someone to *do* something. The distinction is the whole reason this theory
exists — it is the honest home for all the things that were filed as questions
and turned out to be work (`authentic` cut).

The theory is deliberately lean:

- **One sort** — an `Issue`: an id, a status, and a body (the raw prose of the
  work item).
- **A flat, self-describing carrier** — status is frontmatter, not a folder, so
  the set is a flat pile of `.md` files. Any concrete set (a GitHub repository,
  a board, an email queue) is a carrier; `GitHubIssuesCarrier` is one backing.

## 2 · The lifecycle

An issue moves through a small set of statuses:

```text
open → triaged → in-progress → resolved
  ↘  └────────────────────────────┘
     → closed (or wontfix)  →  reopened
```

- **`Triage { to }`** — move an issue to a declared status (the target's own law
  refuses an undeclared one).
- **`Resolve`** — the work is done. **`Close`** — done for good.
- **`Reopen`** — bring a closed issue back.

Each is an **authorial** edit by a `Triager` who holds standing over the
container; the observer reads the post-state back (the edit is verified, not
believed).

## 3 · The sentences

| sentence | gate | what it checks |
|---|---|---|
| `id_matches_the_filename` | decidable | the frontmatter id names its own file |
| `status_is_declared` | decidable | the status is one of the six |
| **`well_scoped`** | judgmental (by a `Reviewer`) | an issue is *one bounded task with a reachable "done"* — not two tasks, not a wish |

Then, as ever, only the recognizable footprints are decidable and the rest — is
this *really* a clear, bounded piece of work? — is a judgment, not a check.

## 4 · The roles

| role | what it does | kind |
|---|---|---|
| **Triager** | files, moves, closes issues | authorial |
| **Reviewer** | rules on `well_scoped` | judgmental |

## 5 · Why it earns its place in rung-std

`questions`, `principals` and `issues` are the three canonical suppliers of the
two halves of Het. `issues` recurs across every domain that tracks work — and it
is the **intake destination of a relegation**: the questions theory, finding a
question not-well-posed, discharges it and (through the catalog / router) routes
it to an issues instance, which re-audits it under *its* law (`well_scoped`)
before admitting it. That is the first real cross-theory rectification a Het
institution can perform.

## 6 · Where things live

| you want | go to |
|---|---|
| the theory and the model | [`rung-std/src/issues.rs`](../../rung-std/src/issues.rs) |
| the theory's tests over a synthetic docket | [`rung-std/tests/issues_theory.rs`](../../rung-std/tests/issues_theory.rs) |
| the cross-theory rectification design | [`docs/theory-of-theories-in-context.md`](../theory-of-theories-in-context.md) |
