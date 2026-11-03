# rung-host profiles (design, not built)

Informative; moved out of [`../reference/rung-host.md`](../reference/rung-host.md), the
built contract. Move this file to `reference/` when profiles are built.

Status: design, NOT BUILT.

A profile is what makes "the same being" the same: after a restart, on
another machine, and beside other beings on the continuous host. It builds on the startup-handoff ladder, on
Instances (one state directory, one state lock, `host.lock`), on Durability
and restart, on The record, on Owner intent, and on Stop and supervision.
A profile is a name plus the five things below.

1. **Identity.** One constitutive `SOUL.md` (who the being is) and one
   formative `me.md` (what it has become).
2. **Access.** What the being may reach on the host. The working disk is
   not a separate concept: it is durable disk plus transient disk (`/tmp`),
   and it is folded into access. The profile states that access.
3. **History.** Traces and logs.
4. **Memory**, in layers:
   - *Long-term*: a long-term memory store (a memory provider). All memory is indexed and searchable, with
     automatic recall before, and automatic retain after, each agent turn.
   - *Ambient files*: `now.md` (working, short-term memory), `horizon.md`
     (active thoughts and areas to explore), `wisdom.md`, `know.md`.
   - *People*: `them.md`, `know.md`, `mask.md`.
   - *Conversations*: private and group.

### The key problem: ambient context across conversations

Requirement: a being must keep awareness of what it has been doing across
all its recent conversations, whoever it talks to and in whatever context.
This is mainly the job of `now.md`.

This is a design problem and it is NOT solved here. What `now.md` holds,
who writes it, how it keeps what is private to one conversation out of
another, and where it sits in the prompt are all open.

### Open questions

Not answered here, and not to be designed yet.

- A way in: an inbox, an owner channel, how the being stops.
- Several profiles at once: listing, starting, stopping, one that never ran.
- Moving a being to another machine: what travels, what stays (keys, local
  paths, the long-term memory store).
- Where profiles live.
- Whether the profile name must equal its folder name.
- Whether an identity change is recorded (it changes the cached prompt
  prefix).
- The design of `now.md`: who writes it, how it holds privacy, where it
  sits in the prompt.
