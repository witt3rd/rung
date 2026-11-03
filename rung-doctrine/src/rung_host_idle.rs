//! The host's idle-mode doctrine, encoded.
//!
//! Written by hand, props first: a claim a machine will settle is
//! [`Kind::Owed`] and names the form its proof will take until the change that
//! adds its test makes it [`Kind::Decidable`]. The markdown is rendered from this.

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
`docs/reference/rung-host.md` (the loop, free time, and "Idle mode: decisions").

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
pack work; those are `docs/reference/rung-host.md`.

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
            kind: Kind::Decidable { proof: "rung-host/tests/idle_mode.rs::a_config_without_idle_time_rests_for_thirty_minutes_and_a_stop_file_ends_it".into() },
            numbering: None,
            prose: r#"An instance started from a configuration file that names no idle time has an
idle time of **30 minutes**. Continuous mode is chosen by setting idle time 0
explicitly; it is never the result of leaving the setting out.

"#.into(),
            mechanism: r#"Start the binary on a configuration without the key and read the idle time the `host.start` line recorded (30 minutes) and the first `idle` line's wake; `idle_time_zero_in_a_config_is_continuous` starts another with idle time 0 and reads 0 and no idle line."#.into(),
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
            kind: Kind::Decidable { proof: "rung-host/tests/idle_mode.rs::an_idle_host_rests_and_a_scheduled_wake_is_one_free_pass".into() },
            numbering: None,
            prose: r#"With idle time above 0, a host that has nothing left to do **rests**: from the
record's idle line until the line that ends the rest it begins no boundary,
runs no turn, spends no request, and polls nothing but its wake conditions.

"#.into(),
            mechanism: r#"A simulated clock over three hours with an empty queue: inside each rest the record holds no boundary, no turn, no model call and no degraded line, and the fold of the record equals the live state."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-rest-ends-at-the-first-wake".into(),
            parent: Some("an-idle-host-really-rests".into()),
            kind: Kind::Decidable { proof: "rung-host/tests/idle_mode.rs::an_arrival_wakes_at_once_drains_the_queue_then_one_free_pass".into() },
            numbering: None,
            prose: r#"A rest ends at the **first** of three things: the **scheduled wake**, a **queue
arrival**, or the **stop authority**. The record's line that ends the rest names
which.

