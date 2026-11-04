# rung-fleet — A tiered fleet of rung hosts

**Status: normative.** This document states what `rung-fleet` guarantees: how a
chief, its stewards and their crew are related, how work moves between them, and
what stays true across a restart. It records no history, cites no artifact, and
names no consumer. Every claim is stated once, in one place, and referred to
elsewhere by number.

**Props come first.** `rung-fleet` is a product crate (J2) that does not exist
yet. A claim a machine will settle is `owed`, with the form its proof will take,
unless a test already settles it (the claims a fleet of unmodified hosts already
meets are decidable), and the conformance view is the work queue of the crate. A
claim moves from `owed` to `decidable` in the change that adds its test, never
earlier.

The numbering is a tree. A proposition `n.m` is a remark on `n`; `n.mm` is a
remark on `n.m`. Interior propositions are the conjunction of their children.
Leaves are single checkable claims.

**This document is generated.** Its source is
`rung-doctrine/src/rung_fleet.rs`, and it is written by
`cargo run -p rung-doctrine --bin render`. Editing it here does not change what
it says; the next render restores this text. Where the two differ, the encoding
is right and this file is stale — CI checks exactly that.

**Numbers are derived, not authored.** A proposition's identity is its slug; its
place in the tree is its declared parent; its order is declaration order. The
decimal number and every reference to it are computed at render time.

**What this document does not say.** It does not say how a chief is instructed,
which models a deployment uses, or what a deployment's policies are. Those are
configuration and consumer text. It reuses, unchanged, what the continuous host
already guarantees (the state lock, the instance index, the record, the inbox, the
calendar, the governor, the stop authority and restart recovery) and states only
what the fleet adds. The informative derivation is `docs/rung-fleet-notes.md`.

---

## 1 · What the fleet is

<a id="fleet-is-a-product-crate"></a>
**1** `rung-fleet` is a **product crate** (J2) beside `rung-host`. It supervises many
rung hosts as one tiered fleet and adds the delegation the host reserves: the
`crew` group name and the `crew.*` record kinds. It changes nothing in the
kernel and nothing in `rung-host`'s own guarantees.

<a id="native-members-only" data-parent="fleet-is-a-product-crate"></a>
**1.1** Every member of the fleet is a native rung host or a native rung agent. Foreign
harnesses and terminal multiplexers are outside version one; an adapter for one
is a later, separate crate that this one does not name.

<a id="fleet-reuses-the-host-unchanged" data-parent="fleet-is-a-product-crate"></a>
**1.2** The fleet **owns no second copy** of what the host already owns. The record is
the one truth, the inbox is the one door in, the calendar and the governor are
the host's; a fleet door reads the record or asks a host to write to it, and
never writes it.

<a id="no-consumer-in-the-repository" data-parent="fleet-is-a-product-crate"></a>
**1.3** No consumer-specific name, type, id or environment variable appears in the
crate, its tests, its documents or its errors. A consumer reaches the fleet only
through a generic extension point: a system text and pack the host already
accepts, a configuration file, an inbox item.

<a id="the-chiefs-instructions-are-consumer-text" data-parent="fleet-is-a-product-crate"></a>
**1.4** The crate carries **no chief instructions**. How a chief behaves toward its
principal is text a consumer supplies when it starts a chief; the crate supplies
the verbs, the typed lifecycle and the records that text can rely on. This is why
the paradigm a deployment wants (intake, supervision, escalation, backlog) can
change without the crate changing.

<a id="no-credential-in-a-committed-file" data-parent="fleet-is-a-product-crate"></a>
**1.5** A fleet configuration names the **environment variable** that holds a key
(`api_key_env`), never the key. No record, brief, inbox item or error carries a
credential; the host's redactor runs on every text the fleet writes.

---

## 2 · Roles

<a id="three-roles"></a>
**2** A fleet has three **roles**: the **chief**, the **steward** and the **crew**.
A role is a position in the fleet, not a kind of program: each is a native rung
agent with its own record, inbox and context.

<a id="chief-is-a-host-per-session" data-parent="three-roles"></a>
**2.1** A **chief** is the principal's chief-of-staff. It is one `rung-host` instance
per session, and a fleet may hold several. A chief delegates to stewards; it does
not do project work itself.

