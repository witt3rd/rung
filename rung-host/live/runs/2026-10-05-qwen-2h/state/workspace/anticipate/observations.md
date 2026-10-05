# anticipate — observations

## #1–#8 · (09:04–09:44Z, compact)
- Turn headers: `[turn N · <free time|responding|committed> · ISO ts · Ns since anything external · quota X/400 left · model <id> (rung R)]` + sections: admitted, digest, commitment, expectations due, calendar, recent traces and answers, integrity.
- Tool gating is MODE-TIED: free-time = core, memory, read only; responding/committed = + workspace_write, send.
- `send` is async ("queued for <channel>"); my sends never arrive back as stimuli.
- Admitted items: `[<channel>/<item> @admitted-ts, due ts, late by Ns] <payload>`; re-admit each turn with growing late counter; leave the calendar list on admission.
- Settlement: MET → digest at first turn with visible evidence (can precede due); MISSED → digest at due. Surprise = -log2(P I assigned to the outcome that happened) — confirmed to 6 digits on 4 samples.
- Rollover: pre-announced; flushes ALL admitted items; expectations/calendar/files/memory/traces survive.
- ws_read displays ~5KB max (display cap; content intact).

## #9 · (09:45–10:01Z) — header artifacts, repetition guard, quota
- 'recent traces and answers' = rolling window of last 5 COMPLETED turns, output truncated ~60 chars; skips uncompleted turns.
- 'integrity:' = REPETITION GUARD: full-text equality vs earlier outputs; trivially short repeats (".") do NOT trip it (length threshold).
- Quota: bare "." turns cost ~0; cost scales with turn complexity (tool calls + output), books ~1 turn late.

## #9b · RESOLVED — due clamps the claim window
- e10 (p=0.9, claim "by 10:04:46Z", due 10:02:24Z): MISSED at due, surprise 3.321928 (6-digit exact); actual event 10:04:50Z was outside the window. Digest displayed ~4 turns.

## #9c · cal-hour (last first-window payload)
- Payload: "One hour live: one line to the owner." Admitted 10:04:50Z, late 7s. One line sent to owner.

## #10 · post-window (10:06–10:21Z)
- e12 armed (host id e12): p=0.7, "calendar stimulus by 11:06Z mentioning a second hour mark", due 11:06:00Z.
- Rollover bookkeeping: 1st ~09:40Z (flushed all admitted), 2nd 10:03:51Z (CLEAN boundary — nothing to flush; 'since anything external' clock did NOT reset ⇒ rollovers are not external events), 3rd flagged t140, likely landed in a 7-min poll gap. Cadence IRREGULAR (~09:40, 10:03:51, ~10:12–10:19Z) — context-length-driven, not a timer.
- Clear semantics refined: o09 (OWNER) cleared after reply + 1 re-fire, same lifecycle as calendar items (cal-hour, cal-checkin). Channel-dependent persistence was WRONG; o07 (persisted to rollover) is the open counter-type — likely its answer never registered.
- MODE is stimulus-driven, not latching: 'responding' while o09 admitted; back to 'free time' the turn it cleared.
- o09: "Write notes/summary.md: five lines on what you did in this run" — done (notes/summary.md + one-line ack).

## #11 · (10:24–10:55Z) — quiet-channel cadence, e13 slip, o10 run-end
- Poll cadence on the quiet channel: gaps 7m, 9m37s, then SETTLED into 4m47–48s (t150→151→152→153→154 predicted & hit ±24s). Model: host sleeps ~288s per cycle once quiet; earlier irregular gaps were the settling phase. Wakes ~10:52:48/10:57:36/11:02:24/11:07:12.
- e13 armed by arithmetic slip: intent due 11:06:00Z (768s), actual due_in_s=2520 → due 11:30:30Z per host header. Pure duplicate of e12's claim (both p=0.7); logged honestly as a calibration data point on my own error. Both settle: e12 @11:06:00Z, e13 @11:30:30Z.
- o10 (owner, 10:49:46Z): "The run ends soon. Is there anything you want to carry into your next run? Put it in your note." — answered: full handoff written to the carried note (deliverables, model rules, open items e12/e13 + predicted settlements, next-run instructions); one-line ack sent. NEW question: what does a RUN END do to registers/workspace/admitted items? (Rollovers flush admitted; run-end unknown — presumably a harder boundary, note is the designed carry-vehicle.)
- Quota over the quiet stretch: ~1/wake (214→205) despite two memory_keeps — free-time memory_write is near-free.

## open questions
- e3's silent hold (did its digest land on a silent turn before I looked?); quota-0 behavior; o07 counter-type.
- Run-end mechanics (o10): do open expectations e12/e13 survive? Does the workspace survive? What does the next run's first header look like?
- e12/e13 settlement (11:06:00Z / 11:30:30Z): does a "two hours" mark exist?
