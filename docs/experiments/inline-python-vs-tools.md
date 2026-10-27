# Experiment: inline Python vs tool loop on long-input cases (issue #268)

Status: **harness ready, experiment NOT run.** No result is claimed.

## Why it was not run
The authoring environment had no Harbor, no Docker and no OpenRouter key.
Also found while reading the code: `rung-agent` cannot select inline Python.
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
