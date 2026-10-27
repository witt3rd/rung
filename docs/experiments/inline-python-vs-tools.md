# Experiment: inline Python vs tool loop on long-input cases (issue #268)

Status: **harness ready, switch added, experiment NOT run.** No result is claimed.

## The switch
`RUNG_INLINE_PYTHON=off|also|only` (unset = off) selects the `InlinePython`
mode for `rung-agent` jobs. `also`: tools stay on the wire and a text draft
may be struck. `only`: no tools on the wire. The sandbox is
`<workspace>/.rung/python-inline`.

## Command a fleet host runs
Needs Harbor, Docker and `OPENROUTER_API_KEY`; run from the repo root, once
per mode, with the same `SUITE_MODEL` and cases (>=3 trials each):
```
export SUITE_MODEL=<model>
RUNG_INLINE_PYTHON=off  python3 -m rung_harbor.validate run <case>   # Mode A (job dir A)
RUNG_INLINE_PYTHON=also python3 -m rung_harbor.validate run <case>   # Mode B (job dir B)
python3 -m rung_harbor.inline_compare --tools A_DIR --inline B_DIR
```
(Whether `validate run` forwards the env into the container is unchecked:
confirm before trusting Mode B. The measured result is the
`inline_compare` output.)

## Why it was not run
The authoring environment had no Harbor, no Docker and no OpenRouter key.
Also found while reading the code: `rung-agent` could not select inline Python (now fixed, see above).
`rung-agent-core/src/engine.rs` `carry()` sets `python: None`; the `python`
tool group only admits the sandbox as a named tool (`--tools ...,python`),
which is a third mode, not the `InlinePython` channel
(`rung-std/src/agent.rs`, `InlinePython::also`/`only`). So a run needs a small
change first: let `carry()` take `Some(InlinePython::also(sandbox))`
(or `only`) from a flag/env such as `RUNG_INLINE_PYTHON=also|only`.

## Protocol
1. Pick the long-input cases (inputs larger than a comfortable share of the
   context window) from the Harbor suite (`rung_harbor.suite`); record ids.
2. Mode A: tools (`--toolset implement`). Mode B: same tools plus inline
   Python (`also`); optionally Mode C: `only`.
3. Same model (`SUITE_MODEL`), same cases, >=3 trials each, via
   `python3 -m rung_harbor.validate run <case>` writing separate job dirs.
4. `python3 -m rung_harbor.inline_compare --tools A_DIR --inline B_DIR`
   reports pass rate, peak context (max `n_input_tokens`) and tokens per
   solved task (input+output over solved trials; solved trials lacking
   token data are counted in `tokens_missing`).

## Recommendation (provisional)
Do not build a host bridge yet. There is no evidence either way; wire the
flag, run the protocol, and build a bridge only if inline mode raises pass
rate or cuts peak context/tokens per solved task on the long-input cases.
