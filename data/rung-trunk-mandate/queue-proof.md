# Master protection: PR + required `check` (strict) + auto-merge (2026-11-02)

rung has **no true merge queue**: GitHub offers it to organizations only, and
`witt3rd/rung` is user-owned. The API refused the `merge_queue` rule with
`422 Invalid rule 'merge_queue'`, for the full parameter set and for a disabled
probe ruleset alike, while a control rule type was accepted (probes deleted).
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

## Proof

Pending: the PR that carries this file is enabled for auto-merge as the real
test. Results are appended below.
