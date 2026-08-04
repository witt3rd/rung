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

VERDICTS = {"enforced", "expressible", "deferred", "collides", "out-of-scope"}
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
        "({#conformance-half-needs-no-judge}). rung enforces *that the token was "
        "constructed*, never that the body computed the set "
        "correctly — SPEC §5, transition-body correctness.",
        "—",
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
        "expressible",
        "One `ladder!` declaration. Rungs are the pass's positions; the branching "
        "transition is `dispose`; verdict arms are the Disposition vocabulary.",
        "—",
    ),
    "disposition-vocabulary": (
        "enforced",
        "G6 exhaustive outcomes. `StepOutcome` is an enum, so every match site must "
        "handle all five; adding a disposition breaks every call site at compile time.",
        "rung/tests/compile_pass.rs::test_verdict_enum",
    ),
    "disposition-is-a-ruling": (
        "enforced",
        "G2. `dispose` returns a verdict; only the separately-declared authorial arrow "
        "produces the revised object. A ruling cannot construct what it rules on.",
        "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624",
    ),
    "no-amending-disposition": (
        "enforced",
        "G2, same mechanism. A judge's arrow has no constructor for the authored "
        "object, so an amending disposition is not expressible.",
        "rung/tests/spec_refusals.rs::external_construction_of_a_mid_ladder_rung_is_e0624",
    ),
    "reproposal-carries-the-chain": (
        "expressible",
        "The chain rides in the rung payload. NOTE: this is exactly what would make "
        "a G8 progress guard vacuous — a strictly growing chain never compares equal "
        "— which is why re-entry must not use a guarded edge "
        "({#guarded-reentry-is-eviction}).",
        "—",
    ),
    "enact-makes-an-endofunctor": (
        "expressible",
        "`enact` is a forward transition returning the revised object's rung. rung "
        "enforces that it ran, not that the edit was right (SPEC §5).",
        "—",
    ),
    "remedy-carries-an-edit": (
        "expressible",
        "The edit is the rung payload's type, supplied by the theory. G10's continue "
        "arm carries its target rung live, so the edit type never leaves the ladder.",
        "—",
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
        "expressible",
        "A continue arm loops with no host-imposed bound, which is what this "
        "proposition requires. Choosing a guarded edge instead would supply a bound "
        "Het declines to declare.",
        "rung/tests/end_to_end.rs::continue_arm_loops_without_a_recover_fn",
    ),

    # ── the residual ─────────────────────────────────────────────────────
    "epsilon-reported-with-verdict": (
        "deferred",
        "GAP — `Verdict` is Boolean (`Conforming | NonConforming`). No metric, no "
        "epsilon, so the satisfaction condition does not survive renaming "
        "({#boolean-breaks-satisfaction}).",
        "—",
    ),
    "no-preference-among-judges": (
        "expressible",
        "UNARGUED in both doctrine and code. `Pool::qualify` returns the FIRST "
        "qualifying principal. Het says any qualifying judge yields a well-formed "
        "verdict, so a deterministic pick is admissible — but whether pool position "
        "constitutes an ordering has not been argued either way. Assumed, not shown.",
        "—",
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

    # ── deferred on open questions ───────────────────────────────────────
    "panels": (
        "deferred",
        "Q5 (fork-join concurrency, open). A panel splits one argument across N "
        "concurrent oracle calls; rung has no fork-join primitive.",
        "docs/questions/open/q5-fork-join-concurrency.md",
    ),
    "panels-cannot-weaken-the-opponent": (
        "deferred",
        "Q5 (fork-join concurrency, open).",
        "docs/questions/open/q5-fork-join-concurrency.md",
    ),
    "judgmental-is-kleisli-arrow": (
        "deferred",
        "Q8 (async driver, open) for the async case only. A *blocking* outside call "
        "works today — rung-std's `LlmCall` ladder puts the call on the arrow — so "
        "this is a constraint, not a blocker.",
        "docs/questions/open/q8-async-driver.md",
    ),
    "target-runs-its-own-models": (
        "deferred",
        "Q4 (composition / nested ladders, open). The pass composed with itself "
        "across a container boundary is ladder-in-ladder.",
        "docs/questions/open/q4-composition-nested-ladders.md",
    ),
    "gate-faithful": (
        "deferred",
        "Q11 (gate-faithfulness, open) — all three rows of Q11's table are now "
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
        "falsifier.",
        "docs/questions/open/q11-gate-faithfulness.md",
    ),
    "mod-only-gate-faithful": (
        "deferred",
        "Q11 (gate-faithfulness, open). Follows gate-faithful.",
        "docs/questions/open/q11-gate-faithfulness.md",
    ),

    # ── structural correspondences worth recording ───────────────────────
    "fractal-property": (
        "expressible",
        "The composite Grothendieck opfibration "
        "([opfibrations-compose](rung-ct-props.md#opfibrations-compose)), "
        "resolved by Q10. The correspondence is proved; no registry hierarchy is built.",
        "docs/questions/resolved/q10-fractal-registry-hierarchy.md",
    ),
    "two-directions-two-bases": (
        "expressible",
        "Conformance is Het's fibration (Mod: Sign^op → Cat, contravariant). "
        "Propagation is rung-CT's opfibration, pushforward and opcartesian "
        "([conformance-and-propagation-run-over-different-bases]"
        "(rung-ct-props.md#conformance-and-propagation-run-over-different-bases)). "
        "Different bases at adjacent levels — not opposite orientations of one tower.",
        "—",
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
        "library — `rung-het` has no edge type and `EdgeKind` lives in the theory "
        "that governs `docs/questions/`, exactly where an edit vocabulary sits "
        "(`edit-required-not-typed`). What the cited test pins is the "
        "**lived-instance** discipline that keeps the taxonomy the theory's: each "
        "of the seven declared kinds must have a real user in `docs/questions/`, "
        "and a speculative eighth fails. NOT enforced: nothing in rung could stop "
        "a future library enumerating edge types — the location is a choice this "
        "theory makes, and the test protects the discipline rather than the "
        "choice.",
        "rung-het/tests/question_registry.rs::every_declared_edge_kind_has_a_lived_instance_in_the_registry",
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
        "rung-het/tests/question_registry.rs::a_strict_edge_propagates_decidably_and_an_advisory_edge_is_ruled_on",
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
            "artifacts — is `unclassified` until someone names a test for it."
        ),
        "default": ("unclassified", "no test is known to protect this proposition.", "—"),
        "curated": {},
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

    for spec in DOCS:
        path = HERE / spec["file"]
        if not path.exists():
            continue
        rows = parse(path)
        num_of = {slug: num for num, slug, _, _ in rows}
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
                lambda m: f"[{num_of[m.group(1)]}]({spec['file']}#{m.group(1)})"
                if m.group(1) in num_of
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
| `deferred` | blocked on a named open question, or on a named gap |
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
            if verdict == "enforced" and conf == "—":
                errs.append(f"row `{slug}`: `enforced` must cite a conformance test")
            continue
        t = re.match(r"^(\S+?)::(\w+)$", conf)
        path, sym = (t.group(1), t.group(2)) if t else (conf.split(" ")[0], None)
        target = ROOT / path
        if not target.exists():
            errs.append(f"row `{slug}`: cites {path}, which does not exist")
        elif sym and not re.search(rf"\bfn {re.escape(sym)}\b", target.read_text()):
            errs.append(f"row `{slug}`: {path} has no fn {sym}")

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
