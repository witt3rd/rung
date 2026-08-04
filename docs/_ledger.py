#!/usr/bin/env python3
"""Generate and check docs/conformance.md.

The ledger joins Het's propositions to their enforcement in rung. It is keyed
on proposition slug, so it survives every renumbering (see `_props.py`).

A mechanism may cite another proposition as `{#slug}`; the number is filled in
at generation time, so no decimal is ever written by hand.

    ./_ledger.py gen      regenerate conformance.md from CURATED + the default
    ./_ledger.py check    exit 1 if the file differs from what gen would write,
                          or if a slug, citation, or cited test does not resolve
"""

import difflib
import re
import sys
from pathlib import Path

HERE = Path(__file__).parent
DOC = HERE / "rung-het-props.md"
LEDGER = HERE / "conformance.md"
ROOT = HERE.parent

# `deferred` used to be here, and it let a row rest on a prose question file:
# "blocked on a named open question, or on a named gap". A reader could not tell
# a gap that had been measured from one that had merely been written down, and
# nothing in a run reported when a deferral went stale — several of them had.
#
# `parked` replaces it and asks for more, not less: a real gap must cite a test
# carrying `#[ignore = "..."]` whose reason names what would close it. Removing
# the attribute then answers the question the deferral could only pose. `check`
# enforces both halves — the citation must be a test, and that test must be
# parked.
VERDICTS = {"enforced", "expressible", "parked", "collides", "out-of-scope"}
DEFAULT = ("out-of-scope", "mathematics of the institution — no host obligation", "—")

# A `{#slug}` in a mechanism expands to a numbered link at generation time.
# Never write a bare decimal here: this file is keyed on slugs precisely so it
# survives renumbering, and a hardcoded number would not.
CITE = re.compile(r"\{#([a-z0-9-]+)\}")

