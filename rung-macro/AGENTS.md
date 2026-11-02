# AGENTS.md — rung-macro

The `ladder!` proc macro: it parses the ladder syntax, runs the static checks and emits the sealed module. A proc-macro crate can export only macros, so the runtime types live in `rung`.
Repo charter: [`../AGENTS.md`](../AGENTS.md).

**Status.** Kernel. Not published to a registry; depend on `rung`, which re-exports the macro.

**Read.**
- Normative rules: [`docs/rung-props.md`](../docs/rung-props.md); the expansion logic: [`docs/rung-notes.md`](../docs/rung-notes.md).
- Its tests live in `rung/tests/` (the compile-pass and compile-fail cases need the runtime types).

**Check.** `cargo test -p rung --locked`
