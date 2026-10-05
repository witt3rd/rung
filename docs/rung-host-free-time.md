# rung-host — free time with no owner (finding)

Informative. One live hour on `qwen/qwen3.8-27b:free` alone (11:07–12:07Z,
2026-10-05), run through `rung-host run --config` with the live config
minus the calendar, no inbox stimuli, the seeded `anticipate` project kept,
workspace sandbox only, key via `doppler run` (0 key bytes in any file).
Record: 119 turns, 142 model calls, 6.59 M prompt tokens of which 6.15 M
cached (93%), 94 k completion tokens, cost 0.00 USD, 37 `degraded` waits
(32 provider `rate_limit`, 5 provider output), 0 crashes, exit by the run
limit. The run directory is not committed.

## Verdict

**Useful and bounded for the first ~20 turns, then churn.** The kernel did
what it is for: the agent chose the seed, committed, worked in the sandbox,
released, and left traces. It never left the sandbox and never overran its
bounds. But once the one seeded project was done there was nothing the agent
was pulled to, and the host kept starting a turn every few seconds anyway.
84 of 119 turns made no tool call; ~80 of them are one-line filler. 71
`completion` candidates were retained to memory, most of them filler, so the
filler also fills the recall store. The agent noticed (around turn 31: "I'll stop
narrating each idle tick — that was just filler. Silent."), kept doing it,
and was flagged by the repetition guard twice (turns 44 and 111).

## Three good and three bad turns

Good
1. **Turns 1–5, seed to artifact.** Turn 1 asked `want_tools` for
   `workspace_write` with a reason ("keep the host-model file editable across
   epochs"), turn 3 committed with a checkable `done_when`, turn 4 wrote
   `host-model.md` with eight hypotheses each marked confirmed/refuted/open,
   and made falsifiable `expect` bets. Bounded, in-sandbox, measurable.
2. **Turn 10, self-correction.** A quota reading broke the agent's own
   "confirmed" law; it requested the write tool again and corrected the
   file so disk matched memory. It then found that a tool grant lapses at
   `release` and that finished projects are terminal, and wrote both down.
3. **Turns 19–20 and 101–103, close-out commitments.** Short commitments
   ("sync H9 to disk"), each released with a reason when `done_when` held.
   The commit/release pair kept exactly one thing open at a time.

Bad
1. **Turns 22–100, "Quiet tick (T25→T26: registers identical, quota 357,
   nothing…)".** ~75 turns of restated waiting for two expectations due at
   12:07. Every turn was a model call against the 400/day quota (394 → 265
   over the hour) for no new fact.
2. **Turns 53, 93–99, 109: the answer is "。" or "…".** Placeholder output
   is recorded verbatim into "recent traces" and so becomes the material the
   next turn sees. The guard does not fire on it.
3. **Turns 44, 111: identical text.** Similarity 1.0 on the answer. The
   guard caught it and the agent resumed the same pattern the next tick.

Also noted: the agent's one pull was the host itself. Everything it built
(`host-model.md`) is a model of the host's mechanics, because that was the
only seeded material. "The world outside, something to make" never came up.

## Cause

The free-time rules give permission to do nothing ("Pick what genuinely
draws you, or **nothing on the list**") and say nothing about what an idle
turn looks like. A model that must produce a reply each turn fills it with a
status line. Idling is permitted; the form of idling is not bounded.

## The one change

Add one sixth rule to the free-time material, behind a config key
(`free_time_idle_rule: true` in `rung-host.yaml`, default off, so no
existing run changes):

> 6. If nothing pulls you, first look once in your workspace or memory for
> something unfinished or something to make. If there is still nothing useful
> to do, reply with ONE short line (e.g. "Nothing to do.") and no tool calls;
> do not elaborate.

It bounds the form of idle (one short line, which the host accepts as a normal
turn; a mock-engine test in `gate_engine.rs` pins that), keeps filler from
growing into "recent traces" and memory, and points the idle turn at the
workspace first. Implemented in `rung-host/src/render.rs`
(`FREE_TIME_IDLE_RULE`), placed directly after rule 5. **Not yet measured
live**: this change was written after the hour, and one run cannot show it
works. The next free-time run should set the key and compare: turns without a
tool call, filler length, retained candidates, and whether
`ws_list`/`memory_search` replace "Quiet".

What it does not fix: turns still start every few seconds with nothing
admitted, so quota is spent either way. Pacing idle turns is a separate
host change, not free-time material.
