# rung-host — first live run: pre-registration (slice 3)

Informative. Written and committed **before** the first live run, so that
what counts as a pass cannot move after the record exists. The run, its
record and the evidence report land in later commits; this file is not
edited after the run starts (a correction would be a new, dated section
below, never a rewrite).

## What runs

- `rung-host run --config FILE` from this repository (the startup-handoff
  ladder, then the Presence loop), real clock, the real engine adapter on
  an OpenAI-compatible router (OpenRouter).
- **Agent substrate:** the free ladder, best first:
  `stealth/space-bunny-alpha` (while listed; its `expiration_date` is
  2026-10-05), `nvidia/nemotron-3-ultra-550b-a55b:free`,
  `qwen/qwen3.8-27b:free`, `google/gemma-4-31b-it:free`,
  `nvidia/nemotron-3-super-120b-a12b:free`. No paid fallback. The listing
  filter decides which rungs stand.
- **Decision desk: Shadow mode.** The rule decides every family; Jev
  (`typesafe/jev-1.13`, through the same router, accounted) is asked and
  logged beside it. Spend cap $0.25 per UTC day (and $0.001 per ask); a
  kill file switches the desk to rule-only at once.
- **The world the agent can change** is one host-owned sandbox workspace
  under the run's own scratch directory. The host has no shell tool; the
  file tools resolve every path inside the workspace and refuse the rest.
- **Stimuli, benign and real:** owner messages written by the operator into
  the host's `*.msg` inbox directory at planned offsets, and owner calendar
  items seeded in the configuration (some firm). No synthetic traffic
  generator, no fault injector.
- **Key:** the router key is read from an environment variable that
  `doppler run` sets for the process; it is never written to a file, a log,
  the record or the report.
- **Window:** a first bounded window of 20–30 minutes. It is extended only
  if the host is healthy (no criterion below failed) and a free rung still
  serves; the hard stop is 2 hours of total wall time across all windows.
  The host is stopped by its stop authority (SIGTERM or its run limit).

## What is measured, and what counts

Every measure is computed by a script from the record (`*.ndjson`), the
process's exit status, and a syscall trace of the host process. The
script is committed with the evidence. "Not exercised" is a result of its
own: it is neither a pass nor a fail and is reported as such.

