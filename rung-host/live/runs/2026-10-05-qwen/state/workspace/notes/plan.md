# Small project: turn-tick ledger

- Keep `data/ticks.tsv` (ts, turn#, kind, external?, gap_s): one append-per-external-event and one per calendar fire; keep it append-only.
- At each `cal-hour` fire (~05:24Z), diff host-model.md against the ledger and correct exactly one hypothesis.
- Success when the ledger + model let me predict the next calendar fire to within ±5s without watching.
- Since written: ledger started (through 62 turns), two fires logged (checkin +13s, firm-status +41s) and model revised to "fires ride the next turn boundary."
