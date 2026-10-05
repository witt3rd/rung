# rung-host — live run 3 on qwen alone, 2 hours (evidence note)

Informative. Same setup as [run 2](rung-host-live-v2-qwen.md) (ladder
`qwen/qwen3.8-27b:free` only, `desk.mode: shadow`, Jev cap $0.25/day), window
extended to 7,200 s (09:04:45–11:04:46Z, 2026-10-05), exit 0 by the run limit,
no panic. Only live run on the host. Record, logs, analyzer output and the audit
sheet: `rung-host/live/runs/2026-10-05-qwen-2h/` (`measures.txt`,
`measures.json`, `shadow-audit.md`). Key scan: 0 occurrences.

## Numbers

| measure | result |
|---|---|
| Crashes | none (1 start, 1 halt by limit) |
| Turns | 88 completed, 65 failed, 1 bounded (154) |
| Provider 429s | 61 provider failures, 0 platform, 0 unroutable. 42% of turns failed |
| Ladder | one rung: 61 failures at the bottom, no step-down or probe-up (as in run 2) |
| Cache efficiency | **0.949** (9,207,552 / 9,703,366 over 197 calls); 14 provider-cold calls, 2 rollovers, 0 other host-caused breaks |
| Owner admission (L5) | **FAIL**: p50 3.4 s, p95 105.4 s, max 105.4 s (10 owner items, 0 deferred past first boundary). Run 2 passed at p95 5.2 s over 6 items |
| Dispositions | 18 of 18 disposed, none open at stop |
| Jev shadow | 317 asks, 316 answered, 1 timeout; 0 decisions by Jev; p50 197 ms, p95 444 ms; cost $0.0212 (charged $0.0212) of the $0.25 cap |
| Agent spend | $0 |
| File writes | 26 agent writes, 2 `ws_write` refusals; no syscall trace (no `strace`), so L8 is not exercised |

## Shadow tally (agree / disagree, rule vs Jev)

| family | agree | disagree | rate |
|---|---:|---:|---:|
| admit | 26 | 13 | 67% |
| consolidate | 19 | 71 | 21% |
| inject | 142 | 84 | 63% |
| pack | 60 | 27 | 69% |
| tools | 137 | 89 | 61% |
| total | 384 | 284 | 57% |

Audit sheet: `shadow-audit.md` holds 50 random disagreements of 284
(`--seed` default). Verdict and note fields are not filled: no principal
disjoint from the rule has judged them.

## Reading

- `consolidate` is the outlier (21% agreement); the rule and Jev diverge most there.
- L5 regressed on one tail: the median is fine, one item waited 105 s. The
  cause was not isolated (likely an owner message landing while a turn sat in
  429 backoff); the record has the sequence.
- Failure rate rose from 22.5% (40 min) to 42% (2 h) on the free provider.
