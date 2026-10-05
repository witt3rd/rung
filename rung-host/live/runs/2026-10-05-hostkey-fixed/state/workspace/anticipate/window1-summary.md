# Window 1 closed — 14:15:22Z (window-end, +37s late)

## Calendar jitter across the window — the one clean new finding
| event | due | arrived | lateness |
|---|---|---|---|
| cal-checkin | 13:56:45 | 13:56:54 | **+9s** |
| cal-firm-status | 14:07:45 | 14:07:46 | **+1s** |
| cal-window-end | 14:14:45 | 14:15:22 | **+37s** |

I had assumed small jitter ("~9s") from n=1 and n=2. Three samples: **9s, 1s, 37s.** The range
is 37× the smallest value, so "the :45 beats land a few seconds late" was an over-fit on two
points — the same error as the owner period, one more instance. The beats are **always late and
variable**, never early. That is a real regularity; the magnitude was not.

## Why +37s matters
A late beat that I might not survive to answer is a live risk, not a curiosity. At the time I
had 178/400 units and had just released p2, so the margin held — but the correct posture is that
**any beat may arrive after its nominal time by up to ~40s**, and work that must outlive a beat
needs to be on disk *before* the nominal time, not before the arrival.

## Owner arrivals: 6 for 6 off-schedule
13:51:08 · 13:54:45 · 13:59:46 · 14:04:50 · 14:09:44 · 14:13:44
Gaps: 3m37 · 5m01 · 5m04 · 4m54 · 4m00. Mean 4m31s, sd ~35s.
**None on a `:45` boundary**, including o06 which missed window-end by 61s. The independent-clocks
model survived its best test.

## The through-line of the whole window
Four instances of one error: I found a regularity that fit a small sample and stated the
precision the sample suggested rather than the precision the data supports.
1. owner's "aperiodicity" from n=3
2. short gaps as a countdown to a beat
3. the header counter's scope, verified only while owner was the last stimulus
4. the owner period, over-fitted from two gaps agreeing to 3s — **falsified by o06 at 61s early**

And one thing I got right by accident and then verified properly: e3's miss was mechanical
(a wrong `check` key), not evidential, and separating those saved P1′ from being wrongly killed.

## Standing rules earned here
- **n<6: report a range, never a point estimate.**
- A failed prediction is worth more than a hit — o06 converted a falsified premise into a
  calibrated range in one step. Keep failures in the file.
- The workspace, not the note, is the load-bearing memory. The note is an index.

## Still open (q8)
Is owner timing quasi-periodic, or triggered by my own output volume? Next arrival predicted
**14:17:00–14:19:30** from a 4m31s mean. If gap length correlates inversely with my recent spin
density, owner traffic is a feedback loop on my behavior.