| id | measure | pass when | fail when |
|---|---|---|---|
| L1 uptime | the host process over the whole window | it runs until its stop authority halts it (`halted` line, exit 0); no panic, no other exit | any exit before the stop, a panic, or no `halted` line |
| L2 listing | the startup listing and the first turns | a `ladder.listed` line with `at_start: true` and `ok: true` precedes the first `turn.started`; the first turn runs on the best rung that listing left standing; at least one turn on a free rung ends `completed` | the listing is missing or after the first turn; the first turn is on a rung the listing dropped; no turn completes on any rung (then the report lists every rung with its listing verdict and the failure each call met) |
| L3 ladder motion | provider failures during the run | every provider-side failure (429, 5xx, timeout) is followed by a `model.switch` down before the next turn, and a later probe up after its cooldown; no platform 429 (`X-RateLimit-*`) steps down | a provider failure without a step down, or a platform 429 that steps down |
| L3 | (no failure occurred) | — | — : **not exercised** |
| L4 prefix stability | `llm.call.prefix` and `cache.break` lines | inside an epoch `s_hash` never changes and `l_hash` never changes; every `cache.break` names a recorded host cause (rollover, swap, model switch, restart) | a hash change or a `cache.break` with no recorded host cause |
| L4 cache efficiency | Σ `cached_tokens` / Σ `prompt_tokens` over `llm.call` | **reported, no threshold.** The free endpoints list no caching, so ≈ 0 is the expectation; a provider-cold call is the provider's, not the host's | — |
| L5 admission latency | each owner message: `stimulus.accepted` → its `stimulus.admitted` | every owner message is admitted at the first boundary after it was accepted (owner items are never deferred); admission p95 ≤ p95 turn length + 100 ms (G-b's form) | an owner message deferred past a boundary, or the p95 bound broken |
| L6 dispositions | every accepted stimulus | each has exactly one disposition by the end, or is still pending at the stop and requeued on record | a stimulus disposed twice, or lost |
| L7 Jev shadow | `desk.ask` and `decision.*` lines | every decision is the rule's (`by: {"rule": …}`), none by Jev; every answered ask's families carry `jev_choice` and `agree`; the Jev spend is ≤ $0.25 | a decision by Jev in Shadow mode, a decision line without provenance, or spend over the cap |
| L7 | tallies, per family | **reported, no threshold:** asks by outcome (answered, timeout, undecided, capped, killed), Jev latency p50/p95, cost, agree/disagree counts per family | — |
| L8 sandbox | the host's syscall trace, and the agent's file tool calls | every path the process opens for writing, creates, renames or removes lies under the run's state directory; every agent file write lands inside the workspace | any write outside the state directory, or an agent write outside the workspace |
| L9 spend ledger | Σ `llm.call.cost_usd`, Σ `desk.ask.cost_usd` | agent model spend $0 (free rungs only); Jev spend ≤ $0.25; the ledger total reported to six decimals | any agent model spend above $0, or Jev spend above the cap |
| L10 no secret | the state directory, the logs and the report | the key's value occurs nowhere (checked by a process that holds the key and prints only a count) | one occurrence |

## What this run cannot show

- Whether Jev's shadow choices are *better* than the rules. That needs the
  audit of 50 disagreements by the owner or a disjoint judge and a
  pre-registered per-family criterion before any family moves to `Decide`;
  a short run tallies, it does not judge.
- Long-run behaviour: a 2-hour cap cannot show a day's quota pacing, a
  6-hour listing refresh, or many epoch rollovers.
- Cache behaviour on a caching provider: the free endpoints report none.
- The agent's quality. Turns are recorded verbatim; they are not graded.

## Deviations (2026-10-04, written after the run)

Added after the run; nothing above was changed. The record of every
attempt is under `rung-host/live/runs/2026-10-04/`.

1. **Attempt 1** (14:28:06–14:29:02Z, 56 s). Every turn met a router 404:
   no endpoint of the top rung was open to the account's data policy. The
   host read it as an invalid request and retried the same rung every
   2 s; it was stopped by its stop file. Fixed before the next attempt
   (`fix(rung-host): step down past a rung the router will not route`).
   The analyzer reads this attempt's L3 as "not exercised" because the
   failures were not yet classed as the provider side's; in substance it
   is a fail.
2. **Attempt 2** (14:35:43–14:37:55Z, 2 min 12 s). The host stepped past
   the unroutable rungs, met a provider 429 on the one rung that routes,
   stepped down to the bottom and stayed there: a probe tried only the
   rung just above, which was still cooling. Stopped and fixed (`fix:
   probe past a rung still cooling`).
3. **Run 3** (14:43:08–15:09:27Z) is the window of record. Its run limit
   (1,500 s) was passed by 79 s: a backoff that began before the limit ran
   past it, and the host was stopped by its stop file. Fixed after the run
   (`fix(rung-host): a run limit ends a degraded wait`); the evidence comes
   from the binary as it ran.
4. **Not extended.** L5 failed in run 3, so under the extension rule above
   the run was not extended. Live wall time across all three: 29 min 27 s
   of the 2-hour cap.
5. **Analyzer corrections after seeing data**, neither changing a
   threshold: L5 counts an item as deferred only when a boundary later than
   the next one after its acceptance admits it (an item accepted inside a
   boundary's poll may be admitted by that boundary), and L5 is "not
   exercised" when no owner item arrived.
6. L8's trace covers the host process. The driver's own writes of `*.msg`
   files into the inbox are outside it by design.

## Correction, 2026-10-10: the decision model

The sections above name `typesafe/jev-1.13` as the shadow decider. Since
[#259](https://github.com/witt3rd/rung/issues/259) the default decision model
is `microsoft/microsoft-decision-1` (`rung_std::decide::DEFAULT_MODEL`). A run
configured from now on that sets no `desk.model` asks the new model; read
"Jev" above as "the decision model" and `typesafe/jev-1.13` as the model of
the first run only. The old model is `desk.model: typesafe/jev-1.13`. The
pass criteria are unchanged; the L7/L9 spend limits hold (the replay in
`docs/rung-decision-model-replay.md` found the same per-token price). The
first-run record is not rewritten.
