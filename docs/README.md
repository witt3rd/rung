# docs — map

Where a document disagrees with a normative one, the normative one wins.
Generated files are written by `cargo run -p rung-doctrine --bin render`; do
not edit them by hand (CI runs `render --check`).

| document | explains | status |
|---|---|---|
| [`rung-props.md`](rung-props.md) | `rung` / `rung-macro`: what `ladder!` accepts, emits and guarantees | **normative**, generated |
| [`rung-ct-props.md`](rung-ct-props.md) | `rung`: the category a `ladder` declaration is | **normative**, generated |
| [`rung-het-props.md`](rung-het-props.md) | `rung-het`, `rung-driver`: the pool, the two filters, provenance | **normative**, generated |
| [`conformance.md`](conformance.md) | `rung-doctrine`: per-proposition view of the three props documents | informative, generated |
| [`rung-notes.md`](rung-notes.md) | `rung`: derivation of `rung-props.md` | informative |
| [`rung-ct-notes.md`](rung-ct-notes.md) | `rung`: derivation of `rung-ct-props.md` | informative |
| [`rung-het-notes.md`](rung-het-notes.md) | `rung-het`: derivation of `rung-het-props.md` | informative |
| [`rung-het-publishing.md`](rung-het-publishing.md) | `rung-het`: brief for an external reviewer of the formalism | informative |
| [`triage.md`](triage.md) | `rung-doctrine`: how each proposition was classified | informative |
| [`composition-notes.md`](composition-notes.md) | `rung-het` / `rung-driver`: what theory composition would need | informative |
| [`theory-of-theories-in-context.md`](theory-of-theories-in-context.md) | `rung-het` / `rung-std`: catalog and router design capture | informative, evolving |
| [`rung-std/issues.md`](rung-std/issues.md) | `rung-std` `issues` theory | informative |
| [`rung-std/principals.md`](rung-std/principals.md) | `rung-std` `principals` theory | informative |
| [`rung-std/questions.md`](rung-std/questions.md) | `rung-std` `questions` theory | informative |
| [`rung-host.md`](rung-host.md) | `rung-host`: the continuous single-agent host | informative |
| [`rung-host-results.md`](rung-host-results.md) | `rung-host`: gate results for slice 1 | informative |
| [`rung-memory.md`](rung-memory.md) | `rung-memory`, `rung-agent-core`: the `rung-memory/1` provider contract | informative |

Tooling (not documents): `_props.py` (`check`, `cited`), `_consumer_guard.py`
(with `_consumer_guard.allow`), `_migrate.py`.

API reference is rustdoc: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace
--no-deps --locked` (a CI step; it builds clean).
