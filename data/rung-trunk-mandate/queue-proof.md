# Merge queue on master: blocked, not applied (2026-11-01)

Status: **the merge queue could not be enabled.** GitHub rejects the `merge_queue`
rule on this repository. The ruleset on master is unchanged
(`ruleset-before.json` is still the live state: one rule, `deletion`).

## Evidence

- Repository `witt3rd/rung` is owned by a **user** account (`owner.type = User`,
  `organization` null), public. Merge queues exist only for repositories owned by
  an organization.
- `PUT /repos/witt3rd/rung/rulesets/19141262` with rules `deletion`,
  `non_fast_forward`, `pull_request` (0 approvals), `required_status_checks`
  (`check`) and `merge_queue` (squash, group size 1) returned
  `422 Validation Failed: Invalid rule 'merge_queue'`.
- Probe: a disabled throwaway ruleset holding only `merge_queue` (full parameter
  set) returned the same 422; a control ruleset holding only `non_fast_forward`
  was created (then deleted) with the same token. So the rule type is refused,
  not the parameters. Both probes were removed; the repo has the one original
  ruleset only.
- PR #357 landed (`merge_group` trigger present in `.github/workflows/ci.yml`,
  aggregator `check` has `if: always()` and `plan` runs the full suite for
  `merge_group`). No `merge_group` run can start until a queue exists, so the
  end-to-end proof (run id, time to green) is **not recorded**.

## Options for the owner

1. Move `witt3rd/rung` to an organization where witt3rd is admin (`janus-forge`
   or `agent-forge`, both free plan, public repos get merge queues), then apply
   the ruleset from `ruleset-after-request.json` here. Old URLs redirect; clone
   remotes, `insteadOf` routing, Actions secrets, the review council and
   CodeQL need a check after the transfer.
2. Stay on the user account and emulate: `pull_request` rule with 0 approvals,
   `required_status_checks` `check` with `strict_required_status_checks_policy`
   true, `non_fast_forward`, `deletion`, and agents merge with
   `gh pr merge --auto --squash`. Strict mode serialises landing but is not a
   queue: when two PRs are green, the second must be updated and re-run (about
   11 minutes) by someone. Auto-merge is already enabled on the repo.

Either way the single committer agent needs a bypass actor (the ruleset has none).
