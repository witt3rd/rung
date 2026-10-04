---
name: rung
description: >
  Lived experience for the rung repo. Load when working in ~/src/witt3rd/rung
  (or a worktree): house git (iterate on master; worktrees for parallel
  features), CI, proposition citations, kernel vs product. Not the ladder
  language itself — that is AGENTS.md + docs/.
metadata:
  home: ~/src/witt3rd/rung/skills/rung
  aliases: [rung-repo, rung-caretaker]
---

# rung — how we act in this tree

Charter: repo-root `AGENTS.md`. This skill is the gotchas.
Custody is AGENTS.md + skills/; no `.agent/` folder, no inhabit, no formal
handoff.

## Git

House `fleet_git`. Debugging: work on `master`, commit often. Session
start: `git checkout master && git pull origin master`. Parallel
features: `git wt-new` → `rung.wt/<branch>/`; after merge `git wt-rm`.
Merge method rebase. Never `git add -A`.

Task isolation worktrees (`{repo}.wt/rung-task--{id}`, branch `rung-task/{id}`)
are product, not `git-wt-new`. Do not use `git-wt-new` for those.

## Release

A release is its own commit: bump `[workspace.package] version` in
`Cargo.toml`, build so `Cargo.lock` follows, commit `release: X.Y.Z
(workspace version matches tag)`, push master, then tag `vX.Y.Z` and push the
tag. The version must match the tag. Downstream consumers pin tags.
Each release adds its `CHANGELOG.md` entry in the release commit.
Wait for CI on the release commit before you report the release.

## CI

Required check is `check` (fmt, clippy `-D warnings`, rustdoc `-D warnings`, tests `--locked`).
Propositions job: `render --check`, `docs/_props.py check`, `cited`,
`docs/_consumer_guard.py` (no consumer-specific names; exemptions in
`docs/_consumer_guard.allow`).
`cited` scans `rung`, `rung-het`, `rung-std` comments for kebab slugs.
Wire identifiers that look like slugs: add to `NOT_A_CITATION` in
`docs/_props.py` (`x-api-key`, `x-should-retry`).

## Kernel vs product

`rung-std` tools: filesystem, `kernel_tools` (apply_patch, todo, webfetch,
skill), `task` as nested `Spawn` (depth 1). Named catalogs, session resume,
background child, isolation worktrees, XDG `config.yaml`, ACP v1 stdio
(`rung-agent --acp`) and experimental Streamable HTTP
(`rung-agent --acp-http [ADDR]`, optional `--acp-token`) = product:
the library `rung-agent-core` (turn engine in `engine.rs`), with
`rung-agent` its thin binary shell.
HTTP matches the TS experimental `AcpServer` / `createHttpStream` (not a
third shape). ACP also: resume, unstable fork, tool-call updates, real
abort (before LLM / around tools), prompt image/audio, MCP HTTP+stdio
(`--mcp-http name=url`). MCP-over-ACP tunnel, ACP WebSocket upgrade, and
Harbor `describe-image` (file vision tool) are still not claimed.

Tool images: `Tool`/`Toolset::execute_output` returns `ToolOutput` (text +
images); `read_file` returns PNG/JPEG/GIF/WebP as an image, MCP `image`
items come through. They reach the model only with `llm.images: true` or
`RUNG_IMAGES=on` (default off: each image is a `[image omitted: …]` note).
A `Toolset` wrapper must forward `execute_output`, or it hands on text only.

Session history: an assistant `Line` keeps the turn's full `messages`
(tool-use, tool-result, final text) from `AgentResult.transcript`, and
`thread_from` replays them. Tool results over 4000 chars are shortened, and
a tool result's images become a note (a session file holds no image data). The
calls are never dropped: text-only history teaches the model to narrate
actions instead of taking them (#128). Old sessions without `messages` still
replay as text. A turn that stops without an answer (error, refusal, doom,
interrupt) stores `Line::failed`: the steps that ran, from
`Filtered.transcript`, and the reason in `failure`, which is never replayed
as assistant speech. An overflow turn stores nothing, not even its ask, so
the request that overflowed is not sent again.

Context overflow: the loop's `Overflowed => Calling` recover edge (`elide`,
G8-guarded) elides the oldest tool results, oldest first until half their
weight is gone, and retries once per turn. The elision is in-memory for the
retry only: stored history is never rewritten, so each prompt that overflows
elides again. Still over: typed `overflow` on ACP, a
`usage_update` with the provider's stated figures before it, nothing stored.
Never retry overflow around `agent::run`: `Filtered` has no live thread and
the turn's tool calls would run twice.

ACP end-to-end with a model: `rung-agent/tests/acp.rs` `mock_llm` is a
std-only OpenAI-compatible server. It serves SSE when the body has
`"stream": true`, which the ACP path always sets, and it records request
bodies. Point `RUNG_BASE_URL` at it and isolate `HOME`, `RUNG_CONFIG` and
`RUNG_HOME`.

ACP handlers run inside the connection's dispatch loop, which reads no
other message until the handler returns. A handler that awaits a turn makes
`session/cancel` a no-op. In `acp.rs` the turn, and any handler that writes
a session file a turn also writes (close, delete, set_mode, fork), go
through `queued`: spawned off the loop, one process-wide FIFO, because a
turn sets the process cwd. Handlers that run beside a turn resolve paths
against `Live::launch`, not the process cwd. Mid-turn tests:
`rung-agent/tests/acp_concurrency.rs` (its mock answers each request on its
own thread, with a delay).

