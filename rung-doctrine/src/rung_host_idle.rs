//! The host's idle-mode doctrine, encoded.
//!
//! Written by hand, props first: idle mode is specified in `docs/rung-host.md`
//! and not built, so a claim a machine will settle is [`Kind::Owed`] and names
//! the form its proof will take. A proposition becomes [`Kind::Decidable`] in
//! the change that adds its test. The markdown is rendered from this.

use crate::{Doctrine, Element, Kind, Prop};

/// What an idle host guarantees, beside what the continuous host already does.
pub fn doctrine() -> Doctrine {
    Doctrine {
        file: "rung-host-idle-props.md".into(),
        elements: vec![
        Element::Verbatim(r#"# rung-host idle mode — resting between wakes

**Status: normative.** This document states what `rung-host` guarantees when it
has an idle time above 0: when it rests, what wakes it, what it does on waking,
how a stop reaches it, and how a reader sees it. It records no history, cites no
artifact, and names no consumer. Every claim is stated once, in one place, and
referred to elsewhere by number.

**Props come first.** A claim a machine will settle is `owed`, with the form its
proof will take, until the change that builds idle mode adds its test and moves
the claim to `decidable`; never earlier. The informative description is
`docs/rung-host.md` (the loop, free time, and "Idle mode: decisions").

The numbering is a tree. A proposition `n.m` is a remark on `n`; `n.mm` is a
remark on `n.m`. Interior propositions are the conjunction of their children.
Leaves are single checkable claims.

**This document is generated.** Its source is
`rung-doctrine/src/rung_host_idle.rs`, and it is written by
`cargo run -p rung-doctrine --bin render`. Editing it here does not change what
it says; the next render restores this text. Where the two differ, the encoding
is right and this file is stale — CI checks exactly that.

**Numbers are derived, not authored.** A proposition's identity is its slug; its
place in the tree is its declared parent; its order is declaration order. The
decimal number and every reference to it are computed at render time.

**What this document does not say.** It does not change the continuous host: idle
time 0 is the shipped behaviour, gated as before (G-a). It does not say what the
agent does in a free-time pass, or how the decision desk, the governor and the
pack work; those are `docs/rung-host.md`.

"#.into()),
        Element::Verbatim(r#"---

## 1 · The setting

"#.into()),
        Element::Prop(Prop {
            slug: "idle-time-is-a-per-instance-setting".into(),
            parent: None,
            kind: Kind::Signature,
            numbering: None,
            prose: r#"**Idle time** is how long a host may rest between wakes. It is a setting of one
instance, not of the process or the fleet. **Idle time 0** is **continuous
mode**; idle time above 0 is **idle mode**.

"#.into(),
            mechanism: r#""#.into(),
        }),
        Element::Prop(Prop {
            slug: "idle-time-defaults-to-thirty-minutes".into(),
            parent: Some("idle-time-is-a-per-instance-setting".into()),
            kind: Kind::Owed { why: r#"untested — a start from a configuration file that names no idle time records an idle time of 30 minutes"#.into() },
            numbering: None,
            prose: r#"An instance started from a configuration file that names no idle time has an
idle time of **30 minutes**. Continuous mode is chosen by setting idle time 0
explicitly; it is never the result of leaving the setting out.

"#.into(),
            mechanism: r#"Start the binary on a configuration without the key and read the idle time the `host.start` line recorded; start another with idle time 0 and read 0."#.into(),
        }),
        Element::Prop(Prop {
            slug: "idle-time-zero-never-rests".into(),
            parent: Some("idle-time-is-a-per-instance-setting".into()),
            kind: Kind::Decidable { proof: "rung-host/tests/gate_time.rs::thirty_quiet_minutes_have_no_rest".into() },
            numbering: None,
            prose: r#"With idle time 0 the host never rests: after every turn the next boundary
begins. The world's own waits (a provider's
backoff, a quota) are recorded as degraded, never as rest.

"#.into(),
            mechanism: r#"G-a (30 simulated minutes, no stimuli) already measures idle wall time outside turns, with idle time 0, at 2% or less. It stays as it is."#.into(),
        }),
        Element::Verbatim(r#"---

## 2 · Resting and waking

"#.into()),
        Element::Prop(Prop {
            slug: "an-idle-host-really-rests".into(),
            parent: None,
            kind: Kind::Owed { why: r#"unimplemented — a fake-clock test over an empty queue: from the moment the host rests until its wake, the record gains no boundary, no turn and no model call"#.into() },
            numbering: None,
            prose: r#"With idle time above 0, a host that has nothing left to do **rests**: from the
record's idle line until the line that ends the rest it begins no boundary,
runs no turn, spends no request, and polls nothing but its wake conditions.

"#.into(),
            mechanism: r#"A simulated clock and a scripted queue: let the host rest across many idle times and count the record's boundaries, turns and `llm.call` lines inside each rest."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-rest-ends-at-the-first-wake".into(),
            parent: Some("an-idle-host-really-rests".into()),
            kind: Kind::Owed { why: r#"unimplemented — a fake-clock test: a rest ends at the earliest of the scheduled wake, a queue arrival and the stop authority, and the record says which"#.into() },
            numbering: None,
            prose: r#"A rest ends at the **first** of three things: the **scheduled wake**, a **queue
arrival**, or the **stop authority**. The record's line that ends the rest names
which.

"#.into(),
            mechanism: r#"Three runs on a simulated clock, one per cause, each asserting the cause the ending line records."#.into(),
        }),
        Element::Prop(Prop {
            slug: "the-scheduled-wake-is-within-idle-time".into(),
            parent: Some("a-rest-ends-at-the-first-wake".into()),
            kind: Kind::Owed { why: r#"unimplemented — a fake-clock test that no rest is longer than idle time, and that a rest ends no later than the next due calendar item"#.into() },
            numbering: None,
            prose: r#"The scheduled wake is at most **idle time** after the rest began, and no later
than the next due calendar item or expectation. A calendar item and an
expectation therefore still fire at the first boundary after they are due.

"#.into(),
            mechanism: r#"A simulated clock with an empty queue and a calendar entry due before the idle time ends: the rest ends at the entry, not at the idle time."#.into(),
        }),
        Element::Prop(Prop {
            slug: "an-arrival-wakes-at-once".into(),
            parent: Some("a-rest-ends-at-the-first-wake".into()),
            kind: Kind::Owed { why: r#"unimplemented — a fake-queue test: an item arriving mid-rest ends the rest at the next wake poll, not at the scheduled wake"#.into() },
            numbering: None,
            prose: r#"An item reaching an inbound queue while the host rests ends the rest at once,
whatever the time of the scheduled wake. The item is on the record before
anything else is.

"#.into(),
            mechanism: r#"A fake queue that delivers an item part-way through a rest on a simulated clock: the rest ends at the arrival's time."#.into(),
        }),
        Element::Prop(Prop {
            slug: "the-stop-authority-interrupts-a-rest".into(),
            parent: Some("a-rest-ends-at-the-first-wake".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that a stop raised mid-rest halts the host within one wake poll, and a process test (SIGTERM, stop file) within the stop gate's bound"#.into() },
            numbering: None,
            prose: r#"The stop authority interrupts a resting host at once: a signal, a stop file or
an explicit stop halts it within one wake poll, with the same `halted` line a
running host writes, and no further turn runs. Rest never delays a stop.

"#.into(),
            mechanism: r#"On a simulated clock, a stop raised mid-rest ends it at that moment and the record's last line is `halted`; on the real clock, SIGTERM and the stop file halt a resting binary within the stop gate's bound (G-h)."#.into(),
        }),
        Element::Prop(Prop {
            slug: "an-arrival-drains-then-one-free-time-pass".into(),
            parent: Some("an-idle-host-really-rests".into()),
            kind: Kind::Owed { why: r#"unimplemented — a fake-queue test: after an arrival the host answers every waiting item through ordinary boundaries, takes exactly one free-time pass, and rests"#.into() },
            numbering: None,
            prose: r#"On a queue arrival the host drains its queue through ordinary boundaries, then
takes **one free-time pass**, then rests again. An item that arrives during the
drain or the pass is drained too, and a free-time pass follows the last answer
given. The host never takes a second free-time pass in a row.

"#.into(),
            mechanism: r#"A fake queue with several items, one of them arriving mid-drain: the turn kinds between the two rests are all responding, then exactly one free."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-scheduled-wake-is-one-free-time-pass".into(),
            parent: Some("an-idle-host-really-rests".into()),
            kind: Kind::Owed { why: r#"unimplemented — a fake-clock test: at the scheduled wake with an empty queue the host takes exactly one free-time pass, then rests"#.into() },
            numbering: None,
            prose: r#"At the scheduled wake the host takes **one free-time pass**, then rests again.
An idle host thus takes a free-time pass at least every idle time, and between
passes runs only on arrivals.

"#.into(),
            mechanism: r#"A simulated clock over several idle times with an empty queue: between consecutive rests there is exactly one turn, of kind free, and the gaps are the idle time."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-rest-is-not-degraded".into(),
            parent: Some("an-idle-host-really-rests".into()),
            kind: Kind::Owed { why: r#"unimplemented — a test that a rest writes idle lines and no `degraded` line, and that a world-imposed wait still writes `degraded` and no idle line"#.into() },
            numbering: None,
            prose: r#"A rest is the host's own choice and is recorded as idle, never as `degraded`.
A wait the world imposes (backoff, quota, a blocked credential) is still
`degraded`, and is never recorded as rest.

"#.into(),
            mechanism: r#"Two scripted runs on a simulated clock, one resting and one in provider backoff, comparing the record's line kinds."#.into(),
        }),
        Element::Verbatim(r#"---

## 3 · Seeing it

"#.into()),
        Element::Prop(Prop {
            slug: "a-resting-host-is-seen-resting".into(),
            parent: None,
            kind: Kind::Owed { why: r#"unimplemented — a record-fold test that the summary of a host whose last line is an idle line says it is idle until the time the line names, and that no other line leaves it so"#.into() },
            numbering: None,
            prose: r#"A resting host shows as **idle until `<time>`**, the time of its scheduled wake,
wherever the host is read: the instance index's listing and the gateway's
summary of the instance. It is not `needs_you`, and it is not `Waiting` (that is
the degraded wait). The line that ends the rest, and any halt, ends the display.

"#.into(),
            mechanism: r#"The listing and the summary are both read from the record alone, so a fixture record ending in an idle line, then one ending in the line that ends it, settles both."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-restart-wakes-the-host".into(),
            parent: Some("a-resting-host-is-seen-resting".into()),
            kind: Kind::Owed { why: r#"unimplemented — a restart test: a state whose record ends in an idle line, started again, shows idle no longer, answers what waits, takes one free-time pass, and rests"#.into() },
            numbering: None,
            prose: r#"A rest does not outlive its process. A host that starts on a record ending in an
idle line is awake: it answers what waits, takes one free-time pass, and rests
with a new scheduled wake.

"#.into(),
            mechanism: r#"Start, rest, kill, start again on the same state and read the record."#.into(),
        }),
        ],
    }
}
