# AGENTS.md — rung

A type ladder: the state machine *is* the type system. Declare rungs and
transitions once; the compiler refuses any path that skips a rung.

This repo is **dotagent-inhabited**. The handoff channel is the event log
(`.agent/log/`) via `scripts/agent`. **Do not create a `HANDOFF.md`.** That
file is retired and taboo here. Orient with `scripts/agent state`, see what
is waiting with `scripts/agent inbox`, hand off on sleep with
`scripts/agent handoff <subject>`.

## Goals (the problem)

You encode state machines by hand. A work item moves `Spec → Designed →
Claimed → Active → Complete`. Each stage should only be reachable through
the transition that produces it. Sealed constructors, private fields, runtime
guards, convention, and review do not make that a compile error. The machine
lives in comments and hope.

rung makes a skipped step a compile error. The only way to hold a `Claimed`
token is to go through the transition that produces it.

## Merits (what is load-bearing)

- **The compiler is the gate.** A skipped transition, a dropped token on an
  error path, a non-exhaustive match on verdicts — compile errors. No runtime
  guards for the graph.
- **The type is the evidence.** Mid-ladder constructors are sealed and
  module-private (G2). You cannot fabricate a verdict from outside.
- **Linear consumption.** Tokens move by value, `#[must_use]`, `!Send +
  !Sync`. Carry is immutable (G5). Recover edges are paired (G7/G9).
- **Normative documents govern.** `docs/*-props.md` is law; `docs/*-notes.md`
  is derivation. Where they disagree, props wins. Do not hand-edit generated
  props or `docs/conformance.md`.
- **Kernel vs product.** `rung-std` admits recurrent, domain-generic blocks
  (J2). Session catalogs, resume, isolation worktrees, background spawn, and
  XDG config belong in a product crate, not the kernel.
- **No credential in a committed file.** Providers name `api_key_env`;
  `~/.rung/auth.yaml` is machine-local.

## Concepts

- **`ladder!`** declares arrows (rungs, transitions, recover). The verb lives
  on the arrow (`the-law`).
- **`theory!`** declares sentences: decidable (a machine settles them) or
  judgmental (an outside with disjoint provenance settles them).
- **`rung-std`** is the canonical blocks: `llm`, `agent`, `python`, `tools`,
  `questions`, `principals`, `driver`.
- **`rung-het`** is the two-filter pool (judge vs author) over one population.
- **`rung-driver`** is theory-blind dispatch over a carrier. It does not
  decide worth.
- **A repo is an active intelligence** when it has a charter (`AGENTS.md`),
  lived experience (`skills/`), state in time (the ledger), and a voice
  (`scripts/agent`). Remove one and it is a static asset again.

## Mechanisms

### Workspace

```
rung          ladder! runtime + re-export of the macro
rung-macro    proc-macro crate (must be separate)
rung-std      canonical blocks
rung-het      Het: pool, gates, questions-of-rung
rung-doctrine encoding of the proposition documents
rung-driver   population → pool; audit-rectify driver
rung-fixture  cross-crate consumption tests
```

Product CLI `rung-agent` (catalog, sessions, isolation, background, XDG
`config.yaml`) lives on `feat/rung-agent` until it lands. Kernel `task` is
nested `Spawn`, depth 1. Catalog / resume / worktrees / background are
product.

### Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -q -p rung-doctrine --bin render -- --check
docs/_props.py check
docs/_props.py cited
```

`docs/_props.py cited` treats kebab tokens in comments in `rung`, `rung-het`,
and `rung-std` as proposition slugs. Wire names that are not slugs go in
`NOT_A_CITATION` (`x-api-key`, `x-should-retry`).

Required CI check is `check`. Merge method: rebase.

### House git

Primary clone stays on `master`. Never commit from it, never check it out to
a feature branch. Work in `<repo>.wt/<branch>/` via `git wt-new <branch>`.
After merge: `git wt-rm <branch>`, fast-forward master. Stage only your
files; never `git add -A`. `scripts/agent` stages only `.agent/` files.

### Configuration

| file | holds |
|---|---|
| `~/.rung/providers.yaml` | endpoint catalog + `default:` (driver) |
| `~/.rung/auth.yaml` | provider → key (never commit) |
| `$XDG_CONFIG_HOME/rung/config.yaml` | `rung-agent` LLM settings (`llm.api_key_env`, not the key) |

`$RUNG_HOME` overrides `~/.rung/`. `RUNG_CONFIG` overrides the XDG path.
Env `RUNG_*` / `XAI_API_KEY` wins over the agent file.

### Spec and CI

| gate | catches |
|---|---|
| `cargo test --workspace` | a guarantee that stopped holding |
| `render --check` | hand-edited `*-props.md` or `conformance.md` |
| `docs/_props.py check` | stale number or dangling reference |
| `docs/_props.py cited` | Rust comment citing a missing slug |

`trybuild` `.stderr` pins refusals. Do not cite `compile_fail` doctests as
evidence. `G1`–`G14` and `J1`–`J2` are labelled subtrees cited from Rust
and test filenames.

Five kinds — not a status field. The counts are the corpus; a test pins them.

| kind | discharged by | count |
|---|---|---:|
| **decidable** | a proof — a test that fails when the proposition is violated | 131 |
| **judgmental** | a principal, **disjoint** from what it judges | 47 |
| **owed** | an author, with **standing** over it | 3 |
| **signature** | nobody — it declares vocabulary | 62 |
| **rationale** | nobody — it argues, or records a limit | 148 |

### Map

| you want | read |
|---|---|
| ladder language (normative) | `docs/rung-props.md` |
| category | `docs/rung-ct-props.md` |
| Het | `docs/rung-het-props.md` |
| principals / questions (informative) | `docs/rung-std/` |
| conformance view | `docs/conformance.md` |
| questions docket | `.het/rung-questions/` |
| crate rustdoc / doctest | `rung/src/lib.rs` |

`rung-doctrine` is the source of the generated props. Render writes them;
do not edit the markdown.

The honest bootstrap measure is how many defects in rung the audit-rectify
loop found and fixed. That number is still zero: machinery exists, no real
judgment has been dispatched.

### Caretaker loop

```
scripts/agent state          # where we are in time (first read on wake)
scripts/agent inbox          # what is waiting
scripts/agent handoff <subject> [-m BODY]   # sleep: append H + derived S
scripts/agent check          # ledger integrity (exit 0/2/3)
```

Prefer `scripts/agent` over PATH `agent`. Never hand-edit events. Never
write `.agent/HANDOFF.md`. Lived experience for *this* repo:
`skills/rung/SKILL.md`. Core discipline (`caretaker`, `agentsmd`, `git`,
`signalling`) lives in `skills/` as the inhabit set; house deltas are the
`fleet_*` skills on the machine.

## Scope and audience

Maintainer: write access on `witt3rd/rung`, work from a linked worktree.
External contributors: PRs against `master`; follow CI; do not rewrite
history.

Last updated: 2026-08-20.
