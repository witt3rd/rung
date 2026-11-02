# AGENTS.md — rung

The ladder runtime: the `ladder!` / `theory!` surface re-exported from `rung-macro`, the sealed token types, and the conformance suite. Consumers depend on this crate alone.
Repo charter: [`../AGENTS.md`](../AGENTS.md).

**Status.** Kernel. Not published to a registry (versions are git tags).

**Read.**
- Language, normative: [`docs/rung-props.md`](../docs/rung-props.md); derivation: [`docs/rung-notes.md`](../docs/rung-notes.md); category: [`docs/rung-ct-props.md`](../docs/rung-ct-props.md).
- Getting Started (a complete program, run as a doctest): the crate docs in `src/lib.rs`, section "Getting started". Ladder examples with their lessons: `examples/` (`cargo run -p rung --example 01_enforced_graph`).
- `tests/` is the conformance suite: a change that violates a guarantee must break at least one cited test; `trybuild` `.stderr` files pin refusals.

**Check.** `cargo test -p rung --locked`