# slug -> (verdict, mechanism, conformance)
CURATED = {
    # ── the refusal: a verb cannot occupy object-position ────────────────
    "self-governing-not-self-closing": (
        "enforced",
        "G2 sealed construction. This proposition *is* rung's founding refusal: "
        "an attempt to fold a live verdict into the next state was rejected by the "
        "sealed constructor — [the law](rung-ct-props.md#the-law). The "
        "algebra runs its own decidable "
        "step; it cannot construct the state that holds a judgmental outcome.",
        "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624",
    ),
    "constant-arrow-hazard": (
        "enforced",
        "G2 sealed construction. A judgmental arrow cannot be interpreted by a "
        "constant drawn from the algebra's own carrier, because no mid-ladder rung "
        "is constructible outside its module.",
        "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624",
    ),
    "non-identity-by-construction": (
        "enforced",
        "G12 + G13. The token witnesses the **pair** this proposition names: "
        "`Qualified<R>` records the principal AND `π(a)`, the argument "
        "disjointness was measured against, and `Qualified::admit` is the one "
        "gate that spends it. The seal closes *fabrication* — there is no public "
        "constructor, `Pool::qualify_for` is the only mint. The binding closes "
        "*transfer* — a licence earned against one argument is refused anywhere "
        "else, as `TokenNotBound` from `dispose` and `settle`, and as the "
        "macro-injected prologue on a `#[judgmental(R)]` transition, which a body "
        "can no more skip than it can skip G8's `must_progress`. Deleting the "
        "`admit` call turns the cited test red. NOT enforced: the *returned* "
        "value. `π(f(a)) ∩ π(a) = ∅` is a body property and inherits SPEC §5.",
        "rung-het/tests/token_binding.rs::dispose_refuses_a_token_minted_against_the_model",
    ),
    "disjointness-against-argument": (
        "enforced",
        "G13. Disjointness is measured against the argument, and the token now "
        "remembers WHICH argument, so spending it elsewhere is a refusal rather "
        "than an unobservable mistake. `dispose` admits a token only against the "
        "**proposal**; `settle` only against the **model**. Until the binding "
        "landed this proposition was satisfied only by the caller passing the "
        "right reference — `qualify_for` was a pure alias for `qualify` and "
        "nothing downstream could tell the two apart.",
        "rung-het/tests/token_binding.rs::settle_refuses_a_token_minted_against_a_different_model",
    ),
    "argument-governs": (
        "enforced",
        "G13, at the point where the two readings come apart. A judge that "
        "authored a Proposal is disjoint from the MODEL by construction, so a "
        "model-relative mint would admit it to rule on its own work; the cited "
        "test performs exactly that laundering, with a token minted honestly "
        "against the model, and `dispose` refuses it. `Pool::qualify` is now the "
        "`audit` reading of `qualify_for`, where π(a) = π(M) — one filter, and "
        "which name the caller used is a comment rather than the check.",
        "rung-het/tests/token_binding.rs::dispose_refuses_a_token_minted_against_the_model",
    ),
    "non-identity-before-dispatch": (
        "expressible",
        "The filter is set operations over declared predicates "
        "({#conformance-half-needs-no-judge}), and it runs **before** dispatch "
        "because dispatch has no other door: a judgmental transition called "
        "without a token is E0061, and the only mint is `Pool::qualify_for`, "
        "which refuses before it returns. The cited `trybuild` case is that "
        "refusal with its message committed. rung enforces *that the token was "
        "constructed*, never that the body computed the set "
        "correctly — SPEC §5, transition-body correctness.",
        "rung/tests/gate_markers.rs::calling_a_judgmental_transition_without_a_token_is_e0061",
    ),

    # ── the surface: two gates are two signatures ────────────────────────
    "two-signatures-not-two-fragments": (
        "enforced",
        "`ladder!` gate markers, now three signatures rather than two. Unmarked "
        "emits `fn t(prev)`; `#[judgmental(R)]` emits "
        "`fn t(prev, q: Qualified<R>)`; `#[authorial(R)]` emits "
        "`fn t(prev, pen: Authorized<'_, R>)`. The gates differ in the ARITY and "
        "the TYPE of the emitted transition, so a pen cannot be passed where a "
        "licence is asked for or the reverse, and the host's type system separates "
        "all three with no knowledge of Het.",
        "rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen",
    ),
    "decidable-cannot-consult-pool": (
        "enforced",
        "G2. The qualifying token has no constructor reachable from a decidable "
        "body, so the prohibition is a term that cannot be written rather than a "
        "rule an author is asked to respect. An unmarked transition has no "
        "parameter a token could enter through, and `Qualified` is sealed: "
        "constructing one outside `Pool::qualify` is E0451.",
        "rung/tests/gate_markers.rs::a_qualified_token_cannot_be_constructed_outside_the_pool",
    ),
    "mismarking-is-not-a-false-claim": (
        "enforced",
        "rustc. Marking a transition judgmental gives it the judgmental "
        "signature; calling it as though it were decidable is E0061, not a "
        "promise someone broke.",
        "rung/tests/gate_markers.rs::calling_a_judgmental_transition_without_a_token_is_e0061",
    ),
    "signature-replaces-fragment-membership": (
        "enforced",
        "rustc. The refusal is an arity error from a compiler that does not know "
        "Het exists and cannot be persuaded — which is the whole claim of this "
        "proposition.",
        "rung/tests/gate_markers.rs::calling_a_judgmental_transition_without_a_token_is_e0061",
    ),

    # ── the pass ─────────────────────────────────────────────────────────
    "the-pass": (
        "enforced",
        "One `ladder!` declaration, and it is now written: `het_pass!` expands "
        "to the spine `Governed => Audited => Proposing => "
        "#[authorial(Author)] Proposed => #[judgmental(Judge)] { .. }`. The "
        "table's `gate` column is a **marker** and its `acts` column is a "
        "**parameter type**, so which principal may move is settled by rustc "
        "rather than by a driver keeping to a convention: `propose` without a "
        "pen is E0061 and `dispose` without a licence is E0061, each with its "
        "message committed as a `trybuild` snapshot. Retargeting the judgmental "
        "marker at the author's role — one token, type-valid, the library still "
        "compiles — turns the cited test red on `expected Qualified<Editor>, "
        "found Qualified<Reader>`. rung proves each move was made by one who "
        "qualified, not that the move was wise (SPEC §5). What is NOT in the "
        "declaration is `enact`: see {#a-cycle-through-an-authorial-act-cannot-close}.",
        "rung-het/tests/pass_ladder.rs::the_pass_runs_end_to_end_as_a_ladder",
    ),
    "disposition-vocabulary": (
        "enforced",
        "G6 exhaustive outcomes. `StepOutcome` is an enum, so every match site must "
        "handle all five; adding a disposition breaks every call site at compile time. "
        "The cited test pins the vocabulary itself — the five, in order, each with "
        "its terminal and affirming flags — so that the two that Het's gate "
        "boundary excludes (`accept-with-mod`, `reject-with-alternative`) cannot "
        "return without the assertion changing.",
        "rung-het/tests/acceptance.rs::the_disposition_vocabulary_is_exactly_the_five_that_survive_the_gate",
    ),
    "disposition-is-a-ruling": (
        "enforced",
        "G2. `dispose` returns a verdict; only the separately-declared authorial arrow "
        "produces the revised object. A ruling cannot construct what it rules on.",
        "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624",
    ),
    "no-amending-disposition": (
        "enforced",
        "G2 plus G10, and the second half is what the pass added. A judge's "
        "arrow has no constructor for the authored object — but a continue arm's "
        "target rung is built INLINE by `step`, i.e. by the judge, so the pass's "
        "re-entry rung is the one place an amendment could have arrived. Its "
        "payload is therefore `Chain`: a concrete, non-generic record of an id, "
        "a container, a count and prose, with no edit and no type parameter one "
        "could hide in. The cited `trybuild` case pins the E0599 that reading an "
        "edit off it produces; giving `Chain` an `edit` accessor — type-valid, "
        "the library still compiles — turns it red on a diff.",
        "rung-het/tests/pass_ladder.rs::a_chain_cannot_be_read_for_an_edit",
    ),
    "reproposal-carries-the-chain": (
        "enforced",
        "The chain rides in the rung payload and there is no other route to a "
        "re-proposal: the pass's authorial transition builds its Proposal from "
        "the `Chain` the continue arm handed back, so an author cannot drop it "
        "by omission. The cited test rejects the identical remedy five times and "
        "reads all five reasons off the sixth chain. Deleting the push in "
        "`Chain::reentered` — type-valid — turns it red at `left: 0, right: 5`, "
        "and `acceptance.rs::reject_remedy_is_non_terminal_and_the_reason_"
        "reaches_the_author` with it. Without the chain an author can cycle on "
        "one objection and nothing downstream can tell. NOTE: this is exactly "
        "what would make "
        "a G8 progress guard vacuous — a strictly growing chain never compares equal "
        "— which is why re-entry must not use a guarded edge "
        "({#guarded-reentry-is-eviction}).",
        "rung-het/tests/pass_ladder.rs::reject_remedy_re_enters_with_no_progress_guard",
    ),
    "enact-makes-an-endofunctor": (
        "expressible",
        "The loop closes, and it closes by COMPOSITION rather than inside the "
        "declaration — which is the honest reading and is now recorded as a "
        "non-guarantee ({#a-cycle-through-an-authorial-act-cannot-close}). "
        "`ladder!` declares a linear spine with backward continue arms, and a "
        "continue arm's target is built inline by `step`, so an `Accept -> "
        "Governed` arm would have the judge apply the edit "
        "({#no-amending-disposition}). `Accept` is therefore terminal and "
        "carries a `Licence`; `enact` is a separate authorial arrow consuming "
        "that licence and a pen, and what comes out is audited again. The cited "
        "test closes the loop that way: the relocated specimen lands in the "
        "fieldbook and the fieldbook's own decidable sentence is run over the "
        "result. STILL `expressible`, and the reason is not shyness — no single "
        "`ladder!` declaration is an endofunctor, and saying otherwise would be "
        "a claim no mutation could falsify. Declaring the composite is Q4, open. "
        "rung enforces that the edit ran, not that it was right (SPEC §5), and "
        "the edit itself is the theory's ({#edit-required-not-typed}).",
        "rung-het/tests/acceptance.rs::the_pass_runs_end_to_end_as_a_chain_of_principals",
    ),
    "licence-is-not-guarantee": (
        "enforced",
        "A `Licence<E>` is now a type, minted only from an affirming `Ruling` "
        "and consumed by `enact` — so the pass's `Accept` arm carries "
        "PERMISSION rather than a revised subject. Permission is all it is: "
        "`enact` still checks the pen against `Applies::territory` and hands the "
        "domain's own refusal back untouched. Making `enact` swallow "
        "`Applies::apply`'s error — type-valid, `world.apply(..)?` to `let _ = "
        "world.apply(..)` — turns the cited test red where it requires the "
        "fieldbook to refuse a write the cabinet's judge already accepted. "
        "The two failure points are {#enact-has-two-failure-points}.",
        "rung-het/tests/acceptance.rs::the_pass_runs_end_to_end_as_a_chain_of_principals",
    ),
    "remedy-carries-an-edit": (
        "enforced",
        "The edit is the rung payload's type, supplied by the theory, and the "
        "requirement is now a *variant shape*: an author answers through "
        "`Answer<E>`, whose `Remedy(E)` has nowhere to put the absence of an "
        "edit. A theory that let a remedy carry none would make `remedy` and "
        "`dispute` indistinguishable, and there is no term for it. Dropping the "
        "edit in `Proposal::from_chain` — type-valid, `Remedy` rewritten to "
        "`Dispute` — reddens the cited test at `rounds: left 1, right 2` (the "
        "judge has nothing to reject, so the loop it exists to exercise never "
        "runs) and `acceptance.rs::the_pass_runs_end_to_end_as_a_chain_of_"
        "principals` with it. The boundary itself is pinned by "
        "`acceptance.rs::an_author_may_dispute_a_verdict_without_first_"
        "authoring_a_remedy`: a dispute's `edit()` is `None`.",
        "rung-het/tests/pass_ladder.rs::the_pass_runs_end_to_end_as_a_ladder",
    ),

    # ── the limit, and the collision it resolves ─────────────────────────
    "guarded-reentry-is-eviction": (
        "enforced",
        "G10 continue arms — 'no recover function, no guard, no source'. Re-entry "
        "must be `RejectRemedy -> Proposing`, never `RejectRemedy => Proposing`: the "
        "recoverable-verdict form injects G8's `must_progress`, which panics on no "
        "progress and is therefore an eviction rule ({#answers-are-worth-shaped}). "
        "CONSTRAINT: a continue arm's target rung is built inline by `dispose`, i.e. "
        "by the judge, so that rung's payload must be classification-only "
        "({#no-amending-disposition}).",
        "rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn",
    ),
    "no-bound-on-reentry": (
        "enforced",
        "A continue arm loops with no host-imposed bound, and the pass now runs "
        "that loop: the cited test drives five identical rounds — the same edit "
        "answered by the same reason — and nothing panics, nothing evicts, and "
        "the subject is still in the loop at attempt six. Choosing a guarded "
        "edge instead would supply a bound Het declines to declare "
        "({#guarded-reentry-is-eviction}); so would giving up quietly after "
        "three tries, which is the mutation that proves the test can fail — "
        "`assert!(chain.attempt() <= 3)` in the re-entry arm reddens it on the "
        "fourth round. `acceptance.rs::het_places_no_bound_on_re_entry` "
        "additionally pins `Disposition::REENTRY_BOUND` as `None`. Either "
        "answer would be a worth-law smuggled in under another name "
        "({#cut-at-valuation}).",
        "rung-het/tests/pass_ladder.rs::reject_remedy_re_enters_with_no_progress_guard",
    ),

    # ── the residual ─────────────────────────────────────────────────────
    "epsilon-reported-with-verdict": (
        "parked",
        "GAP — `Verdict` is Boolean (`Conforming | NonConforming`). No metric, no "
        "epsilon, so the satisfaction condition does not survive renaming "
        "({#boolean-breaks-satisfaction}). The cited test is the gap as an "
        "assertion: two judges settle the same sentence with the same polarity, "
        "one barely persuaded and one certain, and the two verdicts are the same "
        "object. Deleting the `#[ignore]` reports whether an error bar has "
        "reached the caller.",
        "rung-het/tests/gate_law.rs::two_judges_of_differing_confidence_report_differing_verdicts",
    ),
    "judgmental-qualifying-set": (
        "parked",
        "Both conjuncts are implemented and both are tested — competence by "
        "`gate_law.rs::competence_is_filtered_before_provenance_matters`, "
        "disjointness by `::p0_refuses_a_judge_who_authored_the_material`. What "
        "is parked is the set's own **edge**. `Pool::qualify_for` refuses a "
        "model with `\u03c0(a) = \u2205`, because every candidate would then pass "
        "disjointness vacuously; the mirror on the *principal's* side is "
        "unguarded, so a principal declaring `\u03c0(p) = \u2205` is disjoint from "
        "everything and is a universal judge admitted by construction. Het as "
        "written admits it. Whether that is a hole or the honest consequence of "
        "the definition is a change to **this proposition**, which is why the "
        "cited test presumes an answer and is parked rather than run: the engine "
        "invented the model-side guard on its own judgment once, and inventing "
        "its mirror unasked would be the same overreach twice.",
        "rung-het/tests/token_binding.rs::a_principal_with_no_provenance_is_refused",
    ),
    "no-preference-among-judges": (
        "expressible",
        "`Pool::qualify_for` walks the pool and returns the FIRST survivor, and "
        "the cited test shows what that does and does not mean: candidates are "
        "skipped for failing a *conjunct* — wrong role, overlapping provenance — "
        "never for being ranked below another. Het says any qualifying judge "
        "yields a well-formed verdict, so a deterministic pick is admissible; "
        "the seam where HetOpt's `argmin` would land is named in "
        "`Pool::qualify_for`'s own docs and is empty. Still UNARGUED: whether "
        "pool position itself constitutes an ordering. Assumed, not shown.",
        "rung-het/tests/gate_law.rs::qualification_walks_the_pool_and_takes_any_survivor",
    ),
    "judgmental-arrow-shape": (
        "enforced",
        "The `+ A` residual is `Failed<Prev> { token, error }` — the unconsumed "
        "argument handed back. rung-CT names it the Prism's residual "
        "([residual-is-the-optics-residual](rung-ct-props.md#residual-is-the-optics-residual)) "
        "and is why the error structure is not a Kleisli arrow; the monad `P` layers "
        "on the forward pass, which rung-CT explicitly permits "
        "([effects-layer-on-the-forward-pass](rung-ct-props.md#effects-layer-on-the-forward-pass)).",
        "rung/tests/compile_pass.rs::test_failed_type",
    ),
    "adequacy-failure-returns-residual": (
        "enforced",
        "G9 error-path recovery, `Failed(R) => R`, explicitly unguarded — a re-entry "
        "after an unanswered call may reuse the argument. G4 additionally forbids "
        "silently dropping the returned residual.",
        "rung/tests/end_to_end.rs::recovers_from_the_failed_error_path",
    ),

    # ── once deferred on open questions, now measured ────────────────────
    #
    # Four of these rested on Q4, Q5 and Q8. In each case the deferral had
    # widened the question: Q5 asks about *concurrent* fork-join and the row
    # asks only for more than one judge; Q8 asks about the *async* driver and
    # the row asks only that the arrow be Kleisli; Q4 asks about
    # ladder-in-ladder and the row asks only that the target's own law run at
    # the boundary. What each question actually blocks is named below and kept.
    "panels": (
        "expressible",
        "A panel is `⊨` with more than one judge, and the proposition says it is "
        "**not a separate construction** — so the encoding must not add one. It "
        "does not: a seat is a pool of one principal, each seat mints its own "
        "licence against the very same argument, and the cited test convenes "
        "three of them with nothing `rung-het` does not already export. The "
        "combination rule is the theory's, exactly as its edits are "
        "({#edit-required-not-typed}); putting a `panel()` primitive in the "
        "library would legislate a rule Het does not have. What stays with Q5 is "
        "running the seats **at the same time** — latency, which is HetOpt's "
        "([cut-at-valuation](rung-het-props.md#cut-at-valuation)), not Het's.",
        "rung-het/tests/panel.rs::a_panel_is_the_pass_with_more_than_one_judge",
    ),
    "panels-cannot-weaken-the-opponent": (
        "expressible",
        "The observable form of the claim: the same Proponent move, the same "
        "first oracle answer, plus two more — and the seat that played in the "
        "original game answers identically in the composite. Added answers may "
        "take affirmation away and never grant it, so the Proponent's winning "
        "set under the panel is contained in its winning set against any single "
        "seat. rung proves the rulings were reached through qualified licences, "
        "not that unanimity is the right combination rule ({#panels}).",
        "rung-het/tests/panel.rs::a_panel_cannot_weaken_the_opponent",
    ),
    "judgmental-is-kleisli-arrow": (
        "expressible",
        "`A → 𝒫(B)` is a claim about **shape**, and the shape is exhibited "
        "directly: one argument, two qualifying judges, two different and equally "
        "well-formed Dispositions. Were `dispose` an `A → B` the second call "
        "could not disagree. The non-determinism is the outside itself — "
        "{#no-preference-among-judges} forbids Het from ranking the two. A "
        "*blocking* outside call works today; `rung-std`'s `LlmCall` ladder puts "
        "one on the arrow. Q8 constrains **how** the call is made, not whether "
        "the arrow is Kleisli.",
        "rung-het/tests/panel.rs::a_judgmental_arrow_returns_a_set_and_not_a_value",
    ),
    "target-runs-its-own-models": (
        "expressible",
        "The write-guard exists and fires. `enact` checks the pen against "
        "`Applies::territory` and hands `EnactError::TargetRefused` back "
        "untouched, so a destination may decline a write its own judge already "
        "authorized: in the cited test the relocation is accepted by a qualified "
        "judge, refused by the fieldbook for want of a locality, and the source "
        "container is left unchanged. The target's law is the **theory's** — the "
        "library cannot know what admits a specimen — so rung secures the seam "
        "and the standing, not the law. `second_domain.rs::a_pen_for_one_"
        "territory_does_not_authorize_another` pins the standing half. What "
        "stays with Q4 is expressing the composite as a ladder inside a ladder; "
        "the boundary itself no longer waits on it.",
        "rung-het/tests/acceptance.rs::the_pass_runs_end_to_end_as_a_chain_of_principals",
    ),
    "gate-faithful": (
        "parked",
        "Q11 (gate-faithfulness, open), and now with a case rather than only an "
        "argument. The cited test is the load-bearing blocker made runnable: a "
        "judgmental arrow that returns a value carrying π(a) itself passes every "
        "check rung makes. Nothing was rigged for it — the gate-marker suite's "
        "own `Review` ladder has been inadmissible since markers landed, and the "
        "engine could not tell. Deleting the `#[ignore]` reports whether the "
        "return side has been closed. "
        "All three rows of Q11's table are now "
        "built, and none of them is this proposition. The signature is honest "
        "(G12) and the token is bound to its argument (G13), so no judgmental "
        "arrow can be traversed except by a principal drawn from "
        "P_judg(φ, a) for the very `a` — P0 closed. That is the *input* side. "
        "This proposition is stated through "
        "{#admissibility-subcategories}, whose condition is "
        "π(f(a)) ∩ π(a) = ∅ — a constraint on what the arrow RETURNS, which no "
        "signature reaches and which inherits SPEC §5, transition-body "
        "correctness — and G14 added the authorial gate on the same "
        "input side, so it moved no part of THIS row. Two further blockers, one "
        "now smaller: `#[conditional(..)]` remains a parse-time refusal, so an "
        "algebra with a conditional operation cannot state gate-faithfulness here "
        "at all (`#[authorial(Role)]` is implemented as of G14); and "
        "`decidable` still does not factor through η, only past 𝒫 "
        "({#purity-not-secured}). Argued in the question file, with its "
        "falsifier, at `docs/questions/open/q11-gate-faithfulness.md`.",
        "rung/tests/gate_markers.rs::a_judgmental_arrow_may_not_return_the_provenance_it_judged",
    ),
    "mod-only-gate-faithful": (
        "parked",
        "Follows {#gate-faithful}, and parks on the same case: `Mod(Σ)` can "
        "consist only of gate-faithful algebras once gate-faithfulness is "
        "checkable, and the cited test is what reports that it is not yet. Until "
        "then a `theory!` declaration that violates "
        "{#admissibility-subcategories} is admitted to `Mod(Σ)` without "
        "complaint.",
        "rung/tests/gate_markers.rs::a_judgmental_arrow_may_not_return_the_provenance_it_judged",
    ),

    # ── structural correspondences worth recording ───────────────────────
    "fractal-property": (
        "expressible",
        "The composite Grothendieck opfibration "
        "([opfibrations-compose](rung-ct-props.md#opfibrations-compose)), "
        "resolved by Q10 (`docs/questions/resolved/`). The correspondence is "
        "proved and no hierarchy is built — which leaves the property itself "
        "needing a run, and the cited test is one: the pass composed with itself "
        "at a container boundary, where the destination's own law is what "
        "refuses a write the source's judge already authorized "
        "({#target-runs-its-own-models}).",
        "rung-het/tests/acceptance.rs::the_pass_runs_end_to_end_as_a_chain_of_principals",
    ),
    "two-directions-two-bases": (
        "expressible",
        "Conformance is Het's fibration (Mod: Sign^op → Cat, contravariant). "
        "Propagation is rung-CT's opfibration, pushforward and opcartesian "
        "([conformance-and-propagation-run-over-different-bases]"
        "(rung-ct-props.md#conformance-and-propagation-run-over-different-bases)). "
        "Different bases at adjacent levels — not opposite orientations of one tower. "
        "The cited test is where the two are visible at once and are not "
        "conflated: the docket's sentences are run *per question* — conformance, "
        "each model against its own theory — while drift is reported *along "
        "outbound edges*, from a revised question to whatever depended on it. "
        "One suite, two directions, and the edge set is the theory's rather than "
        "Het's ({#governs-who-not-what}).",
        "rung-std/tests/questions_theory.rs::the_docket_reports_its_own_outbound_edge_drift",
    ),
    "monad-is-provenance-strict": (
        "expressible",
        "`carry` is the natural home for provenance: a product factor preserved "
        "across every arrow, immutable by G5. It does not carry a *principal's* "
        "provenance, which lives outside the ladder.",
        "rung/tests/compile_pass.rs::test_carry_accessor_exists",
    ),
    "one-pool-two-filters": (
        "enforced",
        "G14, and this is the row G14 exists for. One `Pool` mints both tokens; "
        "the gate marker on a `ladder!` transition selects which filter runs, not "
        "which pool is consulted. `#[judgmental(R)]` emits `Qualified<R>` and runs "
        "capability + disjointness; `#[authorial(R)]` emits `Authorized<'_, R>` and "
        "runs capability + standing. The cited test drives the same three principals "
        "through both filters over one subject and asserts they DISAGREE. Dropping "
        "the capability conjunct from `Pool::authorize` turns it red.",
        "rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one",
    ),
    "authorial-qualifying-set": (
        "enforced",
        "G14. `Pool::authorize::<R>` is the only mint for `Authorized` and checks "
        "BOTH conjuncts — `capable(p, role(o))` then `standing(p, M)`. Standing "
        "alone mints nothing: the cited test hands it a steward of the container "
        "who is capable of nothing and requires `AuthorizeError::NotCapable`. "
        "NOT enforced: the outcome condition of "
        "{#admissibility-subcategories}, `π(f(a)) ⊆ π(p)`, which is a body property "
        "and inherits SPEC §5.",
        "rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one",
    ),
    "judgment-refuses-authorship-requires": (
        "enforced",
        "G12 + G14 together, which is the only way this proposition can be shown: "
        "it is a claim about two filters, so one filter cannot witness it. The "
        "cited test asserts both directions over one subject — a principal that "
        "PASSES the judgmental filter (capable, provenance-disjoint) is refused a "
        "pen, and the principal that HOLDS the pen is refused as a judge of the "
        "very subject it stewards. An authorial gate built as the judgmental gate "
        "with its token renamed passes every other gate test and fails this one.",
        "rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one",
    ),
    "provenance-overlap-is-the-point": (
        "enforced",
        "G12 + G14, read as the reason the two filters must disagree. The cited "
        "test's subject is authored by the principal that stewards its container, "
        "so the overlap that disqualifies the curator as a judge is the same fact "
        "that makes it the author. Weakening either second conjunct — disjointness "
        "in `qualify_for`, standing in `authorize` — turns the test red, because "
        "the two assertions are about the same principal and the same subject.",
        "rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one",
    ),
    "authorial-declares-standing": (
        "enforced",
        "G14. `#[authorial]` with no role is a `compile_error!` — the qualifying "
        "set is a conjunction and a marker naming no role can witness only its "
        "right half — and the pen that IS emitted carries the container standing "
        "was measured over. The macro then injects "
        "`must_hold_standing_over(&src.payload, &pen)` ahead of the body, so the "
        "declared predicate is consulted whether or not the body mentions it: the "
        "cited ladder's body never does. Stubbing the prologue to a no-op turns it "
        "red. This is what makes a marked transition's source payload have to be "
        "`Situated` — without a container there is nothing standing could be over.",
        "rung/tests/gate_markers.rs::the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads",
    ),
    "standing-conditional-gated": (
        "enforced",
        "`Pool::classify_standing` + `AuthorizeError::StandingIsJudgmental`. What "
        "is enforced is the REFUSAL TO GUESS: where containment does not settle "
        "standing, `authorize` returns the judgmental branch as an error rather "
        "than minting a pen, and the cited test requires that variant by name. "
        "NOT enforced, and not closable here: the branch itself. Closing it needs "
        "a judge, terminating at depth one ({#standing-terminates-at-depth-one}) "
        "and disjoint from the AUTHOR ({#standing-judge-disjoint-from-author}); "
        "rung has no term for that dispatch and inventing a ruling would be worse "
        "than surfacing the gap.",
        "rung/tests/gate_markers.rs::standing_alone_is_not_a_pen_and_disjointness_never_becomes_one",
    ),

    # ── explicitly rung's own non-guarantees ─────────────────────────────
    "termination-not-secured": (
        "out-of-scope",
        "Matches SPEC §5 exactly — 'liveness beyond the guard'. Het and rung state "
        "the same limit independently.",
        "—",
    ),
    "purity-not-secured": (
        "out-of-scope",
        "rung has no effect system; a decidable body may still reach the world. Het "
        "already states this as a limit rather than a guarantee.",
        "—",
    ),
    "sentence-needs-an-inhabitant": (
        "out-of-scope",
        "A signature-claim has no carrier inhabitant to test. Nothing for a host to "
        "run.",
        "—",
    ),
}

