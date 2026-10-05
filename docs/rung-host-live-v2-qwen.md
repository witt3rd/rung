# rung-host — live run 2 on qwen alone (evidence note)

Informative. Rerun of the first live run ([prereg](rung-host-live-v1-prereg.md),
same measures, same analyzer) with the immediate owner ack and the keyed
probe merged. Ladder: `qwen/qwen3.8-27b:free` only (the other free rungs
are refused by the account guardrail; no key was changed). Run through
`rung-host/live/live.sh` with `RUNG_LIVE_LADDER=qwen/qwen3.8-27b:free`,
2,400 s window (04:24:30–05:04:31Z, 2026-10-05), exit 0 by the run limit.
Record, logs and numbers: `rung-host/live/runs/2026-10-05-qwen/`
(`measures.txt` / `measures.json`). Key scan: 0 occurrences.

## Answer

**L5 passed.** Owner messages are now admitted at once; every other
measure also passes.

| measure | run 3 (first run, 26 min) | this run (40 min) |
|---|---|---|
| L5 owner-admission | **FAIL**: p50 16.7 s, p95 316.7 s | **PASS**: p50 247 ms, p95 5.2 s, max 5.2 s (turn p95 53.8 s); 0 deferred; 6 owner items |
| Provider 429s | 15 of 18 turns failed (83%), 12 of them unroutable | 16 of 71 turns failed (22.5%), all `rate_limit` from the provider, 0 platform, 0 unroutable; 16 of 111 calls (14%) |
| Turns completed | 3 | 55 |
| Cache efficiency (Σ cached / Σ prompt) | 0.758 (34,048 / 44,949) | **0.919** (2,614,272 / 2,845,647); 3 provider-cold calls, 0 host-caused breaks beyond 1 rollover |
| Jev shadow | 33 asks, $0.001745 | 97 asks, all answered, 0 decisions by Jev, $0.005407 (cap $0.25); p50 214 ms |
| Agent spend | $0 | $0 |
| Dispositions (L6) | 2 of 9 accepted disposed | 12 of 12 disposed, none open at stop |

Jev agree/disagree (reported, no threshold): admit 10/5, inject 44/48,
tools 48/44, consolidate 0/5, pack 1/1.

## Caveats

- No `strace` on this host, so no syscall trace was taken. L8 reads PASS
  from the analyzer only because nothing was traced (`traces: 0`); read it
  as **not exercised** for the host process. The agent's 19 file-write
  tool calls all stayed inside the workspace and none was refused.
- One ladder rung, so L3 shows backoff on a 429 (16 incidents, each
  retried after a short wait) but no step-down or probe-up.
- The 40-minute window ended before the 60-minute calendar item and the
  later owner messages (o07 onward partially); 6 owner items arrived.
- No PDF: nothing surprising.