<a id="steward-is-a-host-per-project" data-parent="three-roles"></a>
**2.2** A **steward** is the long-lived caretaker of one project. It is one `rung-host`
instance per project, and it outlives every task it delegates.

<a id="crew-is-an-agent-in-isolation" data-parent="three-roles"></a>
**2.3** A **crew** member is a specialist worker on one task, then retired. It is a
`rung-agent` session in an isolation worktree, started by a steward.

<a id="reporting-line-is-a-tree" data-parent="three-roles"></a>
**2.4** The reporting line is a **tree of depth three**. A chief directs stewards, a
steward directs its own crew, and a crew member reports only to the steward that
started it. The command structure stays flat: a crew member cannot start crew.

<a id="one-interface-to-the-principal" data-parent="three-roles"></a>
**2.5** The principal talks to **one chief** and to nobody else. A steward or crew
member never addresses the principal directly; everything it has to say reaches
the principal as an item in its parent's inbox, and ultimately through the
chief.

<a id="chiefs-do-not-share-context" data-parent="three-roles"></a>
**2.6** Several chiefs may run side by side. Their contexts **never mix**, because
their records do not: each chief is its own host with its own state directory,
held by the host's one-live-host lock.

<a id="a-member-is-named-by-its-role" data-parent="three-roles"></a>
**2.7** A member's **instance name** begins with its role: `chief` or `steward`, then a
`-` and what tells it from its peers (the session for a chief, the project for a
steward), as in `chief-main` and `steward-alpha`. The instance id is the slug of
that name, so the role is the first segment of the id. The instance index gains
no field for it: a listing already carries the role, and a reader that wants it
splits the id. A name that does not begin with a role is not a member of a
fleet, and nothing about it changes.

---

## 3 · The delegation lifecycle

<a id="delegation-is-a-ladder"></a>
**3** A delegated task moves along a **typed ladder**: it is *spawned*, *assigned*,
*active*, then *done* and finally *retired*; it may be *held* or *stalled* on
the way and returns to where it was. Each rung is a type, each move a declared
transition, and the only way to hold a value of a rung is the transition that
produces it.

<a id="a-skipped-transition-does-not-compile" data-parent="delegation-is-a-ladder"></a>
**3.1** A skipped transition is a **compile error**, not a runtime refusal. A caller
that holds an `Assigned` task cannot be handed an `Active` one without going
through the activation.

<a id="held-and-stalled-are-recoverable" data-parent="delegation-is-a-ladder"></a>
**3.2** *Held* and *stalled* are **recoverable** states: each has a paired recover edge
back to the rung it left, and a task in either state is never dropped silently.
A hold is a task waiting for a decision only the principal can make, and it stays
open until a resolution that carries its key arrives.

<a id="a-brief-is-a-typed-contract" data-parent="delegation-is-a-ladder"></a>
**3.3** A task is assigned only with a **brief**: what to build or learn, how it is
done (what counts as complete), how it is delivered, and how much autonomy the
worker has. The machinery refuses to guess a missing field.

<a id="a-stalled-crew-is-escalated-not-looped" data-parent="delegation-is-a-ladder"></a>
**3.4** A crew member silent past its **bound** is moved to *stalled* by its steward
through a transition, never by convention. The same obstacle twice is not
retried a third time: it is escalated up one level.

<a id="completion-is-judged-apart-from-its-author" data-parent="delegation-is-a-ladder"></a>
**3.5** A task reaches *done* only on the verdict of a **validator disjoint from its
author**. A worker's own confidence is not evidence of completion; a report, a
diagnosis or a recommendation authorizes nothing by itself.

<a id="unlanded-work-is-never-retired" data-parent="delegation-is-a-ladder"></a>
**3.6** A crew member is **retired** only by a transition that consumes proof its work
has landed (merged, or recorded as intentionally scratch). Uncommitted work is
never discarded by retirement, and a refusal to retire is a state the steward
reports, not an obstacle to be bypassed.

---

## 4 · Delivery