# slug -> (verdict, mechanism, conformance) for rung-ct-props.md.
#
# Kept separate from CURATED because the two documents' defaults differ: a CT
# proposition with no curated row falls to `derive_ct`, which reads the
# guarantees it names. Only propositions that bind a mechanism *at the
# dependency level* — where the fibres are whole items rather than rungs —
# belong here; everything else is the mathematics of the category.
CURATED_CT = {
    "edge-taxonomy-is-the-theorys": (
        "expressible",
        "The edge vocabulary is declared by the governing theory, not by the "
        "formalism — neither `rung` nor `rung-het` has an edge type, and "
        "`EdgeKind` lives in `rung-std::questions`, the theory that governs "
        "bodies of questions, exactly where an edit vocabulary sits "
        "(`edit-required-not-typed`). Moving the theory out of a test and into a "
        "library sharpened the row without changing its verdict: the taxonomy is "
        "now demonstrably neither the formalism's NOR one carrier's, because two "
        "carriers with disjoint id spaces and disjoint edge sets fill the same "
        "seven kinds — rung's `docs/questions/` and a synthetic decision docket. "
        "What the cited test pins is the **lived-instance** discipline, now a "
        "decidable sentence of the theory (`every_declared_kind_is_lived`) rather "
        "than prose: a kind stays in the vocabulary only while some question in "
        "the set under audit uses it, and deleting the sentence turns a test red "
        "in BOTH carriers. STILL NOT enforced, and the reason is unchanged: what "
        "would have to fail is a crate BELOW the theory naming an edge type, and "
        "no test can fail for code that was never written. The location is a "
        "choice this theory makes; the test protects the discipline, not the "
        "choice.",
        "rung-het/tests/questions_of_rung.rs::every_declared_edge_kind_has_a_lived_instance_on_disk",
    ),
    "strict-and-advisory-are-the-gate": (
        "enforced",
        "G12 + G2, read at the dependency level. `premise` routes to a "
        "`decidable` sentence whose `holds` takes only the model — there is no "
        "parameter a pool could enter through — and `justification` routes to a "
        "`judgmental` one whose `settle` consumes a `Qualified<Adjudicator>` that "
        "only `Pool::qualify_for` mints. The two lifts therefore differ in ARITY, "
        "not in convention, and the cited test runs both over the one real "
        "cascade (Q7's resolution) that forced typed edges. Reclassifying "
        "`justification` as strict is type-valid and turns the test red twice — "
        "at the declared gate, and again at the `Propagated::Ruled` match, where "
        "the advisory edge is found to have consulted nobody. That mutation is "
        "what establishes the row.",
        "rung-het/tests/questions_of_rung.rs::a_strict_edge_propagates_decidably_and_an_advisory_edge_is_ruled_on",
    ),
}

