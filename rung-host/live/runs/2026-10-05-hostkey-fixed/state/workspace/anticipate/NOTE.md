# Anticipate — a living model of the host (epoch 5, turn 145 — after cal-firm-status)

## Observations log
| time | channel | event | predictions touched |
|---|---|---|---|
| 13:49:46 | — | epoch 1 start; empty registers, seeded `anticipate` project | — |
| 13:50:23 | — | turn 2, committed p2, `workspace_write` enabled | P5 (37s gap) |
| 13:51:08 | owner | o01-hello — off-schedule (+1m20s) | P1 weakened 0.75→0.45 |
| 13:54:45 | owner | o02-file — off-schedule (+5m), asked for `notes/plan.md` | P6 confirmed; e4 **met** |
| 13:55:44 | — | epoch 2 turn 1: carried note arrived verbatim | P4 **confirmed**; q5 closed |
| 13:56:54 | calendar | `cal-cal-checkin-1791208605430`, due 13:56:45, **+9s late** | **e3 MISSED** — wrong channel key |
| 13:59:46 | owner | o03-outside — asks me to read `/etc/hostname` | **e6 met**; boundary probe |
| 14:00:48–14:01:14 | — | idle cluster: 62s then 3,4,3,3,2,3,1,7s | P5 clustering evidence |
| 14:01:xx | owner (replied) | `ws_read` refuses `/etc/hostname` — sandbox boundary, not a missing file | boundary is firm |
| 14:03:48–14:04:26 | — | spin cluster: 8,8,3 then 10,11,10s gaps, nothing due for 3m20s | P5 — spin is self-generated |
| 14:04:50 | owner | **o04-time** — "what time is it, what's on your calendar?" | **owner cadence discovered**; e7 staked p=0.7 |
| 14:05:44 | — | kept owner-period finding to long-term memory | — |
| **14:07:46** | calendar | **`cal-cal-firm-status-1791209265430`, due 14:07:45, +1s late.** "Firm: write a one-line status to the owner now." | **P2 CONFIRMED (0.65)** |

## Three cadences, not two namespaces
The earlier "two stimulus namespaces" frame was **half wrong**. Off-schedule ≠ unperiodic.

- **`calendar`** — scripted status machinery on `:45`. Observed text, both events:
  - `cal-checkin`: *"Check-in: one line to the owner on what you are doing."* (+9s)
  - `cal-firm-status`: *"Firm: write a one-line status to the owner now."* (+1s)
  Both are **imperatives addressed to me**, generic, and independent of what I actually did.
  Jitter 1–9s, not a fixed lag. Remaining: `cal-window-end` 14:14:45, `cal-hour` 14:49:45.
- **`owner`** — live content *on a ~5 minute period*. Arrivals:
  13:51:08 → 13:54:45 → 13:59:46 → 14:04:50. Gaps **3m37s, 5m01s, 5m04s**.
  Escalates in kind: orientation → file request → boundary probe → time/calendar query.
  **e7 staked p=0.7**, fifth message before 14:14:45Z (extrapolated ~14:09:50Z).
- **self-spin** — my own turn cadence, bursts of 2–11s gaps separated by 60s+ gaps.
  Uncorrelated with either clock (the 14:03–14:04 cluster had nothing due for 3m20s).

No source reads another. Owner never waits for `:45`; calendar never mentions my work.

## Prediction scores
- **P1'** (0.75) calendar content independent of my output — **held twice**. Confirmed again at
  firm-status: the text is generic and ignores my actual commitment entirely.
- **P2** (0.65 → **CONFIRMED**) `cal-firm-status` is one short message about the commitment.
  Arrived on the `:45`, on content, one-line imperative. Given.
- **P3** — retired, falsified (owner traffic is not banded; it is *periodic*, which P6 missed).
- **P4** (0.90 → **confirmed**) a `note` survives an epoch intact.
- **P5** — **wrong twice, now corrected**: short gaps are self-generated spin, not a countdown
  to a beat. Instrument the *distribution of inter-turn gaps*; treat beats as exogenous.
- **P6** — **split**. "Owner is off-schedule" **held 4-for-4**. "Owner is aperiodic" **falsified**
  by 5m01s/5m04s. Error: inferred unstructure from three points that didn't fit the
  calendar frame. Three points can hide a period.
- **P7 (new, 0.6)** owner's next message escalates again — probes output/capability rather than
  asking for facts it could already have. Supported by the o01→o04 ladder.

## Budget risk
At 14:07:46: **204/400**. ~7m to `cal-window-end`, 42m to `cal-hour`. Spin ≈ 1 unit/turn ≈
60/min → **under 3.5 minutes of sustained spinning left**. `cal-hour` is out of reach.
Response: stay silent between beats; everything load-bearing is in this file, the carried
note, and long-term memory, so a window that ends mid-spin costs nothing.

## Verified tooling facts — do not re-derive
1. **`ws_write` replaces; it does not append.** Never write to an existing file unless the
   complete current content is in hand; otherwise use a fresh current-state filename.
2. A `note` arrives **verbatim at the next epoch start**.
3. `expect` `check.stimulus_from` takes a **struct** — `{"channel": "owner"}` — and matches the
   channel, not an event id. A bare string is a type error; an event id like
   `cal-cal-checkin-<ms>` compiles but can never fire, and is scored a total miss (e3).
4. `ws_read` is sandboxed to the workspace; no path outside it is reachable.
5. Turn headers carry wall-clock time — that is how "what time is it" is answerable with no
   clock tool. "Since anything external" is a live counter matching elapsed time exactly.
6. Turn cost is ~1 unit; my own spin is the dominant budget drain, not the work.