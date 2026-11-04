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
[`rung-host.md`](reference/rung-host.md) reserves for delegation: the `crew` group name
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

**What is evidenced.** `rung-gateway/tests/fleet_r1.rs` runs three real hosts
(two chiefs and a steward, mock engine) behind one gateway: a client holding
only the read-only token lists them, reads a record and follows the live
stream, and is refused a write. Kill, restart and recovery are not covered by
that test and are not claimed here.

**The convention.** No registry field. A member is named by its role first:
`chief-<session>`, `steward-<project>`; the id is the slug of the name, so the
role is the id's first segment. A second source of truth for a fact the name
already carries was the larger cost.

**Unknown 2: the resident cost of a host.** Measured: about 20 MB per host
(N hosts cost about 20 MB x N), and the free-time pace, not age, sets CPU, disk
and money; give every member a `quota`. The table and the run are in
[`records/rung-fleet/r1-two-hosts.md`](records/rung-fleet/r1-two-hosts.md).

**Gaps found in rung.**

- Without a `quota` there is no pace but the 600-a-minute governor ceiling (see
  above). Reported, not changed: the pace is a policy a fleet sets.
- A gateway started without `registry_dir` lists no native instance and says
  nothing; its start line prints only the configured-instance count. Not fixed.
- The host's own HTTP door has no `/v1/summary`: the summary is the gateway's
  fold of the record, so a client of a host alone folds the record itself.

The full list, with the fixes made, is in the R1 record linked above.

## R2 progress

**Step 1: the lifecycle.** `rung-fleet/src/lifecycle.rs`. A ladder's functions
take only the token, so the one input a step reads is `Work.signal`, set by the
caller before stepping. A hold stays open by construction: a held task steps to
`Held` again until `Work.resolution` equals its `hold` key. Retirement is a
second ladder (`closure`) whose only input, `Finished`, is built from a `Done`
verdict; it takes no landing proof yet (R3). **Step 2: delivery.** `rung-fleet/src/delivery.rs`. Idempotence needs no
fleet-side ledger: the correlation id is the inbox file's stem, which is the
item id, and the host accepts an id once (a later file of the same name is
removed unread). `Sent::confirm` scans the receiver's record with
`rung_host::read::scan`, a read that writes nothing. Not claimed here: a
receipt after a steward is killed mid-task (`a-kill-loses-no-completion`),
which needs the verbs.
**Step 3: the `crew.*` kinds.** `rung-fleet/src/records.rs`. Ten kinds
(`crew.spawned` through `crew.retired`, and `crew.receipt`), each a typed
`CrewRecord` that encodes to a record line body and decodes from a line. A line
that is not a known crew kind decodes to nothing (skipped); a known kind with a
bad field is an error. A receipt names the receiver's line as `line`, because
`seq` is the line's own number. The module gives the schema only: the host's
append is crate-private, so writing the lines belongs to the verbs.
**The observing verbs.** `rung-fleet/src/verbs.rs` and the `rung-fleet`
command: `list` (the instance index through the host's registry reader) and
`read` (a member's record through the host's scan, or its crew lines). Neither
takes a lock or writes a file; the command only prints what the library returns.
Why a fleet `list` and `read` beside `rung-host ls` and the gateway: the same reads
as a library a steward calls directly (no process, no HTTP), plus the crew view;
the host's index and record readers and the list's JSON builder are reused.
Still owed in R2: `steer`, `stop` (the host's stop file names no one, so the
"by whom" of `stop-records-why` needs a host change or the gateway's door; it
is built as the stop file works today), `spawn`, `assign`, and the writing of
the crew lines.

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
- **`rung-fleet-props.md` is claim-only.** The consumer's instructions live in the
  consumer; how a deployment's instructions map onto this design is in
  [`rung-fleet-paradigm-port.md`](rung-fleet-paradigm-port.md).
- **Away state belongs to the chief and is a record fact**, so that it survives
  a restart; the props do not specify how a principal sets it.
- **Self-update and relay are not in the crate.** A fleet is updated as any rung
  binary is, by replacing it and restarting, which the props already make a
  non-event. An external conversation channel is an inbox item from a channel
  principal, which the host already admits.
