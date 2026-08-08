# `.het/` — the per-carrier state sidecar (Q18, provisional)

**One carrier's audit-rectify state lives in one place.** A theory applies to
many carriers; each instance gets a folder here holding everything the loop
needs to run, restart, and leave a record — so a second carrier has a home
instead of scattering `population.yaml`/`commissions.yaml`/`questions/`/
`judgments/` across a root.

```text
.het/
  <instance>/              e.g. rung-questions, gh-issues, portfolio
    config.yaml            # theory, carrier (colocated or external), population,
                           #   and the state/ home — what the generic driver reads
    population.yaml        # this instance's principals (or shared one level up)
    commissions.yaml       # the commission record — provenance (or shared)
    questions/             # the carrier's subjects (a flat docket, here)
    judgments/             # the dispatched-judgment schema + records
    state/                 # what the loop WRITES: dispatched judgments, park, log
                           #   (generated; gitignored)
```

**.het/rung-issues/ — the second instance.** Rung currently has two
instances under the sidecar convention: `rung-questions` (a colocated folder
carrier) and **`rung-issues`** (an external **GitHub** carrier — the relegation
destination work items are routed to from the questions instance). Both are
governed by **one shared population** at `.het/population.yaml` (referenced as `../population.yaml` from each
instance — a single source, not a copy per instance).

**Encapsulation is the point.** Every piece of self-hosted state for an
instance — the population, the commission record, the subject docket, the
judgments, and the generated `state/` — lives here, together, so that (1) what
constitutes state is obvious, and (2) the repository root is not polluted with
it. The root holds only the `rung*` sources, `docs/`, and the build files.

- **config.yaml** is an [`Instance`] (rung-driver): the governing theory, the
  [`CarrierConfig`] (colocated folder/jsonl/csv, or external github), the
  population (shared one level up, or bespoke inside the instance), and the
  `state/` directory.
- **The driver reads config → carrier → runs the composed loop
  ([`run_cycle`]) → writes its `dispatched` records into `state/`.** The loop is
  the generic `Pass<E>` engine; the sidecar is where the state lives per carrier.

Deliberately provisional: this actualizes the Q18 shape for rung's own
`rung-questions` instance; the name (`/ .het/` vs something else) and the full
carrier/external + shared/bespoke matrix are Q18's open answers, tracked in
`questions/open/q18-het-state-sidecar-convention.md`.
