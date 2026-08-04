# Publishing Het — what an outside expert needs to assess

**Status:** not normative. A working brief for an external reviewer.
**Written:** 2026-08-03 (Augur), at Donald Thompson's request.
**Question it exists to answer:** is there a publishable mathematical
contribution in Het *without* HetOpt, and what would it take to get there?

---

## 0. What we are asking you to do

Not to validate. To find the place where this is either **already known** or
**wrong**, and to say so plainly. The best outcome is *"you are asking the
wrong question, here is the right one."* The second-best is *"this is a
special case of X, published in Y."* Both are more useful to us than
encouragement.

Specifically, four questions:

1. **Is the core claim novel?** (§2 — provenance-constrained satisfaction)
2. **Are the theorems real theorems?** (§4 — three candidates, currently
   phrased as requirements)
3. **What is the right venue and framing?** (§6)
4. **What kills it?** (§7 — our own list; we expect you to add to it)

We are not seeking co-authorship or endorsement. We are seeking the honest
read that a closed system cannot generate about itself — which, as it
happens, is the paper's own thesis.

---

## 1. The one-paragraph version

Institution theory (Goguen & Burstall) abstracts "a logical system" as a
quadruple `(Sign, Sen, Mod, ⊨)` with one axiom: truth is invariant under
change of notation. In every institution we are aware of, `⊨` is a
mathematical relation — it holds or it does not, and *who determines that*
is not part of the structure.

**Het makes the determiner part of the structure.** Sentences carry a *gate
marker*. A `decidable` sentence is machine-checked as usual. A `judgmental`
sentence dispatches to a **principal** drawn from a pool `𝒫`, and the
principal's verdict *is* the satisfaction outcome. `𝒫` is a parameter of
`⊨`, never a sort of the signature.

The consequence that we think is the actual contribution: because principals
are now *in* the structure, the structure can constrain **which** principal
may discharge which sentence. Het requires **provenance disjointness** —
a judge may not have authored the material under judgment — and enforces it
at the level of the model category, not as a side condition on a procedure.

> A theory in which self-certification is not merely discouraged but
> **unrepresentable**.

That is the claim. Everything else is machinery around it.

---

## 2. The novelty claim, stated so it can be attacked

We claim three things are new. We are least certain about the third.

### 2.1 Provenance-constrained satisfaction — the primary claim

Standard institution: `⊨_Σ ⊆ |Mod(Σ)| × Sen(Σ)`.

Het: `⊨` is gate-dispatched, and for judgmental sentences the qualifying set
of principals is

```
𝒫_judg(φ, M) = { p ∈ 𝒫 : capable(p, role(φ)) ∧ π(p) ∩ π(M) = ∅ }
```

where `π` is a provenance map into a discrete category of tags. This is not
a filter applied after the fact. `Mod(Σ)` is *defined* to consist only of
**gate-faithful** algebras (below), so an interpretation that dispatches
judgment to itself is not a model at all.

Semantically, an algebra is a functor `M : T → Kl(𝒫)` into the Kleisli
category of a principal monad, with judgmental arrows required to inhabit

```
Kl_judg(𝒫) = { f : π(f(a)) ∩ π(a) = ∅ }
```

**Why we think this is not just an oracle.** Turing's relative computability
(1939) already gives us "a procedure that consults what it cannot compute,"
and typed holes (`sorry` in Lean, `admit` in Coq) already give us "a declared
gap in a proof." Neither has an *identity* for the thing consulted. An oracle
is a set. It has no author, no provenance, and no possibility of being the
model. There is nothing in relative computability that can express *"this
oracle is disqualified for this query because of who produced the query."*

**What we could not find, and want you to check.** We searched for prior art
on satisfaction relations parameterised by agent identity with a
conflict-of-interest constraint, and found nothing in the abstract-model-theory
literature. We found the concept in *agency theory* (economics) — which is
about incentives, not about the well-formedness of a satisfaction relation.
**If this exists in formal methods and we missed it, that is the single most
valuable thing you can tell us.**

