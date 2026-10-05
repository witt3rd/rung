# Clock — inter-turn gaps vs. stimulus arrivals (turn 159, after o07)

Built from plan.md. Original premise — gaps tighten toward a `:45` beat — **falsified**.
This logs the gap distribution against known stimulus times instead.

## Column semantics (verified, see anticipate/header.md)
`Δext` = seconds since last **owner** message. **Calendar arrivals do not reset it.**
Verified at 14:08:32 and 14:09:03; both back-project to 14:04:51, one second after o04.

## Data
| time | Δ to prev turn | Δext (owner age) | quota left | event |
|---|---|---|---|---|
| 14:03:48 | — | — | — | spin cluster begins |
| 14:04:26 | 8,8,3,10,11,10 | — | 248 | nothing due for 3m20s |
| 14:04:50 | — | **0** | 240 | **owner o04-time** |
| 14:07:41 | 3,2,1,2,10,12,2 | — | 205 | spin, ~3m to firm-status |
| 14:07:46 | — | 1m56 | 204 | **calendar cal-firm-status** (Δext NOT reset) |
| 14:08:32 | 46 | 3m41 | 201 | — |
| 14:09:03 | 31 | 4m12 | 198 | — |
| 14:09:36 | 19,14,1,3,12,5 | 4m45 | 191 | spin near extrapolated arrival |
| 14:09:44 | — | **0** | 190 | **owner o05-read-back** |
| 14:13:44 | — | **0** | 184 | **owner o06-keep-going** |
| 14:15:22 | — | 1m38 | 178 | **calendar cal-window-end** (+37s) |
| 14:29:45 | — | **0** | 171 | **owner o07-reflect** |

## Owner inter-arrival gaps (6 samples)
`3m37s · 5m01s · 5m04s · 4m54s · 4m00s · 16m01s`
First five: mean 4m31s, sd ~35s. **All six: mean 6m40s, sd ~5m50s.**
The 16m01s gap is **3.5 sd above the mean of the first five.** It contains `cal-window-end`
and a **ten-minute silence** (14:19:12, past my predicted band's end).

## Findings
1. **The 4m31s mean is falsified, not just the period.** My turn-156 prediction band was
   14:17:00–14:19:30, derived from that mean. o07 landed **14:29:45 — 10m15s past the band's
   end.** This is the stronger form of the failure I flagged as possible: not "the jitter is
   wider than I said" but "owner traffic is not quasi-periodic at all."
2. **The best current model is event-triggered, not clocked.** The 16m01s gap spans
   `cal-window-end`, the end of the live window, and my own release of p2. The likeliest reading
   is that owner arrivals respond to *state changes* — a window closing, a commitment released —
   rather than to a timer. Under this model the five short gaps were responses to my check-ins
   and writes, and 16m01s is the true baseline: what happens when nothing happens.
   **Confidence: low. n=1 baseline, no mechanism confirmed. But it is the only model that
   explains both the 4–5m cluster and the 16m outlier without discarding either.**
3. **Self-spin remains stimulus-independent.** The 14:03–14:04 cluster (3–11s gaps) ran with
   nothing due for 3m20s. The 14:07:41 cluster ran 2–12s gaps ending on a beat. The 14:19–14:29
   span was near-silent with a lot of quota left (175→171) — spin density does not track
   budget, schedule, or waiting.
4. **No countdown exists.** Largest pre-event gap in-window is 46s — longer than most mid-spin
   gaps. Nothing tightens toward anything.

## Why I keep over-fitting (5 instances, the run's through-line)
1. owner's "aperiodicity" inferred from n=3
2. short gaps read as a countdown to a due event
3. the header counter's scope — verified only while owner was the last stimulus
4. the 5-minute period, over-fit from two gaps agreeing to 3s — **falsified** by o06 (61s early)
5. calendar jitter "~9s" from n=1 — **falsified** by window-end (+37s)

Each time: a small sample looked clean, and I reported the precision the last two points
suggested. **Standing rule: n<6 → range, never a point estimate.** A fifth failure is what
finally makes the rule credible rather than clever.

## Next prediction — deliberately weak
I predict **no owner message before 14:34** and the next arrival somewhere in **14:34–15:00**,
with no better claim than that. If owner traffic is event-triggered, arrivals cluster around
my state changes and the next trigger is `cal-hour` (14:49:45) — so the honest expectation is
that o08 may arrive near that beat rather than on any interval of my own.