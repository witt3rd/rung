# Assessment: is fp.worked-instance relevant to rung now?

**Status: archived — not normative, not landed as a current guide.** This note
is the 2026-08-07 review of Het-archival content (`docs/het-worked-instance.md`
beside this file, originally the `docs/fp-worked-instance` branch) against rung
as it stands on `master`. It exists so the work is preserved and the conclusion
is on record.

## What it is

A hand-run of the audit-rectify loop on the **directedness claim** (§4.2) from
the `heteronomy` archive: a two-judge panel audited the claim, found a gap (M2
quantified over primitives only), proposed conventions C1/C2/C3, and Donald
ratified with **`accept-with-mod`** — carrying a coherence condition as
"axiom 7". Its stated purpose is to be the canonical non-conforming-branch
instance a mechanical audit-rectify engine must reproduce.

## Verdict: archive only — do not land as a current rung document

Three independent reasons, checked against `master`:

1. **Its central mechanic is now forbidden by rung's law.** The document's core
   move is `accept-with-mod` — a judge accepting a proposal *with a
   modification*. `rung-het-props.md` **7.42 (`no-amending-disposition`)**
   rules this out: a judge that amends is transforming, not classifying, and
   being provenance-disjoint from the subject it cannot hold standing over a
   modification it has just authored. The `Disposition` vocabulary is pinned at
   *exactly five*, deliberately excluding `accept-with-mod` and
   `reject-with-alternative`. The honest rung encoding of "accept with a
   change" is `reject-remedy` + a *reason* (7.43), then the author re-proposes
   carrying the change (7.44) — not the judge amending.

2. **Its §4.2 citation is stale and mis-homed.** The document cites
   `docs/rung-het-props.md §4.2` as "theory-morphism laws; M2 is
   `judg`-preserved-upward". On `master`, §4.2 is *verdict spaces* (`[0,1]`,
   simplices, strategy lattices). The directedness / M2 / axiom-7 / homotopy
   layer belongs to the `heteronomy` archive, not to rung's subject matter.

3. **Its stated purpose is already served, better, by rung itself.** The
   document exists to hand the mechanical engine a non-conforming-branch
   worked instance to reproduce. rung already has that, live and mechanical:
   `rung-driver/tests/rectify_questions.rs` / the `rectify_questions` binary
   run the actual driver over rung's own questions — audit finds the real
   `affects_mirrors_inbound` gap, an author proposes mirroring one edge,
   dispose, enact, verify. That is a real
   `audit → gap → propose → dispose → enact → verify` instance on rung's own
   subject, not a hand narrative from another archive.

## The one genuinely current thread it gestures at

A panel — `⊨` with more than one judge — is a real rung proposition
(`rung-het-props.md` 7.6 `panels`, `panels-cannot-weaken-the-opponent`), and
`rung-het/tests/panel.rs` demonstrates the mechanism. What was missing on
`master` today is that the composed audit-rectify loop (`run_cycle`) drives a
**single** judge with a hardcoded `Accept` and does **not** yet execute a
panel. That gap — composing panels into the loop so multiple outside experts
weigh in and agreement versus divergence reshapes the resolution — is the
relevant, actionable takeaway, and it is handled separately from this archive
entry (see the `feat/panel-in-loop` work).