# ── rung's own propositions ──────────────────────────────────────────────
#
# The guarantees name their own conformance tests inside the document, so their
# rows are derived rather than curated (`derive_from_body`). Everything else —
# the grammar, the static-semantics rules, the emitted artifacts, the
# non-guarantees, the conformance discipline, the design judgments — carried no
# verdict at all: 46 propositions of a *normative* document classified as
# `unclassified`, which is a worklist entry rather than a clean bill.
#
# Three kinds of row appear below.
#
# **`enforced`** names a test that fails if the macro stops doing the thing.
# Most of these were already protected and merely unjoined — the workspace had
# 101 test functions and the ledger cited 21 of them. Joining a test to the
# proposition it already defends costs nothing and is the only kind of progress
# that adds no new claim.
#
# **`out-of-scope`** covers three families, and each is a family rather than a
# convenience:
#
#   - the **non-guarantees** (§5). A non-guarantee *withdraws* an obligation —
#     "the macro does not enforce the following, and a claim that it does has no
#     standing" — so there is nothing for a host to check. Where such a row can
#     still point at the test that pins the **boundary** (external fabrication
#     refused, the drop lint, the progress guard) it does, so a reader sees
#     where the guarantee stops rather than only being told that it does.
#   - the **conformance discipline** (§6.1, §6.3, §6.4). These are facts about
#     rustdoc and rules about how evidence is produced. §6.4 — "a refusal test
#     that cannot fail is not a guarantee" — is the mutation discipline itself,
#     and no machine performs it.
#   - the **design judgments** (§7). The document says of them that no machine
#     decides them and that they carry no conformance test.
#
# **`parked`** names an `#[ignore]`d test whose reason says what would close it.
# Two rows are parked, and they are the two real gaps: the returned value of a
# gated arrow is unconstrained, and one of Het's four gates has a refusal rather
# than an encoding.
CURATED_RUNG = {
    # ── 1 · Grammar ──────────────────────────────────────────────────────
    "declaration-is-a-block": (
        "enforced",
        "The macro accepts exactly this shape. The cited ladder is a declaration "
        "block followed by an inline `impl` block and is driven to a terminal "
        "verdict, so both halves of the form are exercised by a run rather than "
        "by an expansion that merely typechecks.",
        "rung/tests/end_to_end.rs::drives_to_convergence",
    ),
    "declaration-grammar": (
        "enforced",
        "The cited declaration uses every production of the grammar at once — a "
        "`carry` block, a multi-hop spine, a verdict block carrying a terminal "
        "marker, a recoverable verdict, and a `recover` edge. A production the "
        "parser dropped would fail to expand. The refusals that keep the grammar "
        "from accepting *more* than this are {#macro-must-reject}.",
        "rung/tests/compile_pass.rs::test_module_exists",
    ),
    "bodies-grammar": (
        "enforced",
        "The cited ladder supplies three inline bodies in the `ident = closure` "
        "form, comma-separated, mixing block and expression closures. They expand "
        "into the module and are called by the driver.",
        "rung/tests/end_to_end.rs::drives_to_convergence",
    ),
    "transition-naming": (
        "enforced",
        "The driver calls `opt::active`, `opt::step` and `opt::iterate` by those "
        "names — the target lowercased, `step` for the branching transition, the "
        "recover edge's own name. Renaming any of the three in the macro turns "
        "the call site into an unresolved path.",
        "rung/tests/end_to_end.rs::drives_to_convergence",
    ),
    "marker-annotates-the-target": (
        "enforced",
        "The cited ladder marks both markable positions — a rung, and the verdict "
        "block — and the test coerces `review::active` and `review::step` to `fn` "
        "pointers of the exact expected types. A marker that annotated the source "
        "rather than the target would put the parameter on the wrong function and "
        "both coercions would fail.",
        "rung/tests/gate_markers.rs::judgmental_transition_takes_a_qualified_token",
    ),
    "at-most-one-marker": (
        "enforced",
        "A `trybuild` case with `#[judgmental(R)] #[authorial(R)]` on one "
        "transition, whose committed `.stderr` holds the macro's message. The "
        "macro has refused this since markers landed; until the case existed "
        "nothing would have noticed if it stopped.",
        "rung/tests/gate_markers.rs::two_markers_on_one_transition_are_refused",
    ),
    "two-markers-implemented": (
        "enforced",
        "Both markers emit, and emit *different* second parameters — the cited "
        "test coerces the authorial transition to "
        "`fn(Filed, Authorized<'_, R>) -> Revised`, and "
        "`judgmental_transition_takes_a_qualified_token` does the same for "
        "`Qualified<R>`. A pen cannot be passed where a licence is asked for, "
        "which is the whole content of \"two gates, two signatures\".",
        "rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen",
    ),
    "conditional-marker-refused": (
        "enforced",
        "A `trybuild` case whose committed `.stderr` holds the refusal, "
        "including the pointer to the open question. A `compile_fail` doctest "
        "would not have distinguished this refusal from a typo "
        "({#compile-fail-asserts-only-non-compilation}).",
        "rung/tests/gate_markers.rs::conditional_is_refused_and_names_the_open_question",
    ),
    "marker-without-role-refused": (
        "enforced",
        "Two `trybuild` cases, one per marker — the cited one for "
        "`#[judgmental]`, `authorial_without_a_role_is_refused` for its mirror. "
        "Both `.stderr` snapshots carry the reason, which is that there is no "
        "signature to emit rather than that the syntax is unfamiliar.",
        "rung/tests/gate_markers.rs::judgmental_without_a_role_is_refused",
    ),

    # ── 2 · Static semantics ─────────────────────────────────────────────
    "macro-must-reject": (
        "enforced",
        "All ten rules, each a `trybuild` case with a committed `.stderr`. Two of "
        "the ten are unreachable through the grammar rather than untested, and "
        "the suite says so where the reachable neighbour lands: rule 2 cannot be "
        "written because every rung of the spine is declared by the hop that "
        "introduces it, and rule 5's *missing recover function* clause cannot be "
        "written because one `recover` entry pushes the edge and the function "
        "together. Before these cases, seven of the ten were prose the macro "
        "happened to implement.",
        "rung/tests/spec_refusals.rs::a_duplicate_carry_field_is_refused",
    ),
    "structural-rules-mirror-the-reference-checker": (
        "out-of-scope",
        "A provenance note about a retired artifact. The Python checker is under "
        "`.archive/`, nothing in the workspace depends on it, and \"verified in "
        "sync\" records a comparison made once by hand rather than a property "
        "anything re-checks. What the note is *about* — that rules 1–8 are "
        "structural — is now pinned rule by rule under {#macro-must-reject}.",
        ".archive/python-poc/rung/checker.py",
    ),
    "body-rules-need-an-impl-block": (
        "enforced",
        "The cited declaration omits the `impl` block entirely and expands "
        "cleanly, so rules 9–10 did not fire on a ladder with no bodies to check. "
        "That they *do* fire when the block is present is "
        "`spec_refusals.rs::an_impl_body_that_names_no_transition_is_refused` and "
        "`::an_impl_block_missing_a_body_is_refused`.",
        "rung/tests/compile_pass.rs::test_module_exists",
    ),
    "extension-refusals-are-pinned": (
        "enforced",
        "The proposition names its own three cases; this is the first of them. "
        "Each holds a committed `.stderr`, which is what makes the refusal's "
        "*message* part of the assertion rather than only its existence.",
        "rung/tests/spec_refusals.rs::a_recoverable_verdict_cannot_declare_a_payload",
    ),

    # ── 3 · Emitted artifacts ────────────────────────────────────────────
    "emitted-module": (
        "enforced",
        "Every path in the cited test goes through `metricoptimization::`, the "
        "ladder name lowercased. A module emitted under another name, or not "
        "emitted, is an unresolved path.",
        "rung/tests/compile_pass.rs::test_module_exists",
    ),
    "emitted-carry": (
        "enforced",
        "`test_module_exists` constructs `Carry` with both declared fields by "
        "name, which needs the struct, the field names, and their public "
        "visibility. The cited test adds the accessor: a type-level coercion that "
        "only holds if `Spec::carry(&self) -> &Carry` exists with that exact "
        "signature.",
        "rung/tests/compile_pass.rs::test_carry_accessor_exists",
    ),
    "emitted-rung-structs": (
        "enforced",
        "The seal and the thread-binding, which are the two clauses a host can "
        "lose silently. The cited test uses autoref specialization to assert "
        "`!Send` for rungs *and* verdicts; the `_seal` field is what "
        "`spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624` "
        "pins. Constructor visibility follows [G2](rung-props.md#g2-sealed-construction).",
        "rung/tests/compile_pass.rs::test_rungs_are_not_send_or_sync",
    ),
    "emitted-verdict-structs": (
        "enforced",
        "All three shapes in one run: `Exhausted::new()` is the bare terminal "
        "marker, `Converged(Report)` is a terminal carrying a payload read back "
        "out through `.payload()`, and `Iterating => Active` is a recoverable "
        "verdict built from its source rung and unwrapped with `.into_source()`. "
        "The fourth clause — that a continue arm emits **no** verdict struct — is "
        "`end_to_end.rs::continue_arm_loops_without_a_recover_fn`.",
        "rung/tests/end_to_end.rs::drives_to_convergence",
    ),
    "emitted-step-outcome": (
        "enforced",
        "The clause that distinguishes `StepOutcome` from an ordinary verdict "
        "enum: a continue arm's variant carries a **live target rung**, not a "
        "verdict marker. The cited test reassigns that rung straight back into "
        "the driver, with no recover function and no guard in between.",
        "rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn",
    ),
    "emitted-failed": (
        "enforced",
        "The cited test takes the error path and reads both fields back — the "
        "unconsumed `token` and the `error` string — which is what makes "
        "`Failed<Prev>` a recovery vehicle rather than a discarded value.",
        "rung/tests/end_to_end.rs::recovers_from_the_failed_error_path",
    ),
    "emitted-guards": (
        "enforced",
        "`must_progress` is the one an author cannot see: the cited ladder's "
        "recover body contains no call to it and panics anyway, because the macro "
        "wrapped the body ([G8](rung-props.md#g8-recovery-progress)). The other "
        "two guards are pinned the same way, at "
        "`gate_markers.rs::a_body_that_ignores_the_token_still_gets_the_binding_check` "
        "and `::a_body_that_ignores_the_pen_still_gets_the_standing_check`.",
        "rung/tests/end_to_end.rs::recover_guard_is_auto_injected",
    ),
    "emitted-functions": (
        "enforced",
        "One `pub fn` per transition and per recover edge, expanded *inside* the "
        "module: the cited bodies call `Active::new`, which is private to the "
        "module and unreachable from the test file. A body expanded outside would "
        "not compile. The type-only case — no `impl` block, no functions — is "
        "`compile_pass.rs::a_marker_on_a_type_only_declaration_is_inert`.",
        "rung/tests/end_to_end.rs::drives_to_convergence",
    ),
    "unmarked-signature": (
        "enforced",
        "The driver calls `opt::active(spec)` with one argument. An unmarked "
        "transition that grew a second parameter is E0061 at that call site — "
        "which is the same diagnostic "
        "`gate_markers.rs::calling_a_judgmental_transition_without_a_token_is_e0061` "
        "pins from the other side.",
        "rung/tests/end_to_end.rs::drives_to_convergence",
    ),
    "judgmental-signature": (
        "enforced",
        "The cited test coerces the emitted `fn` to "
        "`fn(review::Spec, Qualified<Reviewer>) -> review::Active`, so an absent, "
        "extra, or differently-typed second parameter fails to compile. The "
        "injected prologue is separately pinned by "
        "`gate_markers.rs::the_injected_prologue_refuses_a_transferred_token_the_body_never_reads`, "
        "whose ladder never reads the token.",
        "rung/tests/gate_markers.rs::judgmental_transition_takes_a_qualified_token",
    ),
    "authorial-signature": (
        "enforced",
        "The authorial mirror, coerced the same way to "
        "`fn(revision::Filed, Authorized<'_, Curator>) -> revision::Revised`, with "
        "the standing prologue pinned by "
        "`gate_markers.rs::the_injected_prologue_refuses_a_pen_for_another_container_the_body_never_reads`.",
        "rung/tests/gate_markers.rs::authorial_transition_takes_an_authorized_pen",
    ),
    "body-name-resolution": (
        "enforced",
        "The cited bodies name `Active`, `StepOutcome`, `Converged` and `Carry` "
        "unqualified, and `LoopState`/`Report` from the surrounding scope through "
        "the emitted `use super::*`. Dropping either half leaves an unresolved "
        "name at expansion.",
        "rung/tests/end_to_end.rs::drives_to_convergence",
    ),

    # ── 4 · Guarantees — the one the document does not name a test for ───
    "g7-recover-pairing": (
        "enforced",
        "Rules 4–7, one `trybuild` case each. The cited one is the first "
        "direction (a recoverable verdict with no edge); "
        "`::a_recover_edges_target_must_be_a_declared_rung`, "
        "`::a_terminal_verdict_may_not_carry_a_recover_edge` and "
        "`::a_recover_edge_must_name_a_declared_verdict` are the rest. This "
        "guarantee said *(macro — static checks.)* and named no test, so it was "
        "the one guarantee of the fourteen with nothing behind it.",
        "rung/tests/spec_refusals.rs::a_recoverable_verdict_without_a_recover_edge_is_refused",
    ),

    # ── 5 · Non-guarantees ───────────────────────────────────────────────
    "non-guarantees": (
        "out-of-scope",
        "The heading of the withdrawals. A non-guarantee states that the macro "
        "does **not** enforce something and that a claim it does has no standing; "
        "there is no obligation left for a host to discharge. Its children point "
        "at the boundary tests where a boundary exists.",
        "—",
    ),
    "transition-body-correctness": (
        "out-of-scope",
        "The typestate/verification boundary. The type proves a transition ran; "
        "nothing here claims its logic was valid, so there is nothing to check. "
        "Every `expressible` row in this ledger inherits this limit.",
        "—",
    ),
    "cross-crate-provenance": (
        "out-of-scope",
        "A rung crossing a crate boundary is trusted, like any Rust public API. "
        "Closing it needs a sub-crate per ladder, which is a packaging decision "
        "rather than a macro guarantee.",
        "—",
    ),
    "same-module-fabrication": (
        "out-of-scope",
        "The module-boundary limit Rust always has. The cited test pins where the "
        "seal *does* bite — external construction is E0624 — so the withdrawal is "
        "readable as a boundary rather than as an absence.",
        "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624",
    ),
    "drop-proofing-beyond-the-lint": (
        "out-of-scope",
        "`mem::forget`, `let _ = token`, and a dropped container all bypass "
        "`#[must_use]`; true no-drop needs language-level linear types. The cited "
        "test pins the lint's actual reach, which is what is being bounded.",
        "rung/tests/spec_refusals.rs::dropping_a_verdict_under_deny_must_use_is_an_error",
    ),
    "a-cycle-through-an-authorial-act-cannot-close": (
        "out-of-scope",
        "A limit on the DECLARATION, recorded rather than worked around. "
        "`ladder!` declares a linear spine with backward continue arms, and a "
        "continue arm's target rung is built inline by `step` "
        "([G10](rung-props.md#g10-continue-arms)) — by whoever holds that "
        "transition's token. An `Accept -> Governed` arm on the pass would "
        "therefore have the JUDGE produce the revised subject, which "
        "{#no-amending-disposition} forbids. So `enact` sits outside the "
        "branching transition and the loop of {#enact-makes-an-endofunctor} "
        "closes by composition, not inside one declaration. The cited test is "
        "the shape as built: `Accept` is terminal and carries a `Licence`, and "
        "the run leaves the ladder to enact. Expressing the composite as a "
        "declaration is Q4 "
        "(`docs/questions/open/q4-composition-nested-ladders.md`), open — "
        "nothing here is claimed to close it.",
        "rung-het/tests/pass_ladder.rs::the_pass_runs_end_to_end_as_a_ladder",
    ),
    "liveness-beyond-the-guard": (
        "out-of-scope",
        "[G8](rung-props.md#g8-recovery-progress) catches an identical-token "
        "stall; general forward progress is a halting question. The cited test "
        "exercises the guard on exactly the case it does catch, so what is being "
        "withdrawn is legible as the complement of something real.",
        "rung/tests/end_to_end.rs::must_progress_guard_panics_on_no_progress",
    ),
    "gate-faithfulness-not-secured": (
        "parked",
        "The gap has a case now. rung checks the way **in** — G12 the signature, "
        "G13 the token's binding to π(a), G14 the authorial mirror — and nothing "
        "on the way out. The parked test is the demonstration: a judgmental arrow "
        "returning a value that carries π(a) itself passes every check rung "
        "makes. See {#returned-value-unconstrained}.",
        "rung/tests/gate_markers.rs::a_judgmental_arrow_may_not_return_the_provenance_it_judged",
    ),
    "one-gate-unimplemented": (
        "enforced",
        "The refusal itself is enforced, which is the honest reading: "
        "`#[conditional(..)]` is a parse-time `compile_error!` naming the open "
        "question, pinned by a `trybuild` snapshot. What is *not* secured is "
        "gate-faithfulness for an algebra that has a conditional operation — that "
        "algebra cannot be written here at all, and the refusal is what says so.",
        "rung/tests/gate_markers.rs::conditional_is_refused_and_names_the_open_question",
    ),
    "returned-value-unconstrained": (
        "parked",
        "`Prov::contained_in` exists and no guarantee calls it — the proposition "
        "says so, and now a test does. The parked case needed no rigging: the "
        "gate-marker suite's own `Review` ladder has violated "
        "{#admissibility-subcategories} since markers "
        "landed, because `active` is `#[judgmental(Reviewer)]` and returns a "
        "value declaring the provenance of the argument it judged.",
        "rung/tests/gate_markers.rs::a_judgmental_arrow_may_not_return_the_provenance_it_judged",
    ),
    "decidable-is-not-pure": (
        "out-of-scope",
        "rung has no effect system. The unmarked signature excludes Het's "
        "outside — there is no parameter a principal could enter through — and "
        "says nothing about clocks, files, or sockets. Het states the same limit "
        "independently ({#purity-not-secured}).",
        "—",
    ),
    "type-only-marker-is-inert": (
        "enforced",
        "A declaration with no `impl` block emits no transition functions, so a "
        "marker on one has no signature to change. The cited test states that as "
        "something the compiler checks: the marked role type does **not** "
        "implement `Role`, and the declaration compiles anyway — which it could "
        "not if the marker were emitting a `Qualified<R>` parameter or a "
        "prologue.",
        "rung/tests/compile_pass.rs::a_marker_on_a_type_only_declaration_is_inert",
    ),
    "gate-faithfulness-answered-no": (
        "out-of-scope",
        "A claim about an argument, not about the host: it records that Q11 is "
        "open and answered *no*. The two things it stays open on are "
        "{#one-gate-unimplemented}, which is `enforced` as a refusal, and "
        "{#returned-value-unconstrained}, which is `parked`. Both carry their own "
        "row; this one carries the reasoning.",
        "docs/questions/open/q11-gate-faithfulness.md",
    ),

    # ── 6 · Conformance ──────────────────────────────────────────────────
    "conformance-suite": (
        "enforced",
        "\"A change that violates any guarantee MUST break at least the cited "
        "test\" is only a claim if the citation is live. `./_ledger.py check` "
        "regenerates every row from the propositions documents and fails when a "
        "cited file is missing or a cited `fn` has been renamed away, so a "
        "guarantee cannot quietly lose its test.",
        "docs/_ledger.py",
    ),
    "compile-fail-asserts-only-non-compilation": (
        "out-of-scope",
        "A fact about rustdoc — it ignores the `E0NNN` in a `compile_fail` fence "
        "— rather than an obligation on the host. What follows *from* it is "
        "{#no-guarantee-cites-a-compile-fail-doctest}, and that is enforced.",
        "—",
    ),
    "no-guarantee-cites-a-compile-fail-doctest": (
        "enforced",
        "`./_ledger.py check` refuses any conformance citation that points into a "
        "crate's `src/`, which is the only place a doctest can live. A row that "
        "tried to rest on a `compile_fail` fence is a ledger failure rather than "
        "a reviewer's catch. Refusals are pinned by `trybuild` cases in "
        "`rung/tests/ui/`, whose committed `.stderr` makes the message part of "
        "the assertion.",
        "docs/_ledger.py",
    ),
    "two-silent-doctest-traps": (
        "out-of-scope",
        "Two ways to write a doctest that passes while asserting nothing. "
        "Guidance for authors of examples; the guarantees do not rest on "
        "doctests at all ({#no-guarantee-cites-a-compile-fail-doctest}).",
        "—",
    ),
    "a-refusal-test-that-cannot-fail": (
        "out-of-scope",
        "The mutation discipline itself — make the guarded thing legal and watch "
        "the case go red. It is a rule about how evidence is produced, and no "
        "machine performs it; a checker that could would be the guarantee.",
        "—",
    ),

    # ── 7 · Design judgments ─────────────────────────────────────────────
    "design-judgments": (
        "out-of-scope",
        "The document says of this subtree that **no machine decides them** and "
        "that they carry no conformance test. They bind design decisions — where "
        "a ladder stops, what earns a place in `rung-std` — and are amended as "
        "rulings on the record rather than checked.",
        "—",
    ),
    "j1-where-the-tower-bottoms-out": (
        "out-of-scope",
        "A judgment about leverage: extend the tower while structural enforcement "
        "still buys correctness gains. Nothing in a run can answer it.",
        "—",
    ),
    "j2-what-belongs-in-rung-std": (
        "out-of-scope",
        "A judgment about recurrence and canonicity. A test could count "
        "dependents; it could not decide whether the canonical statement is "
        "better than a project's own derivation.",
        "—",
    ),
}


