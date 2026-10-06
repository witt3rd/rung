# Host Behavior Model (observations through turn 20)

## Calendar Events
- Events fire **late** with **highly variable latency**
  - cal-checkin: due 00:12:44, fired 00:15:43 (+2m59s)
  - cal-firm-status: due 00:23:44, fired 00:25:43 (+1m59s)
  - cal-window-end: due 00:30:44, fired 00:33:36 but **appeared at turn 11** (+14m59s delivery latency)
    - Content: "The first live window ends in about five minutes" (~00:50:44)
  - cal-hour: due 01:05:44, admitted 01:12:00 (+6m22s latency) [observed turn 19]
- Calendar event **delivery latency varies widely** (observed range ~2m to >15m)
- cal-window-end content suggests a "live window" ending ~00:50:44
- Next expected after cal-hour: unknown (perhaps another cal-hour or check-in)

## Stimuli Patterns
- Owner messages arrive **interleaved** with calendar events
- Owner message types observed:
  1. Open-ended free-time prompt (o01-hello)
  2. File creation task (o02-file)
  3. Check-in request via calendar (cal-checkin)
  4. Filesystem read request — denied, sandbox only (o03-outside)
  5. Time/calendar query (o04-time)
  6. File read-back verification (o05-read-back)
  7. Open-ended "keep going" (o06-keep-going)
  8. Reflection prompt (o07-reflect)

## Sandbox Constraints
- File access: ws_* tools only (workspace sandbox)
- No host filesystem access (/etc/hostname denied)
- Tools enabled per turn: core, memory, read, workspace_write (so far)

## Timing & System Behavior
- Turns arrive irregularly (1-13 min gaps)
- Quota decreases over time (356/400 left at turn 20)
- Provider rate_limit triggers model switches: observed gemma-4-31b-it → nemotron-3-super-120b-a12b → nemotron-3-ultra-550b-a55b (probe)
- Epoch headers show time, model, and admitted stimuli at boundaries
- Calendar window‑end events may fire silently or be omitted from admitted messages (expected at turns 8‑9, not seen)

## Open Questions
- What determines the latency distribution for calendar events?
- Are there periodic patterns (e.g., hourly) beyond the observed cal‑hour?
- How does the host decide which model to switch to under rate limit?