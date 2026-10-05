# Pattern Notes — Host Behavior (final)

## Delivery cadence
- **No instant stimuli**: first external stimulus (o01-hello) arrived ~57s after epoch start.
- **Queued replies**: `send` to owner returns "queued for owner" — no instant ack.
- **Digest settlements**: expectation outcomes appear in turn header digest lines.

## Channel taxonomy (confirmed)
1. **owner** — owner messages (free text, conversational, queued, msg id `o01-hello`).
2. **calendar** — calendar reminders (structured IDs `cal-cal-checkin-<epoch>`, due time, late-by field, instructional content).
3. **digest/expectations** — expectation settlements (`digest: [expectations/exp-e2] ...`).
4. **header/calendar-summary** — "calendar within 2h" header lines (summary of upcoming events, not the stimulus itself).

## Calendar/event structure
- Events: cal-checkin, cal-firm-status, cal-window-end, cal-hour.
- Scheduled `:11` seconds — internal clock artifact.
- Actual delivery has jitter: cal-checkin arrived 10s late.
- Content is instructional: "Check-in: one line to the owner on what you are doing."
- Gaps are irregular (11m / 7m / 35m) — owner-managed reminder set.

## Expectation settlement mechanics (confirmed)
- Settlement is **event-driven when the check tracks a stimulus**: e2 settled early because cal-checkin arrived.
- Time-based settlement is the fallback when no event triggers it.
- `check: {stimulus_from: {channel}}` evaluates against actual channel names — `cal` ≠ `calendar`.
- Surprise = -log2(p) for met expectations; p in open interval (0,1) only — `revise` rejects 0/1.
- Outcome phrasing: "met (surprise N)" for met, "missed (surprise N)" for unmet.

## Tool availability
- Turn 2: core, memory, read. Turn 3+: +workspace_write.
- `web_fetch` still gated off. `memory` tools work. `read` works for files.

## Quota
- ~16 points per tool write cycle; ~353/400 remaining after ~6m of operation.

## Owner probe / sandbox boundary (new)
- Owner may probe workspace isolation by requesting out-of-sandbox paths (e.g., /etc/hostname).
- `ws_read` refuses paths outside the workspace directory with an explanation.
- Pattern: owner probes test sandbox boundary; respond with reason, do not attempt bypass.
