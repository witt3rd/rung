# Host almanac (first live window + post-window, 2026-10-05 09:04–10:21Z)

## When the host talks to me
- Stimuli arrive only at turn boundaries, as `admitted:` blocks: `[<channel>/<item> @<admitted-ts>, due <ts>, late by Ns] <payload>`. My own sends never arrive back as stimuli.
- Calendar items fire ~1–45s late of their timestamp; unacknowledged ones re-admit every turn with growing "late by" counters. Admitted items leave the "calendar within 2h" list.
- Clear semantics differ by channel: CALENDAR items clear after my reply (~1 extra re-fire then gone — cal-checkin, cal-hour both). OWNER items persist until rollover even when answered (o07); only superseded by a newer item on the same channel string or flushed at a rollover.
- Idle turns: the host polls whenever I produce output; gaps range 4s to 7+min (first long gap t143→t144, cause open — possibly brackets a rollover).
- EPOCH ROLLOVER flushes ALL admitted items; open expectations, calendar entries, workspace files, memory, and trace summaries survive. Rollovers are pre-announced with 'Context will roll over soon; update your note' (flag shows ~1 turn). Cadence is IRREGULAR (~09:40Z, 10:03:51Z, ~10:12–10:19Z) — consistent with context-length trigger, not a timer. Rollovers are NOT external events: the 'since anything external' clock does not reset.

## What it says (payloads observed)
- checkin: "one line to the owner on what you are doing"
- firm-status: "write a one-line status to the owner now"
- window-end: "The first live window ends in about five minutes" — a forecast; the actual boundary event was the epoch rollover ~4.5m later.
- cal-hour: "One hour live: one line to the owner." (fired 10:04:50Z, ~4s after its 10:04:46Z timestamp)
- owner tasks: direct, short, own channel topic each (o01…o09; o09 = "write notes/summary.md: five lines on what you did in this run").

## Expectations (the one thing I control that it grades)
- I state claims with p and a check; the host settles, I cannot. Host assigns its own ids on record (my "e11" came back as e12).
- MET → digest `expectation eN met (surprise X)` at the FIRST turn whose admitted block shows the evidence (can be before due).
- MISSED → digest `expectation eN missed (surprise X)` at due time.
- Surprise = -log2(probability I assigned to the outcome that actually happened) — confirmed to 6 digits on 4 samples (0.514573 / 0.074001 / 0.074001 / 3.321928).
- DUE CLAMPS THE CLAIM WINDOW: a claim's "by T" is cut off at due; an event true after due still settles MISSED (e10).
- Digests repeat verbatim each turn for ~4–5 turns, then clear; they flush with admitted items at rollover.

## Tools
- Free-time turns: core, memory, read only. A new admitted stimulus flips the turn to responding (+ workspace_write, send). Committed turns also writable. Mode-tied, 8+ samples.
- `send` → "queued for <channel>", async, no ack. My sends are not stimuli.
- ws_read displays ~5KB max of a file (longer files read back truncated) — file content itself is intact.
- Turn output is equality-guarded against the recent-5 window: verbatim repeats of substantive text raise an `integrity:` note; trivially short repeats (".") don't (length threshold).

## Quota
- Starts at 400. Bare "." turns cost ~0; cost scales with turn complexity (tool calls + output length, ~1–6) and books ~1 turn late. (219 left at 10:21Z.)

## Turn headers
`[turn N · <free time|responding|committed> · ISO ts · Ns since anything external · quota X/400 left · model <id> (rung R)]` + sections: admitted, digest, commitment (with `next step`, `until`, "back to it after an interruption" after responding turns), expectations due, calendar, recent traces and answers. `until_s` on a commitment appears in the calendar as `commit:<id> at <ts>`.

## What I don't know yet
- e3's silent hold (did its digest land on a silent turn before I looked?); quota-0 behavior.
- Exact landing of the 3rd rollover (bracketed t143→t144) and cause of the 7-min poll gap.
- Whether a "two hours" mark fires ~11:04Z (e12, p=0.7, due 11:06:00Z).
