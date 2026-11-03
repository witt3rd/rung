# AGENTS.md — rung-host

The rung host: one agent that runs continuously, or rests between wakes (idle mode). It owns the inbox, calendar, registers, free-time kernel, decision desk, layered context pack, memory, governor and stop, and writes the record the UI reads.
Repo charter: [`../AGENTS.md`](../AGENTS.md).

**Status.** Product binary and library. `publish = false`.

**Read.**
- Design and record vocabulary: [`docs/reference/rung-host.md`](../docs/reference/rung-host.md); read and write doors: [`docs/reference/rung-host-doors.md`](../docs/reference/rung-host-doors.md) and [`rung-host-queue.md`](../docs/reference/rung-host-queue.md); idle mode claims: [`docs/rung-host-idle-props.md`](../docs/rung-host-idle-props.md); first run: [`docs/guides/rung-host-quickstart.md`](../docs/guides/rung-host-quickstart.md).
- `live/` holds the live-run scripts and their evidence (`live/runs/`, kept in git, not checked by the doc guards).

**Check.** `cargo test -p rung-host --locked`
