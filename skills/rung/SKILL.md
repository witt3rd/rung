---
name: rung
description: >
  Lived experience for the rung repo. Load when working in ~/src/witt3rd/rung
  (or a worktree): house git, CI, proposition citations, kernel vs product,
  the ledger. Not the ladder language itself — that is AGENTS.md + docs/.
metadata:
  home: ~/src/witt3rd/rung/skills/rung
  aliases: [rung-repo, rung-caretaker]
---

# rung — how we act in this tree

Charter: repo-root `AGENTS.md`. This skill is the gotchas.

## Ledger, not a last-words file

This repo is inhabited. Wake = `scripts/agent state` then `scripts/agent inbox`.
Sleep = `scripts/agent handoff <subject>`. **Never create `HANDOFF.md`.** The
growing last-words file is taboo; the append-only log in `.agent/log/` is the
channel. Prefer `scripts/agent` over PATH `agent`.

## Git

Primary clone on `master` only. `git wt-new <branch>` → work in
`rung.wt/<branch>/` (`/` → `--`). After merge: `git wt-rm <branch>`, ff
master. Merge method rebase. Never `git add -A`. `scripts/agent` stages only
its own files.

Task isolation worktrees (`{repo}.wt/rung-task--{id}`, branch `rung-task/{id}`)
are product, not `git-wt-new`. Do not use `git-wt-new` for those.

## CI

Required check is `check` (fmt, clippy `-D warnings`, tests `--locked`).
Propositions job: `render --check`, `docs/_props.py check`, `cited`.
`cited` scans `rung`, `rung-het`, `rung-std` comments for kebab slugs.
Wire identifiers that look like slugs: add to `NOT_A_CITATION` in
`docs/_props.py` (`x-api-key`, `x-should-retry`).

## Kernel vs product

`rung-std` tools: filesystem, `kernel_tools` (apply_patch, todo, webfetch,
skill), `task` as nested `Spawn` (depth 1). Named catalogs, session resume,
background child, isolation worktrees, XDG `config.yaml` = `rung-agent`.

## Config

- Driver: `~/.rung/providers.yaml` + `auth.yaml`. Env first, then auth.yaml.
- Agent: `$XDG_CONFIG_HOME/rung/config.yaml` (`llm.api_key_env`, not the key).
  `RUNG_CONFIG` overrides path. Env `RUNG_*` wins. No key required for LAN
  llama.cpp / vLLM; the client omits Authorization when the key is empty.

## Edit / tools gotchas (kernel)

- Unique `edit` fail-closed: exact count > 1 does not fall through to indent
  match.
- `docs/_props.py cited` kebab-tokens in comments are citations.
- Overflow is `FailureKind::Overflow`, not a content filter.

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

A skip is a **product gap** (e.g. MCP). A fail is a **bug or a missing
affordance**. Read `harbor-runs/<stamp>-<id>/**/rung-agent.txt` before
changing the kernel.

## Next

`rung-agent` and the Harbor adapter have landed. Walk the validation
ladder (`validate next`); then `terminal-bench@2.0` one task at a time.