"#.into(),
            mechanism: r#"Each cause is asserted by a test of its own, on the line that ends the rest: `scheduled` (the rest test), `arrival` (this test), `stop` (the stop test)."#.into(),
        }),
        Element::Prop(Prop {
            slug: "the-scheduled-wake-is-within-idle-time".into(),
            parent: Some("a-rest-ends-at-the-first-wake".into()),
            kind: Kind::Decidable { proof: "rung-host/tests/idle_mode.rs::a_due_calendar_item_ends_a_rest_and_is_answered_before_the_pass".into() },
            numbering: None,
            prose: r#"The scheduled wake is at most **idle time** after the rest began, and no later
than the next due calendar item or expectation. A calendar item and an
expectation therefore still fire at the first boundary after they are due.

"#.into(),
            mechanism: r#"The rest test computes each wake from the fold (idle time, the next due calendar item, an open expectation, the run limit) and compares it with the idle line; this test adds a calendar entry due before the idle time ends: the rest ends at the entry and the item fires at that boundary."#.into(),
        }),
        Element::Prop(Prop {
            slug: "an-arrival-wakes-at-once".into(),
            parent: Some("a-rest-ends-at-the-first-wake".into()),
            kind: Kind::Decidable { proof: "rung-host/tests/idle_mode.rs::an_arrival_wakes_at_once_drains_the_queue_then_one_free_pass".into() },
            numbering: None,
            prose: r#"An item reaching an inbound queue while the host rests ends the rest at once,
whatever the time of the scheduled wake. The item is on the record before
anything else is.

"#.into(),
            mechanism: r#"A fake queue delivers an owner item mid-rest on a simulated clock: the rest ends at the arrival's time, well before the scheduled wake, and the item is accepted (on the record) before the next boundary."#.into(),
        }),
        Element::Prop(Prop {
            slug: "the-stop-authority-interrupts-a-rest".into(),
            parent: Some("a-rest-ends-at-the-first-wake".into()),
            kind: Kind::Decidable { proof: "rung-host/tests/idle_mode.rs::a_stop_mid_rest_halts_at_once".into() },
            numbering: None,
            prose: r#"The stop authority interrupts a resting host at once: a signal, a stop file or
an explicit stop halts it within one wake poll, with the same `halted` line a
running host writes, and no further turn runs. Rest never delays a stop.

"#.into(),
            mechanism: r#"On a simulated clock a stop raised mid-rest ends it at that moment (the `halted` line's time is the stop's, not the scheduled wake's) with no boundary or turn after; on the real clock the stop file and SIGTERM each halt a resting binary within a fraction of the idle time (`a_config_without_idle_time_rests_for_thirty_minutes_and_a_stop_file_ends_it`, `sigterm_ends_a_rest_at_once`)."#.into(),
        }),
        Element::Prop(Prop {
            slug: "an-arrival-drains-then-one-free-time-pass".into(),
            parent: Some("an-idle-host-really-rests".into()),
            kind: Kind::Decidable { proof: "rung-host/tests/idle_mode.rs::an_arrival_wakes_at_once_drains_the_queue_then_one_free_pass".into() },
            numbering: None,
            prose: r#"On a queue arrival the host drains its queue through ordinary boundaries, then
takes **one free-time pass**, then rests again. An item that arrives during the
drain or the pass is drained too, and a free-time pass follows the last answer
given. The host never takes a second free-time pass in a row.

"#.into(),
            mechanism: r#"A fake queue with two owner items, the second arriving while the first is answered: both are disposed, the turns between the two rests end with exactly one free-time pass (a free turn that took no item), and the next rest is idle time after that pass."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-scheduled-wake-is-one-free-time-pass".into(),
            parent: Some("an-idle-host-really-rests".into()),
            kind: Kind::Decidable { proof: "rung-host/tests/idle_mode.rs::an_idle_host_rests_and_a_scheduled_wake_is_one_free_pass".into() },
            numbering: None,
            prose: r#"At the scheduled wake the host takes **one free-time pass**, then rests again.
An idle host thus takes a free-time pass at least every idle time, and between
passes runs only on arrivals.

"#.into(),
            mechanism: r#"A simulated clock over several idle times: between consecutive rests there is exactly one free-time pass, last in the stretch (an item the agent scheduled for itself is drained first), and the first pass is at the start."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-rest-is-not-degraded".into(),
            parent: Some("an-idle-host-really-rests".into()),
            kind: Kind::Decidable { proof: "rung-host/tests/idle_mode.rs::a_world_imposed_wait_is_degraded_and_never_a_rest".into() },
            numbering: None,
            prose: r#"A rest is the host's own choice and is recorded as idle, never as `degraded`.
A wait the world imposes (backoff, quota, a blocked credential) is still
`degraded`, and is never recorded as rest.

"#.into(),
            mechanism: r#"A provider outage while an owner item waits: the wait is a `degraded` wait with no idle line inside it, no rest holds a `degraded` line, and the item is answered once the provider is back."#.into(),
        }),
        Element::Verbatim(r#"---

## 3 · Seeing it

"#.into()),
        Element::Prop(Prop {
            slug: "a-resting-host-is-seen-resting".into(),
            parent: None,
            kind: Kind::Decidable { proof: "rung-gateway/tests/native_doors.rs::a_resting_host_reads_idle_until_its_wake".into() },
            numbering: None,
            prose: r#"A resting host shows as **idle until `<time>`**, the time of its scheduled wake,
wherever the host is read: the instance index's listing and the gateway's
summary of the instance. It is not `needs_you`, and it is not `Waiting` (that is
the degraded wait). The line that ends the rest, and any halt, ends the display.

"#.into(),
            mechanism: r#"The summary and the list are both read from the record alone, so fixture records settle them: one ending in an idle line (and one with a channel opened after it) reads `Idle until <time>`, not `needs_you`, not `Waiting`, `waiting_for` the wake; one ending in the line that ends the rest reads awake; a degraded wait after a rest reads `Waiting`. The listing's `idle_until` and the `ls` table are settled the same way for a running host (`rung-host/tests/gate_registry.rs::a_resting_host_lists_as_idle_until_its_wake_and_a_stop_ends_it` on the binary, and the registry's own tail-reading test)."#.into(),
        }),
        Element::Prop(Prop {
            slug: "a-restart-wakes-the-host".into(),
            parent: Some("a-resting-host-is-seen-resting".into()),
            kind: Kind::Decidable { proof: "rung-host/tests/idle_mode.rs::a_host_that_died_resting_wakes_on_restart_takes_one_pass_and_rests_again".into() },
            numbering: None,
            prose: r#"A rest does not outlive its process. A host that starts on a record ending in an
idle line is awake: it answers what waits, takes one free-time pass, and rests
with a new scheduled wake.

"#.into(),
            mechanism: r#"Run a host, keep its record up to its first idle line (what a kill mid-rest leaves), start again on that state two hours later and read the record: it recovers, is awake as of its start line, takes one free-time pass, and rests with a wake idle time after that pass."#.into(),
        }),
        ],
    }
}
