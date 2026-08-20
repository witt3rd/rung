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
  `RUNG_CONFIG` overrides path. Env `RUNG_*` wins.

## Edit / tools gotchas (kernel)

- Unique `edit` fail-closed: exact count > 1 does not fall through to indent
  match.
- `docs/_props.py cited` kebab-tokens in comments are citations.
- Overflow is `FailureKind::Overflow`, not a content filter.

## Next

Open PR 113 (`feat/rung-agent`) is the product CLI. Do not mix inhabitation
commits onto that branch; land this charter on `master` independently.
