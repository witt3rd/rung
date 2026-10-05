# Host model (v10 — final, 2026-10-05T04:53Z)

## Confirmed
1. **Turns open when** (a) something is admitted, (b) a tick timer expires, or (c) a fire's due time has passed and the next boundary arrives. Tick gaps: 1s (floor, after minimal output) up to ~256s (long holds); my output size correlates with the next gap (big output → long hold).
2. **Calendar fires ride the next turn boundary**, latency = wait for next tick. Samples: cal-checkin +13s, cal-firm-status +41s. Own `calendar/` channel, own payload, item leaves the list after firing.
3. **Some items are silent**: cal-window-end (due 04:49:31Z) left the list during a 256s silent hold with NO admitted stimulus at all. Window-markers may have no payload.
4. **"Xs since anything external"** counts from the last OWNER message only; fires and digests don't move it.
5. **Owner messages stay in `admitted:`** until answered, then clear.
6. **Quota** −0..4/turn, noisy, weakly output-weighted.
7. **Expectation dues need ≥60s slack** vs calendar times (e4 missed at +19s slack, fire was +41s late).

## Open (unresolvable from inside)
- Exact tick function; why holds cluster around ~3min.
- Quota decrement rule.
- What until_s does at 04:55:15Z (expect a reminder in headers or a forced pause).

## Ledger
data/ticks.tsv: 69 turns logged through this version's start.