<a id="the-durable-record-is-the-delivery"></a>
**4** **The durable record is the delivery.** A completion, a hand-off or an
escalation has been delivered when the receiving host's record holds it, and not
before. The receipt a sender gets back is a typed value that names the record
line.

<a id="an-unconsumed-receipt-does-not-compile" data-parent="the-durable-record-is-the-delivery"></a>
**4.1** A delivery receipt is **consumed exactly once**. A function that sends and
discards its receipt fails to compile; the caller either records it, hands it on,
or says explicitly that it does not care.

<a id="duplicate-delivery-is-idempotent" data-parent="the-durable-record-is-the-delivery"></a>
**4.2** Delivery is **idempotent by correlation id**. The same completion delivered
twice, as after a retry or a restart of the sender, is one item in the receiver's
inbox and one line in its record.

<a id="a-kill-loses-no-completion" data-parent="the-durable-record-is-the-delivery"></a>
**4.3** A steward killed in the middle of a task **recovers from its record**: no
completion is lost and none is delivered twice.

<a id="crew-record-kinds-are-fleet-owned" data-parent="the-durable-record-is-the-delivery"></a>
**4.4** The fleet owns the schema of the `crew.*` **record kinds** the host reserves.
They are append-only like every kind, and a reader that does not know one skips
it rather than failing.

---

## 5 · Verbs and attachment

<a id="fleet-verbs"></a>
**5** The fleet's operations are six **verbs**: spawn, assign, steer, read, list and
stop. They exist as a library first and a small command second; the command
holds no behaviour the library lacks.

<a id="a-spawned-member-is-an-ordinary-host" data-parent="fleet-verbs"></a>
**5.1** A member the **spawn** verb brings into being is started as the ordinary
`rung-host` binary from the configuration spawn wrote: it enters itself in
the instance index, reads the brief spawn delivered to its inbox, and stops at
its stop file. The fleet adds no process kind of its own.

<a id="a-steer-is-an-inbox-item" data-parent="fleet-verbs"></a>
**5.2** A **steer** is a durable item in the target's inbox, read at the target's next
boundary. It is never a keystroke into a running process.

<a id="read-and-list-never-write" data-parent="fleet-verbs"></a>
**5.3** `read` and `list` are **observations**: they change nothing in any host's state
and are available to a read-only role.

<a id="a-read-only-role-watches-the-fleet" data-parent="fleet-verbs"></a>
**5.4** A gateway in front of the fleet lists its members from the instance index, and
a holder of the **read-only** token can list them, read any member's record and
follow its live event stream, and cannot write to any member. A stopped member
stays in the list.

<a id="stop-records-why" data-parent="fleet-verbs"></a>
**5.5** `stop` goes through the host's **stop authority** and the record states why and
by whom; a stopped member is not removed from the list.

<a id="an-observer-cannot-prompt" data-parent="fleet-verbs"></a>
**5.6** Attachment reuses the ACP channel roles. An **observer** channel watches and
cannot prompt; an **owner** channel takes the session over. A member is headless
the rest of the time, and a session survives a restart through the host's
session list and load.

<a id="remote-uses-the-same-verbs" data-parent="fleet-verbs"></a>
**5.7** A remote member is reached through the **same verbs**, over the stable stdio
transport carried by SSH, or the streamable HTTP transport once it is stable. The
fleet adds no transport. A scheduler is a later placement layer: to the fleet, an
instance is a long-running process with an entry in the instance index.

---

## 6 · Authority and policy

<a id="authority-is-explicit-and-typed"></a>
**6** Autonomy is an **explicit grant**, never inferred and never a default. A
transition that needs the principal's word takes an *authority* token that only
the principal's channel can produce.

<a id="irreversible-action-needs-authority" data-parent="authority-is-explicit-and-typed"></a>
**6.1** **Merging, discarding work and anything destructive, irreversible or
security-sensitive** requires the principal's explicit authority. A standing
grant is scoped to a project and to the original request, and never widens.

<a id="a-member-cannot-widen-its-own-policy" data-parent="authority-is-explicit-and-typed"></a>
**6.2** A host's **tool policy** is part of its configuration. The policy a steward
gives a crew member is a subset of its own, and the member cannot change it.
Which checks a deployment turns on (for example, refusing certain commands or
confining the working directory) is that deployment's policy, not the crate's.

