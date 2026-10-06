# rung-host — named ladder live soak, idle rule off and on (evidence note)

Informative. Two 2-hour live windows of `rung-host run --config` on the named
default ladder (no `ladder:` key), same harness, stimuli (10 owner messages and
4 calendar entries) and analyzer as [v4](rung-host-live-v4-hostkey.md). Host
key `RUNG_HOST_OPENROUTER_API_KEY` (fleet Doppler `dev_donald`) and Jev shadow
on its accounted key at the existing cap (0.25 USD/day), both through
`doppler run --only-secrets`. Free models only. Key scan of every committed
file: 0 occurrences of either key. Records:
`rung-host/live/runs/2026-10-06-named-ladder/`. New: `live/free_time.py`, and
two `live.sh` switches (`RUNG_LIVE_DEFAULT_LADDER`, `RUNG_LIVE_IDLE_RULE`).

| arm | what | window |
|---|---|---|
| A | ladder as merged, idle rule off | 2026-10-05 17:19–19:19Z |
| B1 | A + `free_time_idle_rule: true` | 19:19–21:19Z — **void** |
| B2 | B1 rerun after the account's daily free limit reset | 2026-10-06 00:05–02:05Z |

**B1 is void.** Every request got HTTP 429 `free-models-per-day-high-balance`
(an account-level daily cap of free requests, not a model's): 16 turns, 0
completed, 0 model calls. The post-run probe returned the same 429; it cleared
at the 00:00Z reset, and B2 ran then. B1's record is kept
(`B1-idle-rule-ratelimited/`), not counted. A and B2 ran on different days and
different upstream load, so the comparison below is weak.

## Per arm

| measure | A (rule off) | B2 (rule on) |
|---|---|---|
| turns (failed / total) | 23 / 51 = **45.1%** | 5 / 23 = **21.7%** |
| failure classes | rate_limit 18, overloaded 3, output 2 | rate_limit 3, overloaded 2 |
| free-time turns (failed / completed) | 20 (11 / 9) | 1 (0 / 1) |
| quiet-tick filler share (completed free turns with no tool call) | 0 / 9 = **0%** | 0 / 1 = **0%** |
| owner-admission p95 (10 owner items) | 789 s (p50 3.4 s) | **0.57 s** (p50 0.32 s) |
| cache ratio (cached / prompt tokens) | **0.666** | **0.074** |
| distinct useful free-time actions (distinct files written) | 2 (`anticipate.py`, `run_test.py`) | 0 |
| rung walks (model switches / probes up) | 14 down, 7 up | 5 down, 3 up |
| Jev shadow cost | 0.0041 USD | 0.0025 USD |

Reading the numbers:

- **The rule was not exercised.** Free-time turns were 20 of 51 in A and 1 of
  23 in B2. The host spent most of both windows backed off (23 and 22
  `degraded` waits), so there were almost no quiet ticks to fill. With 9 and 1
  completed free turns, a filler share of 0% in both says nothing about the
  rule. The 84-of-119 filler of the earlier
  [free-time hour](rung-host-free-time.md) did not recur in A either, with
  the rule off, so that finding is not the default behaviour on this ladder.
- **Failure rate and admission p95 follow the upstream, not the rule.** A ran
  while nemotron-3-super and ultra were rate-limited, B2 after a reset with
  fresh quota. B2's 0.57 s p95 is a quiet-day figure.
- **Cache ratio is the model mix.** A made 72% of its calls on
  nemotron-3-super (cache 0.77); B2 made 71% on nemotron-3-ultra (cache 0.00).
  Both arms cache the same per model.

## Coherence sample

24 completed turns read, 12 per arm (`sample.md` in each arm's directory,
seeded draw, only the first 600 characters of each reply read). Read as:
on-task, consistent with its own earlier output and with the header facts.

- **A: 12 of 12 coherent**, with a flaw: turns 12, 15 and 16 give almost the
  same "What was done" summary three times, after served-model changes. Turn
  21 correctly found its own earlier file cut off mid-method and fixed it on
  turn 24.
- **B2: 12 of 12 coherent.** Turns 2 and 7 are thin (one restated line), and
  turn 4 correctly refused to read `/etc/hostname` outside its sandbox. Turns
  11, 19 and 20 carry a real finding (calendar entries arrive 2–15 minutes
  late) into the workspace notes.

## What it does not show

No arm exercised the idle rule's target case (many quiet ticks), so the rule
is neither confirmed nor refuted. The binding limit here was the free tier:
the account's daily free-request cap was exhausted by the end of one 2-hour arm and
the day's other use of the key; the host's backoff then left a turn every ~15 minutes
after the first half hour.

**Recommendation: no, leave `free_time_idle_rule` off by default; this soak
did not exercise it, so re-measure on a window with quiet ticks (paid or
dedicated quota) before changing the default.**
