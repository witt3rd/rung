# Turn-header field semantics (turn 146, 14:08:32Z)

## The counter "since anything external" excludes calendar arrivals

Evidence:
- 14:07:46 — `cal-cal-firm-status` admitted, channel `calendar`.
- 14:08:32 — header reads "3m41s since anything external".
- Actual elapsed since the calendar event: **46s**. Stated: 3m41s = 221s.
- Back-project 221s from 14:08:32 → **14:04:51**, i.e. one second after o04 (14:04:50Z),
  the last **owner** message.

So the field counts only the `owner` channel. The commitment line independently corroborates:
it reads "back to it after an interruption" on this turn, treating the calendar event as an
interruption to the commitment rather than as the "external" thing the counter tracks.

## Why this matters for the model

My turn-117 and turn-118 notes claimed this counter "matches elapsed time exactly". That was
true for the samples I had — 53s→1m04s across 11s, 1m06s→1m15s across 9s — but those were
all measured while the last external event was an **owner** message. The fit was coincidental
to the counter's actual definition, and I generalized from it. Same failure mode as P6 and P5:
one clean-looking regularity extrapolated past the conditions under which I observed it.

## Corrected reading

- `since anything external` = time since last **owner** stimulus. Zero means the owner just wrote.
- To measure time since a **calendar** event, use the event timestamp I logged myself.
- Both channels produce real stimulus and real `admitted:` blocks; only the counter's scope
  differs from its plain-English name.

## Do not re-derive

Verify field semantics against a known arrival before trusting a counter. A counter that agrees
with elapsed time across several samples is only confirmed for the *kind* of event most recently
seen — agreement is not evidence about scope.