def parse(path):
    """Return [(num, slug, section, body)] for one propositions document."""
    lines = path.read_text().split("\n")
    out, section = [], ""
    for i, line in enumerate(lines):
        if line.startswith("## "):
            section = re.sub(r"^## (?:[\dA-Z.]+ · )?", "", line).strip()
            continue
        m = re.match(r'^<a id="([a-z0-9-]+)"', line)
        if not m:
            continue
        n = re.match(r"^\*\*([A-Z]?[\d.]+)\*\*", lines[i + 1])
        if not n:
            continue
        # the proposition's body runs to the next anchor or heading
        body = []
        for nxt in lines[i + 1:]:
            if re.match(r'^<a id="', nxt) or nxt.startswith("## "):
                break
            body.append(nxt)
        out.append((n.group(1), m.group(1), section, "\n".join(body)))
    return out


TEST_REF = re.compile(r"\b([\w./-]+\.rs)::(\w+)")


def resolve_test_path(name):
    """A guarantee names its test by bare filename; the ledger cites the path."""
    if "/" in name:
        return name
    hits = [p for p in ROOT.rglob(name) if "target" not in p.parts]
    return str(hits[0].relative_to(ROOT)) if len(hits) == 1 else name


def derive_from_body(num, slug, body):
    """rung's own propositions carry their conformance in the text.

    A guarantee that names a test is `enforced` and cites it; one delegated to
    rustc is `enforced` and says so. Everything else is `unclassified` — which
    is the point: an unclassified row is a proposition no test is known to
    protect, and the count is a worklist rather than a reassurance.
    """
    if "*(rustc" in body:
        return ("enforced", "delegated to the Rust compiler.", "(rustc)")
    m = TEST_REF.search(body)
    if m and "Conformance" in body:
        return (
            "enforced",
            f"stated at [{num}](rung-props.md#{slug}), which names its own test.",
            f"{resolve_test_path(m.group(1))}::{m.group(2)}",
        )
    return ("unclassified", "no test is known to protect this proposition.", "—")


