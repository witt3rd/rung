# AGENTS.md — rung-std

The canonical building blocks: `llm`, `agent`, `python`, `tools`, `questions`, `principals`, `driver`, `decide`.
Repo charter: [`../AGENTS.md`](../AGENTS.md).

**Status.** Kernel library. A block is admitted only when it recurs across independent projects and embeds no caller-specific knowledge (J2); session catalogs, resume, worktrees and XDG config belong in a product crate.

**Read.**
- Per-theory reading pages: [`docs/rung-std/`](../docs/rung-std/).
- `guest/` is the jailed CPython guest used by the `python` block.

**Check.** `cargo test -p rung-std --locked`
