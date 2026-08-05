# Conformance

**Status: not normative, and generated.** The three `*-props.md` documents govern.
This is a **view of the doctrine** in [`rung-doctrine/`](../rung-doctrine/), written by
`cargo run -p rung-doctrine --bin render`. Editing it here does nothing; the next
render restores it, and CI fails if the two disagree.

Rows are keyed on a proposition's **slug**, never its number, so the record survives
every renumbering — and the numbers shown are themselves derived at render time.

A proposition's **kind** is what would settle it, which is also who it is dispatched
to. The two middle kinds route to structurally exclusive principals: judgment
requires provenance-disjointness, authorship requires standing.

| kind | meaning |
|---|---|
| `decidable` | a proof exists that fails when the proposition is violated |
| `judgmental` | only a principal can settle it — no test decides this |
| `owed` | decidable in principle; nothing establishes it **yet** |
| `signature` | declares vocabulary; not a claim that could be satisfied |
| `rationale` | an argument, or a recorded limit; not a claim |

**The mechanism column is the part no machine derives.** It says *why* a proof is
the right proof, which is a reading — `establishes_what_it_cites`, judgmental and
unsettled. Where it is blank, nobody has written one down.

---

## `rung-props.md`

**Counts.** 57 decidable · 2 owed · 11 rationale · 70 total.

