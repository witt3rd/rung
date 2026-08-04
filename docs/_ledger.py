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
DOC = HERE / "rung-het-propositions.md"
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
        "sealed constructor — [the law](rung-ct-propositions.md#the-law). The "
        "algebra runs its own decidable "
        "step; it cannot construct the state that holds a judgmental outcome.",
        "rung/src/lib.rs — compile_fail doctest, external `Active::new` → E0624",
    ),
    "constant-arrow-hazard": (
        "enforced",
        "G2 sealed construction. A judgmental arrow cannot be interpreted by a "
        "constant drawn from the algebra's own carrier, because no mid-ladder rung "
        "is constructible outside its module.",
        "rung/src/lib.rs — compile_fail doctest, external `Active::new` → E0624",
    ),
    "non-identity-by-construction": (
        "deferred",
        "GAP — the token is unforgeable but UNBOUND. `rung-het`'s `Qualified<R>` "
        "records the principal and its provenance and forgets the argument it was "
        "measured against (`rung/src/lib.rs:312-318`), so a token earned against "
        "one argument can be spent on another. Sealing the constructor closes "
        "fabrication, not transfer. Binding the token is the substrate rewrite's job.",
        "—",
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
        "rustc. The two gates differ in the arity of the emitted transition; the "
        "host's type system separates them with no knowledge of Het.",
        "rung/tests/compile_pass.rs::test_module_exists",
    ),
    "decidable-cannot-consult-pool": (
        "enforced",
        "G2. The qualifying token has no constructor reachable from a decidable "
        "body, so the prohibition is a term that cannot be written rather than a "
        "rule an author is asked to respect.",
        "rung/src/lib.rs — compile_fail doctest, external `Active::new` → E0624",
    ),
    "mismarking-is-not-a-false-claim": (
        "enforced",
        "rustc. A body needing an outside does not typecheck in a decidable position.",
        "rung/tests/compile_pass.rs::test_module_exists",
    ),
    "signature-replaces-fragment-membership": (
        "enforced",
        "rustc. The compiler does not know Het exists and cannot be persuaded — "
        "which is the whole claim of this proposition.",
        "rung/tests/compile_pass.rs::test_module_exists",
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
        "rung/src/lib.rs — compile_fail doctest, external `Active::new` → E0624",
    ),
    "no-amending-disposition": (
        "enforced",
        "G2, same mechanism. A judge's arrow has no constructor for the authored "
        "object, so an amending disposition is not expressible.",
        "rung/src/lib.rs — compile_fail doctest, external `Active::new` → E0624",
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
        "([residual-is-the-optics-residual](rung-ct-propositions.md#residual-is-the-optics-residual)) "
        "and is why the error structure is not a Kleisli arrow; the monad `P` layers "
        "on the forward pass, which rung-CT explicitly permits "
        "([effects-layer-on-the-forward-pass](rung-ct-propositions.md#effects-layer-on-the-forward-pass)).",
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
        "Q11 (gate-faithfulness, open). The ladder DSL has no gate marker, so an "
        "algebra cannot declare which arrows are judgmental and nothing checks "
        "faithfulness. The largest unclosed distance between Het and rung. Q11 "
        "splits it: a marker makes the SIGNATURE honest; binding the qualifying "
        "token is what would make the ARROW admissible. Whether the conjunction "
        "is gate-faithfulness is itself unargued.",
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
        "([opfibrations-compose](rung-ct-propositions.md#opfibrations-compose)), "
        "resolved by Q10. The correspondence is proved; no registry hierarchy is built.",
        "docs/questions/resolved/q10-fractal-registry-hierarchy.md",
    ),
    "two-directions-two-bases": (
        "expressible",
        "Conformance is Het's fibration (Mod: Sign^op → Cat, contravariant). "
        "Propagation is rung-CT's opfibration, pushforward and opcartesian "
        "([conformance-and-propagation-run-over-different-bases]"
        "(rung-ct-propositions.md#conformance-and-propagation-run-over-different-bases)). "
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
        "expressible",
        "Both filters produce tokens over one pool; the gate selects the predicate. "
        "rung sees two differently-typed tokens and nothing else.",
        "—",
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


def props():
    lines = DOC.read_text().split("\n")
    out = []
    for i, line in enumerate(lines):
        m = re.match(r'^<a id="([a-z0-9-]+)"', line)
        if m:
            n = re.match(r"^\*\*([\d.]+)\*\*", lines[i + 1])
            out.append((n.group(1), m.group(1)))
    return out


SECTIONS = {
    "1": "The relation", "2": "The gate", "3": "The pool", "4": "The verdict",
    "5": "The semantics", "6": "The tower", "7": "The game", "8": "The cut",
    "9": "Composition", "10": "Evaluation", "11": "The surface", "12": "The limit",
}


def render():
    """Return (text, tally, rows). Pure: `gen` writes it, `check` diffs it."""
    rows = props()
    num_of = dict((slug, num) for num, slug in rows)
    tally = {v: 0 for v in VERDICTS}
    body = []
    cur = None
    for num, slug in rows:
        top = num.split(".")[0]
        if top != cur:
            cur = top
            body.append(f"\n### {top} · {SECTIONS[top]}\n")
            body.append("| prop | slug | verdict | mechanism | conformance |")
            body.append("|---|---|---|---|---|")
        verdict, mech, conf = CURATED.get(slug, DEFAULT)
        # an unresolvable {#slug} is left verbatim for `check` to report
        mech = CITE.sub(
            lambda m: f"[{num_of[m.group(1)]}](rung-het-propositions.md#{m.group(1)})"
            if m.group(1) in num_of
            else m.group(0),
            mech,
        )
        tally[verdict] += 1
        body.append(
            f"| [{num}](rung-het-propositions.md#{slug}) | `{slug}` | `{verdict}` | {mech} | {conf} |"
        )

    head = f"""# Het — Conformance

**Status: not normative.**
[`rung-het-propositions.md`](rung-het-propositions.md) governs. This ledger
records where each of its propositions is enforced, and where it is not.

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
| `out-of-scope` | mathematics of the institution; nothing for a host to enforce |

**No Het theory is currently expressed as a `ladder!`.** `rung-het` has empty
`[dependencies]` and zero occurrences of `ladder!`; its guarantees come from
hand-rolled sealed structs. Every `enforced` row below therefore names the rung
guarantee that **will** apply once the pass is a ladder — not one in force
today. The mechanism underneath is bespoke until the substrate rewrite lands.

**The default is `out-of-scope`.** Most propositions state the structure of the
institution and impose no obligation on any host. Only rows that name a
mechanism have been curated; the rest are marked by that rule, not by
inspection. Read a bare `out-of-scope` as *"no claim made"*, not as *"checked
and found irrelevant"*.

**Counts.** {tally['enforced']} enforced · {tally['expressible']} expressible ·
{tally['deferred']} deferred · {tally['collides']} collides ·
{tally['out-of-scope']} out-of-scope · {len(rows)} total.

## What rung guarantees that Het does not state

The join is not onto. Two rung guarantees have no proposition:

- **G1, linear consumption.** An argument is consumed by the arrow that acts on
  it. Het says an object is transformed, never that the prior object is spent.
- **G3, one token one thread.** Het is silent on where a judgment runs.

Neither is a defect; both are places where the host is stricter than the
formalism requires.
"""
    return head + "\n".join(body) + "\n", tally, len(rows)


def gen():
    text, tally, n = render()
    LEDGER.write_text(text)
    return tally, n


def check():
    errs = []
    text, _, _ = render()
    listed = re.findall(r"^\| \[[\d.]+\]\(rung-het-propositions\.md#([a-z0-9-]+)\) \| `([a-z0-9-]+)` \| `([a-z-]+)` \|",
                        text, re.M)
    all_slugs = [s for _, s in props()]

    seen = {}
    for href, slug, verdict in listed:
        if href != slug:
            errs.append(f"row `{slug}`: link target #{href} does not match the slug")
        if verdict not in VERDICTS:
            errs.append(f"row `{slug}`: unknown verdict `{verdict}`")
        seen[slug] = seen.get(slug, 0) + 1

    for slug, n in seen.items():
        if slug not in all_slugs:
            errs.append(f"row `{slug}`: not a proposition of rung-het-propositions.md")
        if n > 1:
            errs.append(f"row `{slug}`: listed {n} times")
    for slug in all_slugs:
        if slug not in seen:
            errs.append(f"proposition `{slug}` is not classified in the ledger")

    for slug in CURATED:
        if slug not in all_slugs:
            errs.append(f"CURATED names `{slug}`, which is not a proposition")

    for slug, (_, mech, _) in CURATED.items():
        for m in CITE.finditer(mech):
            if m.group(1) not in all_slugs:
                errs.append(
                    f"row `{slug}`: cites {{#{m.group(1)}}}, which is not a proposition"
                )
        for m in re.finditer(r"(?<![\w.#/§-])(\d{1,2}\.\d{1,2})(?![\w-]|\.\d)", mech):
            errs.append(
                f"row `{slug}`: bare number {m.group(1)} — write it as {{#slug}}, "
                f"which survives renumbering"
            )

    for slug, (verdict, _, conf) in CURATED.items():
        if conf == "—":
            if verdict == "enforced":
                errs.append(f"row `{slug}`: `enforced` must cite a conformance test")
            continue
        # `path::symbol` cites a test fn; anything else cites a file plus prose
        m = re.match(r"^(\S+?)::(\w+)$", conf)
        path, sym = (m.group(1), m.group(2)) if m else (conf.split(" ")[0], None)
        target = ROOT / path
        if not target.exists():
            errs.append(f"row `{slug}`: cites {path}, which does not exist")
        elif sym and not re.search(rf"\bfn {re.escape(sym)}\b", target.read_text()):
            errs.append(f"row `{slug}`: {path} has no fn {sym}")

    collides = [s for _, s, v in listed if v == "collides"]
    for slug in collides:
        errs.append(f"row `{slug}`: verdict `collides` — an unresolved contradiction")

    # the file on disk is generated output; the only source is CURATED
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