def guarantee_index():
    """`G2` -> (slug, conformance) read from rung-props.md.

    The CT account says of itself that every claim either names a guarantee a
    test protects, or is marked a limit. This is the lookup that makes the
    first half of that checkable rather than asserted.
    """
    path = HERE / "rung-props.md"
    if not path.exists():
        return {}
    out = {}
    for num, slug, _, body in parse(path):
        if not re.fullmatch(r"G\d+", num):
            continue
        verdict, _, conf = derive_from_body(num, slug, body)
        out[num] = (slug, conf if verdict == "enforced" else "—")
    return out


GUARANTEES = None


def derive_ct(num, slug, body):
    """A CT proposition that names a guarantee inherits that guarantee's test."""
    global GUARANTEES
    if GUARANTEES is None:
        GUARANTEES = guarantee_index()
    named = [g for g in re.findall(r"\bG(\d{1,2})\b", body) if f"G{g}" in GUARANTEES]
    if not named:
        return ("out-of-scope", "mathematics of the category — no host obligation", "—")
    labels = sorted({f"G{g}" for g in named}, key=lambda g: int(g[1:]))
    links = ", ".join(f"[{g}](rung-props.md#{GUARANTEES[g][0]})" for g in labels)
    confs = [GUARANTEES[g][1] for g in labels if GUARANTEES[g][1] not in ("—",)]
    return (
        "enforced",
        f"the categorical content of {links}; that guarantee's test is what fails.",
        confs[0] if confs else "(rustc)",
    )


