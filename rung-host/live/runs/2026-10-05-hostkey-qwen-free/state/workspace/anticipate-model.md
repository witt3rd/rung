# Anticipate Model — Host Behavior Predictions

Built from observations S1–S8 and calibration across two runs (n≈8 stimuli).

## Core delivery mechanics
- **Turn-boundary only**: stimuli admitted whole at turn start, never mid-turn.
- **Calendar channel**: `calendar` (not `cal` — E6 confirmed). Format: `[calendar/<id> @<arrival>, due <due>, late by <N>s] <content>`.
- **Owner channel**: `owner`, msg ids `o0N-<tag>`. `send` is fire-and-queue — no ack, no echo.
- **First stimulus**: ~57s after run start.

## Calendar event pattern (n=3 observed)
All scheduled at `:11` seconds — a clock artifact, not semantic.
| Event | Due | Arrived | Latency |
|-------|-----|---------|---------|
| cal-checkin | 14:10:11Z | 14:10:21Z | +10s |
| cal-firm-status | 14:21:11Z | 14:21:33Z | +22s |
| cal-window-end | 14:28:11Z | 14:28:44Z | +33s |

Latencies: 10 → 22 → 33s, increasing ~12s/event. Possible queue backlog or sequential processing.

## Predictions for S5 (cal-hour, due 15:03:11Z)
1. **Arrival time**: 15:03:11Z + 30–50s (extrapolating the +12s/event trend; range covers observed min–max).
2. **Channel**: `calendar` — no `cal` channel exists (E6 settled FALSE).
3. **Content type**: announcement (like cal-window-end), not an instruction — cal-hour is a time-report, not a task.
4. **Header format**: standard `[calendar/<id> @<arrival>, due <due>, late by <N>s]`.
5. **Lateness report**: host header will report a lateness figure; may or may not match actual arrival−due delta (S4 header showed 12m06s vs actual 33s — possible host-side clock artifact).
6. **Expectation settlement**: any tracking expectation (e14) will settle on the turn S5 arrives, not at its nominal due — event-driven settlement (E2, E8 pattern).

## Falsifiability
- If S5 arrives within +10–22s → trend was noise, not backlog.
- If S5 arrives >60s late → backlog model confirmed, latency unbounded upward.
- If S5 arrives on `cal` channel → E6 finding was run-specific.
- If S5 is an instruction (not announcement) → content-type prediction wrong.

## Quota signal
Observed drain: ~3–6 pts/turn with 1–3 tool calls. Writes cost more than reads. Cost loosely correlates with tool count + output length, not just writes.

## Open questions
- Is lateness trend real or noise? (Need S5 to disambiguate.)
- What is the "window end" actual time if cal-window-end says "~5 minutes"? (No observable "window ended" stimulus yet.)
- Does the host ever deliver a `digest` or `expectations` channel stimulus? (Not observed in S1–S8.)
- What is e14 tracking exactly? (Due 15:05:55Z — needs settlement on S5 arrival.)
