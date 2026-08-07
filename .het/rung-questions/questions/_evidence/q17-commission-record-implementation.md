**RESOLUTION (Q17):** Implement the commission contribution record and wire it into the pool.

**KIND:** implementation `owed` work, now discharged — this is the build Q16's
ruling left open and Q17 filed. It lands as `rung-driver` code, a record at the
workspace root, a doctrine encoding, and the tests that prove the mechanism.

**WHAT WAS BUILT.**

1. **`rung-driver/src/commission.rs`** — `CommissionLog`, the record Q16 ruled
   on: `family -> commission -> {artifacts}`, plus the active commission set `S`.
   `authored_for(f) = ⋃_{c∈S} C(f,c)`, deduplicated and order-stable.

2. **`PrincipalSpec::family`** — a discontinuous principal declares a stable
   family instead of a growing `authored` array. A principal with a `family`
   that also declares a static `authored` is refused (`FamilyWithAuthored`): two
   sources of truth for the same fact is exactly what the carrier exists to
   remove.

3. **`Configured::authored()`** — derives `authored(p)` from the log by family
   at qualification time; a principal without a family (a person) keeps its own
   genuine, declared record. `population_pool_with_log` wires a real population
   to its record.

4. **`commissions.yaml`** (workspace root) — the record rung's own population
   reads. Deliberately empty of guessed entries (Q16's meta-refusal): no
   commission has recorded a contribution yet, so every model's derived set is
   open — the honest "nothing recorded" state, not the refused per-invocation
   vacuity.

5. **Doctrine** — eight propositions encoded under
   `principal-provenance-floor` (`commission-record-is-the-carrier` through
   `commission-record-roundtrips`), so the mechanism is a *named* guarantee the
   tests can be claimed against rather than an unaccounted impulse.

**WHY IT CLOSES Q17.** Q17's acceptance conditions are met: the record type is
typed and read at qualification; the three conditions (decidable, non-vacuous,
not-total) are each exercised by a test; and the population's static
placeholder is retired — the pool derives `authored` from the record by family.

**WHAT REMAINS IS OPERATIONAL, NOT A QUESTION.** Building the record was Q17.
*Populating* it — recording which family actually produced which artifact under
a real commission — is harness state, not a question to resolve, and nothing is
guessed. The mechanism dispatches non-vacuously the moment a contribution is
recorded; until then every family's derived set is open by design.

**Conformance.** The mechanism is proven by the seven tests in
`rung-driver/tests/commission.rs`, each claimed as the proof of a decidable
proposition in `docs/rung-het-props.md`; the wiring is exercised by
`rectify_questions` and `oracle.rs::model_provenance_is_derived_from_the_commission_record`.
