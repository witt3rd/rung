# Running the host suites locally

Informative. Conventions for running `rung-host`'s tests on a development
machine; the gates themselves are defined in [`rung-host.md`](rung-host.md).

- **No network, no key.** Every gate is loopback-only and free: a mock
  engine and a scripted provider on 127.0.0.1. Nothing needs a credential or
  Doppler. Only the live run (`rung-host/live/`) uses a real router.
- **Run the crate:** `cargo test -p rung-host --locked`. The process gates
  (`gate_process`, `gate_acp`, `gate_acp_http`, `gate_startup`) start the
  built `rung-host` binary themselves; cargo builds it for them.
- **One gate:** name the test target, e.g.
  `cargo test -p rung-host --test gate_time --locked`; add `-- <name>` to
  narrow to one test.
- **Wall-clock gates want a quiet machine.** G-a measures the host's own
  work per boundary, so it runs alone inside its test binary. Waits in the
  process gates stretch with the machine's load (1-minute load per CPU,
  between 1 and 8); gate thresholds never do. A failure under heavy load is
  worth one rerun on a quiet machine before it is investigated.
- **Measurements are `#[ignore]`d.** They are not gates and run by hand with
  `-- --ignored --nocapture` (for example the `measure_admission` target).
- **The UI (trybuild) suites pin the stable compiler's wording.** CI runs
  them only on the stable toolchain; on another toolchain a `.stderr` diff is
  drift, not a regression.
- **Recorded desk fixtures** under `rung-host/tests/fixtures/` are
  regenerated only on purpose, with `RUNG_HOST_WRITE_FIXTURES=1`; review the
  diff like any other change.
- **Before a PR** run the commands in `AGENTS.md` (fmt, clippy, workspace
  tests, `docs/_props.py check`, `docs/_props.py cited`,
  `docs/_consumer_guard.py`).
