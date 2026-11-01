# rung-fleet — notes

This document is informative. The normative statement is
[`rung-fleet-props.md`](rung-fleet-props.md); where the two disagree, the props
win. The conformance view of every claim, with the form its proof will take, is
[`conformance.md`](conformance.md). The design intent and the decisions it rests
on are in witt3rd/rung#347.

`rung-fleet` is a **product crate** (J2) that supervises many rung hosts as a
tiered fleet: a **chief** (the principal's chief-of-staff), **stewards** (one
long-lived caretaker per project) and **crew** (a specialist worker on one task,
then retired). It fills the extension point
[`rung-host.md`](rung-host.md) reserves for delegation: the `crew` group name
and the `crew.*` record kinds, with the inbox admitting external completion
items. Version one is native only: `rung-host` and `rung-agent`.

## Why props before code

The crate does not exist yet. The props are written first so that every phase
after this one has a list of claims to discharge rather than a design to
re-read. All claims a machine will settle are `owed` (decidable in principle,
nothing establishes them yet); each names the form its proof will take. A claim
becomes `decidable` in the change that adds its test, and `render --check` keeps
the document and the encoding from drifting meanwhile.

The props contain no judgmental claim. Whether completion is confirmed by a
party disjoint from the worker is the kernel's judgmental gate, typed like any
other (`completion-is-judged-apart-from-its-author`), so it is owed as a
compile-fail case; whether what a chief tells its principal states outcomes
rather than machinery is something no machine decides, so the props record it as
a limit that the consumer's chief instructions carry
(`principal-facing-text-states-outcomes`).

## What the props reuse and what they add

| the host already guarantees | the fleet adds |
|---|---|
| one live host per state directory (the state lock) | one chief or steward per host; chiefs share nothing |
| the record, fsynced and replayable; restart recovery | the `crew.*` kinds; recovery that loses no crew completion |
| the inbox, with an owner intent (`next`, `steer`, `free`) | delivery as a typed receipt, idempotent by correlation id |
| the instance index and `rung-host ls` | the parent relation: a tree of depth three |
| ACP channels: owner, peer, observer | the rule that only a chief faces the principal |
| the governor (backoff, quota, pacing) and the stop authority | a hard per-instance spend cap that stops and records why |
| per-call usage in the record | a check of the model called against the model configured |

## What the kernel contributes

The lifecycle of a delegated task is the thing the kernel exists for: a state
machine whose skipped step is a compile error. The ladder in the props is
spawned, assigned, active, done, retired, with held and stalled as recoverable
side states. Linear, `#[must_use]` tokens carry the three properties the props
care most about: a delivery receipt that must be consumed, an authority that
only the principal can produce, and a proof of landed work without which a crew
member cannot be retired.

## Phases and the claims each discharges

| phase | scope | claims that become decidable |
|---|---|---|
| R1 | two hosts (a chief and a steward) on one machine behind the gateway; a registry convention only | `chiefs-do-not-share-context`, `a-member-is-named-by-its-role`, `a-read-only-role-watches-the-fleet` (done; see the R1 findings below). `an-observer-cannot-prompt` stays owed: its observer half is exercised by `rung-host/tests/gate_acp.rs`, its owner and reload halves are not cited yet |
| R2 | the `crew` extension: typed ladders and delivery receipts | `a-skipped-transition-does-not-compile`, `held-and-stalled-are-recoverable`, `a-brief-is-a-typed-contract`, `an-unconsumed-receipt-does-not-compile`, `duplicate-delivery-is-idempotent`, `a-kill-loses-no-completion`, `crew-record-kinds-are-fleet-owned`, `reporting-line-is-a-tree`, `one-interface-to-the-principal`, the verbs, the queue claims, supervision from the record, `native-members-only`, `fleet-reuses-the-host-unchanged`, `no-consumer-in-the-repository`, `no-credential-in-a-committed-file` |
| R3 | crew work: isolation worktrees and a real coding toolset | `unlanded-work-is-never-retired`, `a-stalled-crew-is-escalated-not-looped`, `irreversible-action-needs-authority`, `a-member-cannot-widen-its-own-policy`, `completion-is-judged-apart-from-its-author` |
| R4 | model and budget control | the models-and-spend claims, `a-model-downgrade-needs-authority` |
| R5 | remote placement | `remote-uses-the-same-verbs` stops being a rationale and gets a test |
| R6 | scheduler placement | none; placement is not a guarantee |

## R1 findings: two hosts behind one gateway

**What was run.** Two real `rung-host` processes (names `chief` and
`steward-alpha`) and one `rung-gateway`, on one machine, with a stub provider
(an OpenAI-compatible server on loopback that answers one fixed line) and no
key. A client holding only the read-only token listed both, read a steward's
record, followed a chief's stream live while an owner message was dropped in its
inbox, was refused a write (403), and was refused with a wrong token (401). A
`kill -9` of the steward showed `down` in `rung-host ls`; a restart showed
`running`, with a `recovered` line and a new epoch. The same shape runs in CI as
`rung-gateway/tests/fleet_r1.rs` (three real hosts, mock engine).

