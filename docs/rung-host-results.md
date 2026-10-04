# rung-host — slice 1 gate results

Informative. The gates are defined in `rung-host/src/gates.rs` (thresholds
and evaluators, committed before the host's first run) and the design is
in [`rung-host.md`](rung-host.md). Every run here is seeded, offline and
free: a scripted mock engine, a fake world, a fault injector, a simulated
clock (or the real clock for the process gates), no network, no live
model, no live Jev.

Run them all with:

```bash
cargo test -p rung-host --locked -- --nocapture 2>&1 | grep '^GATE'
```

## Results

| gate | test | measured | result |
|---|---|---|---|
| G-a no rest | `gate_time::thirty_quiet_minutes_have_no_rest` | 31 simulated minutes, 1,356 boundaries; idle 0.47%; boundary → turn p99 21.8 ms; every boundary has decisions | PASS |
| G-b responsiveness | `gate_time::owner_stimuli_under_load_are_admitted_at_the_next_boundary` | 75 owner stimuli over 3 h at 30/h plus 40 peer/h and bursts; admission p95 1,343 ms vs turn p95 1,712 ms; no turn past 120 s; 5 long-work calls refused with the commit-or-steps message | PASS |
| G-c free-time kernel | `gate_kernel::two_thousand_turns_follow_the_kernel`, `material_is_unranked_on_a_rich_register`, `no_path_outside_the_kernel_writes_its_lines` | 2,000 turns: 1,233 free, 752 committed, 15 responding, 0 in the wrong mode; 92 commits, 91 releases, all from the agent's tools; material identical under 100/100 random scores; trybuild refuses forging `KernelEntry`, calling its constructors, or `Core::emit` from outside | PASS |
| G-d schedule | `gate_time::due_items_fire_at_the_first_boundary_and_missed_ones_once` | 31 fires, none early, none duplicated, all at the first boundary after due, lateness = at − due; firm items admitted at that boundary although the decider deferred everything; across a 2 h downtime: 2 missed items fired once, late, and 1 `skip` item skipped | PASS |
| G-e expectations | `gate_time::only_the_host_settles_expectations_and_calibration_recomputes` | 949 settled (met and missed), none by the agent (trybuild refuses building a `Settlement` outside the registers); surprise and calibration recomputed from the record match the host's exactly at every settlement; every decidable verdict matches its predicate | PASS |
| G-g provider failure | `gate_faults::injected_faults_degrade_and_recover_but_never_kill` | provider 429 with `Retry-After` 30 s, a 5xx burst, a platform 429 with a reset, a 10-min outage, a 40-min auth failure, a daily quota; the host never exited; 3 requeued items admitted exactly once; `Retry-After` honoured; every provider failure stepped down (7 probes back up); no platform 429 stepped down; no backoff over 15 min; one owner message for the blocked incident | PASS |
| G-h stop | `gate_process::a_stop_is_prompt_from_a_turn_and_from_any_wait_and_the_watchdog_fires` | SIGTERM mid-call: 810 ms with 797 ms of the call left; from a backoff and from a paced wait: ≤ 20 ms; exit 0 and `halted{stopped}` each time; a wedged engine missed its watchdog and the harness (as supervisor) saw it 3,016 ms after the last ping (`WatchdogSec` 3 s) | PASS |
| G-i restart | `gate_process::fifty_kills_lose_nothing_and_restore_everything` | 50 `kill -9` at random times, 50 restarts; 169 stimuli, each with exactly one disposition; every `turn.ended` projection hash equals the replayed state's; each restart has its `recovered` line, the right mode, and a new epoch whose header states the gap | PASS |
| G-j bounded context | `gate_cache_context::ten_thousand_turns_stay_bounded_and_cache_clean` | 10,000 turns, epoch budget 24k tokens: largest turn start 20,167 (84.0%), largest prompt 20,344; 447 rollovers; record bytes per 1,000 turns within 1.03× of each other; 159 copied traces all flagged, 36 copy loops each with its rollover | PASS |
| G-l cache discipline | same run, and `canonical_bytes_are_stable_across_two_processes` | 25,778 requests: every one inside an epoch is a byte-prefix extension of the one before (0 breaks); `s_hash` never changed without a swap, `l_hash` never without a rollover; mock cache efficiency 1.0 outside the 446 recorded breaks; the canonical request bytes of a seeded run are identical from two processes | PASS |
| G-m decisions | `gate_desk::*` | every family on every path (answered, all 9 `Undecided` variants, a 3 s stall, cap exhausted, no decider, rule-only, an incomplete answer, shadow): 112 family-paths, each with the right `by`; adversarial answers never deferred the owner, never enabled a group outside the ceiling, never rolled over below 40% except on a copy loop; a 3 s stall cost at most 1,900 ms per boundary; `Recorded` replays the four fixtures and panics on a reworded question; an 800-turn host run on an adversarial decider kept every guard | PASS |
| G-k cost | every run above | $0 model spend, $0 decider spend, 0 live model calls, 0 live Jev asks, engine `mock` only | PASS |

## What these results do not show

- **Memory in the long runs.** The 10,000-turn (G-j, G-l), G-e and G-g
  runs turn memory off: the `baseline` provider re-reads its whole store on
  every recall and retain, so a run of thousands of retains is quadratic.
  Memory is exercised in the G-a, G-b, G-c, G-d, G-m and process runs.
- **G-b depends on how often long work happens.** A refused long call still
  spends the tool deadline (30 s): the host cannot know a call is long until
  it is. With long work in 1% of turns (8% of wall time in such turns) the
  owner admission p95 was 8.4 s against a 1.7 s turn p95 and the gate
  failed; the frozen scenario uses 0.2%. Admission latency is
  length-biased by long turns.
- **The `Recorded` fixtures are synthetic.** No live Jev was asked in this
  slice, so the four fixtures under `rung-host/tests/fixtures/decide/` are
  authored answers in the recorded format; they prove the replay path and
  its refusal of a changed question, not Jev's judgement. Slice 3 records
  real ones under the spend ledger.
- **Cache efficiency 1.0** is the mock provider's: it caches the previous
  request byte for byte, so a host that never rewrites a prefix scores 1.0.
  What it shows is that no prefix was broken; it says nothing about any
  real provider.
- **Not built in this slice:** ACP outward, the real engine adapter (it must
  carry a failure's origin and a platform's reset, which
  `rung_agent_core::engine::ProviderFailure` does not), the ladder's
  startup listing filter, the superset swap (`pack.swap` is reserved;
  the stable layer never changes here), and delegation (the `crew` group
  and `crew.*` kinds are reserved).

## Changes to the gate evaluators after they were frozen

No threshold changed. Five evaluator changes, each named in the commit that made it:

1. G-c read the turn kind from a field named `kind`, which the record
   reserves; the turn lines name it `turn_kind`.
2. G-c looked for the agent's tool call before the kernel line it wrote,
   but a call is recorded when it returns; it now collects tool calls per
   turn first.
3. G-e recomputed calibration from the whole record prefix at every
   settlement (quadratic); it now keeps the same arithmetic running.
4. G-m measured a stalled decider per ask; it now sums every ask of a
   boundary, which is what "per boundary" means (stricter).
5. G-i skips a restart whose run holds only a `host.start` (it died
   while waking and wrote nothing else); the restart after it is checked.
