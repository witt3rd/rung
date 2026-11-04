# AGENTS.md — rung-fleet

The tiered fleet of hosts: typed delegation lifecycle, the brief, and (by
phase) receipts, `crew.*` records and verbs. Product crate, `publish = false`.
Charter for the whole repo: [`../AGENTS.md`](../AGENTS.md).

## What it is

- `lifecycle`: the delegation ladder (spawned, assigned, active, with held and
  stalled as recoverable verdicts, ending in done) and the closure ladder (done
  to retired, enterable only from a `Done` verdict).
- `brief`: the typed contract a task is spawned with; fields are private.
- `records`: the `crew.*` record kinds (schema only).
- `verbs` and the `rung-fleet` command: `list` and `read`, read-only; the
  command holds no behaviour the library lacks.
- Claims: [`docs/rung-fleet-props.md`](../docs/rung-fleet-props.md) (generated;
  do not hand-edit); derivation and phase progress:
  [`docs/rung-fleet-notes.md`](../docs/rung-fleet-notes.md).

## Rules

- Generic: no consumer-specific names, ids or env vars (`docs/_consumer_guard.py`).
- Compile-fail cases live in `tests/ui/` (trybuild `.stderr` pins).
- Test: `cargo test -p rung-fleet --locked`.