### 2.2 The gate law — directedness

Gate markers may be preserved or increased along theory morphisms
(`decidable → decidable | judgmental`; `judgmental → judgmental`), never
laundered downward. Stated at five structural levels — objects, equations,
morphisms, 2-cells, models — with the claim that this makes the relevant
morphisms **non-invertible**, i.e. the tower is directed.

This has already survived one round of external stress. Two independent
reviewers found the *same* gap (the law as originally stated quantified over
primitive operations only, and morphisms produce derived operations). It was
repaired by three conventions — syntactic gate contagion, an extension lemma,
and semantic gate invariance — and the directedness claim was thereby lifted
from hypothesis to theorem. We consider it the most tested part of the work
and would still like it attacked again.

### 2.3 The authorial gate — judgment and authorship as opposite conditions

`judgmental` classifies; `authorial` transforms. Both require an outside, in
**opposite directions**:

| gate | condition on the principal |
|---|---|
| judgmental | `π(p) ∩ π(M) = ∅` — provenance **disjointness** |
| authorial | `π(outcome) ⊆ π(p) ∧ standing(p, M)` — provenance **containment** |

One pool, two filters; the gate marker selects the predicate, not the pool.
The asymmetry is forced rather than chosen: a judge must not be the party
under audit, but an author *is* precisely the party who must hold stewardship
over what it revises. We find this structurally surprising and cannot locate
an analogue. **This is our least-confident novelty claim** — it may be a
known pattern under a name we do not have.

---

## 3. Prior art we know about and must engage

Listed so you can tell us what is missing rather than what we already have.

| work | relation to Het |
|---|---|
| **Goguen & Burstall, "Institutions: Abstract model theory for specification and programming"** (JACM 1992) | The base. Het is a conservative extension of `⊨` only; `Sign`, `Sen`, `Mod` are untouched. |
| **Goguen & Rosu, "Institution Morphisms"** (2002, ~271 cit.) | Our theory-morphism layer must be stated in these terms. We currently are not precise about morphism vs. comorphism, and this is a known weakness. |
| **Diaconescu, *Institution-independent Model Theory*** | The canonical modern treatment. We have not systematically checked Het's claims against it. **We consider this the largest unexamined body of prior art.** |
| **Mardare, Panangaden & Plotkin, "Quantitative Algebraic Reasoning"** (LICS 2016, ~157 cit.) — plus Bacci et al. (CALCO 2021), Dal Lago (2022) | **The nearest neighbour, and currently uncited by us.** They index equality by a rational distance: `s =_ε t`. Our §3 relaxes the satisfaction condition to a metric bound `d(…) ≤ ε`. A reviewer will see this immediately. Engaged and differentiated, it strengthens the paper; unaddressed, it is the section that sinks it. **We need help stating the relationship precisely.** |
| **Turing, "Systems of Logic Based on Ordinals"** (1939); Soare on relative computability | Oracles. See §2.1 for why we think this is not that. |
| **Rushby, "Formal Methods and the Certification of Critical Systems"** (1993, ~308 cit.) | The closest thing we found to "human judgment discharging a proof obligation" — but as a practice recommendation, not as structure inside the logic. |
| **LLM-as-judge literature** (2024–2026: rating indeterminacy, LLM-generated oracles, overcorrection in LLM code review) | Empirical, not formal. Relevant to §5's motivation and to threats-to-validity, not to the mathematics. |

---

## 4. The theorem inventory — the biggest structural problem

`docs/formalism.md` is written as a **specification**: MUST / MUST NOT,
numbered normative requirements N1–N43. That is the right form for governing
an implementation and the wrong form for a paper.

There are, we believe, **three real theorems** currently phrased as
requirements:

1. **Directedness.** No theory morphism can invert a judgmental predicate
   into a decidable one. (Currently N27 + conventions C1–C3, stated at five
   levels.) Survived two independent reviews; repaired once.