<a id="a-model-downgrade-needs-authority" data-parent="authority-is-explicit-and-typed"></a>
**6.3** The fleet never lowers the intelligence doing a piece of work without the
principal's standing, explicit permission. An override that downgrades a member
is refused unless authority accompanies it.

---

## 7 · The work queue

<a id="the-queue-is-the-hosts-own"></a>
**7** Each host's **work queue is its own**: items live in that host's record and
inbox, and survive its restart. An external issue tracker is the ledger for
project work, referred to by an item, and is never the queue.

<a id="a-queue-has-one-writer" data-parent="the-queue-is-the-hosts-own"></a>
**7.1** A host's queue has **one writer**: the host. Other members add to it only by
delivering to its inbox, which the host's one-live-host lock already makes safe.

<a id="an-external-reference-is-not-state" data-parent="the-queue-is-the-hosts-own"></a>
**7.2** A reference to an external ledger entry is **data on an item**, never state the
fleet needs in order to resume. Losing the ledger loses the link, not the work.

<a id="a-decision-is-a-held-item" data-parent="the-queue-is-the-hosts-own"></a>
**7.3** A **decision** the principal must make is an item held for the principal, with a
key. Only a resolution that carries the key closes it; a later report that
happens to answer it does not.

---

## 8 · Supervision

<a id="recovery-from-the-record"></a>
**8** **A restart is a non-event.** Everything that matters survives the death of any
member, a chief included: work in flight, promises made and decisions pending
live in records, never in a conversation.

<a id="supervision-reconciles-from-disk" data-parent="recovery-from-the-record"></a>
**8.1** On start, a member **reconciles reality with its own record** and with the
instance index, and takes no step that depends on what a previous process
remembered.

<a id="an-obligation-closes-by-a-record" data-parent="recovery-from-the-record"></a>
**8.2** An obligation (a promised reply, an open hold, a queued item) is retired only by
the **durable event that answers it**, never by a recollection that it was
handled.

<a id="away-state-is-a-record-fact" data-parent="recovery-from-the-record"></a>
**8.3** A chief has an **away state** in which the principal is not watching. It is a
fact in the record, so it survives a restart, and while it holds, only a decision
the principal alone can make, work ready for review, a failure or a missing
credential is surfaced.

<a id="escalation-goes-up-one-level" data-parent="recovery-from-the-record"></a>
**8.4** An escalation goes **up one level at a time**. A crew member's blocker is the
steward's item; the steward's is the chief's; only the chief reaches the
principal.

<a id="principal-facing-text-states-outcomes" data-parent="recovery-from-the-record"></a>
**8.5** What a chief says to its principal states **outcomes, consequences and
decisions**, not the machinery that produced them. An escalation stands alone,
leads with the evidence, and offers a recommendation. No machine decides whether
a message does; this is a limit the crate records and the consumer's chief
instructions carry ([1.4](#the-chiefs-instructions-are-consumer-text)).

---

## 9 · Models and spend

<a id="models-are-resolved-in-one-place"></a>
**9** The fleet resolves which model each member runs **in one place**: a fleet-level
default, a per-role override and a per-instance override, in that order of
precedence, handed to each host when it starts and never baked into an image.

<a id="a-call-is-checked-against-its-configuration" data-parent="models-are-resolved-in-one-place"></a>
**9.1** A **policy check** compares the model each host actually called, as the host's
own per-call usage lines record it, with what the fleet configured, and **fails
loudly on drift**. Configuration that cannot be seen to reach the agents that
cost the most is not configuration.

<a id="a-spend-cap-stops-and-says-why" data-parent="models-are-resolved-in-one-place"></a>
**9.2** A per-instance **spend cap** is a hard stop. The governor paces; the cap stops
the instance and records why.

---

## 10 · Placement

<a id="placement-is-local-first"></a>
**10** Version one runs every member on one machine; remote placement over the stdio
transport follows, and a scheduler after that. Each phase is shippable and
reversible, because a member is an ordinary long-running process with an entry in
the instance index and nothing about it depends on where it runs.

