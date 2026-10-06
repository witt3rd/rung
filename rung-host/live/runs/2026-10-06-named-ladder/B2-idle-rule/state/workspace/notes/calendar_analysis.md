# Calendar Event Latency Analysis

## Raw Data (from host_model.md observations)
| Event | Due | Fired | Latency |
|-------|-----|-------|---------|
| cal-checkin | 00:12:44 | 00:15:43 | +2m 59s (179s) |
| cal-firm-status | 00:23:44 | 00:25:43 | +1m 59s (119s) |
| cal-window-end | 00:30:44 | 00:33:36 | +14m 59s (899s) |
| cal-hour | 01:05:44 | 01:12:00 | +6m 22s (382s) |

## Statistics
- **Count**: 4 events
- **Min latency**: 119s (1m 59s) — cal-firm-status
- **Max latency**: 899s (14m 59s) — cal-window-end
- **Mean latency**: 394.75s (6m 34.75s)
- **Median latency**: 280.5s (4m 40.5s) — between cal-checkin (179s) and cal-hour (382s)

## Observations
1. **High variance**: Latency ranges from ~2 minutes to ~15 minutes (7.5x spread)
2. **cal-window-end is an outlier**: 899s vs. next highest 382s — possible delivery anomaly or different event class
3. **No clear pattern yet**: With only 4 samples, cannot determine if latency correlates with event type, time of day, or system load
4. **cal-hour (+6m22s) is closest to mean**: Suggests hourly events may have "typical" latency, while check-ins and window-ends vary more

## Next Steps for Model
- Need more data points (especially additional cal-hour events) to establish baseline
- Track whether latency correlates with quota remaining, model switches, or turn gaps
- Watch for cal-window-end recurrence to see if 15min latency is consistent or anomalous