DOCS = (
    {
        "file": "rung-props.md",
        "heading": "rung — the ladder language",
        "blurb": (
            "The guarantees name their own conformance tests, so those rows are "
            "**derived from the document**, not curated here. Every other "
            "proposition — the grammar, the static-semantics rules, the emitted "
            "artifacts, the non-guarantees, the conformance discipline, the "
            "design judgments — is curated in `_ledger.py`. A proposition added "
            "to the document and to neither place lands as `unclassified`, which "
            "fails `check`: a new normative claim cannot enter without a verdict."
        ),
        "default": ("unclassified", "no test is known to protect this proposition.", "—"),
        "curated": CURATED_RUNG,
        "derive": derive_from_body,
    },
    {
        "file": "rung-ct-props.md",
        "heading": "rung-CT — the category",
        "blurb": (
            "**The default is `out-of-scope`.** Most propositions state the "
            "mathematics of the category and impose no obligation on any host; "
            "the ones that do bind name a guarantee, and that guarantee's row "
            "under rung above carries the test. Read a bare `out-of-scope` as "
            "*\"no claim made\"*, not as *\"checked and found irrelevant\"*."
        ),
        "default": ("out-of-scope", "mathematics of the category — no host obligation", "—"),
        "curated": CURATED_CT,
        "derive": derive_ct,
    },
    {
        "file": "rung-het-props.md",
        "heading": "Het — the formalism",
        "blurb": (
            "**No Het theory is currently expressed as a `ladder!`.** `rung-het` "
            "has empty `[dependencies]` and zero occurrences of `ladder!`; its "
            "guarantees come from hand-rolled sealed structs. Every `enforced` "
            "row below therefore names the rung guarantee that **will** apply "
            "once the pass is a ladder — not one in force today.\n\n"
            "**The default is `out-of-scope`.** Most propositions state the "
            "structure of the institution and impose no obligation on any host. "
            "Only rows that name a mechanism have been curated; the rest are "
            "marked by that rule, not by inspection. Read a bare `out-of-scope` "
            "as *\"no claim made\"*, not as *\"checked and found irrelevant\"*."
        ),
        "default": DEFAULT,
        "curated": CURATED,
        "derive": None,
    },
)

VERDICTS = VERDICTS | {"unclassified"}