### Grammar

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [1](rung-props.md#declaration-is-a-block) | `declaration-is-a-block` | `decidable` | The macro accepts exactly this shape. The cited ladder is a declaration block followed by an inline `impl` block and is driven to a terminal verdict, so both halves of the form are exercised by a run rather than by an expansion that merely typechecks. | `rung/tests/end_to_end.rs::drives_to_convergence` |
| [1.1](rung-props.md#declaration-grammar) | `declaration-grammar` | `decidable` | The cited declaration uses every production of the grammar at once — a `carry` block, a multi-hop spine, a verdict block carrying a terminal marker, a recoverable verdict, and a `recover` edge. A production the parser dropped would fail to expand. The refusals that keep the grammar from accepting *more* than this are [2](rung-props.md#macro-must-reject). | `rung/tests/compile_pass.rs::test_module_exists` |
| [1.2](rung-props.md#bodies-grammar) | `bodies-grammar` | `decidable` | The cited ladder supplies three inline bodies in the `ident = closure` form, comma-separated, mixing block and expression closures. They expand into the module and are called by the driver. | `rung/tests/end_to_end.rs::drives_to_convergence` |
| [1.3](rung-props.md#transition-naming) | `transition-naming` | `decidable` | The driver calls `opt::active`, `opt::step` and `opt::iterate` by those names — the target lowercased, `step` for the branching transition, the recover edge's own name. Renaming any of the three in the macro turns the call site into an unresolved path. | `rung/tests/end_to_end.rs::drives_to_convergence` |
| [1.4](rung-props.md#marker-annotates-the-target) | `marker-annotates-the-target` | `decidable` | The cited ladder marks both markable positions — a rung, and the verdict block — and the test coerces `review::active` and `review::step` to `fn` pointers of the exact expected types. A marker that annotated the source rather than the target would put the parameter on the wrong function and both coercions would fail. | `rung/tests/gate_markers.rs::judgmental_transition_takes_a_qualified_token` |
| [1.41](rung-props.md#at-most-one-marker) | `at-most-one-marker` | `decidable` | A `trybuild` case with `#[judgmental(R)] #[authorial(R)]` on one transition, whose committed `.stderr` holds the macro's message. The macro has refused this since markers landed; until the case existed nothing would have noticed if it stopped. | `rung/tests/gate_markers.rs::two_markers_on_one_transition_are_refused` |
| [1.42](rung-props.md#two-markers-implemented) | `two-markers-implemented` | `decidable` | Both markers emit, and emit *different* second parameters — the cited test coerces the authorial transition to `fn(Filed, Authorized<'_, R>) -> Revised`, and `judgmental_transition_takes_a_qualified_token` does the same for `Qualified<R>`. A pen cannot be passed where a licence is asked for, which is the whole content of "two gates, two signatures". | `rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen` |
| [1.43](rung-props.md#conditional-marker-refused) | `conditional-marker-refused` | `decidable` | A `trybuild` case whose committed `.stderr` holds the refusal, including the pointer to the open question. A `compile_fail` doctest would not have distinguished this refusal from a typo ([6.1](rung-props.md#compile-fail-asserts-only-non-compilation)). | `rung/tests/gate_markers.rs::conditional_is_refused_and_names_the_open_question` |
| [1.44](rung-props.md#marker-without-role-refused) | `marker-without-role-refused` | `decidable` | Two `trybuild` cases, one per marker — the cited one for `#[judgmental]`, `authorial_without_a_role_is_refused` for its mirror. Both `.stderr` snapshots carry the reason, which is that there is no signature to emit rather than that the syntax is unfamiliar. | `rung/tests/gate_markers.rs::judgmental_without_a_role_is_refused` |

### Static semantics

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [2](rung-props.md#macro-must-reject) | `macro-must-reject` | `decidable` | All ten rules, each a `trybuild` case with a committed `.stderr`. Two of the ten are unreachable through the grammar rather than untested, and the suite says so where the reachable neighbour lands: rule 2 cannot be written because every rung of the spine is declared by the hop that introduces it, and rule 5's *missing recover function* clause cannot be written because one `recover` entry pushes the edge and the function together. Before these cases, seven of the ten were prose the macro happened to implement. | `rung/tests/spec_refusals.rs::a_duplicate_carry_field_is_refused` |
| [2.1](rung-props.md#structural-rules-mirror-the-reference-checker) | `structural-rules-mirror-the-reference-checker` | `rationale` | A provenance note about a retired artifact. The Python checker is under `.archive/`, nothing in the workspace depends on it, and "verified in sync" records a comparison made once by hand rather than a property anything re-checks. What the note is *about* — that rules 1–8 are structural — is now pinned rule by rule under [2](rung-props.md#macro-must-reject). | — |
| [2.2](rung-props.md#body-rules-need-an-impl-block) | `body-rules-need-an-impl-block` | `decidable` | The cited declaration omits the `impl` block entirely and expands cleanly, so rules 9–10 did not fire on a ladder with no bodies to check. That they *do* fire when the block is present is `spec_refusals.rs::an_impl_body_that_names_no_transition_is_refused` and `::an_impl_block_missing_a_body_is_refused`. | `rung/tests/compile_pass.rs::test_module_exists` |
| [2.3](rung-props.md#resume-rules-are-g2) | `resume-rules-are-g2` | `decidable` | The declaration-time refusals. The cited `trybuild` case declares a resume edge with no `#[authorial(R)]` marker and holds the macro's exact message: an edge emitted inside the seal that anyone may call is [G2](rung-props.md#g2-sealed-construction) with a door in it, so a penless resume is not a signature a caller forgot to satisfy — it is not declarable. Making the marker optional is type-valid and turns this red at the snapshot. | `rung/tests/suspension.rs::a_resume_edge_without_an_authorial_marker_is_refused` |
| [2.4](rung-props.md#extension-refusals-are-pinned) | `extension-refusals-are-pinned` | `decidable` | The proposition names its own three cases; this is the first of them. Each holds a committed `.stderr`, which is what makes the refusal's *message* part of the assertion rather than only its existence. | `rung/tests/spec_refusals.rs::a_recoverable_verdict_cannot_declare_a_payload` |

### Emitted artifacts

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [3](rung-props.md#emitted-module) | `emitted-module` | `decidable` | Every path in the cited test goes through `metricoptimization::`, the ladder name lowercased. A module emitted under another name, or not emitted, is an unresolved path. | `rung/tests/compile_pass.rs::test_module_exists` |
| [3.1](rung-props.md#emitted-carry) | `emitted-carry` | `decidable` | `test_module_exists` constructs `Carry` with both declared fields by name, which needs the struct, the field names, and their public visibility. The cited test adds the accessor: a type-level coercion that only holds if `Spec::carry(&self) -> &Carry` exists with that exact signature. | `rung/tests/compile_pass.rs::test_carry_accessor_exists` |
| [3.2](rung-props.md#emitted-rung-structs) | `emitted-rung-structs` | `decidable` | The seal and the thread-binding, which are the two clauses a host can lose silently. The cited test uses autoref specialization to assert `!Send` for rungs *and* verdicts; the `_seal` field is what `spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624` pins. Constructor visibility follows [G2](rung-props.md#g2-sealed-construction). | `rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync` |
| [3.3](rung-props.md#emitted-verdict-structs) | `emitted-verdict-structs` | `decidable` | All three shapes in one run: `Exhausted::new()` is the bare terminal marker, `Converged(Report)` is a terminal carrying a payload read back out through `.payload()`, and `Iterating => Active` is a recoverable verdict built from its source rung and unwrapped with `.into_source()`. The fourth clause — that a continue arm emits **no** verdict struct — is `end_to_end.rs::continue_arm_loops_without_a_recover_fn`. | `rung/tests/end_to_end.rs::drives_to_convergence` |
| [3.4](rung-props.md#emitted-suspended) | `emitted-suspended` | `decidable` | The residual channel as a type. The cited test coerces the emitted `fn` to a `fn` pointer of the exact expected type — `fn(Posed, Qualified<Adjudicator>) -> Result<Answered, Suspended<Posed>>` — so dropping the summand from the return type is a compile error at that line rather than a silently weaker signature, and it then reads the unconsumed token back out and finds the very argument. Emission is CONDITIONAL, which is what keeps [G12](rung-props.md#g12-gate-marked-signature)'s compatibility clause true: an unmarked ladder emits no `Suspended` and its module is byte-identical. | `rung/tests/suspension.rs::a_judgmental_forward_transition_returns_the_argument_unconsumed` |
| [3.41](rung-props.md#suspended-reports-what-it-awaits) | `suspended-reports-what-it-awaits` | `decidable` | The emitted `impl ::rung::Awaiting for Suspended<Prev>` is what lets a holder read what a run awaits off the run instead of being told. The cited suite parks suspensions from a real ladder and matches them by that trait alone; deleting the impl from the macro's emission makes the whole file fail to compile with `Suspended<Filed>: Awaiting is not satisfied`, because `Park<S>` is bounded on it. That the bound carries the claim — rather than a `raised` field read a holder could have done anyway — is the content of the proposition. | `rung-std/tests/driver.rs::a_parked_run_is_released_by_its_evidence_and_resumes_to_a_terminal` |
| [3.5](rung-props.md#emitted-step-outcome) | `emitted-step-outcome` | `decidable` | The clause that distinguishes `StepOutcome` from an ordinary verdict enum: a continue arm's variant carries a **live target rung**, not a verdict marker. The cited test reassigns that rung straight back into the driver, with no recover function and no guard in between. | `rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn` |
| [3.6](rung-props.md#emitted-failed) | `emitted-failed` | `decidable` | The cited test takes the error path and reads both fields back — the unconsumed `token` and the `error` string — which is what makes `Failed<Prev>` a recovery vehicle rather than a discarded value. | `rung/tests/end_to_end.rs::recovers_from_the_failed_error_path` |
| [3.7](rung-props.md#emitted-guards) | `emitted-guards` | `decidable` | `must_progress` is the one an author cannot see: the cited ladder's recover body contains no call to it and panics anyway, because the macro wrapped the body ([G8](rung-props.md#g8-recovery-progress)). The other two guards are pinned the same way, at `gate_markers.rs::a_body_that_ignores_the_token_still_gets_the_binding_check` and `::a_body_that_ignores_the_pen_still_gets_the_standing_check`. | `rung/tests/end_to_end.rs::recover_guard_is_auto_injected` |
| [3.8](rung-props.md#emitted-functions) | `emitted-functions` | `decidable` | One `pub fn` per transition and per recover edge, expanded *inside* the module: the cited bodies call `Active::new`, which is private to the module and unreachable from the test file. A body expanded outside would not compile. The type-only case — no `impl` block, no functions — is `compile_pass.rs::a_marker_on_a_type_only_declaration_is_inert`. | `rung/tests/end_to_end.rs::drives_to_convergence` |
| [3.81](rung-props.md#unmarked-signature) | `unmarked-signature` | `decidable` | The driver calls `opt::active(spec)` with one argument. An unmarked transition that grew a second parameter is E0061 at that call site — which is the same diagnostic `gate_markers.rs::calling_a_judgmental_transition_without_a_token_is_e0061` pins from the other side. | `rung/tests/end_to_end.rs::drives_to_convergence` |
| [3.82](rung-props.md#judgmental-signature) | `judgmental-signature` | `decidable` | The cited test coerces the emitted `fn` to `fn(review::Spec, Qualified<Reviewer>) -> review::Active`, so an absent, extra, or differently-typed second parameter fails to compile. The injected prologue is separately pinned by `gate_markers.rs::the_injected_prologue_refuses_a_transferred_token_the_body_never_reads`, whose ladder never reads the token. | `rung/tests/gate_markers.rs::judgmental_transition_takes_a_qualified_token` |
| [3.83](rung-props.md#judgmental-outcome-bound) | `judgmental-outcome-bound` | `decidable` | The emitted forward judgmental transition is followed by the injected outcome epilogue ([G15](rung-props.md#g15-outcome-provenance)), so its *target* payload must implement `::rung::Provenanced` as well as its source. The cited test is the epilogue firing: a body that returns its own argument does not complete. Removing the injected call from the macro reddens it; a branching judgmental transition gets no epilogue, which is why `review::step` still compiles with a `Report` payload that carries no provenance. | `rung/tests/gate_markers.rs::the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render` |
| [3.84](rung-props.md#resume-signature) | `resume-signature` | `decidable` | Three parameters and two injected prologues. The cited test coerces the emitted resume `fn` to its exact pointer type, so the pen cannot quietly leave the signature; its siblings pin the prologues — a pen minted over another container is refused although the body never mentions it, and evidence about another raised matter resumes nothing. Deleting the injected `must_hold_standing_over` reddens `::resume_refuses_a_pen_over_another_container`. | `rung/tests/suspension.rs::a_suspension_resumes_through_the_authorial_edge` |
| [3.85](rung-props.md#authorial-signature) | `authorial-signature` | `decidable` | The authorial mirror, coerced the same way to `fn(revision::Filed, Authorized<'_, Curator>) -> revision::Revised`, with the standing prologue pinned by `gate_markers.rs::the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads`. | `rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen` |
| [3.9](rung-props.md#body-name-resolution) | `body-name-resolution` | `decidable` | The cited bodies name `Active`, `StepOutcome`, `Converged` and `Carry` unqualified, and `LoopState`/`Report` from the surrounding scope through the emitted `use super::*`. Dropping either half leaves an unresolved name at expansion. | `rung/tests/end_to_end.rs::drives_to_convergence` |

### Guarantees

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [4](rung-props.md#guarantees) | `guarantees` | `decidable` | — | `rung-doctrine/tests/roundtrip.rs::every_guarantee_names_a_proof` |
| [G1](rung-props.md#g1-linear-consumption) | `g1-linear-consumption` | `decidable` | — | `rung/tests/spec_refusals.rs::using_a_rung_after_a_transition_consumed_it_is_e0382` |
| [G2](rung-props.md#g2-sealed-construction) | `g2-sealed-construction` | `decidable` | — | `rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624` |
| [G3](rung-props.md#g3-one-token-one-thread) | `g3-one-token-one-thread` | `decidable` | — | `rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync` |
| [G4](rung-props.md#g4-no-silent-drop) | `g4-no-silent-drop` | `decidable` | — | `rung/tests/spec_refusals.rs::dropping_a_verdict_under_deny_must_use_is_an_error` |
| [G5](rung-props.md#g5-carry-immutability) | `g5-carry-immutability` | `decidable` | — | `rung/tests/compile_pass.rs::test_carry_accessor_exists` |
| [G6](rung-props.md#g6-exhaustive-outcomes) | `g6-exhaustive-outcomes` | `decidable` | — | `rung/tests/spec_refusals.rs::a_match_missing_a_step_outcome_summand_is_e0004` |
| [G7](rung-props.md#g7-recover-pairing) | `g7-recover-pairing` | `decidable` | Rules 4–7, one `trybuild` case each. The cited one is the first direction (a recoverable verdict with no edge); `::a_recover_edges_target_must_be_a_declared_rung`, `::a_terminal_verdict_may_not_carry_a_recover_edge` and `::a_recover_edge_must_name_a_declared_verdict` are the rest. This guarantee said *(macro — static checks.)* and named no test, so it was the one guarantee of the fourteen with nothing behind it. | `rung/tests/spec_refusals.rs::a_recoverable_verdict_without_a_recover_edge_is_refused` |
| [G8](rung-props.md#g8-recovery-progress) | `g8-recovery-progress` | `decidable` | — | `rung/tests/end_to_end.rs::recover_guard_is_auto_injected` |
| [G9](rung-props.md#g9-error-path-recovery) | `g9-error-path-recovery` | `decidable` | — | `rung/tests/end_to_end.rs::recovers_from_the_failed_error_path` |
| [G10](rung-props.md#g10-continue-arms) | `g10-continue-arms` | `decidable` | — | `rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn` |
| [G11](rung-props.md#g11-terminal-payloads) | `g11-terminal-payloads` | `decidable` | — | `rung/tests/end_to_end.rs::drives_to_convergence` |
| [G12](rung-props.md#g12-gate-marked-signature) | `g12-gate-marked-signature` | `decidable` | — | `rung/tests/gate_markers.rs::judgmental_transition_takes_a_qualified_token` |
| [G13](rung-props.md#g13-token-binding) | `g13-token-binding` | `decidable` | — | `rung/tests/gate_markers.rs::the_injected_prologue_refuses_a_transferred_token_the_body_never_reads` |
| [G14](rung-props.md#g14-the-authorial-gate) | `g14-the-authorial-gate` | `decidable` | — | `rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen` |
| [G15](rung-props.md#g15-outcome-provenance) | `g15-outcome-provenance` | `decidable` | — | `rung/tests/gate_markers.rs::the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render` |
| [G16](rung-props.md#g16-the-residual-channel) | `g16-the-residual-channel` | `decidable` | — | `rung/tests/suspension.rs::a_judgmental_forward_transition_returns_the_argument_unconsumed` |

### Non-guarantees

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [5](rung-props.md#non-guarantees) | `non-guarantees` | `rationale` | The heading of the withdrawals. A non-guarantee states that the macro does **not** enforce something and that a claim it does has no standing; there is no obligation left for a host to discharge. Its children point at the boundary tests where a boundary exists. | — |
| [5.1](rung-props.md#transition-body-correctness) | `transition-body-correctness` | `decidable` | The typestate/verification boundary. The type proves a transition ran; nothing here claims its logic was valid, so there is nothing to check. Every `expressible` row in this ledger inherits this limit. | `rung/tests/non_guarantees.rs::a_transition_body_may_be_wrong_and_the_macro_does_not_care` |
| [5.2](rung-props.md#cross-crate-provenance) | `cross-crate-provenance` | `decidable` | A rung crossing a crate boundary is trusted, like any Rust public API. Closing it needs a sub-crate per ladder, which is a packaging decision rather than a macro guarantee. | `rung-fixture/tests/consumer.rs::a_consumer_cannot_tell_a_real_order_from_an_invented_one` |
| [5.3](rung-props.md#same-module-fabrication) | `same-module-fabrication` | `decidable` | The module-boundary limit Rust always has. The cited test pins where the seal *does* bite — external construction is E0624 — so the withdrawal is readable as a boundary rather than as an absence. | `rung/tests/non_guarantees.rs::the_entry_constructor_and_the_module_itself_may_fabricate` |
| [5.4](rung-props.md#drop-proofing-beyond-the-lint) | `drop-proofing-beyond-the-lint` | `decidable` | `mem::forget`, `let _ = token`, and a dropped container all bypass `#[must_use]`; true no-drop needs language-level linear types. The cited test pins the lint's actual reach, which is what is being bounded. | `rung/tests/non_guarantees.rs::a_must_use_token_can_still_be_discarded_three_ways` |
| [5.5](rung-props.md#liveness-beyond-the-guard) | `liveness-beyond-the-guard` | `decidable` | [G8](rung-props.md#g8-recovery-progress) catches an identical-token stall; general forward progress is a halting question. The cited test exercises the guard on exactly the case it does catch, so what is being withdrawn is legible as the complement of something real. | `rung/tests/non_guarantees.rs::the_progress_guard_is_satisfied_by_motion_that_goes_nowhere` |
| [5.6](rung-props.md#suspension-is-in-process-only) | `suspension-is-in-process-only` | `decidable` | A driver may hold a `Suspended<Prev>` in memory for as long as it likes and that is the whole of the claim. Persisting one across process death is not merely unimplemented — a rung read back from bytes is a mid-ladder rung nobody traversed to, which is what [G2](rung-props.md#g2-sealed-construction) exists to refuse, and [6.552](rung-het-props.md#resumption-is-authorial) answers WHO MAY revive a run without saying WHAT a reconstituted token is. Filed rather than guessed at. | `rung/tests/non_guarantees.rs::a_suspension_may_be_held_across_arbitrary_intervening_work` |
| [5.7](rung-props.md#gate-faithfulness-not-secured) | `gate-faithfulness-not-secured` | `rationale` | Narrowed, not closed. rung now checks the way **out** as well as the way in: G15 is the judgmental outcome epilogue and `settle` is its sentence-surface twin, so [5.72](rung-props.md#returned-value-unconstrained) no longer describes the whole outward side. What keeps this non-guarantee standing is [5.71](rung-props.md#one-gate-unimplemented) — one of Het's four gates has a refusal rather than an encoding — and the residue at [5.621](rung-props.md#outward-conditions-remaining). The cited test is the first of those, made runnable. | — |
| [5.71](rung-props.md#one-gate-unimplemented) | `one-gate-unimplemented` | `owed` | The refusal itself is enforced, which is the honest reading: `#[conditional(..)]` is a parse-time `compile_error!` naming the open question, pinned by a `trybuild` snapshot. What is *not* secured is gate-faithfulness for an algebra that has a conditional operation — that algebra cannot be written here at all, and the refusal is what says so. | **owed** — unimplemented — #[conditional(..)] is a parse-time refusal, not an encoding |
| [5.72](rung-props.md#returned-value-unconstrained) | `returned-value-unconstrained` | `decidable` | The proposition used to read "the returned value is unconstrained", and the measure of it was that `Prov::contained_in` existed and no guarantee called it. Two guarantees call it now. G15 injects `must_derive_from_judge` after every forward `#[judgmental(R)]` body, and `theory!`'s `settle` refuses a `Judgment` whose provenance the licence does not contain. The cited test is the constant arrow as a ladder — a judgmental body that returns the argument it was handed — and deleting the injected call from the macro reddens it. The companion `::a_judgmental_arrow_may_not_return_the_provenance_it_judged` is the positive case, and minting the token's `Judgment` with the argument's provenance instead of the judge's reddens that one. What is NOT covered is stated at [5.621](rung-props.md#outward-conditions-remaining) and parked on its own test rather than folded in here. | `rung/tests/gate_markers.rs::the_injected_epilogue_refuses_an_outcome_the_judge_did_not_render` |
| [5.721](rung-props.md#outward-conditions-remaining) | `outward-conditions-remaining` | `owed` | The two halves of the outward side that G15 does not reach. The authorial conjunct π(f(a)) ⊆ π(p) from [5.41](rung-het-props.md#admissibility-subcategories) is left to the body exactly as the judgmental one was before R2 — G14 secures `standing` on the way in and nothing looks on the way out — and the cited test is that arrow: an honest pen over the right container, and a revision carrying someone else's provenance. A branching judgmental transition takes the prologue and no epilogue, because its recoverable and continue arms carry the argument onward by design ([7.44](rung-het-props.md#reproposal-carries-the-chain)) and which arms are *outcomes* is unsettled; that is a question rather than a hole, and it is recorded in the same `#[ignore]` reason. | **owed** — unimplemented — the authorial containment conjunct is left to the body |
| [5.73](rung-props.md#decidable-is-not-pure) | `decidable-is-not-pure` | `decidable` | rung has no effect system. The unmarked signature excludes Het's outside — there is no parameter a principal could enter through — and says nothing about clocks, files, or sockets. Het states the same limit independently ([11.42](rung-het-props.md#purity-not-secured)). | `rung/tests/non_guarantees.rs::an_unmarked_transition_may_touch_the_world` |
| [5.74](rung-props.md#type-only-marker-is-inert) | `type-only-marker-is-inert` | `decidable` | A declaration with no `impl` block emits no transition functions, so a marker on one has no signature to change. The cited test states that as something the compiler checks: the marked role type does **not** implement `Role`, and the declaration compiles anyway — which it could not if the marker were emitting a `Qualified<R>` parameter or a prologue. | `rung/tests/compile_pass.rs::a_marker_on_a_type_only_declaration_is_inert` |
| [5.75](rung-props.md#gate-faithfulness-answered-no) | `gate-faithfulness-answered-no` | `rationale` | A claim about an argument, not about the host: it records that Q11 is open and answered *no*. The two things it stays open on are [5.71](rung-props.md#one-gate-unimplemented), which is `enforced` as a refusal, and [5.72](rung-props.md#returned-value-unconstrained), which is `parked`. Both carry their own row; this one carries the reasoning. | — |
| [5.8](rung-props.md#a-cycle-through-an-authorial-act-cannot-close) | `a-cycle-through-an-authorial-act-cannot-close` | `rationale` | A limit on the DECLARATION, recorded rather than worked around. `ladder!` declares a linear spine with backward continue arms, and a continue arm's target rung is built inline by `step` ([G10](rung-props.md#g10-continue-arms)) — by whoever holds that transition's token. An `Accept -> Governed` arm on the pass would therefore have the JUDGE produce the revised subject, which [7.42](rung-het-props.md#no-amending-disposition) forbids. So `enact` sits outside the branching transition and the loop of [7.5](rung-het-props.md#enact-makes-an-endofunctor) closes by composition, not inside one declaration. The cited test is the shape as built: `Accept` is terminal and carries a `Licence`, and the run leaves the ladder to enact. Expressing the composite as a declaration is Q4 (`questions/open/q4-composition-nested-ladders.md`), open — nothing here is claimed to close it. | — |

### Conformance

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [6](rung-props.md#conformance-suite) | `conformance-suite` | `decidable` | "A change that violates any guarantee MUST break at least the cited test" is only a claim if the citation is live. `./_ledger.py check` regenerates every row from the propositions documents and fails when a cited file is missing or a cited `fn` has been renamed away, so a guarantee cannot quietly lose its test. | `rung-doctrine/tests/roundtrip.rs::the_conformance_record_is_rendered_from_the_doctrine` |
| [6.1](rung-props.md#compile-fail-asserts-only-non-compilation) | `compile-fail-asserts-only-non-compilation` | `rationale` | A fact about rustdoc — it ignores the `E0NNN` in a `compile_fail` fence — rather than an obligation on the host. What follows *from* it is [6.2](rung-props.md#no-guarantee-cites-a-compile-fail-doctest), and that is enforced. | — |
| [6.2](rung-props.md#no-guarantee-cites-a-compile-fail-doctest) | `no-guarantee-cites-a-compile-fail-doctest` | `decidable` | `./_ledger.py check` refuses any conformance citation that points into a crate's `src/`, which is the only place a doctest can live. A row that tried to rest on a `compile_fail` fence is a ledger failure rather than a reviewer's catch. Refusals are pinned by `trybuild` cases in `rung/tests/ui/`, whose committed `.stderr` makes the message part of the assertion. | `rung-doctrine/tests/roundtrip.rs::every_decidable_proposition_names_a_proof_that_resolves` |
| [6.3](rung-props.md#two-silent-doctest-traps) | `two-silent-doctest-traps` | `rationale` | Two ways to write a doctest that passes while asserting nothing. Guidance for authors of examples; the guarantees do not rest on doctests at all ([6.2](rung-props.md#no-guarantee-cites-a-compile-fail-doctest)). | — |
| [6.4](rung-props.md#a-refusal-test-that-cannot-fail) | `a-refusal-test-that-cannot-fail` | `rationale` | The mutation discipline itself — make the guarded thing legal and watch the case go red. It is a rule about how evidence is produced, and no machine performs it; a checker that could would be the guarantee. | — |

### Design judgments

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [7](rung-props.md#design-judgments) | `design-judgments` | `rationale` | The document says of this subtree that **no machine decides them** and that they carry no conformance test. They bind design decisions — where a ladder stops, what earns a place in `rung-std` — and are amended as rulings on the record rather than checked. | — |
| [J1](rung-props.md#j1-where-the-tower-bottoms-out) | `j1-where-the-tower-bottoms-out` | `rationale` | A judgment about leverage: extend the tower while structural enforcement still buys correctness gains. Nothing in a run can answer it. | — |
| [J2](rung-props.md#j2-what-belongs-in-rung-std) | `j2-what-belongs-in-rung-std` | `rationale` | A judgment about recurrence and canonicity. A test could count dependents; it could not decide whether the canonical statement is better than a project's own derivation. | — |

---

## `rung-het-props.md`

**Counts.** 48 decidable · 25 judgmental · 1 owed · 101 rationale · 27 signature · 202 total.

### The relation

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [1](rung-het-props.md#one-relation) | `one-relation` | `signature` | — | — |
| [1.1](rung-het-props.md#institution-quadruple) | `institution-quadruple` | `signature` | — | — |
| [1.11](rung-het-props.md#sign-category) | `sign-category` | `signature` | — | — |
| [1.12](rung-het-props.md#sen-functor) | `sen-functor` | `signature` | — | — |
| [1.13](rung-het-props.md#mod-functor) | `mod-functor` | `signature` | — | — |
| [1.14](rung-het-props.md#satisfaction-typing) | `satisfaction-typing` | `signature` | — | — |
| [1.2](rung-het-props.md#satisfaction-condition) | `satisfaction-condition` | `signature` | — | — |
| [1.3](rung-het-props.md#signature-declares) | `signature-declares` | `signature` | — | — |
| [1.31](rung-het-props.md#extension-is-in-models) | `extension-is-in-models` | `rationale` | — | — |
| [1.32](rung-het-props.md#no-layer-above-sigma) | `no-layer-above-sigma` | `rationale` | — | — |
| [1.4](rung-het-props.md#rest-is-bookkeeping) | `rest-is-bookkeeping` | `rationale` | — | — |

### The gate

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [2](rung-het-props.md#gate-marker-required) | `gate-marker-required` | `decidable` | — | `rung-het/tests/gate_law.rs::every_sentence_carries_a_gate_from_the_declared_vocabulary` |
| [2.1](rung-het-props.md#four-gates) | `four-gates` | `decidable` | — | `rung-het/tests/gate_law.rs::every_sentence_carries_a_gate_from_the_declared_vocabulary` |
| [2.11](rung-het-props.md#no-other-gate-value) | `no-other-gate-value` | `signature` | — | — |
| [2.2](rung-het-props.md#unmarked-not-wellformed) | `unmarked-not-wellformed` | `signature` | — | — |
| [2.3](rung-het-props.md#judgmental-declares-role) | `judgmental-declares-role` | `signature` | — | — |
| [2.31](rung-het-props.md#role-not-kind) | `role-not-kind` | `decidable` | Two axes, and a supplier that declares both is what makes their independence visible. `rung-std::principals::Kind` is substrate — the supplier's, closed, with identity fields and a tier; `Role` is what a sentence needs done and is `rung`'s type. The cited test plays one role across all four kinds and shows a kind entitled to no role it has not earned. The one apparent exception — a competence that excludes a bare model — is stated in that role's own minimum qualifications and never in the partition, which is the asymmetry this proposition names. | `rung-std/tests/principals_theory.rs::role_is_not_kind_and_the_two_axes_are_independent` |
| [2.32](rung-het-props.md#role-declared-pointwise) | `role-declared-pointwise` | `rationale` | — | — |
| [2.4](rung-het-props.md#authorial-declares-standing) | `authorial-declares-standing` | `decidable` | G14. `#[authorial]` with no role is a `compile_error!` — the qualifying set is a conjunction and a marker naming no role can witness only its right half — and the pen that IS emitted carries the container standing was measured over. The macro then injects `must_hold_standing_over(&src.payload, &pen)` ahead of the body, so the declared predicate is consulted whether or not the body mentions it: the cited ladder's body never does. Stubbing the prologue to a no-op turns it red. This is what makes a marked transition's source payload have to be `Situated` — without a container there is nothing standing could be over. | `rung/tests/gate_markers.rs::the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads` |
| [2.5](rung-het-props.md#conditional-names-classifier) | `conditional-names-classifier` | `signature` | — | — |
| [2.51](rung-het-props.md#classifier-not-judgmental) | `classifier-not-judgmental` | `rationale` | — | — |
| [2.52](rung-het-props.md#conditional-partitions-fiber) | `conditional-partitions-fiber` | `judgmental` | — | *awaits a* `category-theorist` |
| [2.53](rung-het-props.md#classifier-one-level-up) | `classifier-one-level-up` | `judgmental` | — | *awaits a* `category-theorist` |
| [2.54](rung-het-props.md#decidability-expressible-internally) | `decidability-expressible-internally` | `judgmental` | — | *awaits a* `category-theorist` |

### The pool

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [3](rung-het-props.md#pool-is-parameter) | `pool-is-parameter` | `signature` | — | — |
| [3.1](rung-het-props.md#pool-not-a-sort) | `pool-not-a-sort` | `rationale` | — | — |
| [3.11](rung-het-props.md#internalizing-outside-collapses) | `internalizing-outside-collapses` | `judgmental` | — | *awaits a* `category-theorist` |
| [3.2](rung-het-props.md#pool-is-opaque) | `pool-is-opaque` | `rationale` | — | — |
| [3.21](rung-het-props.md#supplier-interface) | `supplier-interface` | `signature` | — | — |
| [3.22](rung-het-props.md#interface-by-signature-inspection) | `interface-by-signature-inspection` | `rationale` | — | — |
| [3.23](rung-het-props.md#nothing-further-required) | `nothing-further-required` | `rationale` | The division is now observable from both sides. `rung::Principal` asks for `capable` and `id`, `Provenanced` for `π`, `Steward` for standing — and nothing anywhere in `rung` names a kind, a substrate partition, an identity field, a cost tier or a population. `rung-std::principals` names all five, because a supplier that named none of them would have supplied nothing. The cited test binds the interface at its declared arities and shows the licence that comes back out carrying an id, a provenance and a role — the kind, its required fields and its tier stay on the supplier's side of the line. What is NOT enforced: that a future `rung` stays incurious. Nothing structurally prevents the library growing a `Kind`; this row records that it has not. | — |
| [3.24](rung-het-props.md#capable-single-arity) | `capable-single-arity` | `rationale` | `Principal::capable(&self, role_name: &str)` — one arity, and the second argument is a NAME. A supplier keys its qualification table on that name (`rung-std::principals::RoleSpec`), because a `Role` type cannot be recovered from a string; that is the shape this proposition forces, met rather than worked around. The cited test passes a *sentence* name where a role name goes and gets `false`: a principal does not have the theory's sentences and cannot be asked to inspect them. rung proves the arity, not that any supplier's table is right. | — |
| [3.25](rung-het-props.md#principal-provenance-floor) | `principal-provenance-floor` | `rationale` | — | — |
| [3.3](rung-het-props.md#three-belonging-predicates) | `three-belonging-predicates` | `rationale` | — | — |
| [3.31](rung-het-props.md#ordering-is-hetopts) | `ordering-is-hetopts` | `decidable` | Cost tier is declared — per substrate kind, in `rung-std::principals::Kind::cost_tier` — and ordered nowhere. The cited test is the direct observation: roster A is laid out so the qualifying set opens with the costliest substrate and closes with the cheapest, and `Pool::qualify_for` picks the human over the model. Under the minimal-judge rule the order inverts. Deriving `Ord` on `CostTier` and sorting the set by it in `qualifying_set` is type-valid and turns the test red at the kind sequence. This row was `out-of-scope` while nothing in the workspace declared a tier; a supplier now does, and ordering it is a thing a host can refuse to do. | `rung-std/tests/principals_theory.rs::the_qualifying_set_is_not_ordered_by_cost` |
| [3.32](rung-het-props.md#epsilon-declared-not-ranked) | `epsilon-declared-not-ranked` | `signature` | HALF HOLDS, HALF IS A GAP. *Never ranked*: `rung-std::principals` declares an `Epsilon` per principal and no accessor and no comparison exist for one, so nothing can read it as a preference; `principals_theory.rs::nothing_in_the_workspace_orders_by_cost_or_epsilon` enforces that across every source file. *Declared so the verdict can carry its error bar*: it cannot. `Settled::Judgmental` carries sentence, role, principal and verdict, and there is no field for an error bar — so the ε a supplier already declares stops at the supplier. This is a **different** gap from [4.6](rung-het-props.md#epsilon-reported-with-verdict), which asks whether a judge's confidence is expressible at all; this one asks whether the ε that IS declared reaches the caller. Deleting the `#[ignore]` reports it. | — |
| [3.4](rung-het-props.md#one-pool-two-filters) | `one-pool-two-filters` | `decidable` | G14, and this is the row G14 exists for. One `Pool` mints both tokens; the gate marker on a `ladder!` transition selects which filter runs, not which pool is consulted. `#[judgmental(R)]` emits `Qualified<R>` and runs capability + disjointness; `#[authorial(R)]` emits `Authorized<'_, R>` and runs capability + standing. The cited test drives the same three principals through both filters over one subject and asserts they DISAGREE. Dropping the capability conjunct from `Pool::authorize` turns it red. | `rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one` |
| [3.5](rung-het-props.md#judgmental-qualifying-set) | `judgmental-qualifying-set` | `signature` | Both conjuncts are implemented and both are tested — competence by `gate_law.rs::competence_is_filtered_before_provenance_matters`, disjointness by `::p0_refuses_a_judge_who_authored_the_material`. What is parked is the set's own **edge**. `Pool::qualify_for` refuses a model with `π(a) = ∅`, because every candidate would then pass disjointness vacuously; the mirror on the *principal's* side is unguarded, so a principal declaring `π(p) = ∅` is disjoint from everything and is a universal judge admitted by construction. Het as written admits it. Whether that is a hole or the honest consequence of the definition is a change to **this proposition**, which is why the cited test presumes an answer and is parked rather than run: the engine invented the model-side guard on its own judgment once, and inventing its mirror unasked would be the same overreach twice. | — |
| [3.51](rung-het-props.md#disjointness-against-argument) | `disjointness-against-argument` | `decidable` | G13. Disjointness is measured against the argument, and the token now remembers WHICH argument, so spending it elsewhere is a refusal rather than an unobservable mistake. `dispose` admits a token only against the **proposal**; `settle` only against the **model**. Until the binding landed this proposition was satisfied only by the caller passing the right reference — `qualify_for` was a pure alias for `qualify` and nothing downstream could tell the two apart. | `rung-het/tests/token_binding.rs::settle_refuses_a_token_minted_against_a_different_model` |
| [3.52](rung-het-props.md#argument-governs) | `argument-governs` | `decidable` | G13, at the point where the two readings come apart. A judge that authored a Proposal is disjoint from the MODEL by construction, so a model-relative mint would admit it to rule on its own work; the cited test performs exactly that laundering, with a token minted honestly against the model, and `dispose` refuses it. `Pool::qualify` is now the `audit` reading of `qualify_for`, where π(a) = π(M) — one filter, and which name the caller used is a comment rather than the check. | `rung-het/tests/token_binding.rs::dispose_refuses_a_token_minted_against_the_model` |
| [3.53](rung-het-props.md#non-identity-before-dispatch) | `non-identity-before-dispatch` | `rationale` | The filter is set operations over declared predicates ([10.22](rung-het-props.md#conformance-half-needs-no-judge)), and it runs **before** dispatch because dispatch has no other door: a judgmental transition called without a token is E0061, and the only mint is `Pool::qualify_for`, which refuses before it returns. The cited `trybuild` case is that refusal with its message committed. rung enforces *that the token was constructed*, never that the body computed the set correctly — SPEC §5, transition-body correctness. | — |
| [3.54](rung-het-props.md#non-identity-by-construction) | `non-identity-by-construction` | `decidable` | G12 + G13. The token witnesses the **pair** this proposition names: `Qualified<R>` records the principal AND `π(a)`, the argument disjointness was measured against, and `Qualified::admit` is the one gate that spends it. The seal closes *fabrication* — there is no public constructor, `Pool::qualify_for` is the only mint. The binding closes *transfer* — a licence earned against one argument is refused anywhere else, as `TokenNotBound` from `dispose` and `settle`, and as the macro-injected prologue on a `#[judgmental(R)]` transition, which a body can no more skip than it can skip G8's `must_progress`. Deleting the `admit` call turns the cited test red. NOT enforced: the *returned* value. `π(f(a)) ∩ π(a) = ∅` is a body property and inherits SPEC §5. | `rung-het/tests/token_binding.rs::dispose_refuses_a_token_minted_against_the_model` |
| [3.55](rung-het-props.md#non-identity-not-deferrable) | `non-identity-not-deferrable` | `rationale` | — | — |
| [3.56](rung-het-props.md#no-preference-among-judges) | `no-preference-among-judges` | `decidable` | The set is now **exposed as a set**, and that is what moves this row. `Pool::qualify_for` still walks the pool and returns the first survivor — candidates skipped for failing a *conjunct*, never for being ranked below another (`gate_law.rs::qualification_walks_the_pool_and_takes_any_survivor`) — but a single-survivor API could only ever IMPLY that any other survivor would have done. `rung-std::principals::qualifying_set` returns all of them, and the cited test takes each of the four in turn, mints a licence against the very same argument and settles the very same sentence: four well-formed dispatches, one per member. Truncating the set to its first member is type-valid and turns the test red at the count. The UNARGUED residue is gone with it — pool position cannot constitute an ordering over a value that carries every member. | `rung-std/tests/principals_theory.rs::every_member_of_the_qualifying_set_is_a_well_formed_dispatch` |
| [3.6](rung-het-props.md#authorial-qualifying-set) | `authorial-qualifying-set` | `decidable` | G14. `Pool::authorize::<R>` is the only mint for `Authorized` and checks BOTH conjuncts — `capable(p, role(o))` then `standing(p, M)`. Standing alone mints nothing: the cited test hands it a steward of the container who is capable of nothing and requires `AuthorizeError::NotCapable`. NOT enforced: the outcome condition of [5.41](rung-het-props.md#admissibility-subcategories), `π(f(a)) ⊆ π(p)`, which is a body property and inherits SPEC §5. | `rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one` |
| [3.61](rung-het-props.md#judgment-refuses-authorship-requires) | `judgment-refuses-authorship-requires` | `decidable` | G12 + G14 together, which is the only way this proposition can be shown: it is a claim about two filters, so one filter cannot witness it. The cited test asserts both directions over one subject — a principal that PASSES the judgmental filter (capable, provenance-disjoint) is refused a pen, and the principal that HOLDS the pen is refused as a judge of the very subject it stewards. An authorial gate built as the judgmental gate with its token renamed passes every other gate test and fails this one. | `rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one` |
| [3.62](rung-het-props.md#provenance-overlap-is-the-point) | `provenance-overlap-is-the-point` | `decidable` | G12 + G14, read as the reason the two filters must disagree. The cited test's subject is authored by the principal that stewards its container, so the overlap that disqualifies the curator as a judge is the same fact that makes it the author. Weakening either second conjunct — disjointness in `qualify_for`, standing in `authorize` — turns the test red, because the two assertions are about the same principal and the same subject. | `rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one` |
| [3.63](rung-het-props.md#standing-conditional-gated) | `standing-conditional-gated` | `decidable` | `Pool::classify_standing` + `AuthorizeError::StandingIsJudgmental`. What is enforced is the REFUSAL TO GUESS: where containment does not settle standing, `authorize` returns the judgmental branch as an error rather than minting a pen, and the cited test requires that variant by name. NOT enforced, and not closable here: the branch itself. Closing it needs a judge, terminating at depth one ([3.64](rung-het-props.md#standing-terminates-at-depth-one)) and disjoint from the AUTHOR ([3.65](rung-het-props.md#standing-judge-disjoint-from-author)); rung has no term for that dispatch and inventing a ruling would be worse than surfacing the gap. | `rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one` |
| [3.64](rung-het-props.md#standing-terminates-at-depth-one) | `standing-terminates-at-depth-one` | `rationale` | — | — |
| [3.65](rung-het-props.md#standing-judge-disjoint-from-author) | `standing-judge-disjoint-from-author` | `rationale` | — | — |
| [3.66](rung-het-props.md#two-escalation-triggers) | `two-escalation-triggers` | `rationale` | — | — |
| [3.67](rung-het-props.md#standing-escalation-precedes-valuation) | `standing-escalation-precedes-valuation` | `rationale` | — | — |

### The verdict

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [4](rung-het-props.md#verdict-space-with-metric) | `verdict-space-with-metric` | `signature` | — | — |
| [4.1](rung-het-props.md#judges-are-stochastic) | `judges-are-stochastic` | `rationale` | — | — |
| [4.11](rung-het-props.md#boolean-breaks-satisfaction) | `boolean-breaks-satisfaction` | `judgmental` | — | *awaits a* `category-theorist` |
| [4.2](rung-het-props.md#typical-verdict-spaces) | `typical-verdict-spaces` | `rationale` | — | — |
| [4.3](rung-het-props.md#satisfaction-condition-relaxed) | `satisfaction-condition-relaxed` | `judgmental` | — | *awaits a* `category-theorist` |
| [4.31](rung-het-props.md#drift-within-tolerance) | `drift-within-tolerance` | `rationale` | — | — |
| [4.4](rung-het-props.md#metric-carried-by-verdict-space) | `metric-carried-by-verdict-space` | `rationale` | — | — |
| [4.5](rung-het-props.md#metric-measures-not-ranks) | `metric-measures-not-ranks` | `rationale` | — | — |
| [4.51](rung-het-props.md#order-as-preference-is-hetopts) | `order-as-preference-is-hetopts` | `rationale` | — | — |
| [4.6](rung-het-props.md#epsilon-reported-with-verdict) | `epsilon-reported-with-verdict` | `owed` | GAP — `Verdict` is Boolean (`Conforming | NonConforming`). No metric, no epsilon, so the satisfaction condition does not survive renaming ([4.11](rung-het-props.md#boolean-breaks-satisfaction)). The cited test is the gap as an assertion: two judges settle the same sentence with the same polarity, one barely persuaded and one certain, and the two verdicts are the same object. Deleting the `#[ignore]` reports whether an error bar has reached the caller. | **owed** — the test exists and is #[ignore]d: `Settled` does not yet carry an error bar, so nothing runs |
| [4.7](rung-het-props.md#translation-invariance-is-candidates-burden) | `translation-invariance-is-candidates-burden` | `rationale` | — | — |

### The semantics

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [5](rung-het-props.md#algebra-is-kleisli-functor) | `algebra-is-kleisli-functor` | `judgmental` | — | *awaits a* `category-theorist` |
| [5.1](rung-het-props.md#not-a-set-functor) | `not-a-set-functor` | `judgmental` | — | *awaits a* `category-theorist` |
| [5.11](rung-het-props.md#set-functor-decides-everything) | `set-functor-decides-everything` | `rationale` | — | — |
| [5.12](rung-het-props.md#set-functor-violates-refusal) | `set-functor-violates-refusal` | `rationale` | — | — |
| [5.2](rung-het-props.md#monad-reading) | `monad-reading` | `rationale` | — | — |
| [5.21](rung-het-props.md#unit-is-no-outside) | `unit-is-no-outside` | `rationale` | — | — |
| [5.22](rung-het-props.md#judgmental-is-kleisli-arrow) | `judgmental-is-kleisli-arrow` | `decidable` | `A → 𝒫(B)` is a claim about **shape**, and the shape is exhibited directly: one argument, two qualifying judges, two different and equally well-formed Dispositions. Were `dispose` an `A → B` the second call could not disagree. The non-determinism is the outside itself — [3.56](rung-het-props.md#no-preference-among-judges) forbids Het from ranking the two. A *blocking* outside call works today; `rung-std`'s `LlmCall` ladder puts one on the arrow. Q8 constrains **how** the call is made, not whether the arrow is Kleisli. | `rung-het/tests/panel.rs::a_judgmental_arrow_returns_a_set_and_not_a_value` |
| [5.23](rung-het-props.md#monad-is-what-outside-adds) | `monad-is-what-outside-adds` | `rationale` | — | — |
| [5.24](rung-het-props.md#kleisli-composition-interleaves) | `kleisli-composition-interleaves` | `judgmental` | — | *awaits a* `category-theorist` |
| [5.25](rung-het-props.md#judgmental-arrow-shape) | `judgmental-arrow-shape` | `decidable` | The `+ A` residual is `Failed<Prev> { token, error }` — the unconsumed argument handed back. rung-CT names it the Prism's residual ([residual-is-the-optics-residual](rung-ct-props.md#residual-is-the-optics-residual)) and is why the error structure is not a Kleisli arrow; the monad `P` layers on the forward pass, which rung-CT explicitly permits ([effects-layer-on-the-forward-pass](rung-ct-props.md#effects-layer-on-the-forward-pass)). | `rung/tests/compile_pass.rs::test_failed_type` |
| [5.3](rung-het-props.md#provenance-structure) | `provenance-structure` | `signature` | — | — |
| [5.31](rung-het-props.md#morphisms-preserve-provenance) | `morphisms-preserve-provenance` | `signature` | — | — |
| [5.32](rung-het-props.md#monad-is-provenance-strict) | `monad-is-provenance-strict` | `judgmental` | `carry` is the natural home for provenance: a product factor preserved across every arrow, immutable by G5. It does not carry a *principal's* provenance, which lives outside the ladder. | *awaits a* `category-theorist` |
| [5.4](rung-het-props.md#constant-arrow-hazard) | `constant-arrow-hazard` | `decidable` | G2 sealed construction. A judgmental arrow cannot be interpreted by a constant drawn from the algebra's own carrier, because no mid-ladder rung is constructible outside its module. | `rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624` |
| [5.41](rung-het-props.md#admissibility-subcategories) | `admissibility-subcategories` | `judgmental` | — | *awaits a* `category-theorist` |
| [5.42](rung-het-props.md#judgment-provenance-is-the-judges) | `judgment-provenance-is-the-judges` | `decidable` | — | `rung-het/tests/gate_law.rs::a_settled_receipt_carries_the_judges_provenance` |
| [5.43](rung-het-props.md#authorial-admissibility-stronger) | `authorial-admissibility-stronger` | `rationale` | — | — |
| [5.44](rung-het-props.md#one-monad) | `one-monad` | `judgmental` | — | *awaits a* `category-theorist` |
| [5.45](rung-het-props.md#gate-relative-admissibility-licensed) | `gate-relative-admissibility-licensed` | `rationale` | — | — |
| [5.5](rung-het-props.md#gate-faithful) | `gate-faithful` | `signature` | Q11 (gate-faithfulness, open), with **one** blocker rather than two. Blocker (1) — the returned value — has CLOSED, and it closed by derivation rather than by an epilogue guard on the condition itself. R2 obliges a judgmental outcome to carry its judge's provenance ([5.42](rung-het-props.md#judgment-provenance-is-the-judges)): `Judgment` is sealed, `Principal::judgment` is the only mint and calls the oracle `Principal::rule`, and π(f(a)) ⊆ π(p) is asserted where a `Judgment` is spent — by `theory!`'s `settle` and by G15's injected epilogue. With G13 already enforcing π(p) ∩ π(a) = ∅, [5.41](rung-het-props.md#admissibility-subcategories)'s judgmental clause is a THEOREM of two enforced facts, so nothing calls `Prov::overlaps` on the way out and nothing should. What is left of blocker (1) is narrower and is recorded as such: the authorial outward conjunct and branching judgmental arms ([5.621](rung-props.md#outward-conditions-remaining)). Blocker (2) STANDS and is why this row does not move: `#[conditional(..)]` is a parse-time refusal, gate-faithfulness quantifies over EVERY operation, and an algebra with a conditional operation therefore cannot state this proposition here at all. The cited test is that blocker made runnable — it asks the macro to accept a conditional marker, and deleting its `#[ignore]` reports whether it does. Purity was a third blocker and is CLOSED on received advisory input: η is 𝒫's unit, so "factors through η" IS 𝒫-purity and never claimed absolute purity; that a decidable body may read a clock is [11.42](rung-het-props.md#purity-not-secured), a limit already stated. Argued with its falsifiers at `questions/open/q11-gate-faithfulness.md`. | — |
| [5.51](rung-het-props.md#mod-only-gate-faithful) | `mod-only-gate-faithful` | `rationale` | Follows [5.5](rung-het-props.md#gate-faithful), and parks on the same remaining blocker: `Mod(Σ)` can consist only of gate-faithful algebras once gate-faithfulness is checkable, and it is not checkable for an algebra with a conditional operation, because such an algebra cannot be declared. The outward half that used to park this row has closed — a `theory!` declaration can no longer settle a judgmental sentence with a verdict its judge never gave ([5.42](rung-het-props.md#judgment-provenance-is-the-judges)) — which narrows the row without moving it. | — |
| [5.52](rung-het-props.md#refusal-at-model-category) | `refusal-at-model-category` | `rationale` | — | — |
| [5.53](rung-het-props.md#condition-propagates-by-reindexing) | `condition-propagates-by-reindexing` | `judgmental` | — | *awaits a* `category-theorist` |
| [5.6](rung-het-props.md#subject-defined) | `subject-defined` | `signature` | — | — |
| [5.61](rung-het-props.md#decidable-runs-pure) | `decidable-runs-pure` | `rationale` | — | — |
| [5.62](rung-het-props.md#judgmental-runs-kleisli) | `judgmental-runs-kleisli` | `rationale` | — | — |
| [5.63](rung-het-props.md#self-governing-not-self-closing) | `self-governing-not-self-closing` | `decidable` | G2 sealed construction. This proposition *is* rung's founding refusal: an attempt to fold a live verdict into the next state was rejected by the sealed constructor — [the law](rung-ct-props.md#the-law). The algebra runs its own decidable step; it cannot construct the state that holds a judgmental outcome. | `rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624` |
| [5.64](rung-het-props.md#autopoiesis-made-precise) | `autopoiesis-made-precise` | `rationale` | — | — |

### The tower

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [6](rung-het-props.md#fractal-property) | `fractal-property` | `judgmental` | The composite Grothendieck opfibration ([opfibrations-compose](rung-ct-props.md#opfibrations-compose)), resolved by Q10 (`questions/resolved/`). The correspondence is proved and no hierarchy is built — which leaves the property itself needing a run, and the cited test is one: the pass composed with itself at a container boundary, where the destination's own law is what refuses a write the source's judge already authorized ([7.52](rung-het-props.md#target-runs-its-own-models)). | *awaits a* `category-theorist` |
| [6.1](rung-het-props.md#tower-is-a-fibration) | `tower-is-a-fibration` | `judgmental` | — | *awaits a* `category-theorist` |
| [6.11](rung-het-props.md#same-relation-every-level) | `same-relation-every-level` | `rationale` | — | — |
| [6.12](rung-het-props.md#kleisli-iterates) | `kleisli-iterates` | `judgmental` | — | *awaits a* `category-theorist` |
| [6.13](rung-het-props.md#tower-semantic-every-level) | `tower-semantic-every-level` | `rationale` | — | — |
| [6.14](rung-het-props.md#two-directions-two-bases) | `two-directions-two-bases` | `rationale` | Conformance is Het's fibration (Mod: Sign^op → Cat, contravariant). Propagation is rung-CT's opfibration, pushforward and opcartesian ([conformance-and-propagation-run-over-different-bases](rung-ct-props.md#conformance-and-propagation-run-over-different-bases)). Different bases at adjacent levels — not opposite orientations of one tower. The cited test is where the two are visible at once and are not conflated: the docket's sentences are run *per question* — conformance, each model against its own theory — while drift is reported *along outbound edges*, from a revised question to whatever depended on it. One suite, two directions, and the edge set is the theory's rather than Het's ([11.21](rung-het-props.md#governs-who-not-what)). | — |
| [6.2](rung-het-props.md#two-kinds-of-pointing) | `two-kinds-of-pointing` | `rationale` | — | — |
| [6.21](rung-het-props.md#pointings-are-duals) | `pointings-are-duals` | `judgmental` | — | *awaits a* `category-theorist` |
| [6.22](rung-het-props.md#declaration-on-models-only) | `declaration-on-models-only` | `rationale` | — | — |
| [6.23](rung-het-props.md#model-without-theory-is-empty) | `model-without-theory-is-empty` | `rationale` | — | — |
| [6.24](rung-het-props.md#declaration-is-not-a-morphism) | `declaration-is-not-a-morphism` | `rationale` | — | — |
| [6.25](rung-het-props.md#three-relations-not-conflated) | `three-relations-not-conflated` | `rationale` | — | — |
| [6.3](rung-het-props.md#gate-law) | `gate-law` | `signature` | — | — |
| [6.31](rung-het-props.md#no-laundering-along-morphisms) | `no-laundering-along-morphisms` | `rationale` | — | — |
| [6.4](rung-het-props.md#tower-floor) | `tower-floor` | `signature` | — | — |
| [6.41](rung-het-props.md#wellformedness-clauses) | `wellformedness-clauses` | `signature` | — | — |
| [6.42](rung-het-props.md#clauses-decidable-by-inspection) | `clauses-decidable-by-inspection` | `rationale` | — | — |
| [6.43](rung-het-props.md#floor-not-gate-marked) | `floor-not-gate-marked` | `rationale` | — | — |
| [6.44](rung-het-props.md#w-checks-declaration-not-adequacy) | `w-checks-declaration-not-adequacy` | `rationale` | — | — |
| [6.5](rung-het-props.md#adequacy-defined) | `adequacy-defined` | `decidable` | Adequacy is a CONJUNCTION — a qualifying judge exists AND returns a verdict — and the engine now has a term for each conjunct failing separately. An empty qualifying set is `QualifyError::NotCapable` / `NonIdentityViolated` / `PoolExhausted`; a judge that exists and has not answered is `QualifyError::JudgeDeferred`, which is documented as NOT a filter failure. Before the deferral there was one outcome for both and the second conjunct was unrepresentable, so the definition could not be wrong about anything. The cited test settles both halves against one argument: the deferring pool mints no licence, and an answering pool does. Collapsing `JudgeDeferred` into `PoolExhausted` is type-valid and reddens it at the `Err(other)` arm. | `rung/tests/suspension.rs::the_pool_propagates_a_deferral_and_mints_no_licence` |
| [6.51](rung-het-props.md#adequacy-is-judgmental) | `adequacy-is-judgmental` | `rationale` | — | — |
| [6.52](rung-het-props.md#adequacy-failure-is-not-a-w-defect) | `adequacy-failure-is-not-a-w-defect` | `rationale` | — | — |
| [6.53](rung-het-props.md#adequacy-asks-for-a-judge) | `adequacy-asks-for-a-judge` | `rationale` | — | — |
| [6.54](rung-het-props.md#adequacy-local-not-global) | `adequacy-local-not-global` | `rationale` | — | — |
| [6.55](rung-het-props.md#adequacy-failure-returns-residual) | `adequacy-failure-returns-residual` | `decidable` | G9 error-path recovery, `Failed(R) => R`, explicitly unguarded — a re-entry after an unanswered call may reuse the argument. G4 additionally forbids silently dropping the returned residual. A judgmental FORWARD transition now carries the same residual too ([G16](rung-props.md#g16-the-residual-channel)); the children below are that case. | `rung/tests/end_to_end.rs::recovers_from_the_failed_error_path` |
| [6.551](rung-het-props.md#suspension-is-the-residual) | `suspension-is-the-residual` | `decidable` | G16, and the whole of the claim is that it adds nothing. A judgmental forward transition returns `Result<Next, Suspended<Prev>>`, and the cited test coerces the emitted `fn` to that exact pointer type and then reads the token back out to find the very argument it passed in — unconsumed, as [6.55](rung-het-props.md#adequacy-failure-returns-residual) requires. What is new is the channel, not the summand: before it a forward judgmental transition returned its target rung and a theory whose principal could not answer yet had no term for saying so. Emitting `#to` instead of the `Result` is type-valid at the macro and turns the test red at the coercion. | `rung/tests/suspension.rs::a_judgmental_forward_transition_returns_the_argument_unconsumed` |
| [6.5511](rung-het-props.md#raised-reference-is-opaque) | `raised-reference-is-opaque` | `decidable` | [3.2](rung-het-props.md#pool-is-opaque) reaches the raised matter. `rung::Raised` carries two strings the crate never reads — no ordering, no well-formedness, no roster of live references — and the cited test raises `¶ anything at all §` and gets it back unchanged. `Terminated::of` is the one derived constructor, which is what keeps opacity from becoming laxity: evidence is built FROM a `Raised`, so it cannot name a reference nobody raised. Adding any predicate over the reference to `rung` is type-valid and makes the cited case a refusal instead of a round trip. | `rung/tests/suspension.rs::the_raised_reference_is_carried_and_never_interpreted` |
| [6.5512](rung-het-props.md#deferral-is-not-a-verdict) | `deferral-is-not-a-verdict` | `decidable` | The R2 seal, on the side where the judge is real and the verdict is not. `Principal::judgment` is the only mint for a `Judgment` and it calls `rule`; when `rule` defers there is no verdict, and the sealed `Consulted` says so rather than manufacturing one. There is no `From<Raised> for Judgment` and no `unwrap_or`. The mutation is the direct one: making the deferring branch of `Principal::judgment` build a `Judgment` anyway is type-valid — any verdict at all will do, which is exactly the point — and turns the cited test red at its `Rendered` arm. | `rung/tests/suspension.rs::a_deferral_is_not_a_judgment` |
| [6.5513](rung-het-props.md#no-preference-after-a-deferral) | `no-preference-after-a-deferral` | `decidable` | The pool reports what the principal it selected said, and does not walk on. `Pool::consult` and `Pool::qualify_for` return `QualifyError::JudgeDeferred` carrying the reference; the cited test also shows a pool whose member answers is unaffected, so the deferral is a distinct outcome and not a new way for the FILTER to fail. Looping to the next survivor is type-valid and is a preference among qualifying judges, which [3.56](rung-het-props.md#no-preference-among-judges) forbids; it turns the cited test red at the `JudgeDeferred` arm as soon as the pool holds a second, answering principal. | `rung/tests/suspension.rs::the_pool_propagates_a_deferral_and_mints_no_licence` |
| [6.552](rung-het-props.md#resumption-is-authorial) | `resumption-is-authorial` | `decidable` | G16, and it is FORCED rather than chosen. Reviving a suspended run constructs a rung, which [G2](rung-props.md#g2-sealed-construction) seals from outside the module — so the resume edge is emitted inside it, and an edge inside the seal that anyone may call is the seal with a door in it. The marker is therefore mandatory: a resume edge with no `#[authorial(R)]` is a `compile_error!` (`suspension.rs::a_resume_edge_without_an_authorial_marker_is_refused`), and calling one without its pen is E0061. The cited test drives the round trip and coerces the emitted `fn` to its exact pointer type. Deleting the injected `must_hold_standing_over` from the resume path is type-valid and reddens `::resume_refuses_a_pen_over_another_container`, where the body never mentions the pen at all. | `rung/tests/suspension.rs::a_suspension_resumes_through_the_authorial_edge` |
| [6.5521](rung-het-props.md#resumption-needs-a-terminal) | `resumption-needs-a-terminal` | `decidable` | `must_answer_the_raised`, injected. A `Terminated` is derived from the `Raised` it is about, which closes fabrication; the guard closes TRANSFER, exactly as `must_be_bound_to` does for a licence and `must_hold_standing_over` for a pen. The cited test resumes with evidence about `q-99` and is refused. It asserts nothing about termination and must not: [12](rung-het-props.md#no-bound-on-reentry) stands, and a matter that never terminates yields no evidence and leaves the arrow suspended, visibly. | `rung/tests/suspension.rs::resume_refuses_evidence_from_another_raised_matter` |
| [6.5522](rung-het-props.md#resumption-is-unguarded) | `resumption-is-unguarded` | `decidable` | The ABSENCE, pinned. The macro injects no `must_progress` on a resume edge, and the cited test suspends and resumes the same run twice with a payload that does not change — which is the normal case, not a stall: the argument was never consumed and the raised matter took another round. Injecting `must_progress` there is type-valid and reddens the test on the FIRST round, which is what makes this row an enforcement rather than an observation that nothing happened. A guard would be the bound Het declines to declare ([12.5](rung-het-props.md#guarded-reentry-is-eviction)). | `rung/tests/suspension.rs::the_same_suspension_resumes_twice_with_no_progress_guard` |
| [6.6](rung-het-props.md#self-grounding-is-a-pair) | `self-grounding-is-a-pair` | `rationale` | — | — |
| [6.61](rung-het-props.md#het-self-grounding-condition) | `het-self-grounding-condition` | `rationale` | — | — |
| [6.62](rung-het-props.md#neither-stands-on-itself) | `neither-stands-on-itself` | `rationale` | — | — |
| [6.63](rung-het-props.md#first-question-is-hets-own-signature) | `first-question-is-hets-own-signature` | `rationale` | — | — |
| [6.7](rung-het-props.md#signature-claims-are-w-clauses) | `signature-claims-are-w-clauses` | `rationale` | — | — |
| [6.71](rung-het-props.md#sentence-needs-an-inhabitant) | `sentence-needs-an-inhabitant` | `rationale` | A signature-claim has no carrier inhabitant to test. Nothing for a host to run. | — |
| [6.72](rung-het-props.md#empty-equation-is-a-misfiling) | `empty-equation-is-a-misfiling` | `rationale` | — | — |

### The game

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [7](rung-het-props.md#satisfaction-is-a-game) | `satisfaction-is-a-game` | `judgmental` | — | *awaits a* `category-theorist` |
| [7.1](rung-het-props.md#proponent-and-opponent) | `proponent-and-opponent` | `rationale` | — | — |
| [7.11](rung-het-props.md#decidable-games-are-bounded) | `decidable-games-are-bounded` | `rationale` | — | — |
| [7.12](rung-het-props.md#judgmental-games-have-an-oracle) | `judgmental-games-have-an-oracle` | `rationale` | — | — |
| [7.13](rung-het-props.md#game-resolves-disagreement) | `game-resolves-disagreement` | `rationale` | — | — |
| [7.2](rung-het-props.md#the-pass) | `the-pass` | `decidable` | One `ladder!` declaration, and it is now written: `het_pass!` expands to the spine `Governed => Audited => Proposing => #[authorial(Author)] Proposed => #[judgmental(Judge)] { .. }`. The table's `gate` column is a **marker** and its `acts` column is a **parameter type**, so which principal may move is settled by rustc rather than by a driver keeping to a convention: `propose` without a pen is E0061 and `dispose` without a licence is E0061, each with its message committed as a `trybuild` snapshot. Retargeting the judgmental marker at the author's role — one token, type-valid, the library still compiles — turns the cited test red on `expected Qualified<Editor>, found Qualified<Reader>`. rung proves each move was made by one who qualified, not that the move was wise (SPEC §5). What is NOT in the declaration is `enact`: see [5.8](rung-props.md#a-cycle-through-an-authorial-act-cannot-close). | `rung-het/tests/pass_ladder.rs::the_pass_runs_end_to_end_as_a_ladder` |
| [7.21](rung-het-props.md#propose-is-authorial) | `propose-is-authorial` | `rationale` | — | — |
| [7.22](rung-het-props.md#judgmental-propose-swaps-roles) | `judgmental-propose-swaps-roles` | `rationale` | — | — |
| [7.23](rung-het-props.md#difficulty-is-not-an-outside) | `difficulty-is-not-an-outside` | `rationale` | — | — |
| [7.24](rung-het-props.md#proposal-provenance-is-authors) | `proposal-provenance-is-authors` | `rationale` | — | — |
| [7.3](rung-het-props.md#proposal-vocabulary) | `proposal-vocabulary` | `decidable` | — | `rung-het/tests/acceptance.rs::an_author_may_dispute_a_verdict_without_first_authoring_a_remedy` |
| [7.31](rung-het-props.md#dispute-is-still-judged) | `dispute-is-still-judged` | `rationale` | — | — |
| [7.32](rung-het-props.md#dispute-is-the-only-contest) | `dispute-is-the-only-contest` | `rationale` | — | — |
| [7.33](rung-het-props.md#remedy-carries-an-edit) | `remedy-carries-an-edit` | `decidable` | The edit is the rung payload's type, supplied by the theory, and the requirement is now a *variant shape*: an author answers through `Answer<E>`, whose `Remedy(E)` has nowhere to put the absence of an edit. A theory that let a remedy carry none would make `remedy` and `dispute` indistinguishable, and there is no term for it. Dropping the edit in `Proposal::from_chain` — type-valid, `Remedy` rewritten to `Dispute` — reddens the cited test at `rounds: left 1, right 2` (the judge has nothing to reject, so the loop it exists to exercise never runs) and `acceptance.rs::the_pass_runs_end_to_end_as_a_chain_of_principals` with it. The boundary itself is pinned by `acceptance.rs::an_author_may_dispute_a_verdict_without_first_authoring_a_remedy`: a dispute's `edit()` is `None`. | `rung-het/tests/pass_ladder.rs::the_pass_runs_end_to_end_as_a_ladder` |
| [7.4](rung-het-props.md#disposition-vocabulary) | `disposition-vocabulary` | `decidable` | G6 exhaustive outcomes. `StepOutcome` is an enum, so every match site must handle all five; adding a disposition breaks every call site at compile time. The cited test pins the vocabulary itself — the five, in order, each with its terminal and affirming flags — so that the two that Het's gate boundary excludes (`accept-with-mod`, `reject-with-alternative`) cannot return without the assertion changing. | `rung-het/tests/acceptance.rs::the_disposition_vocabulary_is_exactly_the_five_that_survive_the_gate` |
| [7.41](rung-het-props.md#disposition-is-a-ruling) | `disposition-is-a-ruling` | `decidable` | G2. `dispose` returns a verdict; only the separately-declared authorial arrow produces the revised object. A ruling cannot construct what it rules on. | `rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624` |
| [7.42](rung-het-props.md#no-amending-disposition) | `no-amending-disposition` | `decidable` | G2 plus G10, and the second half is what the pass added. A judge's arrow has no constructor for the authored object — but a continue arm's target rung is built INLINE by `step`, i.e. by the judge, so the pass's re-entry rung is the one place an amendment could have arrived. Its payload is therefore `Chain`: a concrete, non-generic record of an id, a container, a count and prose, with no edit and no type parameter one could hide in. The cited `trybuild` case pins the E0599 that reading an edit off it produces; giving `Chain` an `edit` accessor — type-valid, the library still compiles — turns it red on a diff. | `rung-het/tests/pass_ladder.rs::a_chain_cannot_be_read_for_an_edit` |
| [7.43](rung-het-props.md#reason-is-not-an-edit) | `reason-is-not-an-edit` | `decidable` | — | `rung-het/tests/acceptance.rs::reject_remedy_is_non_terminal_and_the_reason_reaches_the_author` |
| [7.44](rung-het-props.md#reproposal-carries-the-chain) | `reproposal-carries-the-chain` | `decidable` | The chain rides in the rung payload and there is no other route to a re-proposal: the pass's authorial transition builds its Proposal from the `Chain` the continue arm handed back, so an author cannot drop it by omission. The cited test rejects the identical remedy five times and reads all five reasons off the sixth chain. Deleting the push in `Chain::reentered` — type-valid — turns it red at `left: 0, right: 5`, and `acceptance.rs::reject_remedy_is_non_terminal_and_the_reason_reaches_the_author` with it. Without the chain an author can cycle on one objection and nothing downstream can tell. NOTE: this is exactly what would make a G8 progress guard vacuous — a strictly growing chain never compares equal — which is why re-entry must not use a guarded edge ([12.5](rung-het-props.md#guarded-reentry-is-eviction)). | `rung-het/tests/pass_ladder.rs::reject_remedy_re_enters_with_no_progress_guard` |
| [7.5](rung-het-props.md#enact-makes-an-endofunctor) | `enact-makes-an-endofunctor` | `judgmental` | The loop closes, and it closes by COMPOSITION rather than inside the declaration — which is the honest reading and is now recorded as a non-guarantee ([5.8](rung-props.md#a-cycle-through-an-authorial-act-cannot-close)). `ladder!` declares a linear spine with backward continue arms, and a continue arm's target is built inline by `step`, so an `Accept -> Governed` arm would have the judge apply the edit ([7.42](rung-het-props.md#no-amending-disposition)). `Accept` is therefore terminal and carries a `Licence`; `enact` is a separate authorial arrow consuming that licence and a pen, and what comes out is audited again. The cited test closes the loop that way: the relocated specimen lands in the fieldbook and the fieldbook's own decidable sentence is run over the result. STILL `expressible`, and the reason is not shyness — no single `ladder!` declaration is an endofunctor, and saying otherwise would be a claim no mutation could falsify. Declaring the composite is Q4, open. rung enforces that the edit ran, not that it was right (SPEC §5), and the edit itself is the theory's ([11.12](rung-het-props.md#edit-required-not-typed)). | *awaits a* `category-theorist` |
| [7.51](rung-het-props.md#licence-is-not-guarantee) | `licence-is-not-guarantee` | `decidable` | A `Licence<E>` is now a type, minted only from an affirming `Ruling` and consumed by `enact` — so the pass's `Accept` arm carries PERMISSION rather than a revised subject. Permission is all it is: `enact` still checks the pen against `Applies::territory` and hands the domain's own refusal back untouched. Making `enact` swallow `Applies::apply`'s error — type-valid, `world.apply(..)?` to `let _ = world.apply(..)` — turns the cited test red where it requires the fieldbook to refuse a write the cabinet's judge already accepted. The two failure points are [7.53](rung-het-props.md#enact-has-two-failure-points). | `rung-het/tests/acceptance.rs::the_pass_runs_end_to_end_as_a_chain_of_principals` |
| [7.52](rung-het-props.md#target-runs-its-own-models) | `target-runs-its-own-models` | `decidable` | The write-guard exists and fires. `enact` checks the pen against `Applies::territory` and hands `EnactError::TargetRefused` back untouched, so a destination may decline a write its own judge already authorized: in the cited test the relocation is accepted by a qualified judge, refused by the fieldbook for want of a locality, and the source container is left unchanged. The target's law is the **theory's** — the library cannot know what admits a specimen — so rung secures the seam and the standing, not the law. `second_domain.rs::a_pen_for_one_territory_does_not_authorize_another` pins the standing half. What stays with Q4 is expressing the composite as a ladder inside a ladder; the boundary itself no longer waits on it. | `rung-het/tests/questions_of_rung.rs::resolved_runs_its_own_law_on_a_write_the_ruling_already_authorized` |
| [7.53](rung-het-props.md#enact-has-two-failure-points) | `enact-has-two-failure-points` | `rationale` | — | — |
| [7.6](rung-het-props.md#panels) | `panels` | `decidable` | A panel is `⊨` with more than one judge, and the proposition says it is **not a separate construction** — so the encoding must not add one. It does not: a seat is a pool of one principal, each seat mints its own licence against the very same argument, and the cited test convenes three of them with nothing `rung-het` does not already export. The combination rule is the theory's, exactly as its edits are ([11.12](rung-het-props.md#edit-required-not-typed)); putting a `panel()` primitive in the library would legislate a rule Het does not have. What stays with Q5 is running the seats **at the same time** — latency, which is HetOpt's ([cut-at-valuation](rung-het-props.md#cut-at-valuation)), not Het's. | `rung-het/tests/panel.rs::a_panel_is_the_pass_with_more_than_one_judge` |
| [7.61](rung-het-props.md#panels-cannot-weaken-the-opponent) | `panels-cannot-weaken-the-opponent` | `decidable` | The observable form of the claim: the same Proponent move, the same first oracle answer, plus two more — and the seat that played in the original game answers identically in the composite. Added answers may take affirmation away and never grant it, so the Proponent's winning set under the panel is contained in its winning set against any single seat. rung proves the rulings were reached through qualified licences, not that unanimity is the right combination rule ([7.6](rung-het-props.md#panels)). | `rung-het/tests/panel.rs::a_panel_cannot_weaken_the_opponent` |

### The cut

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [8](rung-het-props.md#het-settles-hetopt-orders) | `het-settles-hetopt-orders` | `rationale` | — | — |
| [8.1](rung-het-props.md#metric-and-preference-same-furniture) | `metric-and-preference-same-furniture` | `judgmental` | — | *awaits a* `category-theorist` |
| [8.2](rung-het-props.md#cut-at-valuation) | `cut-at-valuation` | `rationale` | — | — |
| [8.21](rung-het-props.md#het-declares-no-worth-law) | `het-declares-no-worth-law` | `decidable` | **The α cut, given teeth.** This row and [3.31](rung-het-props.md#ordering-is-hetopts) were `out-of-scope` by default and never inspected — correctly, while nothing in the workspace declared a cost tier or an ε for a worth-law to be built out of. `rung-std::principals` declares both, so the refusal is now a property a run can check: the cited test reads every line of Rust in all four crates that names a cost tier or an ε and fails on any that also sorts, compares, ranks or takes an extremum. It reads attribute lines above a hit as well, so `#[derive(.., Ord)]` on `CostTier` is caught although the derive names no cost of its own — that derive is the cheapest possible crossing of the cut and it is the mutation this test exists to see. `CostTier` and `Epsilon` independently carry no `Ord`, no `PartialOrd` and no accessor, so the minimal-judge rule of [8.22](rung-het-props.md#v-applies-to-conforming-sets) has neither a comparison nor a value to read. | `rung-std/tests/principals_theory.rs::nothing_in_the_workspace_orders_by_cost_or_epsilon` |
| [8.22](rung-het-props.md#v-applies-to-conforming-sets) | `v-applies-to-conforming-sets` | `rationale` | — | — |
| [8.23](rung-het-props.md#valuation-instantiated-twice) | `valuation-instantiated-twice` | `judgmental` | — | *awaits a* `category-theorist` |
| [8.3](rung-het-props.md#filter-then-optimize) | `filter-then-optimize` | `rationale` | — | — |
| [8.31](rung-het-props.md#cut-lands-no-later) | `cut-lands-no-later` | `rationale` | — | — |
| [8.32](rung-het-props.md#cut-lands-no-earlier) | `cut-lands-no-earlier` | `rationale` | — | — |
| [8.4](rung-het-props.md#hetopt-is-a-theory-extension) | `hetopt-is-a-theory-extension` | `rationale` | — | — |
| [8.41](rung-het-props.md#enrichment-base-is-the-metric) | `enrichment-base-is-the-metric` | `judgmental` | — | *awaits a* `category-theorist` |

### Composition

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [9](rung-het-props.md#composition-is-closed) | `composition-is-closed` | `rationale` | — | — |
| [9.1](rung-het-props.md#composite-monad) | `composite-monad` | `judgmental` | — | *awaits a* `category-theorist` |
| [9.11](rung-het-props.md#non-identity-extends-to-composite) | `non-identity-extends-to-composite` | `judgmental` | — | *awaits a* `category-theorist` |
| [9.12](rung-het-props.md#composite-qualifying-set) | `composite-qualifying-set` | `rationale` | — | — |
| [9.2](rung-het-props.md#composite-kinds) | `composite-kinds` | `rationale` | — | — |
| [9.3](rung-het-props.md#adequacy-composes) | `adequacy-composes` | `judgmental` | — | *awaits a* `category-theorist` |
| [9.4](rung-het-props.md#theory-combination-closed) | `theory-combination-closed` | `rationale` | — | — |

### Evaluation

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [10](rung-het-props.md#models-defined-by-dispatch) | `models-defined-by-dispatch` | `signature` | — | — |
| [10.1](rung-het-props.md#run-over-every-sentence) | `run-over-every-sentence` | `rationale` | — | — |
| [10.2](rung-het-props.md#dispatch-is-two-operations) | `dispatch-is-two-operations` | `decidable` | — | `rung-het/tests/gate_law.rs::competence_is_filtered_before_provenance_matters` |
| [10.21](rung-het-props.md#dispatch-argument-is-the-argument) | `dispatch-argument-is-the-argument` | `rationale` | — | — |
| [10.22](rung-het-props.md#conformance-half-needs-no-judge) | `conformance-half-needs-no-judge` | `rationale` | — | — |
| [10.23](rung-het-props.md#any-is-specified-argmin-is-the-seam) | `any-is-specified-argmin-is-the-seam` | `rationale` | — | — |

### The surface

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [11](rung-het-props.md#theory-declares-four-things) | `theory-declares-four-things` | `signature` | — | — |
| [11.1](rung-het-props.md#het-declares-the-slots) | `het-declares-the-slots` | `rationale` | — | — |
| [11.11](rung-het-props.md#role-declared-not-enumerated) | `role-declared-not-enumerated` | `rationale` | — | — |
| [11.12](rung-het-props.md#edit-required-not-typed) | `edit-required-not-typed` | `rationale` | — | — |
| [11.13](rung-het-props.md#verdict-space-required-not-fixed) | `verdict-space-required-not-fixed` | `rationale` | — | — |
| [11.14](rung-het-props.md#interface-required-not-populated) | `interface-required-not-populated` | `rationale` | — | — |
| [11.2](rung-het-props.md#enact-generic-over-edit) | `enact-generic-over-edit` | `rationale` | — | — |
| [11.21](rung-het-props.md#governs-who-not-what) | `governs-who-not-what` | `rationale` | — | — |
| [11.3](rung-het-props.md#decidable-is-a-total-predicate) | `decidable-is-a-total-predicate` | `signature` | — | — |
| [11.31](rung-het-props.md#two-signatures-not-two-fragments) | `two-signatures-not-two-fragments` | `decidable` | `ladder!` gate markers, now three signatures rather than two. Unmarked emits `fn t(prev)`; `#[judgmental(R)]` emits `fn t(prev, q: Qualified<R>)`; `#[authorial(R)]` emits `fn t(prev, pen: Authorized<'_, R>)`. The gates differ in the ARITY and the TYPE of the emitted transition, so a pen cannot be passed where a licence is asked for or the reverse, and the host's type system separates all three with no knowledge of Het. | `rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen` |
| [11.32](rung-het-props.md#decidable-cannot-consult-pool) | `decidable-cannot-consult-pool` | `decidable` | G2. The qualifying token has no constructor reachable from a decidable body, so the prohibition is a term that cannot be written rather than a rule an author is asked to respect. An unmarked transition has no parameter a token could enter through, and `Qualified` is sealed: constructing one outside `Pool::qualify` is E0451. | `rung/tests/gate_markers.rs::a_qualified_token_cannot_be_constructed_outside_the_pool` |
| [11.33](rung-het-props.md#mismarking-is-not-a-false-claim) | `mismarking-is-not-a-false-claim` | `decidable` | rustc. Marking a transition judgmental gives it the judgmental signature; calling it as though it were decidable is E0061, not a promise someone broke. | `rung/tests/gate_markers.rs::calling_a_judgmental_transition_without_a_token_is_e0061` |
| [11.34](rung-het-props.md#signature-replaces-fragment-membership) | `signature-replaces-fragment-membership` | `decidable` | rustc. The refusal is an arity error from a compiler that does not know Het exists and cannot be persuaded — which is the whole claim of this proposition. | `rung/tests/gate_markers.rs::calling_a_judgmental_transition_without_a_token_is_e0061` |
| [11.4](rung-het-props.md#two-properties-not-secured) | `two-properties-not-secured` | `rationale` | — | — |
| [11.41](rung-het-props.md#termination-not-secured) | `termination-not-secured` | `rationale` | Matches SPEC §5 exactly — 'liveness beyond the guard'. Het and rung state the same limit independently. | — |
| [11.42](rung-het-props.md#purity-not-secured) | `purity-not-secured` | `rationale` | rung has no effect system; a decidable body may still reach the world. Het already states this as a limit rather than a guarantee. | — |
| [11.43](rung-het-props.md#neither-limit-closed-here) | `neither-limit-closed-here` | `rationale` | — | — |

### Vocabulary

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [12](rung-het-props.md#no-bound-on-reentry) | `no-bound-on-reentry` | `decidable` | A continue arm loops with no host-imposed bound, and the pass now runs that loop: the cited test drives five identical rounds — the same edit answered by the same reason — and nothing panics, nothing evicts, and the subject is still in the loop at attempt six. Choosing a guarded edge instead would supply a bound Het declines to declare ([12.5](rung-het-props.md#guarded-reentry-is-eviction)); so would giving up quietly after three tries, which is the mutation that proves the test can fail — `assert!(chain.attempt() <= 3)` in the re-entry arm reddens it on the fourth round. `acceptance.rs::het_places_no_bound_on_re_entry` additionally pins `Disposition::REENTRY_BOUND` as `None`. Either answer would be a worth-law smuggled in under another name ([8.2](rung-het-props.md#cut-at-valuation)). | `rung-het/tests/pass_ladder.rs::reject_remedy_re_enters_with_no_progress_guard` |
| [12.1](rung-het-props.md#reentry-never-terminates) | `reentry-never-terminates` | `rationale` | — | — |
| [12.2](rung-het-props.md#answers-are-worth-shaped) | `answers-are-worth-shaped` | `rationale` | — | — |
| [12.3](rung-het-props.md#bound-belongs-to-hetopt) | `bound-belongs-to-hetopt` | `rationale` | — | — |
| [12.4](rung-het-props.md#stated-as-limit-not-closed) | `stated-as-limit-not-closed` | `rationale` | — | — |
| [12.5](rung-het-props.md#guarded-reentry-is-eviction) | `guarded-reentry-is-eviction` | `decidable` | G10 continue arms — 'no recover function, no guard, no source'. Re-entry must be `RejectRemedy -> Proposing`, never `RejectRemedy => Proposing`: the recoverable-verdict form injects G8's `must_progress`, which panics on no progress and is therefore an eviction rule ([12.2](rung-het-props.md#answers-are-worth-shaped)). CONSTRAINT: a continue arm's target rung is built inline by `dispose`, i.e. by the judge, so that rung's payload must be classification-only ([7.42](rung-het-props.md#no-amending-disposition)). The resume edge is the SECOND unguarded re-entry and is pinned separately by [6.5522](rung-het-props.md#resumption-is-unguarded): injecting `must_progress` there is type-valid and reddens the double-resume test on the first round. | `rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn` |

---

## `rung-ct-props.md`

**Counts.** 18 decidable · 22 judgmental · 36 rationale · 32 signature · 108 total.

### The category

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [1](rung-ct-props.md#ladder-declares-a-category) | `ladder-declares-a-category` | `signature` | — | — |
| [1.1](rung-ct-props.md#rungs-are-objects) | `rungs-are-objects` | `signature` | — | — |
| [1.2](rung-ct-props.md#transitions-are-morphisms) | `transitions-are-morphisms` | `signature` | — | — |
| [1.3](rung-ct-props.md#the-law) | `the-law` | `signature` | — | — |
| [1.31](rung-ct-props.md#verb-in-object-position-refused) | `verb-in-object-position-refused` | `rationale` | — | — |
| [1.32](rung-ct-props.md#sealing-is-the-axiom-not-a-guard) | `sealing-is-the-axiom-not-a-guard` | `decidable` | — | `rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624` |
| [1.33](rung-ct-props.md#law-is-the-second-axis-of-one-refusal) | `law-is-the-second-axis-of-one-refusal` | `rationale` | — | — |
| [1.4](rung-ct-props.md#category-is-freely-generated) | `category-is-freely-generated` | `signature` | — | — |
| [1.41](rung-ct-props.md#freeness-enforced-only-with-bodies) | `freeness-enforced-only-with-bodies` | `rationale` | — | — |
| [1.411](rung-ct-props.md#entry-constructor-is-public) | `entry-constructor-is-public` | `signature` | — | — |
| [1.412](rung-ct-props.md#module-boundary-is-the-limit) | `module-boundary-is-the-limit` | `rationale` | — | — |
| [1.5](rung-ct-props.md#well-typed-program-is-a-functor) | `well-typed-program-is-a-functor` | `signature` | — | — |
| [1.6](rung-ct-props.md#composition-consumes) | `composition-consumes` | `decidable` | — | `rung/tests/spec_refusals.rs::using_a_rung_after_a_transition_consumed_it_is_e0382` |
| [1.61](rung-ct-props.md#intermediate-survives-only-as-a-record) | `intermediate-survives-only-as-a-record` | `rationale` | — | — |

### Branching is a coproduct

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [2](rung-ct-props.md#branching-is-a-coproduct) | `branching-is-a-coproduct` | `signature` | — | — |
| [2.1](rung-ct-props.md#shape-of-the-branching-transition) | `shape-of-the-branching-transition` | `signature` | — | — |
| [2.11](rung-ct-props.md#injections-point-into-the-coproduct) | `injections-point-into-the-coproduct` | `signature` | — | — |
| [2.2](rung-ct-props.md#coproduct-is-heterogeneous) | `coproduct-is-heterogeneous` | `signature` | — | — |
| [2.21](rung-ct-props.md#verdict-summand) | `verdict-summand` | `signature` | — | — |
| [2.22](rung-ct-props.md#continue-summand-carries-an-object) | `continue-summand-carries-an-object` | `signature` | — | — |
| [2.23](rung-ct-props.md#residual-summand) | `residual-summand` | `decidable` | The `+ A` is emitted. A judgmental forward transition returns `Result<Next, Suspended<Prev>>` and the `Suspended` carries the INPUT OBJECT unconsumed, which is what this proposition says the summand is — the cited test reads the very argument back out of it. This row was `out-of-scope` while the residual existed only as `Failed`'s error string, which carries no object the caller handed in and no identity for what went unanswered. Emitting `#to` instead of the `Result` is type-valid at the macro and turns the cited test red at its `fn`-pointer coercion. | `rung/tests/suspension.rs::a_judgmental_forward_transition_returns_the_argument_unconsumed` |
| [2.3](rung-ct-props.md#elimination-is-exhaustive) | `elimination-is-exhaustive` | `decidable` | — | `rung/tests/spec_refusals.rs::a_match_missing_a_step_outcome_summand_is_e0004` |
| [2.31](rung-ct-props.md#adding-a-summand-breaks-every-eliminator) | `adding-a-summand-breaks-every-eliminator` | `rationale` | — | — |
| [2.32](rung-ct-props.md#closed-vocabularies-rest-on-this) | `closed-vocabularies-rest-on-this` | `rationale` | — | — |
| [2.4](rung-ct-props.md#continue-arm-is-an-ordinary-generating-morphism) | `continue-arm-is-an-ordinary-generating-morphism` | `signature` | — | — |
| [2.41](rung-ct-props.md#continue-arm-needs-no-backward-edge) | `continue-arm-needs-no-backward-edge` | `rationale` | — | — |
| [2.42](rung-ct-props.md#continue-arm-has-no-verdict-object) | `continue-arm-has-no-verdict-object` | `decidable` | — | `rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn` |
| [2.43](rung-ct-props.md#two-arms-two-readings) | `two-arms-two-readings` | `rationale` | — | — |

### Carry is a product factor

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [3](rung-ct-props.md#carry-is-a-product-factor) | `carry-is-a-product-factor` | `signature` | — | — |
| [3.1](rung-ct-props.md#projection-onto-carry) | `projection-onto-carry` | `decidable` | — | `rung/tests/compile_pass.rs::test_carry_accessor_exists` |
| [3.2](rung-ct-props.md#carry-factor-is-unrestricted) | `carry-factor-is-unrestricted` | `signature` | — | — |
| [3.3](rung-ct-props.md#carry-is-copied-per-object) | `carry-is-copied-per-object` | `signature` | — | — |
| [3.31](rung-ct-props.md#copying-is-what-makes-it-cartesian) | `copying-is-what-makes-it-cartesian` | `judgmental` | — | *awaits a* `category-theorist` |
| [3.4](rung-ct-props.md#carry-is-a-comonadic-context) | `carry-is-a-comonadic-context` | `judgmental` | — | *awaits a* `category-theorist` |
| [3.41](rung-ct-props.md#constancy-is-not-enforced) | `constancy-is-not-enforced` | `rationale` | — | — |

### The ladder is an indexed monad

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [4](rung-ct-props.md#ladder-is-an-indexed-monad) | `ladder-is-an-indexed-monad` | `judgmental` | — | *awaits a* `category-theorist` |
| [4.1](rung-ct-props.md#index-alignment-is-composition) | `index-alignment-is-composition` | `signature` | — | — |
| [4.2](rung-ct-props.md#unrepresentable-paths) | `unrepresentable-paths` | `signature` | — | — |
| [4.3](rung-ct-props.md#monad-laws-hold-by-construction) | `monad-laws-hold-by-construction` | `judgmental` | — | *awaits a* `category-theorist` |
| [4.4](rung-ct-props.md#indexed-monad-is-a-reading) | `indexed-monad-is-a-reading` | `rationale` | — | — |

### The trace is a writer monad

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [5](rung-ct-props.md#trace-is-a-writer-monad) | `trace-is-a-writer-monad` | `judgmental` | — | *awaits a* `category-theorist` |
| [5.1](rung-ct-props.md#trace-is-a-free-monoid) | `trace-is-a-free-monoid` | `judgmental` | — | *awaits a* `category-theorist` |
| [5.2](rung-ct-props.md#graded-writer) | `graded-writer` | `judgmental` | — | *awaits a* `category-theorist` |
| [5.3](rung-ct-props.md#trace-is-the-proof-term) | `trace-is-the-proof-term` | `rationale` | — | — |
| [5.4](rung-ct-props.md#trace-is-not-emitted) | `trace-is-not-emitted` | `rationale` | — | — |
| [5.5](rung-ct-props.md#trace-is-not-authorship-provenance) | `trace-is-not-authorship-provenance` | `rationale` | — | — |

### A transition is a Prism

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [6](rung-ct-props.md#transition-is-a-prism) | `transition-is-a-prism` | `judgmental` | — | *awaits a* `category-theorist` |
| [6.1](rung-ct-props.md#match-is-the-forward-pass) | `match-is-the-forward-pass` | `signature` | — | — |
| [6.2](rung-ct-props.md#build-is-the-backward-pass) | `build-is-the-backward-pass` | `signature` | — | — |
| [6.3](rung-ct-props.md#residual-is-the-optics-residual) | `residual-is-the-optics-residual` | `signature` | — | — |
| [6.4](rung-ct-props.md#not-a-monad) | `not-a-monad` | `judgmental` | — | *awaits a* `category-theorist` |
| [6.5](rung-ct-props.md#effects-layer-on-the-forward-pass) | `effects-layer-on-the-forward-pass` | `rationale` | — | — |
| [6.51](rung-ct-props.md#strength-carries-linearity) | `strength-carries-linearity` | `judgmental` | — | *awaits a* `category-theorist` |
| [6.52](rung-ct-props.md#error-and-effect-are-orthogonal) | `error-and-effect-are-orthogonal` | `rationale` | — | — |
| [6.53](rung-ct-props.md#generative-body-is-a-kernel) | `generative-body-is-a-kernel` | `judgmental` | — | *awaits a* `category-theorist` |

### The dagger is partial and contractive

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [7](rung-ct-props.md#the-dagger-is-partial-and-contractive) | `the-dagger-is-partial-and-contractive` | `judgmental` | — | *awaits a* `category-theorist` |
| [7.1](rung-ct-props.md#three-shapes-of-loop-back) | `three-shapes-of-loop-back` | `decidable` | — | `rung/tests/end_to_end.rs::recover_guard_is_auto_injected` |
| [7.11](rung-ct-props.md#only-a-departure-can-be-a-return) | `only-a-departure-can-be-a-return` | `rationale` | — | — |
| [7.2](rung-ct-props.md#verdict-dagger-is-mandatory) | `verdict-dagger-is-mandatory` | `decidable` | — | `rung/tests/spec_refusals.rs::a_recoverable_verdict_without_a_recover_edge_is_refused` |
| [7.3](rung-ct-props.md#verdict-dagger-is-contractive) | `verdict-dagger-is-contractive` | `decidable` | — | `rung/tests/end_to_end.rs::recover_guard_is_auto_injected` |
| [7.31](rung-ct-props.md#contraction-is-on-the-payload) | `contraction-is-on-the-payload` | `signature` | — | — |
| [7.32](rung-ct-props.md#well-foundedness-over-symmetry) | `well-foundedness-over-symmetry` | `rationale` | — | — |
| [7.33](rung-ct-props.md#contraction-is-a-runtime-guard) | `contraction-is-a-runtime-guard` | `rationale` | — | — |
| [7.4](rung-ct-props.md#error-dagger-is-optional-and-unguarded) | `error-dagger-is-optional-and-unguarded` | `decidable` | — | `rung/tests/end_to_end.rs::recovers_from_the_failed_error_path` |
| [7.41](rung-ct-props.md#resume-edge-is-the-residual-dagger) | `resume-edge-is-the-residual-dagger` | `decidable` | [G16](rung-props.md#g16-the-residual-channel). The residual's adjoint, declared: `resume { revive: #[authorial(R)] Suspended(Rung) => Rung }`. It inherits both halves of [7.4](rung-ct-props.md#error-dagger-is-optional-and-unguarded) — OPTIONAL, because a driver may hold a `Suspended` and never resume, and UNGUARDED, which `suspension.rs::the_same_suspension_resumes_twice_with_no_progress_guard` pins by resuming an unchanged payload twice. What it does not inherit is freedom of WHO may take it, and that is a condition on the principal rather than on the arrow, so [7.1](rung-ct-props.md#three-shapes-of-loop-back) is undisturbed: still three shapes, with the second one's adjoint now written down. | `rung/tests/suspension.rs::a_suspension_resumes_through_the_authorial_edge` |
| [7.5](rung-ct-props.md#terminal-verdicts-have-no-adjoint) | `terminal-verdicts-have-no-adjoint` | `signature` | — | — |
| [7.6](rung-ct-props.md#dagger-laws-are-not-verified) | `dagger-laws-are-not-verified` | `rationale` | — | — |

### The substrate is affine

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [8](rung-ct-props.md#substrate-is-affine) | `substrate-is-affine` | `judgmental` | — | *awaits a* `category-theorist` |
| [8.1](rung-ct-props.md#linear-logic-dictionary) | `linear-logic-dictionary` | `signature` | — | — |
| [8.2](rung-ct-props.md#at-most-once-not-exactly-once) | `at-most-once-not-exactly-once` | `signature` | — | — |
| [8.21](rung-ct-props.md#must-use-is-the-affine-approximation) | `must-use-is-the-affine-approximation` | `decidable` | — | `rung/tests/spec_refusals.rs::dropping_a_verdict_under_deny_must_use_is_an_error` |
| [8.22](rung-ct-props.md#lint-is-escapable) | `lint-is-escapable` | `rationale` | — | — |
| [8.3](rung-ct-props.md#one-token-one-thread) | `one-token-one-thread` | `decidable` | — | `rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync` |
| [8.31](rung-ct-props.md#move-semantics-alone-are-insufficient) | `move-semantics-alone-are-insufficient` | `decidable` | — | `rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync` |
| [8.4](rung-ct-props.md#true-linearity-needs-the-language) | `true-linearity-needs-the-language` | `rationale` | — | — |

### Types are propositions

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [9](rung-ct-props.md#types-are-propositions) | `types-are-propositions` | `judgmental` | — | *awaits a* `category-theorist` |
| [9.1](rung-ct-props.md#object-asserts-its-history) | `object-asserts-its-history` | `signature` | — | — |
| [9.2](rung-ct-props.md#residual-is-a-conjunction) | `residual-is-a-conjunction` | `signature` | — | — |
| [9.3](rung-ct-props.md#terminal-payload-is-the-witness) | `terminal-payload-is-the-witness` | `decidable` | — | `rung/tests/end_to_end.rs::drives_to_convergence` |
| [9.4](rung-ct-props.md#proof-is-of-traversal-not-correctness) | `proof-is-of-traversal-not-correctness` | `rationale` | — | — |

### The verification boundary

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [10](rung-ct-props.md#verification-boundary) | `verification-boundary` | `rationale` | — | — |
| [10.1](rung-ct-props.md#guarantees-carry-categorical-content) | `guarantees-carry-categorical-content` | `rationale` | — | — |
| [10.2](rung-ct-props.md#what-is-not-verified) | `what-is-not-verified` | `rationale` | — | — |
| [10.21](rung-ct-props.md#gate-guarantees-constrain-the-domain-not-the-arrow) | `gate-guarantees-constrain-the-domain-not-the-arrow` | `decidable` | — | `rung/tests/gate_markers.rs::judgmental_transition_takes_a_qualified_token` |
| [10.3](rung-ct-props.md#boundary-is-typestate-not-verification) | `boundary-is-typestate-not-verification` | `rationale` | — | — |

### The dependency structure is an opfibration

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [11](rung-ct-props.md#dependency-structure-is-an-opfibration) | `dependency-structure-is-an-opfibration` | `judgmental` | — | *awaits a* `category-theorist` |
| [11.1](rung-ct-props.md#fibre-is-a-ladder) | `fibre-is-a-ladder` | `signature` | — | — |
| [11.11](rung-ct-props.md#declaration-names-no-foreign-object) | `declaration-names-no-foreign-object` | `signature` | — | — |
| [11.2](rung-ct-props.md#typed-edge-is-an-opcartesian-lift) | `typed-edge-is-an-opcartesian-lift` | `judgmental` | — | *awaits a* `category-theorist` |
| [11.21](rung-ct-props.md#orientation-is-load-bearing) | `orientation-is-load-bearing` | `rationale` | — | — |
| [11.22](rung-ct-props.md#edge-type-selects-the-pushforward) | `edge-type-selects-the-pushforward` | `signature` | — | — |
| [11.221](rung-ct-props.md#edge-taxonomy-is-the-theorys) | `edge-taxonomy-is-the-theorys` | `decidable` | The edge vocabulary is declared by the governing theory, not by the formalism — neither `rung` nor `rung-het` has an edge type, and `EdgeKind` lives in `rung-std::questions`, the theory that governs bodies of questions, exactly where an edit vocabulary sits (`edit-required-not-typed`). Moving the theory out of a test and into a library sharpened the row without changing its verdict: the taxonomy is now demonstrably neither the formalism's NOR one carrier's, because two carriers with disjoint id spaces and disjoint edge sets fill the same seven kinds — rung's `questions/` and a synthetic decision docket. What the cited test pins is the **lived-instance** discipline, now a decidable sentence of the theory (`every_declared_kind_is_lived`) rather than prose: a kind stays in the vocabulary only while some question in the set under audit uses it, and deleting the sentence turns a test red in BOTH carriers. STILL NOT enforced, and the reason is unchanged: what would have to fail is a crate BELOW the theory naming an edge type, and no test can fail for code that was never written. The location is a choice this theory makes; the test protects the discipline, not the choice. | `rung-het/tests/questions_of_rung.rs::every_declared_edge_kind_has_a_lived_instance_on_disk` |
| [11.222](rung-ct-props.md#strict-and-advisory-are-the-gate) | `strict-and-advisory-are-the-gate` | `decidable` | G12 + G2, read at the dependency level. `premise` routes to a `decidable` sentence whose `holds` takes only the model — there is no parameter a pool could enter through — and `justification` routes to a `judgmental` one whose `settle` consumes a `Qualified<Adjudicator>` that only `Pool::qualify_for` mints. The two lifts therefore differ in ARITY, not in convention, and the cited test runs both over the one real cascade (Q7's resolution) that forced typed edges. Reclassifying `justification` as strict is type-valid and turns the test red twice — at the declared gate, and again at the `Propagated::Ruled` match, where the advisory edge is found to have consulted nobody. That mutation is what establishes the row. | `rung-het/tests/questions_of_rung.rs::a_strict_edge_propagates_decidably_and_an_advisory_edge_is_ruled_on` |
| [11.3](rung-ct-props.md#advisory-lift-lands-in-a-coproduct) | `advisory-lift-lands-in-a-coproduct` | `judgmental` | — | *awaits a* `category-theorist` |
| [11.31](rung-ct-props.md#same-coproduct-at-both-levels) | `same-coproduct-at-both-levels` | `rationale` | — | — |
| [11.32](rung-ct-props.md#vertical-morphisms-preserve-agency) | `vertical-morphisms-preserve-agency` | `rationale` | — | — |
| [11.4](rung-ct-props.md#edges-are-dependent-optics) | `edges-are-dependent-optics` | `judgmental` | — | *awaits a* `category-theorist` |
| [11.41](rung-ct-props.md#exposure-is-the-backward-pass) | `exposure-is-the-backward-pass` | `signature` | — | — |
| [11.5](rung-ct-props.md#opfibrations-compose) | `opfibrations-compose` | `judgmental` | — | *awaits a* `category-theorist` |
| [11.51](rung-ct-props.md#iteration-not-a-second-level) | `iteration-not-a-second-level` | `judgmental` | — | *awaits a* `category-theorist` |
| [11.52](rung-ct-props.md#transport-is-scale-invariant) | `transport-is-scale-invariant` | `judgmental` | — | *awaits a* `category-theorist` |
| [11.53](rung-ct-props.md#horizontal-and-vertical-coincide) | `horizontal-and-vertical-coincide` | `judgmental` | — | *awaits a* `category-theorist` |
| [11.6](rung-ct-props.md#conformance-and-propagation-run-over-different-bases) | `conformance-and-propagation-run-over-different-bases` | `rationale` | — | — |

### The mathematics is the implementation, not the surface

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|
| [12](rung-ct-props.md#mathematics-is-the-implementation-not-the-surface) | `mathematics-is-the-implementation-not-the-surface` | `rationale` | — | — |
| [12.1](rung-ct-props.md#surface-is-the-programmers-model) | `surface-is-the-programmers-model` | `rationale` | — | — |
| [12.2](rung-ct-props.md#same-move-as-the-substrate) | `same-move-as-the-substrate` | `rationale` | — | — |
| [12.3](rung-ct-props.md#hiding-is-not-optional) | `hiding-is-not-optional` | `rationale` | — | — |
| [12.4](rung-ct-props.md#correspondence-is-falsifiable) | `correspondence-is-falsifiable` | `rationale` | — | — |

### Withdrawn claims

| prop | slug | kind | mechanism | discharged by |
|---|---|---|---|---|

---

## The whole corpus

| kind | count |
|---|---:|
| `decidable` | 123 |
| `judgmental` | 47 |
| `owed` | 3 |
| `rationale` | 148 |
| `signature` | 59 |
| **total** | **380** |

**What this table does not say.** Naming a proof is one thing; having watched it
fail is another. A test that cannot fail is not a proof, and the mutation that
demonstrates one is recorded in prose rather than counted here. Nor does anything
check that a cited proof is *apt* for the proposition citing it.

---

## The gap, both ways

The join is not onto in either direction, and each direction is a queue.

| direction | meaning | tells an author to | count |
|---|---|---|---:|
| **owed** | a proposition with no proof | write the test — or build the thing it would run against | 3 |
| **unclaimed** | a proof with no proposition | record the citation, **or write the proposition it proves** | 173 |

The second is the sharper one. A test guarding a real property the documents never
state is a guarantee this project makes and cannot account for — and one day someone
reads it as incidental and deletes it, because nothing says otherwise.

Not filtered by crate: some of these test the tooling and will never cite a
proposition. Excluding them by name would be the quiet narrowing that makes a queue
look shorter than it is.

**`rung-doctrine/tests/governed.rs`** — 9 unclaimed

- `a_proposition_cannot_become_its_own_parent`
- `an_edit_to_the_real_doctrine_shows_up_where_it_should_and_nowhere_else`
- `an_editor_that_does_more_than_the_edit_is_caught`
- `every_sentence_can_fail`
- `only_structural_edits_renumber`
- `reclassifying_refuses_a_gate_with_no_filler`
- `reparenting_renumbers_with_no_number_to_update`
- `retiring_is_refused_when_it_would_break_the_document`
- `the_real_doctrine_satisfies_every_decidable_sentence`

**`rung-doctrine/tests/judgments.rs`** — 15 unclaimed

- `a_judge_may_not_rule_on_what_it_authored`
- `a_judge_without_a_verdict_is_refused`
- `a_named_ruling_must_exist`
- `a_panel_of_any_size_is_carried_and_none_is_required`
- `a_record_may_not_settle_a_decidable_proposition`
- `a_record_may_not_settle_a_proposition_that_does_not_exist`
- `a_record_may_not_substitute_a_different_role`
- `a_record_with_no_judge_settles_nothing`
- `a_ruling_against_the_doctrine_is_a_valid_record`
- `a_sound_record_passes`
- `a_split_panel_is_recorded_not_resolved`
- `a_verdict_needs_an_argument_behind_it`
- `every_record_in_the_collection_is_well_formed`
- `the_collection_is_empty_and_says_so`
- `the_tier_records_whether_a_ruling_can_be_audited`

**`rung-doctrine/tests/roundtrip.rs`** — 21 unclaimed

- `coverage_is_reported`
- `every_derived_number_matches_the_document`
- `every_encoded_doctrine_renders_its_document_byte_for_byte`
- `every_judgmental_proposition_names_the_role_that_could_settle_it`
- `every_proposition_in_the_corpus_carries_a_kind`
- `hand_written_counts_in_prose_match_the_doctrine`
- `mechanism_prose_cites_by_slug_and_every_citation_resolves`
- `no_document_depends_on_a_number_read_off_a_page`
- `no_proof_names_an_ignored_test`
- `no_proposition_leans_on_the_compiler_without_a_case`
- `only_claims_carry_a_gate`
- `proofs_that_claim_no_proposition_are_counted`
- `the_corpus_triage_is_recorded`
- `the_curated_mechanism_prose_survived`
- `the_owed_proofs_are_the_work_queue`
- `the_proven_fraction_of_the_decidable_fragment_is_reported`
- `the_record_lists_every_proposition_once`
- `the_source_holds_no_number_and_no_rendered_link`
- `the_test_scan_includes_this_very_test`
- `the_triage_is_recorded`
- `verbatim_blocks_carry_only_non_propositional_matter`

**`rung-driver/tests/oracle.rs`** — 16 unclaimed

- `a_bare_failure_still_carries_a_reason`
- `a_missing_credential_is_unreachable_and_not_a_verdict`
- `an_out_of_band_principal_is_not_reachable_by_a_model`
- `an_undeclared_provider_is_caught_before_dispatch`
- `declining_to_rule_is_not_a_claim_failing`
- `each_principal_resolves_to_the_provider_that_serves_it`
- `hedging_is_not_a_verdict`
- `leading_whitespace_is_tolerated`
- `provider_settings_are_per_provider`
- `the_author_may_write_the_source_and_not_the_rendering`
- `the_declared_judges_and_authors_are_disjoint_sets`
- `the_model_principals_provenance_is_still_a_placeholder`
- `the_population_names_credentials_and_never_holds_one`
- `the_repositorys_population_parses_and_is_well_formed`
- `the_three_declared_forms_are_read`
- `trailing_prose_after_the_first_line_is_ignored`

**`rung-driver/tests/population.rs`** — 12 unclaimed

- `a_capability_no_role_requires_is_reported`
- `a_capable_principal_is_still_refused_for_what_it_authored`
- `a_duplicate_declaration_is_reported`
- `a_population_round_trips_through_yaml`
- `a_role_is_filled_by_whoever_declares_what_it_requires`
- `a_role_requiring_nothing_admits_everyone`
- `an_undeclared_role_admits_nobody`
- `an_unwired_oracle_defers_rather_than_agreeing`
- `backing_decides_nothing`
- `capability_alone_does_not_authorize_a_write`
- `kind_decides_nothing`
- `the_driver_offers_no_way_to_prefer_one_qualifying_principal`

**`rung-fixture/tests/consumer.rs`** — 4 unclaimed

- `a_consumer_cannot_construct_a_mid_ladder_rung`
- `a_consumer_may_place_an_order_and_drive_it`
- `an_invented_order_settles_exactly_as_a_real_one_does`
- `the_knowledge_exists_upstream_and_cannot_cross`

**`rung-het/tests/acceptance.rs`** — 2 unclaimed

- `a_judge_may_not_dispose_on_a_proposal_it_authored`
- `het_places_no_bound_on_re_entry`

**`rung-het/tests/gate_law.rs`** — 16 unclaimed

- `a_judgment_rendered_by_another_principal_is_refused`
- `a_judgmental_verdict_may_be_non_conforming`
- `an_empty_pool_qualifies_no_one`
- `an_exhausted_pool_reports_exhaustion_not_the_last_failure`
- `decidable_sentence_reports_its_own_failure_reason`
- `decidable_sentence_settles_without_any_principal`
- `disjointness_and_containment_are_different_conditions`
- `empty_provenance_overlaps_nothing`
- `judgmental_sentence_records_the_principal_that_settled_it`
- `p0_admits_a_judge_with_disjoint_provenance`
- `p0_is_not_vacuous_when_the_model_claims_no_author`
- `p0_refuses_a_judge_who_authored_the_material`
- `p0_refuses_on_partial_overlap_not_only_identity`
- `qualification_walks_the_pool_and_takes_any_survivor`
- `the_theory_exposes_its_sentences_with_their_gates`
- `the_verdict_comes_from_the_oracle_and_not_from_the_caller`

**`rung-het/tests/pass_ladder.rs`** — 4 unclaimed

- `a_token_minted_against_the_model_is_refused_at_dispose`
- `calling_dispose_without_a_token_is_e0061`
- `calling_propose_without_a_pen_is_e0061`
- `proposing_carries_classification_only`

**`rung-het/tests/questions_of_rung.rs`** — 9 unclaimed

- `a_blocked_question_re_enters_at_gathered_rather_than_terminating`
- `every_internal_dependency_in_the_real_files_resolves`
- `every_per_question_decidable_sentence_holds_over_all_fifteen_questions`
- `no_question_is_blocked_on_itself`
- `p0_refuses_the_curator_as_a_judge_of_this_repositorys_own_questions`
- `standing_over_a_folder_can_be_refused_with_nowhere_to_appeal`
- `the_fifteen_questions_are_read_from_disk`
- `the_lifecycle_ladder_runs_the_authorial_and_judgmental_gates_in_turn`
- `the_real_questions_report_their_outbound_edge_drift`

**`rung-het/tests/second_domain.rs`** — 5 unclaimed

- `a_domain_with_entirely_different_edits_runs_the_same_pass`
- `a_pen_for_one_territory_does_not_authorize_another`
- `p0_holds_here_too_without_the_library_knowing_the_domain`
- `the_pass_is_indifferent_to_which_vocabulary_it_carries`
- `wont_fix_closes_an_issue_that_remains_non_conforming`

**`rung-std/tests/driver.rs`** — 8 unclaimed

- `a_matter_that_never_terminates_leaves_its_run_parked_and_named`
- `any_reference_the_theory_names_is_matched_and_none_is_parsed`
- `depth_is_unbounded_and_every_parked_run_is_visible`
- `every_terminal_releases_alike_and_the_park_reads_none_of_them`
- `evidence_for_an_unparked_matter_releases_nothing_and_disturbs_nothing`
- `evidence_releases_the_run_it_answers_and_not_the_one_parked_first`
- `one_terminal_releases_every_run_that_awaits_it`
- `the_same_run_parks_and_resumes_without_bound`

**`rung-std/tests/principals_theory.rs`** — 11 unclaimed

- `a_kind_fixes_its_identity_fields_and_a_principal_missing_one_is_refused`
- `a_principal_that_declares_no_epsilon_is_not_well_formed`
- `capability_is_a_mechanical_comparison_and_a_claimed_role_is_not_an_earned_one`
- `cost_is_declared_per_kind_and_epsilon_per_principal`
- `every_decidable_sentence_holds_over_both_rosters`
- `nothing_further_than_the_declared_interface_crosses_into_rung`
- `one_pool_two_filters_over_the_same_roster`
- `p0_refuses_a_principal_as_the_examiner_of_its_own_competence_claim`
- `the_kind_partition_is_ruled_on_by_an_outside_and_not_computed`
- `the_library_names_no_role_or_principal_of_either_roster`
- `the_theory_exposes_its_sentences_with_their_gates`

**`rung-std/tests/questions_theory.rs`** — 16 unclaimed

- `a_gate_cycle_is_a_deadlock_and_the_sentence_refuses_it`
- `a_parked_question_re_enters_at_gathered_rather_than_terminating`
- `a_pen_for_one_folder_does_not_author_a_question_in_another`
- `a_ruling_on_one_exposure_does_not_carry_to_another`
- `a_strict_edge_propagates_decidably_and_an_advisory_edge_is_ruled_on`
- `every_declared_edge_kind_has_a_lived_instance_in_the_docket`
- `every_per_question_sentence_holds_over_the_whole_docket`
- `nesting_is_not_a_cycle_a_premise_up_and_a_gate_down`
- `p0_refuses_the_curator_as_a_judge_of_the_questions_it_filed`
- `the_docket_has_no_dangling_dependency_and_no_duplicate_id`
- `the_docket_has_no_gate_cycle`
- `the_docket_parses_into_six_questions_with_a_disjoint_id_space`
- `the_docket_reports_its_own_outbound_edge_drift`
- `the_done_pile_runs_its_own_law_on_a_write_the_ruling_already_authorized`
- `the_theory_exposes_its_sentences_with_their_gates`
- `the_two_paths_differ_in_arity_not_in_convention`

**`rung/tests/compile_pass.rs`** — 1 unclaimed

- `test_verdict_enum`

**`rung/tests/end_to_end.rs`** — 3 unclaimed

- `exhausts_budget_when_target_unreachable`
- `long_spine_registers_every_hop`
- `must_progress_guard_panics_on_no_progress`

**`rung/tests/gate_markers.rs`** — 6 unclaimed

- `a_body_that_ignores_the_pen_still_gets_the_standing_check`
- `a_body_that_ignores_the_token_still_gets_the_binding_check`
- `a_judgmental_arrow_may_not_return_the_provenance_it_judged`
- `an_authorized_pen_cannot_be_constructed_outside_the_pool`
- `authorial_without_a_role_is_refused`
- `calling_an_authorial_transition_without_a_pen_is_e0061`

**`rung/tests/provenance_floor.rs`** — 4 unclaimed

- `a_hand_written_provenanced_impl_for_a_principal_is_a_coherence_error`
- `a_principal_can_never_present_an_empty_provenance`
- `a_principals_provenance_always_contains_its_identity`
- `the_newcomer_is_no_longer_disjoint_from_its_own_work`

**`rung/tests/spec_refusals.rs`** — 8 unclaimed

- `a_continue_arm_target_must_be_a_declared_rung`
- `a_failed_source_rung_must_be_declared`
- `a_recover_edge_must_name_a_declared_verdict`
- `a_recover_edges_target_must_be_a_declared_rung`
- `a_recover_target_must_be_a_declared_rung`
- `a_terminal_verdict_may_not_carry_a_recover_edge`
- `an_impl_block_missing_a_body_is_refused`
- `an_impl_body_that_names_no_transition_is_refused`

**`rung/tests/suspension.rs`** — 3 unclaimed

- `an_answered_dispatch_still_produces_the_next_rung`
- `calling_resume_without_a_pen_is_e0061`
- `resume_refuses_a_pen_over_another_container`

