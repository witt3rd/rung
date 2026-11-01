# Master protection: PR + required `check` (strict) + auto-merge (2026-11-01)

rung has **no true merge queue**: GitHub offers it to organizations only, and
`witt3rd/rung` is user-owned. The API refused the `merge_queue` rule with
`422 Invalid rule 'merge_queue'`, for the full parameter set and for a disabled
probe ruleset alike, while a control rule type was accepted (probes deleted; the API responses are quoted here, they are the retained
evidence). The table of remaining remote branches is kept outside the repo
(the consumer guard forbids the branch family names in tracked files).
The owner chose to emulate it.

## Applied (ruleset `protect-default-branch`, id 19141262)

Before: `ruleset-before.json` (one rule, `deletion`). After: `ruleset-after.json`
(read back from the API after the PUT): `deletion`, `non_fast_forward`,
`pull_request` (0 required approvals), `required_status_checks` with the one
context `check` and `strict_required_status_checks_policy: true`; no bypass
actors; enforcement active. Repository auto-merge was already enabled
(`allow_auto_merge: true`); squash, merge and rebase stay allowed.

Open PRs are not blocked: `check` reports on every pull request, and they
merge through `gh pr merge --auto`.

## Proof (real pull request, merged only by auto-merge)

PR https://github.com/witt3rd/rung/pull/364 (the change that wrote this text
and the house-git rule):

- Auto-merge enabled 2026-11-01T22:34:09Z (`gh pr merge --auto --squash`).
- First head: `check` failed (the consumer guard in `propositions` found
  branch-family names in a data file), so auto-merge did not fire: the
  required check holds.
- Second head `63159aa`: CI run 41358728625 (pull_request, docs-only so the
  planner skipped the Rust jobs) started 22:45:35Z, `check` green at
  22:46:34Z (59 s). Auto-merge squash-merged it at 22:46:39Z as `b330a65`,
  by auto-merge, no human action. The push run on master is 41358820614.
- Auto-merge stayed armed across a rebase push: yes.
- Strictness observed: the PR showed `BEHIND` when master moved first; a
  rebase was needed before it could merge.
- Not proven here: a full Rust CI run on the new path (about 11 minutes,
  see AGENTS.md); this proof change was docs-only.

Open PRs 355, 356 and 360 are mergeable through the same path (they report
`check`; no approval is required).
