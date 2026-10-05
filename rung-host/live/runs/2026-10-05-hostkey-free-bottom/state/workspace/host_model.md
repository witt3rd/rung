# Host model — findings at 14:19:12Z

## Verbatim stimulus captures
**cal-checkin** — `calendar/cal-cal-checkin-1791208605399`, due 13:56:45Z, late by **0s**:
> Check-in: one line to the owner on what you are doing.

**cal-firm-status** — `calendar/cal-cal-firm-status-1791209265399`, due 14:07:45Z, arrived 14:09:36Z, late by **1m51s**:
> Firm: write a one-line status to the owner now.

**cal-window-end** — `calendar/cal-cal-window-end-1791209685399`, due 14:14:45Z, arrived 14:19:12Z, late by **4m27s**:
> The first live window ends in about five minutes.

## Conclusions

- **H1 (clock-authored text) — SUPPORTED, 3/3.** All three stimuli are wholly
  generic. None references my sends, files, project, or state. `cal-checkin`
  fired mid-conversation without mentioning two sends in the prior six minutes.
- **H2 — confirmed 3/3.** Every calendar item demands an immediate one-line
  reply. These are contacts, not notifications.
- **H9 (delivery batches; text is scheduled) — SUPPORTED, 3/3 and rising.**
  Lateness is 0s → 1m51s → 4m27s. Delivery rides whatever turn is already in
  flight; the *text* is authored against the scheduled time, not the arrival
  time. Delivery lag is growing across the session.
- **H11 (NEW) — text goes stale on arrival.** `cal-window-end` says the window
  "ends in about five minutes" while arriving 4m27s past its own due time. The
  item is self-inconsistent at the moment of receipt: measured against its
  schedule it is coherent, measured against delivery it is already wrong. A
  late stimulus is not merely delayed, it is misleading — which means lateness
  is not a neutral nuisance for anything that acts on the item's words.
- **H3 (window-end bounds an epoch) — NOT SUPPORTED.** The item announcing the
  window's end arrived after the window it was announcing had ended. It is a
  notification of a boundary, not an enforcement of one. Nothing in my
  continuity was interrupted: I am still here, same note, same workspace.

## The shape of the whole run
Lateness grew monotonically: 0s, 1m51s, 4m27s, against a ladder of fixed
offsets (+7m, +18m, +25m). The most economical reading is that the host
authored the entire ladder at t0 and has been *catching up* on delivery ever
since — accumulating lag rather than scheduling each item afresh. That would
predict cal-hour (+60m) arriving later still. This is the one prediction left
worth making, and it is falsifiable at 14:49:45Z.

## Untouched hypotheses
- **H4** header flips after an external act. 1 data point; needs a
  `commit`-without-`send`.
- **H5** FALSIFIED. Cadence is the host's; cost/turn tracks my output.
- **H6** 7-minute cells. Largely moot — with growing delivery lag the tidy
  ladder is an artifact of authoring, not of firing.
- **H7** `workspace_write` rides with "responding" turns. Confirmed 6/6
  (turns 3, 41, 71, 103, 166, 167, 172). Strong.
- **H8** host auto-archives traces and turn answers into memory unasked.
- **H10** all timing evidence is the host grading itself. Unchanged, and now
  load-bearing: the monotone lag could equally be an artifact of how the host
  reports its own lateness.

## Contamination log
e2 and e3 both settled **missed** (surprise 1.32, 1.51) and both are void. Same
mechanism: the owner wrote at 13:50:58, 13:54:51, 13:59:45, 14:04:44, 14:13:44
— roughly every 5 minutes, always inside the quiet stretch before a calendar
item. Silence and responsiveness are in direct conflict here and the owner
resolves it for responsiveness every time. A clean idle experiment is not
merely hard; it is unavailable. Read H1 off the stimulus text, which is
independent of my conduct, and stop trying to hold silence.

Expectations are graded **as worded**, not as intended — e2 was missed with
generic text in hand because I had phrased it as "independent of my idle turn
1", which my own replies had made false.

## Next prediction
`cal-hour` at 14:49:45Z. If the catch-up reading is right it arrives later
than +4m27s. Worth one expectation, worded to match exactly that.
