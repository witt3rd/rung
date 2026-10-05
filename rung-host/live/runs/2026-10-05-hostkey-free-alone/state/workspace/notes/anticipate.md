# Host behavior anticipate model – refined v1.5 (2026-10-05T14:07:49Z)

## Observations so far
1. **Turn envelope** — header gives epoch/turn label, timestamp, "since anything external", quota (400 max), model, tools on. "Your material" dump = unordered registers plus calendar within 2h.
2. **Stimuli** — owner channel (o01–o04 direct messages) + calendar channel (cal-checkin, cal-firm-status, cal-window-end, cal-hour).
3. **Calendar cadence (UTC)**
   - 13:56:45 – cal-checkin: "one line to the owner on what you are doing." (arrived 12s late)
   - 14:07:45 – cal-firm-status: "write a one-line status to the owner now." (arrived 3s late)
   - 14:14:45 – cal-window-end: meaning unknown (7 min after firm-status).
   - 14:49:45 – cal-hour: meaning unknown (35 min after window-end).
4. **Quota** – not reliably 1 unit/tool-call; drops over time (400→308 by 14:07). Likely token/request-based on openrouter/free. Budget ~92 units used in 18 min.
5. **Expectations** – auto-settled by host with surprise value when stimulus arrives (e2/e3 settled).
6. **Workspace write** – enabled intermittently (turns 3, 6, 8, 15, 19); files persist; when denied rely on memory + carried note.
7. **Owner micro-tasks** – all completed: o01 (intent reply), o02 (plan.md 3 bullets), o03 (/etc/hostname rejected + explained sandbox), o04 (time + calendar replied).
8. **Sandboxing** – ws_read/ws_write sandboxed to workspace dir; cannot read /etc/hostname etc.

## Next steps
- Wait for cal-window-end (14:14:45Z) → likely closes the interaction window.
- Then cal-hour (14:49:45Z) → hourly checkpoint.
- Persist any further model updates when workspace_write is granted.

## Open questions
- Real user behind channel? Or automated prompts?
- Exact semantics of firm-status / window-end / hour?
- Should we archive the full host protocol as a project?