**The convention.** No registry field. A member is named by its role first:
`chief-<session>`, `steward-<project>`; the id is the slug of the name, so the
role is the id's first segment. A second source of truth for a fact the name
already carries was the larger cost.

**Unknown 2: the resident cost of a host** (Linux x86-64, release build, memory
on, stub provider; 60 s windows unless noted; one core is 100%):

| hosts | provider and pacing | RSS per host | CPU per host | record growth per host |
|---|---|---|---|---|
| 1 to 20 | 0.3 s per call, no quota | 19.6 to 20.7 MB (20 hosts: 408 MB in all) | about 5% (20 hosts: one full core) | about 1 MB a minute |
| 10 | 0.3 s per call, `quota.rpm: 60` | 20.5 MB | 1.9% | 0.4 MB a minute |
| 10 | 0.3 s per call, `quota.rpm: 6` | 19.8 MB | 0.5% | 0.08 MB a minute |
| 10 | 5 s per call, no quota | 20.1 MB | 0.5% | 0.08 MB a minute |
| 3, 10 minutes | 2 s per call, `quota.rpm: 30` | 19.8 at start, 20.2 to 21.4 at the end | 1.1% | 0.17 MB a minute |

The gateway holds about 5.5 MB and no measurable CPU for ten instances. A host
has three threads and about ten file descriptors. The memory is flat enough that
the count of hosts, not their age, sets the cost. What sets CPU and disk is the
**free-time pace**: a host with nothing to do keeps taking turns, so the number
of turns a minute is the provider's speed unless a `quota` (or a slow provider)
holds it back. With a real provider each of those turns is a paid call, so a
fleet configuration should give every member a `quota` from the start; that
belongs to the model and budget phase (R4).

**Gaps found in rung.**

- Without a `quota` there is no pace but the 600-a-minute governor ceiling (see
  above). Reported, not changed: the pace is a policy a fleet sets.
- A gateway started without `registry_dir` lists no native instance and says
  nothing; its start line printed `(0 instances)` for a gateway whose instances
  all come from the registry. Fixed in its own change: the line names the
  registry folder.
- The workspace did not pass `clippy -D warnings` on the current stable
  toolchain (one `drop` of a non-`Drop` value in a gateway test). Fixed in its
  own change.
- The host's own HTTP door has no `/v1/summary`: the summary is the gateway's
  fold of the record, so a client of a host alone folds the record itself.

## Notes on particular claims

**The tree of depth three.** The command structure is capped. Every layer
between the principal's intent and the acting agent costs fidelity and tokens,
so a crew member cannot start crew. The kernel's own `task` is nested `Spawn`
at depth one; the fleet's cap is a statement about hosts, not about that.

**The one interface.** A steward or crew member may hold any channel to its
parent, but never an owner channel to the principal. That is a statement about
who can be addressed, checked at admission, rather than a convention about who
should speak.

**A hold stays open until its key arrives.** A decision the principal must make
is held with a key and closes only on a resolution carrying that key. A report
that happens to answer it does not close it, because the same report could be
about another question. This keeps an open decision from being forgotten by a
later, unrelated success.

**Policy is a subset.** The props say that a steward can only pass a subset of
its own tool policy to its crew and that a crew member has no verb that edits it.
They deliberately do not say which checks a deployment turns on.

**Credentials.** The same rule as the rest of the repository: a configuration
names the environment variable, never the key.

## Still to confirm (owner)

These two are a reading of the decision to keep the paradigm and replace the
implementation; the props above assume them, and each is cheap to change before
R2 builds on it.

1. **The work queue.** Each host's queue is its own record and inbox; the
   external issue tracker is the ledger for project work, referred to by an item
   and never the queue (props: the work queue section). The existing backlog
   contract (items for work, never for agents; a decision is an item held for
   the principal; notes carry no rotting paths) is kept as text a consumer
   supplies to its chief. Confirm that the tracker is a reference and the host's
   record is the queue.
2. **Guards as host tool policies.** Two guard policies in use today, one that
   refuses destructive commands and one that confines the working directory, are
   ported as **tool policies of the host** rather than as something the fleet
   implements. The props fix only the subset rule and the authority rule; the
   exact set of checks per role is chosen in R3, when a crew toolset exists.
   Confirm the two as the starting set, and that they are policy, not crate
   behaviour.

## Decisions taken in R0 that the owner may overturn

- **The props are encoded in `rung-doctrine`.** Every `*-props.md` must be
  generated (a test fails on a hand-written one), so the document needs a source
  file in that crate. That file is documentation; it contains no fleet behaviour.
- **`rung-fleet-props.md` is claim-only.** The consumer's instructions, and the
  analysis of how they map onto this design, live outside this repository.
- **Away state belongs to the chief and is a record fact**, so that it survives
  a restart; the props do not specify how a principal sets it.
- **Self-update and relay are not in the crate.** A fleet is updated as any rung
  binary is, by replacing it and restarting, which the props already make a
  non-event. An external conversation channel is an inbox item from a channel
  principal, which the host already admits.
