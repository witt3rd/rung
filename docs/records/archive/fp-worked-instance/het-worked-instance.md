# fp.worked-instance — the §4.2 directedness panel

**Ledger identifier:** `fp.worked-instance`  
**Status:** canonical. Written 2026-07-27; captured here 2026-08-07.  
**Source material:** archive in `witt3rd/.archive/heteronomy`, commit `16189e9`.  
**Purpose:** a hand-run of the full audit-rectify loop on a real element of the
theory *about* audit-rectify loops — the reference instance Forge's mechanical
engine must reproduce to prove it is not vacuous.

---

## What this is

The §4.2 directedness claim could not self-certify. It went to a two-judge
panel. In doing so, it **enacted every operation of the audit-rectify loop by
hand** — not as an analogy, but as the algebra, run on a real element of the
theory. This is the first end-to-end demonstration that the Het signature
describes work that actually closes, honestly, from outside.

---

## The element under audit

The **directedness claim** (as of 2026-07-27, prior to the panel):

> M2 monotonicity forces non-invertibility of add-an-outside morphisms —
> the category of heteronomy-theories is directed.

Specifically: if a theory-morphism `F : T → T'` strictly increases judgmental
content (maps some decidable operation to a judgmental one), then `F` cannot be
an isomorphism. The claim was a candidate proof sitting in the spec register, not
yet machine-checked. It is the `Element` the loop ran on.

---

## The loop, enacted by hand

| algebra | what actually happened |
|---|---|
| **Element** | the directedness claim, sitting in the heteronomy spec register |
| **audit** (`χ → Verdict`) | both judges read the candidate proof and returned **GAP** — the claim is non-conforming. The failure was located precisely at attack surfaces B∩C: M2 as written quantifies over primitive operations only, but morphisms in the heteronomy category map primitives to *derived* operations (composites). M2 therefore did not cover the full space it needed to. |
| **propose** | the judges returned the rectification: three conventions — **C1** (contagious gate: `judg` defined inductively, judgmental iff any constituent is), **C2** (M2 extension lemma: M2 on primitives extends to all derived operations by induction under C1), **C3** (gate invariance: `judg` is constant across axiom-equivalent terms) — plus the coherence condition that makes C1+C3 jointly consistent. |
| **dispose** (judgmental, gated) | **Donald's `accept-with-mod` ratification**: adopt C1/C2/C3, and rule the coherence condition (no axiom may equate a judgmental operation with a decidable one) as **axiom 7, the equational gate law**. |
| **the outside (P0)** | the panel — structurally separate from the thing judged. Augur authored the claim; the judges audited it; Donald ratified it. No single party closed the loop on itself. |

Every operation fired, in order, on the non-conforming branch:
`dispose ∘ propose ∘ audit`. The full composite, run manually.

---

## The two laws that held in the enactment

**P0 held.** The claim could not self-certify. Augur *recommended* directedness;
the recommendation was not the ratification. It had to go to an outside that was
not Augur. The gate refused to close from inside — exactly what axiom 7 and M2
forbid.

**The judgment was genuinely judgmental.** The judges did not compute GAP from a
fixed rule. They *read* the proof and found where it smoothed over the
primitive/derived boundary. That is `dispose` being judgmental, not decidable —
the very thing the gate marks as requiring an outside.

---

## The outcome

Commit `d6d1ef8` (witt3rd/.archive/heteronomy, 2026-07-27):

- **C1/C2/C3 adopted** into §4.1 of the algebra-spec as explicit rulings: the
  `judg` predicate pinned as contagious, M2 extended to derived operations,
  gate invariance declared.
- **Coherence condition ruled (b):** adopted as **axiom 7** — the equational
  gate law — P0 at the equational layer. The algebra-spec now has seven axioms.
- **Gate law recurrence at four levels confirmed:** object (R4 typing) →
  equation (axiom 7) → morphism (M2) → 2-cell (§4.3, then closed by C4). One
  law, four floors.

**Directedness is a DERIVED THEOREM.** With axiom 7 the repaired proof is
routine; the algebra-side directedness meets the homotopy-side Joyal directedness
as one result by two roads.

§4.2 CLOSED. §4.3 subsequently closed by C4 (2-cell level).

---

## Why this is the canonical worked instance

The panel ran the audit-rectify loop on an element of the theory *about* the
audit-rectify loop. Self-application, honest:

- The loop did not collapse into self-certification (P0 held).
- The gap was found by reading, not by computation (judgmental content stayed
  judgmental).
- The rectification was proposed by the outside and ratified by the member —
  not derived and self-applied.
- The coherence condition (axiom 7) emerged *from* the repair, not from the
  original spec — the loop surfaced a law the objects alone did not carry.

This is the reference instance for the mechanical version. When Forge builds the
Rust audit-rectify engine, **this is the run it must reproduce.** The panel is
the proof that the algebra is not vacuous — it describes work that actually
closes, honestly, from outside.

---

## Provenance chain (trace for Forge)

| artifact | location | content |
|---|---|---|
| Panel submission package | `.archive/heteronomy/spec/panels/2026-08-02_m2-directedness.md` | full proof sketch, attack surfaces A–D, both reviewer verdicts, Donald's ratification |
| Axiom 7 commit | `.archive/heteronomy` commit `d6d1ef8` | README.md updated with C1/C2/C3 and axiom 7 |
| Self-application note | `.archive/heteronomy` commit `16189e9` | the original `fp.worked-instance` capture in the README |
| M2 gate law | `docs/rung-het-props.md` §4.2 | theory-morphism laws; M2 is `judg`-preserved-upward |
| Panels proposition | `docs/rung-het-props.md` §7.6 `panels` | a panel is `⊨` with more than one judge; not a separate construction |
| Mechanical test | `rung-het/tests/panel.rs` commit `3107f39` | panel as N ordinary `dispose` calls, combination rule by theory |

---

## For the mechanization

The mechanical engine must reproduce this run:

1. **Audit** a candidate element against its theory's charter → produce a
   `Verdict` that is `GAP` (non-conforming), with the located failure.
2. **Propose** a rectification drawn from the qualifying set (outside, non-identity)
   → produce a `Proposal` carrying C1/C2/C3 conventions.
3. **Dispose** the proposal with a judgmental gate (Donald's role) → produce an
   `accept-with-mod` `Disposition` that carries axiom 7 as the modification.

The test in `rung-het/tests/panel.rs` demonstrates one property of this
structure (multiple judges, combination rule by theory). It does not demonstrate
the non-conforming branch. The non-conforming-branch run — `audit` → GAP →
`propose` → `dispose` → axiom-7 as outcome — is what the engine must add to
have a complete worked demonstration.

(`fp.worked-instance`, `fp.self-application`.)
