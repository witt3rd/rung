# AGENTS.md — gui/

The new rung UI's workspace: a typed gateway client (`rung-client`) and a
desktop app on GPUI Kit (`rung-ui`). Architecture and slices: the owner's UI
design note (ten slices; this is slice 6). The old web page in `../ui/` keeps
working and is not touched.

## Rules

- **Its own cargo root, outside the main workspace.** `cargo build` or
  `cargo test --workspace` at the repository root never builds it. Work here:
  `( cd gui && cargo test )`. It has its own `Cargo.lock`. Rust 1.92 or newer
  (GPUI's floor; the main workspace's MSRV is lower on purpose).
- **CI:** the `gui` job in `.github/workflows/ci.yml`, not part of the required
  `check`. It runs when `gui/` changes or the gateway or host is in scope
  (`scripts/ci_plan.py`, output `gui`), and always on master and the merge
  group.
- **No logic copies.** The UI shows what the host's own doors say (state word,
  "doing", turns, spend). No fold of the record, no secret-remover here: the
  gateway removes secrets from every byte it sends.
- **Read-only first.** Writes (queue, stop, release) need an explicit
  click to turn controls on. A role is read from the instance name's first segment (`chief-`,
  `steward-`); there is no registry field for it.
- **No consumer names** (the guard applies here too) and **no README.md**:
  this file is the agent file.
- Heavy builds: never on roger. Use the build lane (chef or x) under
  `flock /tmp/fm-heavy-<host>.lock` and a capped scope
  (`systemd-run --user --scope -p MemoryMax=6G -p MemorySwapMax=0`).

## Crates

| crate | what |
|---|---|
| `rung-client` | blocking typed client of the gateway: `health`, `instances`; no GUI (`rung-client/AGENTS.md`) |
| `rung-ui` | the GPUI Kit window: the instance list and instance page from a live gateway (`rung-ui/AGENTS.md`, with the build lane's needs) |