2. **Composition closure.** Combining two judgmental institutions yields a
   judgmental institution: `𝒫₁₊₂ = 𝒫₁ + 𝒫₂` with provenance preserved
   componentwise, non-identity extending to the composite Kleisli category,
   and adequacy composing. (Currently N36–N38.)

3. **Termination without global fixed point.** The regress closes because the
   *doctrine* — what makes a theory well-formed — is a decidable shape-check
   that is not itself expressed in the object language. Adequacy ("a
   qualifying judge exists") is local to each level and is itself judgmental,
   rather than a global condition requiring proof. (Currently N28–N31,
   N40–N41a.)

**What we need from you:** are these theorems? Are they *interesting*
theorems? Is (3) doing real work or is it a definitional dodge? Our own
suspicion is that (1) is the strongest and (3) is the most likely to be
attacked as circular.

---

## 5. The demonstration that already exists

A paper of this kind needs to show the structure does something. We think we
have the right demonstration, and notably it is a **negative** result.

On 2026-07-19 we ran a conformance audit of a relational being's constitutive
document against a declared belonging-law. Two runs, two different judge
models, 26 and 19 findings proposed respectively. **Zero were applied.**

The reason is the point: the non-identity constraint disqualified the
available judges, because their provenance overlapped the authorship of the
material under audit. The system could not close the loop on itself and
correctly refused to.

> A formalism whose own instrument declines to self-certify — and whose
> refusal is forced by the structure rather than by a policy — is the
> paper's best evidence.

We would rather lead with this than with a success case, and we would like
your read on whether that is a defensible rhetorical choice or reads as
special pleading.

**Second, weaker demonstration:** the encoding of Het is checked against a
decidable schema, and passing that check *demonstrates* self-grounding rather
than asserting it. This is real but small.

---

## 6. Scope — what is deliberately excluded, and why it matters

Het was cut cleanly from a larger system so that the mathematical part could
stand alone. Two things sit outside it:

- **Optimization (`HetOpt`).** Het settles *belonging* — is this a
  conforming item? It has no worth-law `V`, no ranking, no cost model, and
  explicitly forbids declaring one. Selecting *the best* judge, or the best
  configuration among conforming ones, is out of scope by construction.
- **The operational system.** Het is one component of a working system for
  self-governing containers (constitutive documents, project portfolios,
  issue trackers) under budget constraints. That system needs both halves.
  **The paper does not.**

**The honest framing of the cut**, which we would like you to pressure-test:
a formalism paper claims nothing about operational sufficiency. Peano
arithmetic is not a programming language. Het claims to be a judgmental
institution with a directed tower and a non-self-certification guarantee;
whether that suffices to run a portfolio is a different question with a
different answer.

**One genuine strength of the cut:** Het is *parametric* in `𝒫`. The theory
never names what a principal is made of — it requires only four predicates
(`capable`, `π`, `standing`, `ε`) at declared arities. The mathematics
therefore survives intact if LLM judges turn out to be unreliable, or are
superseded, or are replaced entirely by human panels. **We think the paper
should lean on this rather than on the current capability of language
models.** We would like your view on whether that is the right bet for a
2026 venue or whether it under-sells the timeliness.

---

## 7. Where we think it is weak — our own list

Please add to this. We would rather hear a fourth item than a defence of
these three.

1. **§3 (quantitative satisfaction) vs. quantitative algebra.** As above.
   Currently uncited; likely the most exposed section.

2. **Morphism precision.** We use "theory morphism" loosely. The institution
   literature distinguishes morphisms from comorphisms and this distinction
   is load-bearing for the directedness theorem. Our encoding of theory-level
   morphisms is, as of today, an explicitly open question — the field that
   previously carried it was retired as the wrong carrier, and its replacement
   has not been designed.

3. **The spec/theorem mismatch.** §4. It is a genuine question whether
   rewriting into theorem form *survives* — the specification form may be
   hiding places where the argument is a convention rather than a proof.

4. **`role(φ)` is unsupplied.** The theory requires every judgmental sentence
   to declare the competence role it needs, and requires `capable` at arity
   `𝒫 × Role`. Nothing in our current encoding supplies the map from
   sentences to roles. It is a hole we found today, by writing the interface
   down.

5. **The implementation does not conform.** Stated for completeness because
   you may ask. A Rust implementation exists; an audit today established that
   it answers substantially to a *prior* design rather than to this one, and
   that its authorial gate is a stub. We do not think this bears on the
   mathematics, but we would rather you hear it from us. If you think a
   formalism paper needs a conforming implementation, say so — that changes
   the timeline considerably.

---

## 8. Reading order

| file | what it is |
|---|---|
| `docs/formalism.md` | **Normative.** The specification. Start here; §1–§5 carry the mathematics, §7 states the Het/HetOpt cut. |
| `docs/institutional_judgment.md` | **Development archaeology, not normative.** How the formalism was derived, including the reasoning that was later corrected. Useful for seeing what was tried and abandoned. §7 is the quantitative-satisfaction derivation. |
| `het/theory.yaml` | Het encoded in its own encoding — the pass (`audit → propose → dispose → enact`) as a signature with gate-marked sentences. |
| `spec/het-theory.schema.json` | The decidable floor. Deliberately an ordinary JSON Schema, deliberately *not* self-encoded — this is what closes the regress. |
| `GAPS.md` | Open problems, honestly kept. |

A caution on `docs/formalism.md`: it changed materially on 2026-08-03. Five
rulings landed that day (the doctrine/schema separation, the opacity of `𝒫`,
retirement of the theory-level conformance edge, relocation of
signature-claims out of the sentence language, and requiring an equation on
every decidable sentence). Anything written about Het before that date may
describe a different object.

---

## 9. What a "yes" would look like

If your read is that there is a paper here, we would want your view on:

- **Venue.** LICS / CSL / CALCO / FoSSaCS for the mathematics; FM or a
  formal-methods venue for the applied framing; something else entirely.
- **Framing.** Is the contribution best stated as *"an institution whose
  satisfaction relation is agent-relative"* or as *"a formal account of
  non-self-certification"*? These attract different reviewers and different
  objections.
- **Minimum viable paper.** Which of the three theorems must be in it, and
  which can be deferred to a follow-up.
- **What must be cut.** Our instinct is that the tower, the fractal property,
  and the game semantics are all real but that including everything produces
  a paper that argues for nothing in particular.

And if your read is that there is not — we would like to know what the
smaller, truer claim is. Finding that the shape is more standard than we
thought is a good outcome, not a disappointing one.

---

## 10. Outside response — reviewer 1, 2026-08-03

The brief above went out and came back the same day. Recorded here because the
record of what an outside said, and when, is worth more than a summary of it.

**One reviewer. Not two.** The July-27 discipline applies: a finding becomes
structural when independent reviewers converge on it, not when one voice is
persuasive. Treat everything below as a strong single read.

### Verdict on the primary claim

> "The primary claim survives the most obvious prior-art filters."

Reasoning given: relative computability supplies oracles that are *pure sets*;
typed holes supply *declared gaps*; neither supplies an identity for the
consulted object that can be tested for provenance overlap with the model under
evaluation. Agency-theoretic conflict-of-interest lives in economics, not inside
a satisfaction relation. The nearest formal neighbours — many-valued and graded
institutions — parameterise satisfaction by *truth degree*, not by *agent
identity*.

**Caveat we are keeping.** This is a reviewer's assertion, not a systematic
check. Diaconescu remains the largest unexamined body of prior art (§3), and
neither we nor this reviewer has walked it.

**On §2.3 (the authorial/judgmental asymmetry):** confirmed as the weakest of
the three novelty claims. "A clean categorical dual, but dualities of this shape
are common once one begins to equip Kleisli arrows with extra structure; it may
already exist under another name."

### Verdict on the three theorems (§4)

| | reviewer's read |
|---|---|
| **Directedness** | **Strongest.** A genuine theorem about the directedness of the category of gate-marked signatures. The earlier gap (quantification over primitives only) is closed. "Interesting precisely because it makes the tower non-invertible at every level." |
| **Composition closure** | Real, but "essentially definitional once the Kleisli construction is fixed. Not the contribution that carries the paper." |
| **Termination** | **Not circular** — it is the standard separation of well-formedness meta-theory from object language. But "the least novel of the three." Its force depends on whether one regards the schema as external to the institution; the reviewer holds that it is. |

The spec-versus-theorem mismatch is confirmed as **the largest structural risk**.
MUST/MUST-NOT is the right form for an implementation contract and the wrong form
for a mathematics venue.

### Two additions to our own weakness list (§7)

Both are gaps that opened on **2026-08-03**, the same day this brief was written
— found independently by us in the morning and by the reviewer in the afternoon.
That convergence is itself worth recording.

1. **Theory morphisms remain open (N31a).** Directedness *quantifies over them*.
   Until the encoding supplies a carrier for signature morphisms — distinct from
   the retired conformance edge — **the strongest theorem is only half-stated.**
   Institution morphisms vs. comorphisms must be fixed before submission.

2. **`role(φ)` is still unsupplied.** Without it the capability filter has no
   input and the interface is incomplete. "A genuine hole, not a cosmetic
   omission."

The quantitative section (§3) is confirmed as the most exposed surface.
Implementation non-conformance is confirmed irrelevant to the mathematics —
"but will be asked about."

### Venue and framing

- **LICS or CALCO** if framed as *"an institution whose satisfaction relation is
  agent-relative and provenance-constrained."*
- **FoSSaCS or a formal-methods venue** if framed as *non-self-certification as
  a structural invariant.*
- Mardare–Panangaden–Plotkin must be engaged **explicitly**: "the distance-bound
  satisfaction condition is a close cousin of quantitative equational theories,
  and failure to differentiate will be fatal."
- Leading with the negative demonstration (§5) is **defensible, not special
  pleading** — "the only empirical evidence the formalism currently possesses
  that the constraint is operative rather than ornamental."
- Parametricity in `𝒫` is "the durable bet… the correct mathematical stance,"
  while under-selling timeliness for a 2026 venue. The tension we flagged in §6
  is real and the reviewer takes the same side we did.

### The minimum viable paper

**Keep:** directedness, and the provenance-constrained definition of `⊨`.
**Defer:** composition closure, the full tower.
**Cut for now:** game semantics, the fractal property.
**Add:** systematic engagement with Diaconescu; one precise paragraph
differentiating the metric relaxation from quantitative algebra.

The claim, reduced to what is defensible today:

> An institution in which the satisfaction relation for judgmental sentences is
> defined only on models that are provenance-disjoint from every admissible
> principal, rendering self-certification unrepresentable inside the model
> category.

"That is already publishable once the theorems are extracted and the morphism
carrier is supplied."

### The downside case, stated plainly

> "If that claim collapses under further scrutiny, the residue is a useful
> engineering pattern (gate-marked operations + external schema), not a
> contribution to institution theory."

Carried explicitly rather than assumed away.

---

## 11. What this changes, operationally

**Two blockers, both narrow, both already on the board.** The morphism carrier
and `role(φ)` were opened by our own work on 2026-08-03; the reviewer named them
independently as publication blockers. Neither is a redesign.

**The publishable core and the operationally-needed core are different subsets.**
The reviewer cuts the fractal property and game semantics — which are precisely
the parts the operational system (self-governing containers under budget) depends
on most. Publication and implementation are therefore **parallel tracks, not
sequential**. Neither blocks the other. That is a better position than it sounds:
the paper needs two narrow things fixed; the system needs the parts the paper
does not.

**What would make this structural rather than persuasive:** a second independent
reviewer, ideally one who walks Diaconescu. One good read is a hypothesis.

---

*Prepared by Augur 🦉 for Donald Thompson, 2026-08-03. Corrections and
counter-arguments to this document are the point of it.*
