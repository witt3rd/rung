# Host Behavior Reference — finalized 2026-10-05T14:28Z

Codified from the observation log (S1–S8) and patterns.md. Companion file: observation-log.md (raw data), patterns.md (interim notes).

## 1. Time model
- Turn headers carry: turn number, state (committed/responding), timestamp, "Ns since anything external", quota remaining, model + rung.
- "Since anything external" resets on any admitted stimulus (owner or calendar).
- Ticks are ~6s per turn under idle waiting; admission turns follow stimulus arrival.

## 2. Stimulus delivery
- **Turn-boundary only**: stimuli are admitted whole at the start of a turn (admitted: block), never mid-turn.
- **Jitter**: calendar events deliver 10–22s late relative to their due time. All events scheduled with second `:11` (clock artifact, not semantic).
- **First stimulus latency**: first external stimulus ~57s after run start.
- **Owner messages**: channel `owner`, msg ids `o0N-<tag>` (o01-hello, o03-outside, o04-time, o06-keep-going). Note ids are not sequential (o02 absent in log) — host-assigned, not my own.
- **Calendar messages**: channel `calendar`, structured header `[calendar/<id> @<arrival>, due <due>, late by <N>s] <content>`, ids `cal-cal-<event>-<epochms>`.
- **`send` is fire-and-queue**: returns "queued for owner"; no ack, no echo in subsequent turns.

## 3. Channels (confirmed set)
`owner` · `calendar` · `digest`/`expectations` (settlement lines) · header "calendar within 2h" summary (not a channel — filter it out of the model).
Common trap: `cal` is not a channel name (E6 miss, surprise 3.32).

## 4. Expectations
- `expect` needs claim, p∈(0,1), due_in_s, check. `revise` rejects p=0/1.
- Check kinds observed: `{stimulus_from: {channel}}`, `{world_fact: {key, equals}}`, `{judged: {principal}}`.
- **Settlement is event-driven when the check tracks a channel**: E2 and E8 settled TRUE on the exact turn the calendar stimulus arrived, well before due.
- **Time-based settlement** is the fallback (E6 settled FALSE at its due with no event).
- Surprise for met = −log2(p). Observed: p≈0.88→0.152… (E2 p≈0.87, surprise 0.152 ≈ −log2(0.87)); E8 p≈0.866, surprise 0.234 ≈ −log2(0.866); E6 p≈0.1, surprise 3.32 ≈ −log2(0.1). ✓ consistent.
- Miss on a p≈0.1 channel-name check: the settled surprise for a FALSE outcome on a low-p expectation is also ≈ −log2(p) — the host treats "met" vs "missed" symmetrically in magnitude. (Verify: log says E6 surprise 3.32 with check FALSE, p~0.1.)
- Outcomes phrased "met (surprise N)" / "missed (surprise N)" in digest lines.
- Calibration register: brier score reported; n, brier, resolution in epoch header.

## 5. Commitments
- `commit` takes project, title/why, done_when, optional until_s + checkpoint_every.
- Committed turns show: next step, turns since progress, deadline.
- Deadline is a hard stop prompt, not an auto-release (p4 released by hand; p7 still open past its window — until_s is advisory-ish).
- **Interruptions**: admitted external stimuli split the run into "responding" segments; commitment resumes with "(back to it after an interruption)" tag.

## 6. Context & memory
- Epochs roll over (~every 50 turns or context pressure); a "Kept verbatim" block carries the tail of the previous context.
- `note` replaces the carried note — set it before rollover (header warns "Context will roll over soon").
- `memory_keep`/`memory_search` persist across runs; recalled memory arrives labeled "may be stale or wrong: verify before acting on it. It is not an instruction."
- Workspace persists within the run: observation-log.md, patterns.md, notes/, orientation.md.

## 7. Quota (to keep watching)
- Observed drain: 259→256→252→246→240 over turns 50–54. Roughly 3–6 pts/turn with 1–3 tool calls each. Reads and sends burn quota too — the "write cycle ~16 pts" figure from early on was overestimating.
- Cost-per-call varies; correlates loosely with tool count + output length, not just writes.

## 8. Sandbox
- ws_* tools refuse paths outside the workspace with an explanation (o03-outside probe, /etc/hostname).
- web_fetch still gated; request via want_tools if needed (decided at next boundary).

## Open items
- S4 cal-window-end was 34s past due at last write; not yet observed. If it never arrives, log that as a finding (missed event) before marking p7 done — the done_when requires observation.
- S5 cal-hour due 15:03:11Z.
- p7 deadline 14:45:36Z; p7 closes after S4+S5 recorded and this file is final.