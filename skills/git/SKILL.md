---
name: git
description: >-
  House git discipline that applies to all repos. The primary clone stays on the mainline and
  is never checked out to a feature branch; all work happens in per-branch linked worktrees
  under <repo>.wt/<branch>/, mechanized by git-wt-new / git-wt-rm helpers. Covers repo hygiene
  and the clean end-state: no stale worktrees, no local branches beyond the mainline,
  mainline at tip of origin, primary clone clean. Use when creating a branch, making any
  change, committing, landing/merging, cleaning up a merged branch, repairing a moved
  worktree, auditing a repo's hygiene, or asking 'is this repo clean' — in any repo.
metadata:
  aliases: [worktree, repo-hygiene, clean-end-state]
  deps: [caretaker]
---

# House git discipline (all repos)

The rule: the primary clone is a clean mirror of the mainline. **Never commit from it, never
check it out to a feature branch.** All work happens in a per-branch **linked worktree** under
`<repo>.wt/<branch>/` (a sibling directory of the primary clone, named `<repo>.wt`).

This is mechanized — do not hand-run `git worktree add`.

## The two commands

```bash
# Create a worktree + branch for a change (run from anywhere in the repo)
git wt-new docs/foo
#   -> <parent>/<repo>.wt/docs--foo/ on branch docs/foo
#   forks from the mainline (--start <ref> overrides)
cd <parent>/<repo>.wt/docs--foo
# ...work, commit, push, open a PR from inside the worktree...

# After the merge: remove the worktree AND delete the branch, together
git wt-rm docs/foo
#   safe-delete: refuses dirty trees / unmerged branches unless --force
```

The helpers are small scripts on PATH (`git-wt-new`, `git-wt-rm`; git auto-discovers
`git-<cmd>` on PATH); they work in any repo.

## Conventions

- Branch `docs/<x>` / `fix/<x>` / `feat/<x>` / `task/<x>` → folder `docs--<x>` (kebab-case,
  `/` → `--`). Match the repo's existing branch shape (`git branch -a`) when unsure.
- Keep the main tree clean; **verify `origin/main` (or `master`) hasn't moved before
  landing**.
- Never commit from the primary clone — always from the feature worktree.

## State and repair

- `git worktree list` — see every worktree and its branch.
- If a worktree got moved out-of-band with a plain `mv`, git's registration still points at
  the old path (listed `prunable`). Repair from inside the moved worktree: `git worktree
  repair` then `git worktree prune`.

## Repo hygiene — the clean end-state (a contract, not a nicety)

"Clean" is a **checkable end-state**, not a feeling. A repo is clean — ready for the next
agent to pick up cold — when ALL of these hold:

1. **No stale worktrees.** Every worktree under `<repo>.wt/` belongs to a branch whose work
   is still in flight. Once a branch is merged (or abandoned), the worktree goes away:
   `git wt-rm <branch>` (removes the worktree AND the branch together). A worktree whose
   branch is already in the mainline is **stale** — remove it unless it's the deliberate
   live wiring for a test. A directory in `<repo>.wt/` that is **not** a registered worktree
   is debris — remove it. The `.wt/` namespace is **flat** (one worktree per branch as a
   direct child); never category subfolders.
2. **No local branches beyond the mainline.** `git branch` should show only the mainline
   plus any branch with a live worktree. A merged branch lingering as a local branch is a
   cleanup miss — `git wt-rm` deletes both.
3. **The mainline is at the tip of origin** (or deliberately ahead/behind and recorded).
   `git status -sb` on the mainline should read `## main...origin/main` with no
   `ahead`/`behind` — unless a push/PR is intentionally deferred, in which case that pending
   state is written down in the handoff so no one is surprised.
4. **The primary clone is clean.** `git status --porcelain` is empty; the mainline is never
   checked out to a feature branch and never has uncommitted/unpushed work sitting in it.

**The one legitimate exception — a worktree wired to a live profile for a test.** A worktree
whose plugin/provider symlinks point at it from a running profile is *intentionally* kept
even after its branch content is merged, because removing it would break the running profile.
That's fine — but say so in the handoff so a future caretaker knows it's deliberate.

**As a caretaker, end every session by checking this list** and either restoring the clean
state or recording the deliberate deviation. Leaving a repo with stale worktrees + leftover
branches + an unpushed mainline is a known, named failure — not an accident.

## As an agent

- Any change = `git wt-new <branch>` → work there → commit → the rest is the repo's normal
  flow. Delete the worktree + branch on merge with `git wt-rm <branch>`.
- Read each repo's AGENTS.md for repo-specific mainline/landing rules (some use `main`,
  some `master`).
- Never `git add -A` — stage only the files you touched. Never force push or `reset --hard`.
- A clean mainline, clean log (`agent check`), and a written handoff is what "done" means.

## Sibling skills

- **`caretaker`** — the loop that applies this hygiene at session end.
- **`signalling`** — the event-log ledger whose integrity this discipline protects.