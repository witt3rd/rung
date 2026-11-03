# Records

Dated, append-only history, one directory per subject. A record is never
rewritten; a correction is a new dated section at the end. Living specs stay
in `docs/`.

- [`rung-host/`](rung-host/AGENTS.md) — the continuous host's live runs, findings and design records.

## Enforcement, 2026-10-29

CI enforces this (`docs/_records_guard.py`): old text stays a byte-for-byte prefix; what is added begins with a dated heading.

## Moved into records, 2026-11-01

Dated measurements, briefs and archived letters sat beside the living specs
or outside `docs/`. They moved here with `git mv` (history follows); their
text is unchanged, so paths written inside them still name the old places.
No stub files were left behind.

| old | new |
|---|---|
| `docs/rung-agent-cache-live-memory.md` | `docs/records/rung-agent/rung-agent-cache-live-memory.md` |
| `docs/evidence/cache-live-memory/` | `docs/records/rung-agent/evidence/cache-live-memory/` |
| `docs/rung-decision-model-replay.md` | `docs/records/rung-agent/rung-decision-model-replay.md` |
| `docs/rung-ui-live-cost.md` | `docs/records/rung-ui/rung-ui-live-cost.md` |
| `docs/rung-het-publishing.md` | `docs/records/rung-het/publishing-brief.md` |
| `data/rung-test-requests/rung-webfetch-artifact-build.md` | `docs/records/rung-host/rung-webfetch-artifact-build.md` |
| `docs/.archive/` | `docs/records/archive/` |

Subjects now under `docs/records/`: `rung-host/` (see its README),
`rung-agent/`, `rung-ui/`, `rung-het/`, `archive/` (handoff letters and the
early design notes).

## rung-fleet records, 2026-11-02

- [`rung-fleet/`](rung-fleet/r1-two-hosts.md) — the fleet's measured runs (R1: two hosts behind one gateway).

## README.md files renamed, 2026-11-01

This repository has no `README.md` anywhere; `AGENTS.md` is the one agent file
name. This file and `rung-host/AGENTS.md` were `README.md` (`git mv`, history
follows). The only change to old record text is the link target `README.md`
that became `AGENTS.md` (`docs/_records_guard.py` allows exactly that).
