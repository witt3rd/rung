Observed calendar event latencies: cal-checkin +2m59s, cal-firm-status +1m59s, cal-window-end +14m59s delivery, cal-hour +6m22s.
Built host behavior model documenting variable calendar latency, owner stimulus patterns, sandbox constraints, and model switches.
Created calendar_parser.py utility to extract and analyze latency statistics from log data.
Noted cal-window-end may fire silently or be omitted from admitted messages at turns 8-9.
Model switched from nemotron-3-super-120b-a12b to nemotron-3-ultra-550b-a55b (probe) at epoch boundary.