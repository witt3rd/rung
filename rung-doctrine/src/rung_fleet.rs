//! The fleet doctrine, encoded.
//!
//! Written by hand, props first: `rung-fleet` is a product crate that has no
//! code yet, so a claim a machine will settle is [`Kind::Owed`] and names the
//! form its proof will take, until a test settles it. A proposition becomes
//! [`Kind::Decidable`] in the change that adds its test. The markdown is rendered from this.

use crate::{Doctrine, Element, Kind, Prop};

/// What a chief, its stewards and their crew guarantee.
pub fn doctrine() -> Doctrine {
    Doctrine {
        file: "rung-fleet-props.md".into(),
        elements: vec![
        Element::Verbatim(r#"# rung-fleet — A tiered fleet of rung hosts

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

"#.into()),
        Element::Verbatim(r#"---

## 1 · What the fleet is

"#.into()),
        Element::Prop(Prop {
            slug: "fleet-is-a-product-crate".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"`rung-fleet` is a **product crate** (J2) beside `rung-host`. It supervises many
rung hosts as one tiered fleet and adds the delegation the host reserves: the
`crew` group name and the `crew.*` record kinds. It changes nothing in the
kernel and nothing in `rung-host`'s own guarantees.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "native-members-only".into(),
            parent: Some("fleet-is-a-product-crate".into()),
            kind: Kind::Owed { why: r#"unimplemented — a dependency-graph test that the crate depends on `rung-host` and `rung-agent-core` and on no foreign-harness adapter"#.into() },
            numbering: None,
            prose: r#"Every member of the fleet is a native rung host or a native rung agent. Foreign
harnesses and terminal multiplexers are outside version one; an adapter for one
is a later, separate crate that this one does not name.

"#.into(),
            mechanism: r#"A `cargo metadata` test over the crate's dependency graph: the allow-list is short enough to read, and a new dependency is a red test rather than a review catch."#.into(),
        }),
        Element::Prop(Prop {
            slug: "fleet-reuses-the-host-unchanged".into(),
            parent: Some("fleet-is-a-product-crate".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that the crate's public surface re-exports no second record, inbox, calendar or governor"#.into() },
            numbering: None,
            prose: r#"The fleet **owns no second copy** of what the host already owns. The record is
the one truth, the inbox is the one door in, the calendar and the governor are
the host's; a fleet door reads the record or asks a host to write to it, and
never writes it.

"#.into(),
            mechanism: r#"The host's own rule that a door never writes the record is the premise; this checks the crate adds no writer beside it."#.into(),
        }),
        Element::Prop(Prop {
            slug: "no-consumer-in-the-repository".into(),
            parent: Some("fleet-is-a-product-crate".into()),
            kind: Kind::Owed { why: r#"unimplemented — the repository-wide scan (`docs/_consumer_guard.py`) exists but is not a test the audit can run, and does not yet name this crate's scope; R2 adds a test that scans the crate and its documents"#.into() },
            numbering: None,
            prose: r#"No consumer-specific name, type, id or environment variable appears in the
crate, its tests, its documents or its errors. A consumer reaches the fleet only
through a generic extension point: a system text and pack the host already
accepts, a configuration file, an inbox item.

"#.into(),
            mechanism: r#"A scan over every tracked file under the crate and its documents for the consumer terms the repository guard already lists. A guard that scans nothing is not a proof, so the test also asserts that it scanned a non-empty set."#.into(),
        }),
        Element::Prop(Prop {
            slug: "the-chiefs-instructions-are-consumer-text".into(),
            parent: Some("fleet-is-a-product-crate".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"The crate carries **no chief instructions**. How a chief behaves toward its
principal is text a consumer supplies when it starts a chief; the crate supplies
the verbs, the typed lifecycle and the records that text can rely on. This is why
the paradigm a deployment wants (intake, supervision, escalation, backlog) can
change without the crate changing.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "no-credential-in-a-committed-file".into(),
            parent: Some("fleet-is-a-product-crate".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that a started fleet's records, briefs and configuration contain no value read from a named environment variable"#.into() },
            numbering: None,
            prose: r#"A fleet configuration names the **environment variable** that holds a key
(`api_key_env`), never the key. No record, brief, inbox item or error carries a
credential; the host's redactor runs on every text the fleet writes.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 2 · Roles

"#.into()),
        Element::Prop(Prop {
            slug: "three-roles".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A fleet has three **roles**: the **chief**, the **steward** and the **crew**.
A role is a position in the fleet, not a kind of program: each is a native rung
agent with its own record, inbox and context.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "chief-is-a-host-per-session".into(),
            parent: Some("three-roles".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A **chief** is the principal's chief-of-staff. It is one `rung-host` instance
per session, and a fleet may hold several. A chief delegates to stewards; it does
not do project work itself.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "steward-is-a-host-per-project".into(),
            parent: Some("three-roles".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A **steward** is the long-lived caretaker of one project. It is one `rung-host`
instance per project, and it outlives every task it delegates.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "crew-is-an-agent-in-isolation".into(),
            parent: Some("three-roles".into()),
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A **crew** member is a specialist worker on one task, then retired. It is a
`rung-agent` session in an isolation worktree, started by a steward.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "reporting-line-is-a-tree".into(),
            parent: Some("three-roles".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/reporting_line.rs::whatever_sequence_of_edges_ran_the_line_is_a_tree_of_depth_three".into() },
            numbering: None,
            prose: r#"The reporting line is a **tree of depth three**. A chief directs stewards, a
steward directs its own crew, and a crew member reports only to the steward that
started it. The command structure stays flat: a crew member cannot start crew.

"#.into(),
            mechanism: r#"The cited test attempts generated sequences of edges (seeded, so a failure replays) over valid, invalid and unnamed members and, after every attempt, checks that each member has one parent, that every edge goes one tier down, that a chief has no parent and that every walk upward ends within three members, never at a crew member. `Line::attach` is the one way to add an edge and refuses the rest by name; a spawn yields the edge it makes. Enforcing the line on the verbs of a running fleet is left to the authority tokens."#.into(),
        }),
        Element::Prop(Prop {
            slug: "one-interface-to-the-principal".into(),
            parent: Some("three-roles".into()),
            kind: Kind::Owed { why: r#"unimplemented — a channel-admission test: a steward or crew session cannot hold an owner channel to a principal, and addresses the principal only through its chief's inbox"#.into() },
            numbering: None,
            prose: r#"The principal talks to **one chief** and to nobody else. A steward or crew
member never addresses the principal directly; everything it has to say reaches
the principal as an item in its parent's inbox, and ultimately through the
chief.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "chiefs-do-not-share-context".into(),
            parent: Some("three-roles".into()),
            kind: Kind::Decidable { proof: "rung-gateway/tests/fleet_r1.rs::two_chiefs_and_a_steward_behind_one_gateway".into() },
            numbering: None,
            prose: r#"Several chiefs may run side by side. Their contexts **never mix**, because
their records do not: each chief is its own host with its own state directory,
held by the host's one-live-host lock.

"#.into(),
            mechanism: r#"The cited test starts two chiefs and a steward as three real hosts, drops an owner message in the first chief's inbox, and reads every file under the other two members' directories: the message is in the first chief's state and in nothing else. It runs on the mock engine, so it shows that the state directories are disjoint, not what a model does with them."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-member-is-named-by-its-role".into(),
            parent: Some("three-roles".into()),
            kind: Kind::Decidable { proof: "rung-gateway/tests/fleet_r1.rs::two_chiefs_and_a_steward_behind_one_gateway".into() },
            numbering: None,
            prose: r#"A member's **instance name** begins with its role: `chief` or `steward`, then a
`-` and what tells it from its peers (the session for a chief, the project for a
steward), as in `chief-main` and `steward-alpha`. The instance id is the slug of
that name, so the role is the first segment of the id. The instance index gains
no field for it: a listing already carries the role, and a reader that wants it
splits the id. A name that does not begin with a role is not a member of a
fleet, and nothing about it changes.

"#.into(),
            mechanism: r#"A convention, not a mechanism: nothing refuses a name that breaks it, and the crate adds no role field to the instance index (a second source of truth for a fact the name already states). The cited test starts three members under role-first names and checks that the gateway's list returns them in id order with the role as each id's first segment. Crew naming is left to the crew extension."#.into(),
        }),
        Element::Verbatim(r#"---

## 3 · The delegation lifecycle

"#.into()),
        Element::Prop(Prop {
            slug: "delegation-is-a-ladder".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"A delegated task moves along a **typed ladder**: it is *spawned*, *assigned*,
*active*, then *done* and finally *retired*; it may be *held* or *stalled* on
the way and returns to where it was. Each rung is a type, each move a declared
transition, and the only way to hold a value of a rung is the transition that
produces it.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-skipped-transition-does-not-compile".into(),
            parent: Some("delegation-is-a-ladder".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/lifecycle.rs::the_refusals_do_not_compile".into() },
            numbering: None,
            prose: r#"A skipped transition is a **compile error**, not a runtime refusal. A caller
that holds an `Assigned` task cannot be handed an `Active` one without going
through the activation.

"#.into(),
            mechanism: r#"Same mechanism as the kernel's refusals: the case is a compile-fail test whose snapshot holds the message, so a refusal that changes its reason is caught."#.into(),
        }),
        Element::Prop(Prop {
            slug: "held-and-stalled-are-recoverable".into(),
            parent: Some("delegation-is-a-ladder".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/lifecycle.rs::held_and_stalled_are_recoverable".into() },
            numbering: None,
            prose: r#"*Held* and *stalled* are **recoverable** states: each has a paired recover edge
back to the rung it left, and a task in either state is never dropped silently.
A hold is a task waiting for a decision only the principal can make, and it stays
open until a resolution that carries its key arrives.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-brief-is-a-typed-contract".into(),
            parent: Some("delegation-is-a-ladder".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/lifecycle.rs::the_refusals_do_not_compile".into() },
            numbering: None,
            prose: r#"A task is assigned only with a **brief**: what to build or learn, how it is
done (what counts as complete), how it is delivered, and how much autonomy the
worker has. The machinery refuses to guess a missing field.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-stalled-crew-is-escalated-not-looped".into(),
            parent: Some("delegation-is-a-ladder".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/stall.rs::silence_past_the_bound_is_retried_once_then_escalated_up_one_level".into() },
            numbering: None,
            prose: r#"A crew member silent past its **bound** is moved to *stalled* by its steward
through a transition, never by convention. The same obstacle twice is not
retried a third time: it is escalated up one level.

"#.into(),
            mechanism: r#"The clock is the caller's (`Watch` takes the time as an argument), so the cited test is a fake-clock test: silence within the bound goes on, silence past it becomes `stalled` only through `supervise::tick` and the ladder, the first stall is retried, and the second stall of the same task is escalated to the steward's parent in the reporting line (`Kind::Escalation`, idempotent by id) and never retried. A steward with no parent has nowhere to escalate to and is told so. The correlation of the escalation is the task and the stall count; whether a deployment's chief then reaches the principal is `escalation-goes-up-one-level`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "completion-is-judged-apart-from-its-author".into(),
            parent: Some("delegation-is-a-ladder".into()),
            kind: Kind::Owed { why: r#"unimplemented — a compile-fail case: the transition to done takes a qualified-judgment token of the kernel's judgmental gate, which only a principal disjoint from the author can produce"#.into() },
            numbering: None,
            prose: r#"A task reaches *done* only on the verdict of a **validator disjoint from its
author**. A worker's own confidence is not evidence of completion; a report, a
diagnosis or a recommendation authorizes nothing by itself.

"#.into(),
            mechanism: r#"The kernel already types this: a transition marked judgmental takes a token that no author can mint. This proposition is the fleet's use of that gate, and the case checks the completion transition carries the marker."#.into(),
        }),
        Element::Prop(Prop {
            slug: "unlanded-work-is-never-retired".into(),
            parent: Some("delegation-is-a-ladder".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/landing.rs::an_uncommitted_or_unmerged_worktree_is_refused_retirement".into() },
            numbering: None,
            prose: r#"A crew member is **retired** only by a transition that consumes proof its work
has landed (merged, or recorded as intentionally scratch). Uncommitted work is
never discarded by retirement, and a refusal to retire is a state the steward
reports, not an obstacle to be bypassed.

"#.into(),
            mechanism: r#"`Finished`, which the closure ladder consumes, is made only from a `Done` and a `Landed`; the `retire_without_landed` and `landed_forged` compile-fail cases pin that. The cited test reads real git worktrees: a clean head contained in the line lands, an untracked or modified path is refused by name and left untouched, and a commit not yet on the line is refused until it is. `Landed::scratch` consumes a one-time authority to discard (`scratch_with_standing` pins that no standing one exists)."#.into(),
        }),
        Element::Verbatim(r#"---

## 4 · Delivery

"#.into()),
        Element::Prop(Prop {
            slug: "the-durable-record-is-the-delivery".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"**The durable record is the delivery.** A completion, a hand-off or an
escalation has been delivered when the receiving host's record holds it, and not
before. The receipt a sender gets back is a typed value that names the record
line.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "an-unconsumed-receipt-does-not-compile".into(),
            parent: Some("the-durable-record-is-the-delivery".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/delivery.rs::the_refusals_do_not_compile".into() },
            numbering: None,
            prose: r#"A delivery receipt is **consumed exactly once**. A function that sends and
discards its receipt fails to compile; the caller either records it, hands it on,
or says explicitly that it does not care.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "duplicate-delivery-is-idempotent".into(),
            parent: Some("the-durable-record-is-the-delivery".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/delivery.rs::the_record_is_the_delivery_and_a_duplicate_adds_nothing".into() },
            numbering: None,
            prose: r#"Delivery is **idempotent by correlation id**. The same completion delivered
twice, as after a retry or a restart of the sender, is one item in the receiver's
inbox and one line in its record.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-kill-loses-no-completion".into(),
            parent: Some("the-durable-record-is-the-delivery".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/kill_recovery.rs::a_killed_steward_recovers_every_completion_once".into() },
            numbering: None,
            prose: r#"A steward killed in the middle of a task **recovers from its record**: no
completion is lost and none is delivered twice.

"#.into(),
            mechanism: r#"The host already requeues a stimulus admitted to a turn that never ended; this proposition adds that no crew completion falls between the crew's inbox write and the steward's recovery. The cited test kills the real `rung-host` binary with SIGKILL after one completion was read, with a second just written and a third written while it is down; after a restart from the same configuration each is in the record once, and a retry of the first names the same line."#.into(),
        }),
        Element::Prop(Prop {
            slug: "crew-record-kinds-are-fleet-owned".into(),
            parent: Some("the-durable-record-is-the-delivery".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/records.rs::every_crew_kind_round_trips_through_a_record_line".into() },
            numbering: None,
            prose: r#"The fleet owns the schema of the `crew.*` **record kinds** the host reserves.
They are append-only like every kind, and a reader that does not know one skips
it rather than failing.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 5 · Verbs and attachment

"#.into()),
        Element::Prop(Prop {
            slug: "fleet-verbs".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The fleet's operations are six **verbs**: spawn, assign, steer, read, list and
stop. They exist as a library first and a small command second; the command
holds no behaviour the library lacks.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-spawned-member-is-an-ordinary-host".into(),
            parent: Some("fleet-verbs".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/launch_real_host.rs::a_launched_member_is_the_real_host_and_reads_its_brief".into() },
            numbering: None,
            prose: r#"A member the **spawn** verb brings into being is started as the ordinary
`rung-host` binary from the configuration spawn wrote: it enters itself in
the instance index, reads the brief spawn delivered to its inbox, and stops at
its stop file. The fleet adds no process kind of its own.

"#.into(),
            mechanism: r#"The cited test builds the real `rung-host` binary, launches a spawned member with it under a scratch instance-index home, confirms the brief against the member's record, finds the member in the index, and stops it through its stop file."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-steer-is-an-inbox-item".into(),
            parent: Some("fleet-verbs".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/steer.rs::a_steer_is_one_owner_item_in_the_inbox_and_nothing_else".into() },
            numbering: None,
            prose: r#"A **steer** is a durable item in the target's inbox, read at the target's next
boundary. It is never a keystroke into a running process.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "read-and-list-never-write".into(),
            parent: Some("fleet-verbs".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/verbs.rs::list_and_read_change_nothing".into() },
            numbering: None,
            prose: r#"`read` and `list` are **observations**: they change nothing in any host's state
and are available to a read-only role.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-read-only-role-watches-the-fleet".into(),
            parent: Some("fleet-verbs".into()),
            kind: Kind::Decidable { proof: "rung-gateway/tests/fleet_r1.rs::two_chiefs_and_a_steward_behind_one_gateway".into() },
            numbering: None,
            prose: r#"A gateway in front of the fleet lists its members from the instance index, and
a holder of the **read-only** token can list them, read any member's record and
follow its live event stream, and cannot write to any member. A stopped member
stays in the list.

"#.into(),
            mechanism: r#"The cited test holds only the read-only token: it lists three running members, reads a steward's record, follows a chief's stream until a message dropped in its inbox appears, is refused (403) a write to the queue, and after the members stop finds all three still listed as `stopped`. A wrong token is 401."#.into(),
        }),
        Element::Prop(Prop {
            slug: "stop-records-why".into(),
            parent: Some("fleet-verbs".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/stop.rs::stop_records_who_and_why_then_the_member_halts_and_stays_listed".into() },
            numbering: None,
            prose: r#"`stop` goes through the host's **stop authority** and the record states why and
by whom; a stopped member is not removed from the list.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "an-observer-cannot-prompt".into(),
            parent: Some("fleet-verbs".into()),
            kind: Kind::Owed { why: r#"unimplemented — an attach test over the three ACP channel roles: an observer channel to a member is refused a prompt; an owner channel is accepted and takes over; a session reloads after a restart"#.into() },
            numbering: None,
            prose: r#"Attachment reuses the ACP channel roles. An **observer** channel watches and
cannot prompt; an **owner** channel takes the session over. A member is headless
the rest of the time, and a session survives a restart through the host's
session list and load.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "remote-uses-the-same-verbs".into(),
            parent: Some("fleet-verbs".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"A remote member is reached through the **same verbs**, over the stable stdio
transport carried by SSH, or the streamable HTTP transport once it is stable. The
fleet adds no transport. A scheduler is a later placement layer: to the fleet, an
instance is a long-running process with an entry in the instance index.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 6 · Authority and policy

"#.into()),
        Element::Prop(Prop {
            slug: "authority-is-explicit-and-typed".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"Autonomy is an **explicit grant**, never inferred and never a default. A
transition that needs the principal's word takes an *authority* token that only
the principal's channel can produce.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "authority-comes-from-an-owner-item".into(),
            parent: Some("authority-is-explicit-and-typed".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/authority.rs::only_an_owner_item_is_a_grant".into() },
            numbering: None,
            prose: r#"An authority token is made only from an **owner item** the principal's channel put
in a host's record. An item of any other role (a peer, an agent) never is a grant,
whatever its text says, and a token's fields are private.

"#.into(),
            mechanism: r#"The host records the role of every item it admits, so a member cannot write an owner item. The cited test reads a record holding the grant text under the peer, agent and owner roles and finds a grant only in the owner item; the `token_forged` compile-fail case pins that a token cannot be written out by hand."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-one-time-grant-is-spent-once".into(),
            parent: Some("authority-is-explicit-and-typed".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/authority.rs::a_one_time_grant_makes_one_token_for_its_own_act".into() },
            numbering: None,
            prose: r#"A **one-time** grant makes one token, for the act it names. The token is not
`Clone` and is spent by being passed, and the grant cannot be minted again,
by another process or after a restart.

"#.into(),
            mechanism: r#"Minting creates the grant's id in a ledger directory; a second mint of the same id is refused. The `token_used_twice` and `token_for_another_act` compile-fail cases pin the by-value spend and the act in the type."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-standing-grant-never-widens".into(),
            parent: Some("authority-is-explicit-and-typed".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/authority.rs::a_standing_grant_never_widens".into() },
            numbering: None,
            prose: r#"A **standing** grant is bound to the project and the request the principal
named, and covers only routine acts (spawn, assign, steer). It cannot be
asked for another project or request, and it is not a one-time token.

"#.into(),
            mechanism: r#"The `standing_irreversible` compile-fail case pins that the irreversible acts have no standing token at all."#.into(),
        }),
        Element::Prop(Prop {
            slug: "irreversible-action-needs-authority".into(),
            parent: Some("authority-is-explicit-and-typed".into()),
            kind: Kind::Owed { why: r#"partly built — stop, spawn, assign and steer take a token and the compile-fail cases pass (`stop_without_authority`, `stop_with_standing`, `standing_irreversible`); retirement as scratch consumes the discard token, while merge and the destructive and security-sensitive acts have a token type but no verb yet"#.into() },
            numbering: None,
            prose: r#"**Merging, discarding work and anything destructive, irreversible or
security-sensitive** requires the principal's explicit authority. A standing
grant is scoped to a project and to the original request, and never widens.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-member-cannot-widen-its-own-policy".into(),
            parent: Some("authority-is-explicit-and-typed".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that the tool policy a steward hands a crew member is a subset of the steward's own, and that the member has no verb that edits it"#.into() },
            numbering: None,
            prose: r#"A host's **tool policy** is part of its configuration. The policy a steward
gives a crew member is a subset of its own, and the member cannot change it.
Which checks a deployment turns on (for example, refusing certain commands or
confining the working directory) is that deployment's policy, not the crate's.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-model-downgrade-needs-authority".into(),
            parent: Some("authority-is-explicit-and-typed".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that a per-instance override naming a weaker model than the role default is refused without authority"#.into() },
            numbering: None,
            prose: r#"The fleet never lowers the intelligence doing a piece of work without the
principal's standing, explicit permission. An override that downgrades a member
is refused unless authority accompanies it.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 7 · The work queue

"#.into()),
        Element::Prop(Prop {
            slug: "the-queue-is-the-hosts-own".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"Each host's **work queue is its own**: items live in that host's record and
inbox, and survive its restart. An external issue tracker is the ledger for
project work, referred to by an item, and is never the queue.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-queue-has-one-writer".into(),
            parent: Some("the-queue-is-the-hosts-own".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/queue_one_writer.rs::items_arrive_only_as_inbox_files_and_a_second_host_is_refused".into() },
            numbering: None,
            prose: r#"A host's queue has **one writer**: the host. Other members add to it only by
delivering to its inbox, which the host's one-live-host lock already makes safe.

"#.into(),
            mechanism: r#"The cited test delivers a peer item to a member that is not running and finds the inbox file written and the receiver's state directory absent; it then runs the real host, which reads the file into its record, and starts a second real host on the same state directory, which is refused and adds nothing to the record while the first goes on. It does not show that the fleet has no other writer in its code; that is `fleet-reuses-the-host-unchanged`."#.into(),
        }),
        Element::Prop(Prop {
            slug: "an-external-reference-is-not-state".into(),
            parent: Some("the-queue-is-the-hosts-own".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that a host resumes every queued item with the external ledger unreachable"#.into() },
            numbering: None,
            prose: r#"A reference to an external ledger entry is **data on an item**, never state the
fleet needs in order to resume. Losing the ledger loses the link, not the work.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-decision-is-a-held-item".into(),
            parent: Some("the-queue-is-the-hosts-own".into()),
            kind: Kind::Decidable { proof: "rung-fleet/tests/decision_hold.rs::only_a_resolution_with_the_key_closes_a_hold".into() },
            numbering: None,
            prose: r#"A **decision** the principal must make is an item held for the principal, with a
key. Only a resolution that carries the key closes it; a later report that
happens to answer it does not.

"#.into(),
            mechanism: r#"The cited test folds `crew.*` lines with `records::open_holds`: a hold opened by `crew.held` stays open through completion reports, retirement, a resolution of another key and a resolution of another task, and closes only on a `crew.resolved` of its own task and key; a resolution that came before its hold closes nothing. It shows the fold over lines, not that a running steward reads it; the ladder's own `resume` has the same rule (`held-and-stalled-are-recoverable`)."#.into(),
        }),
        Element::Verbatim(r#"---

## 8 · Supervision

"#.into()),
        Element::Prop(Prop {
            slug: "recovery-from-the-record".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"**A restart is a non-event.** Everything that matters survives the death of any
member, a chief included: work in flight, promises made and decisions pending
live in records, never in a conversation.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "supervision-reconciles-from-disk".into(),
            parent: Some("recovery-from-the-record".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that a restarted steward reconstructs its crew list, their rungs and its open holds from its record and the instance index alone"#.into() },
            numbering: None,
            prose: r#"On start, a member **reconciles reality with its own record** and with the
instance index, and takes no step that depends on what a previous process
remembered.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "an-obligation-closes-by-a-record".into(),
            parent: Some("recovery-from-the-record".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that a promised reply, an open hold and a queued item are each retired only by the durable event that answers it"#.into() },
            numbering: None,
            prose: r#"An obligation (a promised reply, an open hold, a queued item) is retired only by
the **durable event that answers it**, never by a recollection that it was
handled.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "away-state-is-a-record-fact".into(),
            parent: Some("recovery-from-the-record".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that a chief's away state is written to its record, replayed on restart, and that while it holds only escalations, review-ready work and failures reach the principal"#.into() },
            numbering: None,
            prose: r#"A chief has an **away state** in which the principal is not watching. It is a
fact in the record, so it survives a restart, and while it holds, only a decision
the principal alone can make, work ready for review, a failure or a missing
credential is surfaced.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "escalation-goes-up-one-level".into(),
            parent: Some("recovery-from-the-record".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that a crew's blocker is an item in its steward's inbox, a steward's is in its chief's, and none skips a level"#.into() },
            numbering: None,
            prose: r#"An escalation goes **up one level at a time**. A crew member's blocker is the
steward's item; the steward's is the chief's; only the chief reaches the
principal.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "principal-facing-text-states-outcomes".into(),
            parent: Some("recovery-from-the-record".into()),
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"What a chief says to its principal states **outcomes, consequences and
decisions**, not the machinery that produced them. An escalation stands alone,
leads with the evidence, and offers a recommendation. No machine decides whether
a message does; this is a limit the crate records and the consumer's chief
instructions carry ({#the-chiefs-instructions-are-consumer-text}).

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 9 · Models and spend

"#.into()),
        Element::Prop(Prop {
            slug: "models-are-resolved-in-one-place".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"The fleet resolves which model each member runs **in one place**: a fleet-level
default, a per-role override and a per-instance override, in that order of
precedence, handed to each host when it starts and never baked into an image.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-call-is-checked-against-its-configuration".into(),
            parent: Some("models-are-resolved-in-one-place".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test over a deliberately mis-configured host: the model each host actually served (the host's per-call usage lines) differs from the configured one, and the check fails and says which host and which models"#.into() },
            numbering: None,
            prose: r#"A **policy check** compares the model each host actually called, as the host's
own per-call usage lines record it, with what the fleet configured, and **fails
loudly on drift**. Configuration that cannot be seen to reach the agents that
cost the most is not configuration.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-spend-cap-stops-and-says-why".into(),
            parent: Some("models-are-resolved-in-one-place".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that an instance reaching its cap stops through the stop authority and records the cap, the total and the reason"#.into() },
            numbering: None,
            prose: r#"A per-instance **spend cap** is a hard stop. The governor paces; the cap stops
the instance and records why.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Verbatim(r#"---

## 10 · Placement

"#.into()),
        Element::Prop(Prop {
            slug: "placement-is-local-first".into(),
            parent: None,
            kind: Kind::Rationale,
            numbering: None,
            prose: r#"Version one runs every member on one machine; remote placement over the stdio
transport follows, and a scheduler after that. Each phase is shippable and
reversible, because a member is an ordinary long-running process with an entry in
the instance index and nothing about it depends on where it runs.

"#.into(),
            mechanism: r#""#.into(),
        }),
        ],
    }
}
