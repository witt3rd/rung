# AGENTS.md — rung-doctrine

The normative doctrine, encoded: the source the `docs/*-props.md` documents and `docs/conformance.md` are rendered from.
Repo charter: [`../AGENTS.md`](../AGENTS.md).

**Status.** Tooling. Owns the proposition kinds and the proof census.

**Read.**
- Render the documents: `cargo run -q -p rung-doctrine --bin render`; verify: `... --bin render -- --check`. Never hand-edit the generated markdown: edit `src/*.rs` and re-render in the same change.
- `tests/roundtrip.rs` pins that every document renders byte for byte, every proof resolves and every link in the doctrine documents resolves.
- Hand-kept documents cited from `src/*.rs` (for example `docs/rung-notes.md`) cannot move without editing the source and re-rendering.

**Check.** `cargo test -p rung-doctrine --locked && cargo run -q -p rung-doctrine --bin render -- --check`
