---
name: git
description: >-
  House git discipline for this repo. Two modes: debug on master (commit
  frequently); worktrees only for parallel features. Covers hygiene, rebase
  merge, never git add -A. Use when committing, branching, landing, or asking
  'is this repo clean'.
metadata:
  aliases: [worktree, repo-hygiene, clean-end-state]
  deps: [caretaker]
---

# Git in this tree

House source: `fleet_git` (`~/.agents/skills/fleet_git`). This file is the
rung overlay. If they disagree, `fleet_git` wins except where this file
names a **rung-only** rule.

## Two modes

**Debugging / iteration** — work on `master`. Commit small batches. Never
leave uncommitted work. When stable, branch and PR (`gh pr merge --rebase`).

**Parallel features** — `git wt-new feat/x` → `rung.wt/feat--x/`. Different
agents, different files. After merge: `git wt-rm feat/x`.

Do not hand-run `git worktree add`.

## Rung-only

- Mainline is **`master`**, not `main`.
- Merge method: **rebase**.
- Never `git add -A`. Stage only files you touched. `scripts/agent` stages
  only `.agent/` files.
- Task isolation worktrees (`rung-task/{id}`) are **product**. Do not use
  `git-wt-new` for those.

```bash
git checkout master && git pull origin master   # start of session
# …commit often on master…
git checkout -b fix/foo
git push origin fix/foo
gh pr create --repo witt3rd/rung --base master --head fix/foo
# after merge:
git checkout master && git pull origin master
git branch -d fix/foo
```

## Worktrees (mode 2)

```bash
git wt-new docs/foo
cd <parent>/rung.wt/docs--foo
# commit, push, PR
git wt-rm docs/foo
```

Branch `docs/<x>` / `fix/<x>` / `feat/<x>` → folder `docs--<x>`.

## Hygiene

A repo is clean when: no stale worktrees, no leftover local branches, `master`
at origin tip, **no uncommitted work**. Unpushed commits on `master` during
debugging are allowed until you branch for a PR; then push the branch.

Before a PR: sync with `origin/master`, rebase if needed.

## Sibling skills

- **`caretaker`** — session-end hygiene.
- **`signalling`** — ledger (`scripts/agent`), not a `HANDOFF.md`.