## Turn check (rung-agent-core)

`rung-agent-core/src/turn_check.rs` is a ladder after the agent loop: a judge (Jev, via
`rung_std::decide`) reads the turn's final message against its actions.
Arms: completed / nudge once / unverified / unchecked. Off by default; the
switch is `turn_check.backend` or `RUNG_TURN_CHECK=off|jev`. Off must stay
byte-identical (`off_output_is_byte_identical_to_before`). A nested `task`
spawn (depth 1) is checked like a top-level turn when the check is on.

Tests replay `rung-agent/tests/fixtures/decide/turn_check/*.json` through
a local mock of `/systemone`; CI never touches the network. After changing
the question wording or the state builder, record again (stale and missing
fixtures only; `rerecord` redoes all):

```bash
RUNG_DECIDE=record doppler run -p fleet -c dev_work -- \
  cargo test -p rung-agent --test turn_check -- --test-threads=1
```

The recorder keeps a spend ledger and stops at `RUNG_DECIDE_BUDGET_USD`
(default $0.02). A full record of the set costs about $0.0008. Build
fixtures from real transcripts (`tests/fixtures/transcripts/`), not
author-written easy ones: the one real done turn reads as unsure, and the
gate escalates it to `unverified`.

## Memory (rung-memory + rung-agent-core)

Contract and wire: `docs/rung-memory.md`. One setting, `--memory` >
`RUNG_MEMORY` > `memory.provider` > `off`: `off`, `external` (caller owns
memory: no store, no hooks, no memory tools), `baseline` (BM25, local),
`mcp:<url|command>` (`rung-memory/1` marker, hidden hook tools
`rung_memory_recall` / `rung_memory_retain`). Off must stay byte-identical
(`off_is_byte_for_byte_the_response_before_memory`). Retain takes a
`Turnover`, built only from a `Completion`. A recall block goes in front of
the ask for that call only and is never stored in the session. Provider
failures are outcomes, never a failed turn.

Tests: `rung-agent/tests/acp_memory.rs` drives ACP with a mock model and
the reference provider `rung-agent --memory-fixture` (stdio, `--file` to
persist across the per-prompt respawn). `rung-agent --memory-check SETTING`
runs the contract against any provider. Product crate, not kernel:
`rung-memory` is `publish = false`.

## Config

- Driver: `~/.rung/providers.yaml` + `auth.yaml`. Env first, then auth.yaml.
- Agent: `$XDG_CONFIG_HOME/rung/config.yaml` (`llm.api_key_env`, not the key).
  `RUNG_CONFIG` overrides path. Env `RUNG_*` wins. No key required for LAN
  llama.cpp / vLLM; the client omits Authorization when the key is empty.

## Edit / tools gotchas (kernel)

- Unique `edit` fail-closed: exact count > 1 does not fall through to indent
  match.
- `docs/_props.py cited` kebab-tokens in comments are citations.
- Overflow is `FailureKind::Overflow`, not a content filter. The loop
  elides and retries it once (`elide`) before it gets that far.
- `rung` trybuild `a_match_missing_a_step_outcome_summand_is_e0004`
  (`rung/tests/spec_refusals.rs`)
  can fail locally on roger (rustc diagnostic drift) while CI passes. Check it
  on a stash before blaming your change; CI is the gate.

## Harbor eval → validation suite

Out-of-tree adapter: `rung-agent/python/rung_harbor/agent.py`. Do not fork
Harbor. Harbor `examples/tasks` mixes **agent** tasks with **harness**
tests (network policy, verifier modes, CUA, CUDA). The suite
(`rung_harbor.suite`) is only the agent tasks, cheap-to-dear. Terminal-Bench
2.0 is phase 2 after that ladder is green.

Key: `doppler run -p fleet -c dev_work` (`OPENROUTER_API_KEY`).
Suite model: `openrouter/~deepseek/deepseek-v4-flash-latest`.

```bash
cargo build -p rung-agent --release   # when the binary changed
PYTHONPATH=<rung>/rung-agent/python python3 -m rung_harbor.validate list
PYTHONPATH=<rung>/rung-agent/python \
  doppler run -p fleet -c dev_work -- \
  python3 -m rung_harbor.validate next
python3 -m rung_harbor.validate run cwd-capture   # redo one case
python3 -m rung_harbor.validate show cwd-capture
python3 -m rung_harbor.validate import            # once: copy Harbor jobs in
```

Evidence: `rung-agent/harbor-runs/<UTC>-<id>/` plus `index.jsonl`.
Gitignored. `list` / `next` read the index in this repo, not Harbor's
`jobs/`. `run` always makes a new timestamped folder.

A skip is a **product gap** (e.g. MCP-over-ACP). A fail is a **bug or a missing
affordance**. Read `harbor-runs/<stamp>-<id>/**/rung-agent.txt` before
changing the kernel.

Alpine (`hello-alpine`) is skipped: the release binary is glibc
(`ld-linux-x86-64.so.2`). Without a loader Alpine says `required file not
found`. `gcompat` loads the ELF then dies on `__res_init` (resolver). A
musl/static build would unskip this; do not treat it as a Harbor harness
bug.

## Next

`rung-agent` and the Harbor adapter have landed. Walk the validation
ladder (`validate next`); then `terminal-bench@2.0` one task at a time.