def render():
    """Return (text, tally, total). Pure: `gen` writes it, `check` diffs it."""
    tally = {v: 0 for v in VERDICTS}
    total = 0
    parts, summary = [], []

    # `{#slug}` resolves across all three documents, not only the one the row
    # sits in. Three documents share one slug space by design, and a mechanism
    # that had to know which file its referent lived in would be a hardcode of
    # the same kind the slug keying exists to avoid.
    where = {}
    for spec in DOCS:
        p = HERE / spec["file"]
        if p.exists():
            for num, slug, _, _ in parse(p):
                where[slug] = (spec["file"], num)

    for spec in DOCS:
        path = HERE / spec["file"]
        if not path.exists():
            continue
        rows = parse(path)
        sub = {v: 0 for v in VERDICTS}
        body, cur = [], None

        for num, slug, section, text in rows:
            if section != cur:
                cur = section
                body.append(f"\n### {section}\n")
                body.append("| prop | slug | verdict | mechanism | conformance |")
                body.append("|---|---|---|---|---|")
            if slug in spec["curated"]:
                verdict, mech, conf = spec["curated"][slug]
            elif spec["derive"]:
                verdict, mech, conf = spec["derive"](num, slug, text)
            else:
                verdict, mech, conf = spec["default"]
            # an unresolvable {#slug} is left verbatim for `check` to report
            mech = CITE.sub(
                lambda m: "[{}]({}#{})".format(
                    where[m.group(1)][1], where[m.group(1)][0], m.group(1)
                )
                if m.group(1) in where
                else m.group(0),
                mech,
            )
            tally[verdict] += 1
            sub[verdict] += 1
            total += 1
            body.append(
                f"| [{num}]({spec['file']}#{slug}) | `{slug}` | `{verdict}` | {mech} | {conf} |"
            )

        counts = " · ".join(f"{n} {v}" for v, n in sorted(sub.items()) if n)
        summary.append(f"| [`{spec['file']}`]({spec['file']}) | {len(rows)} | {counts} |")
        parts.append(
            f"\n## {spec['heading']}\n\n{spec['blurb']}\n\n"
            f"**Counts.** {counts} · {len(rows)} total.\n" + "\n".join(body)
        )

    head = f"""# Conformance

**Status: not normative.** The three `*-propositions.md` documents govern. This
ledger records where each of their propositions is enforced, and where it is
not.

Rows are keyed on the proposition's **slug**, not its number, so the ledger
survives every renumbering. Generated by `./_ledger.py gen`; verified by
`./_ledger.py check`, which regenerates the file and fails on any difference —
so a proposition cannot go missing, a slug cannot go unknown, an `enforced` row
cannot cite a file that does not exist, and this text cannot be edited by hand.

| verdict | meaning |
|---|---|
| `enforced` | a rung guarantee makes the proposition hold, and a named test fails if it stops |
| `expressible` | encodable in a ladder; rung proves it ran, not that it was right |
| `parked` | a real gap, pinned by an `#[ignore]`d test whose reason names what would close it |
| `collides` | contradicts a rung guarantee — must be empty |
| `out-of-scope` | mathematics of the account; nothing for a host to enforce |
| `unclassified` | no verdict recorded — a worklist entry, not a clean bill |

**The join is not onto, in either direction.** A proposition may have no test,
and a guarantee may have no proposition. Two guarantees have no Het
proposition at all:

- **G1, linear consumption.** An argument is consumed by the arrow that acts on
  it. Het says an object is transformed, never that the prior object is spent.
- **G3, one token one thread.** Het is silent on where a judgment runs.

Neither is a defect; both are places where the host is stricter than the
formalism requires.

| document | propositions | verdicts |
|---|---|---|
{chr(10).join(summary)}

**Total.** {total} propositions across {len(summary)} documents.
"""
    return head + "\n".join(parts) + "\n", tally, total


def gen():
    text, tally, n = render()
    LEDGER.write_text(text)
    return tally, n


def check():
    errs = []
    text, _, _ = render()
    all_slugs, curated_all = set(), {}
    for spec in DOCS:
        path = HERE / spec["file"]
        if not path.exists():
            continue
        for _, slug, _, _ in parse(path):
            all_slugs.add(slug)
        for slug, row in spec["curated"].items():
            curated_all[slug] = (spec["file"], row)

    listed = re.findall(
        r"^\| \[[A-Z]?[\d.]+\]\(([\w.-]+)#([a-z0-9-]+)\) \| `([a-z0-9-]+)` \| `([a-z-]+)` \|",
        text, re.M,
    )
    seen = {}
    for _, href, slug, verdict in listed:
        if href != slug:
            errs.append(f"row `{slug}`: link target #{href} does not match the slug")
        if verdict not in VERDICTS:
            errs.append(f"row `{slug}`: unknown verdict `{verdict}`")
        seen[slug] = seen.get(slug, 0) + 1

    for slug, n in seen.items():
        if slug not in all_slugs:
            errs.append(f"row `{slug}`: not a proposition of any governing document")
        if n > 1:
            errs.append(f"row `{slug}`: listed {n} times")
    for slug in all_slugs:
        if slug not in seen:
            errs.append(f"proposition `{slug}` is not classified in the ledger")

    for slug, (doc, (verdict, mech, conf)) in curated_all.items():
        if slug not in all_slugs:
            errs.append(f"CURATED names `{slug}`, which is not a proposition")
        for m in CITE.finditer(mech):
            if m.group(1) not in all_slugs:
                errs.append(f"row `{slug}`: cites {{#{m.group(1)}}}, which is not a proposition")
        for m in re.finditer(r"(?<![\w.#/§-])(\d{1,2}\.\d{1,2})(?![\w-]|\.\d)", mech):
            errs.append(
                f"row `{slug}`: bare number {m.group(1)} — write it as {{#slug}}, "
                f"which survives renumbering"
            )

    # every cited test must exist. `(rustc)` and `—` cite no file by design.
    for _, _, slug, verdict in listed:
        pass
    for line in text.split("\n"):
        m = re.match(r"^\| \[[A-Z]?[\d.]+\]\([\w.-]+#([a-z0-9-]+)\) \| `[a-z0-9-]+` \| `([a-z-]+)` \| .* \| (.+) \|$", line)
        if not m:
            continue
        slug, verdict, conf = m.group(1), m.group(2), m.group(3).strip()
        if conf in ("—", "(rustc)"):
            # Every row that makes a claim about the host must point at
            # something a run can check. `out-of-scope` is the only verdict
            # that asserts no host obligation, so it is the only one exempt.
            if verdict != "out-of-scope" and conf == "—":
                errs.append(
                    f"row `{slug}`: `{verdict}` must cite a conformance test, "
                    f"a parked test, or an open question"
                )
            continue
        t = re.match(r"^(\S+?)::(\w+)$", conf)
        path, sym = (t.group(1), t.group(2)) if t else (conf.split(" ")[0], None)
        target = ROOT / path
        if not target.exists():
            errs.append(f"row `{slug}`: cites {path}, which does not exist")
            continue
        if sym and not re.search(rf"\bfn {re.escape(sym)}\b", target.read_text()):
            errs.append(f"row `{slug}`: {path} has no fn {sym}")
            continue

        # rung-props.md §6.2 — no guarantee may rest on a `compile_fail`
        # doctest. rustdoc ignores the error code in the fence, so such a
        # doctest asserts only "this did not compile" and cannot tell the
        # refusal it was written for from a typo. Doctests live in `src/`;
        # `trybuild` cases, which diff a committed `.stderr`, live in `tests/`.
        if "/src/" in f"/{path}":
            errs.append(
                f"row `{slug}`: cites {path}, which is inside a crate's `src/` — "
                f"a conformance citation may not be a doctest "
                f"(rung-props.md#no-guarantee-cites-a-compile-fail-doctest)"
            )

        # A `parked` row must cite a test that is actually parked, so that the
        # gap answers back when the attribute comes off.
        if verdict == "parked":
            if not (path.endswith(".rs") and sym):
                errs.append(
                    f"row `{slug}`: `parked` must cite a test as `file.rs::fn`, "
                    f"not `{conf}` — a gap is pinned by something a run reports"
                )
            else:
                body = target.read_text()
                attrs = body.split(f"fn {sym}")[0].rsplit("\n}\n", 1)[-1]
                if "#[ignore" not in attrs:
                    errs.append(
                        f"row `{slug}`: `parked` cites {conf}, which carries no "
                        f"`#[ignore = \"..\"]` — either the gap closed and the "
                        f"row is now `enforced`, or the test does not pin it"
                    )

    for _, _, slug, verdict in listed:
        if verdict == "collides":
            errs.append(f"row `{slug}`: verdict `collides` — an unresolved contradiction")

    on_disk = LEDGER.read_text() if LEDGER.exists() else None
    if on_disk is None:
        errs.append("conformance.md does not exist — run ./_ledger.py gen")
    elif on_disk != text:
        for line in difflib.unified_diff(
            on_disk.split("\n"), text.split("\n"),
            "conformance.md", "generated", lineterm="", n=0,
        ):
            errs.append(line)
        errs.append("conformance.md is stale or hand-edited — run ./_ledger.py gen")

    if errs:
        print("\n".join(f"  {e}" for e in errs), file=sys.stderr)
        print(f"\n{len(errs)} problem(s)", file=sys.stderr)
        return 1
    print(f"ok — {len(listed)} propositions classified, all slugs and cited files resolve")
    return 0


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "gen":
        tally, n = gen()
        print(f"wrote conformance.md — {n} rows: " +
              ", ".join(f"{k}={v}" for k, v in sorted(tally.items()) if v))
        sys.exit(0)
    sys.exit(check())
