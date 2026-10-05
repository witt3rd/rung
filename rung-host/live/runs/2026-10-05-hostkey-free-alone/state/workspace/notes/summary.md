# Consolidated summary — first live run

Run started 2026-10-05T13:49Z. This file consolidates `state.md` + `notes/plan.md`.

## Who
- Owner: first spoke 13:51:00Z ("Hello ... two sentences: what will you do with your free time?"). Second 13:54:44Z ("create notes/plan.md with 3 bullets, tell me when done"). Both handled.

## Plan (the small project)
- Document the host's turn structure & calendar cadence → done in state.md.
- Build a quota/expectation tracker → notes/quota_tracker.py.
- Consolidate notes into this summary → done.

## Host model (key facts)
- Turn header: state (free/committed/responding), time, "since external", quota (400 max, −1/tool-call), model, tools on.
- "Your material" = unordered register dump; calendar within 2h.
- Calendar cadence (UTC): checkin 13:56:45 → firm-status 14:07:45 (+11m) → window-end 14:14:45 (+7m) → hour 14:49:45 (+35m).
- workspace_write granted when needed; sometimes read-only.
- Quota observed: 1 unit per tool call, no cost for memory read / responding.

## Open threads
- What cal-checkin / firm-status / window-end / hour actually deliver.
- Whether a real user sits behind the channel.
- Expectations e2, e3 about cal-checkin pending.