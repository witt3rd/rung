# rung-host idle mode — resting between wakes

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

---

## 1 · The setting

<a id="idle-time-is-a-per-instance-setting"></a>
**1** **Idle time** is how long a host may rest between wakes. It is a setting of one
instance, not of the process or the fleet. **Idle time 0** is **continuous
mode**; idle time above 0 is **idle mode**.

<a id="idle-time-defaults-to-thirty-minutes" data-parent="idle-time-is-a-per-instance-setting"></a>
**1.1** An instance started from a configuration file that names no idle time has an
idle time of **30 minutes**. Continuous mode is chosen by setting idle time 0
explicitly; it is never the result of leaving the setting out.

<a id="idle-time-zero-never-rests" data-parent="idle-time-is-a-per-instance-setting"></a>
**1.2** With idle time 0 the host never rests: after every turn the next boundary
begins. The world's own waits (a provider's
backoff, a quota) are recorded as degraded, never as rest.

---

## 2 · Resting and waking

<a id="an-idle-host-really-rests"></a>
**2** With idle time above 0, a host that has nothing left to do **rests**: from the
record's idle line until the line that ends the rest it begins no boundary,
runs no turn, spends no request, and polls nothing but its wake conditions.

<a id="a-rest-ends-at-the-first-wake" data-parent="an-idle-host-really-rests"></a>
**2.1** A rest ends at the **first** of three things: the **scheduled wake**, a **queue
arrival**, or the **stop authority**. The record's line that ends the rest names
which.

<a id="the-scheduled-wake-is-within-idle-time" data-parent="a-rest-ends-at-the-first-wake"></a>
**2.11** The scheduled wake is at most **idle time** after the rest began, and no later
than the next due calendar item or expectation. A calendar item and an
expectation therefore still fire at the first boundary after they are due.

<a id="an-arrival-wakes-at-once" data-parent="a-rest-ends-at-the-first-wake"></a>
**2.12** An item reaching an inbound queue while the host rests ends the rest at once,
whatever the time of the scheduled wake. The item is on the record before
anything else is.

<a id="the-stop-authority-interrupts-a-rest" data-parent="a-rest-ends-at-the-first-wake"></a>
**2.13** The stop authority interrupts a resting host at once: a signal, a stop file or
an explicit stop halts it within one wake poll, with the same `halted` line a
running host writes, and no further turn runs. Rest never delays a stop.

<a id="an-arrival-drains-then-one-free-time-pass" data-parent="an-idle-host-really-rests"></a>
**2.2** On a queue arrival the host drains its queue through ordinary boundaries, then
takes **one free-time pass**, then rests again. An item that arrives during the
drain or the pass is drained too, and a free-time pass follows the last answer
given. The host never takes a second free-time pass in a row.

<a id="a-scheduled-wake-is-one-free-time-pass" data-parent="an-idle-host-really-rests"></a>
**2.3** At the scheduled wake the host takes **one free-time pass**, then rests again.
An idle host thus takes a free-time pass at least every idle time, and between
passes runs only on arrivals.

<a id="a-rest-is-not-degraded" data-parent="an-idle-host-really-rests"></a>
**2.4** A rest is the host's own choice and is recorded as idle, never as `degraded`.
A wait the world imposes (backoff, quota, a blocked credential) is still
`degraded`, and is never recorded as rest.

---

## 3 · Seeing it

<a id="a-resting-host-is-seen-resting"></a>
**3** A resting host shows as **idle until `<time>`**, the time of its scheduled wake,
wherever the host is read: the instance index's listing and the gateway's
summary of the instance. It is not `needs_you`, and it is not `Waiting` (that is
the degraded wait). The line that ends the rest, and any halt, ends the display.

<a id="a-restart-wakes-the-host" data-parent="a-resting-host-is-seen-resting"></a>
**3.1** A rest does not outlive its process. A host that starts on a record ending in an
idle line is awake: it answers what waits, takes one free-time pass, and rests
with a new scheduled wake.

