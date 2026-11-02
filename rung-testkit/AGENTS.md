# AGENTS.md — rung-testkit

Test-only helpers shared by the product crates: `TempDir`, a scripted OpenAI-compatible mock LLM server and a scripted HTTP server (also the `mock-llm` binary).
Repo charter: [`../AGENTS.md`](../AGENTS.md).

**Status.** Test-only. `publish = false`. A test makes scratch directories with `TempDir` only; CI fails if a test run leaves anything in its private temp root (`scripts/test_tmp_clean.sh`).

**Check.** `cargo test -p rung-testkit --locked`
