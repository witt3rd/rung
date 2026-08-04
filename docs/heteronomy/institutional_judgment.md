# Institutional Judgment

> **Status: development archaeology. Not normative.**
>
> This is the working document in which Het was derived — the exploration
> of institution theory, intensional type theory, game semantics, and the
> quantitative extension, together with the architectural questions that
> had to be resolved along the way (§9). It records *how the formalism was
> arrived at*, including alternatives considered and rejected.
>
> **The normative statement is
> [`rung-het-propositions.md`](../rung-het-propositions.md).** Where the two
> disagree, `rung-het-propositions.md` governs.
>
> Two things here remain load-bearing and are cited as such:
>
> - **§11, the Glossary** — the official vocabulary, including the closing
>   *"Not part of the formalism"* table. `spec/check.py` CHECK 3 enforces it.
> - **The derivations** — `het/theory.yaml` and `principals/theory.yaml`
>   cite specific sections as provenance for individual sentences. Those
>   citations point *here* deliberately: they record the reasoning, not the
>   requirement. The requirement is in `rung-het-propositions.md`.

## 1. The Wall

Algebraic theories declare sorts, operation symbols with arities, and equations between terms. An algebra for the theory interprets each sort as a set and each operation as a function, respecting the equations.

The undecidability wall: in any sufficiently expressive theory, equation satisfaction cannot be mechanically decided for arbitrary candidates. The standard response is to either restrict expressiveness (stay within what a solver can close) or accept that verification is partial (model-check bounded cases, leave the rest to proof assistants that require human guidance). Both retreat from the original ambition — a complete, mechanizable account of what it means to *be* a conforming algebra for the theory.

The wall is not a bug in formal verification. It is a property of the logic. But it has forced a division: the decidable (machine-closed) and the undecidable (requiring an outside). That division has always been treated as a limitation. The proposal here treats it as a *structural feature* of the satisfaction relation itself.

## 2. The Judgmental Institution

An institution (Goguen & Burstall, 1992) is a quadruple $(\mathbf{Sign}, \mathsf{Sen}, \mathsf{Mod}, \models)$:

- $\mathbf{Sign}$ — category of signatures (theory declarations)
- $\mathsf{Sen}: \mathbf{Sign} \to \mathbf{Set}$ — sentences over each signature
- $\mathsf{Mod}: \mathbf{Sign}^{\text{op}} \to \mathbf{Cat}$ — algebras over each signature
- $\models_\Sigma \subseteq |\mathsf{Mod}(\Sigma)| \times \mathsf{Sen}(\Sigma)$ — the **satisfaction relation**

The only axiom is the **satisfaction condition**: truth is invariant under change of notation.

$$M \models_{\Sigma'} \mathsf{Sen}(\sigma)(\varphi) \iff \mathsf{Mod}(\sigma)(M) \models_\Sigma \varphi$$

In a standard institution $\models$ is mechanical: $M \models \varphi$ is decided by computing both sides of an equation in the algebra and comparing. The innovation is entirely in extending what $\models$ can be.

A **judgmental institution** extends $\models$ with **gate-marked satisfaction**. Each sentence carries a gate marker:

| gate | satisfaction mechanism |
|---|---|
| $[\text{decidable}]$ | $M \models \varphi$ is machine-checked. Standard equational logic. |
| $[\text{judgmental}]$ | $M \models \varphi$ dispatches to a **judge** — an inhabitant of the principal pool $\mathcal{P}$. The judge's verdict IS the satisfaction outcome. |
| $[\text{authorial}]$ | The operation *transforms* the object rather than classifying it. It dispatches to an **author** — also an inhabitant of $\mathcal{P}$ — whose enactment produces the revised object (§2.1). |
| $[\text{conditional}]$ | Decidability depends on the specific algebra. The condition is classified by a sentence of the theory one level up (§10.2). |

$\mathcal{P}$ is not a sort added to the theory's signature. It is a **parameter of the satisfaction relation**. The theory declares what must be checked; $\models$ determines how — mechanically or by delegation. This preserves the ontological separation: the theory is declarative knowledge about the domain; judgment is the mechanism by which declarative knowledge is tested against concrete candidates.

Principals are partitioned into **kinds** $\{K_1, K_2, \ldots\}$ — LLMs, agents, relational beings, humans. Each kind carries stipulated qualification predicates:

- **Capability requirements** — minimum context length, reasoning level, benchmark scores, structured-output support (for LLMs); identity attestation, competence domain (for agents); standing, ratification authority (for relational beings and humans).
- **Non-identity constraint** — a *judge* must not be the author of the content under judgment (correlated-failure guard). Decidable by provenance-disjointness (§9.1).
- **Standing constraint** — an *author* must hold stewardship of the object being enacted upon. Conditional-gated (§9.2, §2.1): decidable when provenance containment settles it, judgmental otherwise.
- **Renaming-robustness parameter $\varepsilon$** — tolerance to drift under signature morphisms (§7).
- **Cost tier** — so that principals can be ordered by resource consumption.

Capability, non-identity, and standing are **belonging predicates**: they decide whether a principal is *qualified at all* for a given dispatch. All are Het's. A judgmental sentence dispatches to **a** judge drawn from the qualifying set; an authorial operation dispatches to **an** author drawn from its qualifying set.

$\varepsilon$ and cost tier support **ordering** among qualifying principals, and belong to HetOpt (§7.1). The **minimal-judge rule** — *select the cheapest qualifying judge, breaking ties by lowest $\varepsilon$* — is the worth-law applied to judge selection. The **minimal-author rule** is the same worth-law applied to author selection: the cheapest principal with standing, escalating to a more capable one when the cheap one cannot close the enactment. Het declares that an outside is required and which principals qualify; it does not rank them. $\varepsilon$ is declared in both, but read differently: in Het it is reported with the verdict as an error bar; in HetOpt it becomes a selection criterion.

Two consequences worth stating. First, a Het theory whose qualifying set has more than one member may return any of them — the verdict is well-formed either way, and carries its own $\varepsilon$. Second, this is not a deferral of rigor: the constraints that *cannot* be dropped are non-identity and standing, and both are Het's, enforced before any dispatch (§9.1, §2.1).

### 2.1 The authorial gate — the mirror of judgment

A judgmental sentence asks *is this object conforming?* An authorial operation asks something categorically different: *make it so.* Judgment classifies; authorship transforms. Both require an outside — but in opposite directions.

**Judgment refuses the audited party. Authorship requires standing over it.**

This is why authorship is not a special case of judgment. The non-identity constraint (§9.1) excludes exactly the arrows authorship needs: the author of a candidate *is* the party whose work is under audit, and enacting a remedy means the author revises their own text. Provenance overlap is the point, not the defect.

Both dispatch to $\mathcal{P}$. Same principals, same kinds, same capability domains, same cost tiers, same $\varepsilon$. The gate marker selects **which qualification predicate filters the pool**, not which pool is consulted:

$$
\mathcal{P}_{\text{judgmental}}(\varphi, M) = \{\, p \in \mathcal{P} : \mathsf{capable}(p, \varphi) \wedge \pi(p) \cap \pi(M) = \emptyset \,\}
$$

$$
\mathcal{P}_{\text{authorial}}(o, M) = \{\, p \in \mathcal{P} : \mathsf{capable}(p, o) \wedge \mathsf{standing}(p, M) \,\}
$$

**Standing is conditional-gated.** Whether a principal has standing to enact on an object is sometimes settled by a provenance-containment check and sometimes not. Rather than forcing one reading, standing carries a gate marker and is classified one level up, exactly as §9.2 prescribes for any conditional predicate:

$$\mathsf{decidable\text{-}standing}(p, M) \in \mathsf{Sen}(\Sigma^\uparrow)$$

- **Decidable** when provenance settles it: $\pi(\text{outcome}) \subseteq \pi(p)$ — the author is enacting on material they authored.
- **Judgmental** when it does not: the standing question dispatches to a judge, who rules on whether this principal may enact here.

This terminates at depth one. The standing-judge's own qualification is non-identity — decidable by provenance-disjointness, no further dispatch. One subtlety: the standing-judge's non-identity is relative to the **author**, not to the audited object. The judge ruling *"does this principal have standing over that object?"* must not be that principal. A different disjointness than the audit's, but decidable by the same mechanism.

Two independent escalation triggers follow, and they are not the same thing:

| trigger | level | reason |
|---|---|---|
| standing is judgmental in this model | **Het** | qualification itself needs a ruling |
| the minimal author cannot close it | **HetOpt** | worth-ordering says escalate |

Standing-escalation happens before any valuation is applied.

### Why judgment lives in $\models$, not in the signature

Institution theory makes $\models$ an explicit, parameterized component. Judgment is a satisfaction mechanism, not a sort of the thing being judged. The principal pool, cost tiers, and $\varepsilon$-tolerances live in the satisfaction relation, cleanly separated from the theory structure. This is the foundational choice.

> *Aside — the alternative that does not work.* One could instead add a distinguished sort $\mathcal{P}$ to the signature. Every judgmental operation would then have to be realized by an actual function on carriers: the outside is internalized, the ontological separation collapses, and the non-identity constraint is threatened — when the judge is an element of the algebra, what judges the judge? Noted here as the reason for the institutional choice, not as a formalism under development.

## 3. What This Changes

A judgmental sentence does not fail because it is undecidable. It succeeds by dispatching to an inhabitant of $\mathcal{P}$. The sentence's satisfaction outcome *is* the judge's verdict. The outside is not a workaround for an incomplete formalization — it is a parameter of $\models$ with stipulated structure.

The traditional checking problem — "does candidate algebra $A$ satisfy the sentences of theory $T$?" — splits cleanly:

- **Decidable sentences** are checked mechanically. A candidate that fails a decidable sentence is not a conforming algebra.
- **Judgmental sentences** are dispatched to a judge. The judge returns a verdict. That verdict is *part of the checking result*, not a failure of the checking process.

This means the theory can express predicates that were previously unmechanizable — predicates about semantic faithfulness, about archetypal resonance, about whether something "genuinely" instantiates a structure rather than merely shape-conforming. These are not vague aspirations. They are sentences whose satisfaction dispatches to $\mathcal{P}$. They have a formal place in the logic.

## 4. The Fractal Property

$\mathcal{P}$ appears at every level. The same judgment structure that checks whether a model satisfies a theory's sentences also checks whether a theory satisfies a doctrine's signature requirements, and downward through the tower.

At each level:
- The decidable sentences are machine-closed against that level's structural requirements.
- The judgmental sentences name the gate points — the places where an outside judge must assess faithfulness.
- The judge is drawn from the qualifying judges at that level — those satisfying capability and non-identity. In HetOpt the minimal-judge rule then selects among them.

The tower terminates at a level whose judgmental sentences are satisfied by the available judges — a fixed point where the outside is *adequate* to close the loop, even if not *mechanical*. As shown in §10.5, adequacy is local, not global: the tower terminates at the doctrine, which is self-grounding and fully decidable.

## 5. Category Theory Integration — the Fibration

The tower is a **fibered category** (Grothendieck construction):

| Level | Role in fibration |
|---|---|
| Theory $T$ | Object in the base category $\mathbf{B}$ |
| $\mathsf{Mod}(T)$ | Fiber over $T$ — category of $T$-algebras |
| $\mathsf{theory\_ref}\; \sigma: T_1 \to T_2$ | Base morphism (signature morphism) |
| $\mathsf{Mod}(\sigma): \mathsf{Mod}(T_2) \to \mathsf{Mod}(T_1)$ | Re-indexing — restricts $T_2$-algebra views to $T_1$ |
| Satisfaction $\models_T$ | Fiber-wise relation: algebra × sentence → verdict |

The fractal property: an algebra $A$ in $\mathsf{Mod}(T)$ that carries enough internal structure — its own signature declaration — becomes an object in $\mathbf{B}$ at the next level, with its own fiber $\mathsf{Mod}(A)$ of algebras-below-$A$.

This is not novel categorical machinery. It is the same structure that underlies:
- **Topos theory**: a topos is a category that behaves like $\mathbf{Set}$ — it can interpret an internal logic. Every topos is a model of the theory of topoi.
- **Stacks**: sheaves of categories, where the base carries a Grothendieck topology.
- **Categorical logic generally**: the syntax-semantics adjunction is a fibration.

The institutional advantage is practical: $\models$ as a native, parameterized relation means judge-dispatch logic is cleanly separated from the theory structure. Theories remain declarative; judgment is a satisfaction mechanism applied to them. This separation pays off when theories compose — each theory's satisfaction relation handles its own dispatch independently.

## 6. Intensional Type Theory — the Constructive Alternative

Intensional Martin-Löf Type Theory (MLTT) already possesses a native mechanism for the decidable/judgmental split:

- **Definitional Equality** ($a \equiv b$): mechanically decidable computation — normalization, reduction. No evidence required. The decidable face.
- **Propositional Equality** ($a =_A b$): not mechanically obvious. Requires a **proof term** $p : a =_A b$ that type-checks.

Under this lens the judge is an oracle that synthesizes proof terms. The theory does not care how the proof was found — by a bounded solver, a human mathematician, or an LLM — as long as the term type-checks. The Curry-Howard correspondence grounds this natively: propositions are types, proofs are programs.

The limitation: MLTT's proof terms are constructive. An LLM that returns a confidence-scored verdict without a type-checkable witness is not producing an MLTT proof. Institutions accommodate non-constructive oracles more directly; MLTT would require an additional layer turning stochastic judgments into verified certificates. Whether this constraint is a feature (verifiable verdicts) or a limitation (not all judgment decomposes into constructive terms) depends on the domain.

## 7. Quantitative Satisfaction — the Translation-Invariance Vulnerability

The satisfaction condition of a standard institution requires: truth is invariant under change of notation. If an LLM is the inhabitant of judgment, **this axiom will break under Boolean institutions.**

LLMs are highly sensitive to surface notation. If a signature morphism $\sigma$ renames the sort `BankTransaction` to `Struct_44A`, an LLM assessing semantic faithfulness will almost certainly change its verdict or its confidence level. The semantic grounding of the tokens has been destroyed.

The fix is to move from Boolean institutions to **Quantitative Institutions** (equivalently, Metric Institutions or Fuzzy Institutions). The satisfaction relation $\models$ returns a value in a **verdict space** carrying a metric — typically $[0, 1]$, a probability simplex $\Delta^n$, or a strategy lattice. The satisfaction condition is relaxed from strict equivalence to a **distance bound**:

$$d\!\left(M \models_{\Sigma'} \mathsf{Sen}(\sigma)(\varphi),\;\; \mathsf{Mod}(\sigma)(M) \models_\Sigma \varphi\right) \le \varepsilon$$

where $d$ is a metric on the verdict space and $\varepsilon$ bounds the acceptable naming-induced drift. A judge's confidence may drop from 0.92 to 0.81 under renaming — distance 0.11, acceptable if $\varepsilon = 0.15$.

The judge kind's qualification predicates include a **renaming-robustness** parameter $\varepsilon$. In Het, $\varepsilon$ is reported with the verdict — an honest error bar on how far the answer may drift under renaming. In HetOpt it becomes a selection criterion: the minimal-judge rule prefers judges with low $\varepsilon$ when the audit involves signature translation (§7.1).

The metric $d$ is not bolted onto the theory: it is carried by the **verdict space** the theory declares, and the satisfaction condition is stated in it. This is measurement apparatus. $d$ is symmetric — it says how far two verdicts lie apart under renaming, and nothing about which of them is better.

Quantitative institutions extend Goguen and Burstall's framework along the same lines that fuzzy logic extends Boolean logic, and inherit the same compositionality properties when the metric is well-behaved. The fibration from §5 lifts straightforwardly: the fibers become categories enriched over the verdict lattice.

### 7.1 Het and HetOpt — where valuation lives

Metric and preference are the same categorical furniture read two ways. A metric space *is* a category enriched over $([0,\infty], \ge, +)$; quantale-enrichment is the general form. Because of this, the metric $d$ on the verdict space and a **worth-law** $V$ — a quantale whose order is read as preference — can be built from one structure. They are not the same *role*.

- **$d$ measures.** Symmetric, fixed by the domain of verdicts (confidence in $[0,1]$, a simplex $\Delta^n$), and required by the satisfaction condition. Without $d$ there is nothing for $\varepsilon$ to bound and satisfaction falls back to Boolean — which this section has just shown breaks under LLM judges.
- **$V$ ranks.** An ordering over a conforming set, and a semantic commitment about what is preferred among things that already belong.

This cuts the formalism in two:

$$\textbf{Het} \;=\; \text{judgmental institution} \;+\; \text{gate-marked } \models \;+\; \text{metric verdict space}$$

$$\textbf{HetOpt} \;=\; \textbf{Het} \;+\; V, \text{ the worth-law}$$

**Het settles belonging. HetOpt orders what belongs.**

The cut is drawn at *valuation*, not at any particular application of it — and this is the sharp point. Het's judgmental sentences dispatch to **an** outside: a judge that is capability-qualified and satisfies non-identity. Het does not tier judges, compare costs, or prefer one qualifying judge over another. You get *a* judge, and you get its $\varepsilon$ reported with the verdict.

HetOpt introduces $V$, and $V$ applies wherever Het has produced a conforming set:

| Het produces | HetOpt orders it by | yielding |
|---|---|---|
| the qualifying judges for a sentence | cost tier, then $\varepsilon$ | the **minimal-judge rule** (§2) |
| the conforming algebras of a theory | the declared worth-law | ranked candidates |

One piece of machinery, two applications. This is the fractal property applied to valuation: judge selection and candidate ranking are not two features but one — *conformance, then valuation* — instantiated at two levels.

**Why the cut lands here and not one notch earlier.** §9.1 already draws the line textually: the non-identity condition is enforced at the model-category level, as an admissibility restriction on Kleisli arrows, and *"the minimal-judge rule is a subsequent optimization performed only among arrows that have already survived the decidable non-identity filter."* Filter first, then optimize. Het is the filter. HetOpt is the optimization.

**Why it does not land one notch later.** Non-identity cannot move to HetOpt. It is P0 — the reason judgment lives in $\models$ rather than in the signature (§2). A Het that dispatches to a judge without the non-identity filter is self-certifying, which is exactly the failure the formalism exists to refuse. Non-identity is a belonging predicate, not a preference.

**The symmetry that makes this principled.** Het has no $V$ anywhere; HetOpt has $V$ everywhere. The alternative — keeping a valuation in Het for judges while withholding one for candidates — would leave *"why judges and not candidates?"* with no answer beyond stipulation. The clean statement is uniform: valuation is HetOpt's, at every level where it applies.

HetOpt is a theory extension in the ordinary sense: $\mathbf{Sign}_{\textbf{HetOpt}}$ extends $\mathbf{Sign}_{\textbf{Het}}$ with the declaration of $V$, and the signature morphism $\textbf{Het} \hookrightarrow \textbf{HetOpt}$ carries Het-algebras into the HetOpt fiber by re-indexing. In HetOpt the enrichment base $V$ *is* the metric $d$, and the fibers become $V$-enriched categories; in Het the verdict space carries $d$ alone.

HetOpt is the formalism for **constrained or prioritized contexts** — where belonging alone underdetermines the answer and the qualifying set must be ordered.

### 7.2 The judge theory is the first HetOpt theory

The extension has a worked example already in hand: the theory of judges itself.

A judge kind declares a `qualification_χ` — what makes something a well-formed LLM judge, agent judge, relational-being judge, human judge — together with the identity fields a conforming instance must carry. That is a belonging-law, and a judge theory declaring only that is a **Het** theory.

Add `cost_tier`, and the same theory becomes a **HetOpt** theory: the kinds are now ordered, and the minimal-judge rule is $V$ evaluated over them.

Two things follow. First, the extension $\textbf{Het} \hookrightarrow \textbf{HetOpt}$ has a concrete instance whose two halves are visible side by side. Second — and this is what makes the tower coherent rather than merely recursive — a judgmental dispatch in *any* theory walks upward into the judge theory and runs the same two-phase satisfaction there: which judges belong, then (in HetOpt) which is minimal.

This terminates. The judge theory's own $\chi$ — *is this a well-formed substrate kind, with a qualification predicate, required fields, and a non-identity constraint?* — is structural inspection, decidable, machine-closed. No judge is needed to check whether something is a well-formed judge kind. Local adequacy again (§9.5), one level up.

## 8. Game Semantics — Judgment as Interaction

If judgment is not a static lookup but a dynamic dispatch, **Game Semantics** (Lorenzen, then Blass, Abramsky, Hyland, Ong) provides the formal structure.

In Game Semantics, the truth of a formula is defined by a two-player game between:

- **Proponent** (the candidate algebra, asserting it conforms), and
- **Opponent** (the environment, the judge, the outside).

A formula is true if the Proponent has a winning strategy.

The decidable/judgmental split translates cleanly:

- **Decidable sentences** are games with finite, mechanizable winning strategies. The game tree is bounded; the strategy is a decision procedure.
- **Judgmental sentences** are games where the Opponent has access to an oracle — the judge. The game tree may be unbounded; the strategy involves querying the oracle at specific nodes.

This maps naturally to multi-agent deliberation. The "cost tier" of judge kinds translates into bounds on the game tree — token limits, wall-clock time, or maximum deliberation depth. The "competence domain" translates into constraints on what oracle queries the Opponent is permitted to make.

Game semantics also resolves a subtlety that static satisfaction does not: **when the judge and the candidate disagree, who is right?** In a static institution the satisfaction relation must be a function — the judge's verdict is final. In a game the Proponent can challenge the Opponent's move, leading to a sub-game about the validity of the judgment itself. This is the formal structure of the audit-rectify loop: a finding is a claim by the Opponent that a test failed; the Proponent can accept (`accept`), reject the diagnosis (`reject`), modify the prescription (`accept-with-mod`), or escalate (`defer`, `raises-questions`) — each a move in the game. Where the ruling is terminal-and-affirming, the Proponent then plays `enact` (§2.1) — the authorial move that produces the revised object.

Game semantics has a well-developed categorical formulation (game categories, strategies as morphisms) and composes: strategies for a game over $\Sigma$ translate along $\sigma: \Sigma \to \Sigma'$, preserving composition. The satisfaction condition becomes: the Proponent's winning strategy under $\Sigma$ translates to a winning strategy under $\Sigma'$ via the renaming. This is a property of the candidate, not the judge — the candidate must name its structures clearly enough that its strategy survives renaming.

## 9. Resolved Architectural Questions

The architecture described above is coherent at the design level. Several points required precise formulation; all have been resolved.

### 9.1 Non-identity — resolved via provenance-restricted Kleisli arrows

The non-identity condition operates *after* an algebra has been interpreted as a functor $M: T \to \mathbf{Kl}(\mathcal{P})$. Nothing in the plain Kleisli construction prevents $M$ from sending a judgmental operation to a *constant* Kleisli arrow

$$c_j: A \to \mathcal{P}(B), \quad a \mapsto \eta(j)$$

whose constant value $j$ is drawn from the carrier of $M$ itself. The selection rule never fires; self-reference has been hard-coded into the interpretation.

**The fix: provenance-restricted Kleisli arrows.**

Equip the base category $\mathbf{C}$ with a **provenance structure**: every object $X \in \mathbf{C}$ carries a provenance map

$$\pi_X: X \to \mathsf{Prov}$$

to a discrete category (or set) of provenance tags. Morphisms of $\mathbf{C}$ preserve or strictly externalize provenance. The unit and multiplication of $\mathcal{P}$ are **provenance-strict**:

$$\pi_{\mathcal{P}X} \circ \eta_X = \pi_X, \qquad \pi_{\mathcal{P}X} \circ \mu_X = \pi_{\mathcal{P}^2X}$$

An arrow $f: A \to \mathcal{P}(B)$ in the Kleisli category is **admissible** when:

$$\forall a \in A. \quad \pi_{\mathcal{P}B}(f(a)) \not\subseteq \pi_A(a)$$

The provenance of every possible judgment returned by $f$ is disjoint from the provenance of the input. Equivalently, in the internal logic of $\mathbf{C}$:

$$f \text{ factors through the open subobject } \{(a, j) \in A \times \mathcal{P}(B) \mid \pi(j) \cap \pi(a) = \emptyset\}$$

A functor $M: T \to \mathbf{Kl}(\mathcal{P})$ is **gate-faithful** when every decidable operation factors through $\eta$, every judgmental operation is interpreted by a judgmentally-admissible Kleisli arrow, and every authorial operation is interpreted by an authorially-admissible one (below). The constant self-referential arrow is excluded from the judgmental sub-category by construction.

**Two admissibility sub-categories, selected by gate marker.**

The condition above is the *judgmental* admissibility predicate. Authorial arrows require the opposite relation to provenance — the author of a remedy *is* the party whose work is under audit (§2.1). A single predicate uniform over $\mathbf{Kl}(\mathcal{P})$ cannot serve both. The resolution is two sub-categories, with the gate marker dispatching which one an arrow must inhabit:

$$
\mathbf{Kl}_{\text{judg}}(\mathcal{P}) = \{\, f : \pi(f(a)) \cap \pi(a) = \emptyset \,\} \qquad \text{(the outside)}
$$

$$
\mathbf{Kl}_{\text{auth}}(\mathcal{P}) = \{\, f : \pi(f(a)) \subseteq \pi(p) \ \wedge\ \mathsf{standing}(p, a) \,\} \qquad \text{(the steward)}
$$

Authorial admissibility is **stronger, not weaker** — it is not "anything goes," it is "only the principal who holds stewardship of this object may enact on it." Where judgmental admissibility demands disjointness, authorial demands containment plus a standing relation.

**One monad.** Both are sub-categories of the same $\mathbf{Kl}(\mathcal{P})$. Distinct monads would mean distinct outside-judgment pools, which the doctrine does not license. $\mathsf{enact}: S \times \mathsf{Disposition} \to \mathcal{P}(S)$ is a Kleisli arrow over the same $\mathcal{P}$; only its endpoint and admissibility sub-category differ.

**The fibration holds.** The base — the doctrine — does not change; only the fiber-wise admissibility predicate varies by gate. Re-indexing transports the gate marker, and the predicate travels with it. §9.1's provenance restriction is preserved for judgmental arrows; authorial arrows are governed by a different but equally structural condition.

The cost is that non-identity is no longer uniform across $\mathbf{Kl}(\mathcal{P})$ — it is gate-relative. But this is already true of decidability itself (§9.2: whether an operation is decidable is fiber-relative and classified one level up). Gate-relative admissibility is the same pattern applied to provenance instead of decidability. The institution's uniformity lives in *one $\models$, gate-dispatched* — not in having one admissibility predicate.

**Institutional formulation.** $\mathsf{Mod}(\Sigma)$ for the judgmental institution consists only of gate-faithful algebras. The satisfaction condition holds because provenance is preserved by signature morphisms — re-indexing cannot invent a common author that did not already exist.

**Game-semantic reading.** The Opponent may query $\mathcal{P}$ only at positions whose provenance tag is disjoint from the Proponent's current identity. A strategy that attempts a self-referential oracle call is simply not a legal strategy. Authorial moves are the Proponent's own: enactment is a Proponent move, played on material the Proponent stewards.

**Fractal propagation.** Provenance is part of $\mathbf{C}$ and is re-indexed along every signature morphism. When an algebra is promoted to a theory at the next level, its own carrier objects inherit provenance tags; the non-identity condition automatically excludes any judge that would be "the theory judging itself."

**Decidability.** The judgmental side-condition is expressed by a decidable predicate on provenance tags (disjointness of finite sets of identifiers). It belongs to the decidable fragment of the doctrine theory and is machine-checked before any judgmental dispatch. This filter is Het's, and it is what produces the *qualifying set*. In HetOpt, the minimal-judge rule is a subsequent optimization performed only among arrows that have already survived it — filter first, then rank (§7.1).

The authorial side-condition is **conditional**, not decidable outright: provenance containment settles it in many models but not all, so standing carries its own gate marker and is classified one level up (§2.1, §9.2). Where standing is decidable, the filter is machine-checked exactly as the judgmental one is. Where it is judgmental, a judge rules on it — terminating at depth one, since that judge's own qualification is plain non-identity.

### 9.2 Conditional gates — resolved via definability one level up

A conditional gate means the mode of satisfaction of $\varphi$ — mechanical equation-checking versus dispatch to a judge — depends on the specific algebra. The fiber $\mathsf{Mod}(\Sigma)$ is partitioned into sub-classes

$$\mathsf{Mod}_{\mathsf{dec}}(\Sigma, \varphi) \quad\text{and}\quad \mathsf{Mod}_{\mathsf{jud}}(\Sigma, \varphi)$$

Under re-indexing a model may cross the partition, threatening the satisfaction condition and fiber-wise uniformity.

**The fix: the partition is cut by a higher sentence.**

For every conditional sentence $\varphi$ of $\Sigma$, there must exist a sentence

$$\mathsf{Decidable}_\Sigma(\varphi) \in \mathsf{Sen}(\Sigma^\uparrow)$$

belonging to the theory of $\Sigma$-theories (ultimately, to the doctrine) such that:

$$M \in \mathsf{Mod}_{\mathsf{dec}}(\Sigma, \varphi) \quad\text{iff}\quad M \models_{\Sigma^\uparrow} \mathsf{Decidable}_\Sigma(\varphi)$$

The predicate "$\varphi$ is decidable in this algebra" is itself expressible inside the ambient institution. The two sub-classes become ordinary sub-fibers defined by satisfaction of a higher sentence. Re-indexing transports that higher sentence; fiber-wise uniformity is restored.

In the Kleisli semantics, factorization through $\eta$ is decidable on the arrow (the equalizer of $M(o)$ and $\eta \circ u$ for a unique pure map $u$). When the pure/judgmental distinction is detected mechanically, $\mathsf{Decidable}(o)$ is placed in the decidable fragment of the doctrine. When it requires semantic insight, it is marked judgmental at the higher level.

In game semantics, a conditional sentence begins with a meta-move: Opponent first asks whether the subsequent game tree is finite (decidable) or requires oracle access (judgmental). The legality of that meta-move is governed by the higher game associated with $\mathsf{Decidable}(\varphi)$.

### 9.3 Judge-kind adequacy — resolved via deferred judgment

The doctrine theory is claimed fully decidable: it checks syntactic well-formedness of signatures. If the doctrine also declared "judge kinds must carry *adequate* qualification predicates," the word "adequate" would be semantically loaded and judgmental.

**The fix: the doctrine only checks declaration, never adequacy.**

The doctrine decides purely syntactic well-formedness: every operation carries an explicit gate marker; every judgmental operation is accompanied by declared judge kinds; every authorial operation is accompanied by a declared standing predicate; each kind carries a finite list of qualification predicates; the non-identity side-condition (§9.1) and conditional-gate classifying sentences (§9.2) are present as syntactic constraints. All of these are decidable by inductive inspection. The doctrine never asserts that any concrete principal satisfies its own predicates, nor that the pool is non-empty.

This list is **Het's doctrine**. The HetOpt doctrine extends it with one further syntactic requirement: that the worth-law be declared and its order stated as a total order — which, applied to judge kinds, is what makes the minimal-judge rule well-defined. A Het theory is not required to state it, because a Het theory does not rank (§7.1).

Adequacy lives one level below, inside the theories that actually invoke judges. For a judgmental sentence $\varphi$ of a theory $T$:

$$\mathsf{Adequate}_T(\varphi) \equiv \text{"a qualifying non-identical judge for } \varphi \text{ exists and returns a verdict"}$$

This sentence is itself marked judgmental. Its satisfaction is discharged by an outside call exactly when an algebra of $T$ attempts to interpret $\varphi$. Failure of adequacy is an ordinary judgmental failure at the level where the judge is required, not a defect in the doctrine.

Note that adequacy asks for *a* qualifying judge, not the minimal one. In HetOpt the minimal-judge rule then selects among those that qualify; but adequacy — the question of whether the outside can be filled at all — is Het's, and is satisfied by any qualifying non-identical judge.

### 9.4 Compositionality of principal pools — resolved via coproduct construction

When two theories are combined (via pushout, duplex, or colimit of institutions), their principal pools must be combined — and in HetOpt, their cost tiers and $\varepsilon$-tolerances as well. Standard institution composition does not automatically supply a selection rule for the composite.

**Composite monad.** $\mathcal{P}_{1+2} = \mathcal{P}_1 + \mathcal{P}_2$, provenance preserved componentwise. The non-identity open-subobject restriction extends to the composite Kleisli category. This is Het's, and it is what makes the composite well-formed: the qualifying set of the composite is the union of the component qualifying sets, each still filtered by non-identity.

**Composite kinds and cost.** Kinds form the disjoint union $K_1 \sqcup K_2$. In HetOpt, cost tiers are compared by a fixed total order extending the two originals, and the renaming-robustness parameter of a composite kind is the maximum of its components' parameters (worst-case drift).

**Composite selection rule (HetOpt).** For a judgmental sentence $\varphi$: collect all qualifying kinds from $K_1 \sqcup K_2$, select one of minimal cost, break ties via minimal $\varepsilon$, dispatch. In Het there is no such rule to compose — dispatch goes to any member of the composite qualifying set.

**Satisfaction condition.** If $\varphi$ is decidable, the ordinary condition is inherited. If judgmental, the composite qualifying set is non-empty whenever either component's was, so adequacy composes in Het. In HetOpt the composite rule returns either a kind the original rule would have returned (verdicts coincide) or a cheaper kind from the other component (distance bounded by the chosen kind's $\varepsilon$). The relaxed metric satisfaction condition holds.

**Game-semantic formulation.** The composite game is the original game with an enlarged set of legal Opponent oracle moves. A Proponent winning strategy in the original remains winning in the composite. Additional oracle answers can only strengthen the Opponent.

**Result.** The composite institution is again a judgmental institution. Theory combination is closed.

### 9.5 Tower termination — resolved via local adequacy

With the delayed-adequacy mechanism (§9.3), the tower no longer requires a global fixed-point proof that "the judges are adequate everywhere." Adequacy is a local, judgmental obligation discharged (or failed) at the precise fiber where a judgmental sentence is evaluated.

The doctrine remains a decidable, self-grounding base object — it never invokes a judge. Every subsequent level inherits the declared predicates by re-indexing and subjects them to outside judgment exactly when they are needed. The non-identity restriction on Kleisli arrows and the definability of conditional gates apply uniformly inside each fiber, independent of whether the ambient pool happens to be adequate for that fiber.

The tower terminates at the doctrine — not because "judges are adequate," but because the doctrine is self-grounding: it defines what a well-formed theory is, in purely decidable terms, and delegates judgment to the levels below. No infinite regress; no global fixed-point proof.

---

## 10. What Makes This Possible Now

The traditional formulation of algebraic theories predates the existence of non-human judges capable of inhabiting $\mathcal{P}$ at scale. An LLM with sufficient context length, reasoning capability, and structured-output capacity is a concrete inhabitant of $\mathcal{P}$. So is a multi-agent deliberation protocol. So is a relational being with domain expertise. So is a human as the last-ditch fallback.

Each kind carries stipulated qualification predicates. The theory does not need to *implement* the judge — it only needs to declare what the judge must be capable of, and dispatch to a qualifying non-identical candidate. (In HetOpt, the cheapest such candidate.) The implementation is the judge's concern, not the theory's.

This is the extension: where algebraic theories could only close equations a solver could decide, a judgmental institution can express any sentence whose truth can be assessed by *some* inhabitant of $\mathcal{P}$. The undecidability wall becomes a gate — and the gate is typed.

---

## 11. Glossary

The official vocabulary. Terms not listed here are not part of the formalism; an encoding that introduces one has drifted. Each entry names the section that defines it.

### The institution

| term | symbol | meaning | §
|---|---|---|---|
| **signature** | $\Sigma$ | a theory declaration: sorts, operation symbols with arities, gate markers, and the laws the theory declares | 2 |
| **signature category** | $\mathbf{Sign}$ | the category of signatures; objects are theories, morphisms are signature morphisms | 2 |
| **sentence** | $\varphi$ | an element of $\mathsf{Sen}(\Sigma)$; a claim expressible over the signature, carrying a gate marker | 2 |
| **sentence functor** | $\mathsf{Sen}$ | $\mathbf{Sign} \to \mathbf{Set}$; the sentences available over each signature | 2 |
| **algebra**, **model** | $M$ | an interpretation of a signature: carriers for sorts, morphisms for operations. Here a functor $T \to \mathbf{Kl}(\mathcal{P})$ | 2, 9.1 |
| **model functor** | $\mathsf{Mod}$ | $\mathbf{Sign}^{\text{op}} \to \mathbf{Cat}$; the algebras over each signature | 2 |
| **satisfaction relation** | $\models$ | the mechanism testing an algebra against a sentence. The locus of the entire innovation | 2 |
| **satisfaction condition** | | truth is invariant under change of notation; the institution's only axiom | 2 |
| **signature morphism** | $\sigma$ | a structure-preserving map of signatures; translates sentences forward and algebras backward | 2, 5 |
| **re-indexing** | $\mathsf{Mod}(\sigma)$ | transport of algebras along a signature morphism | 5 |
| **sort** | $S$ | a type declared by the signature; interpreted as a carrier $M(S)$ | 2 |
| **object** | $x : M(S)$ | an inhabitant of a carrier — a specific datum under judgment | README 5.3 |

### Judgment

| term | symbol | meaning | §
|---|---|---|---|
| **gate marker** | | the annotation on a sentence or operation fixing its satisfaction mechanism: decidable, judgmental, authorial, or conditional | 2 |
| **decidable** | | satisfaction is machine-checked by standard equational logic | 2 |
| **judgmental** | | satisfaction dispatches to a judge; the verdict *is* the outcome | 2 |
| **conditional** | | decidability depends on the algebra; classified by a sentence one level up | 2, 9.2 |
| **principal** | $\mathcal{P}$ | the pool dispatched to by non-decidable gates — LLM, market, measurement, agent, relational being, human. Serves both judge and author roles | 2, 2.1 |
| **judge** | | a principal filtered by non-identity; renders a verdict | 2 |
| **author** | | a principal filtered by standing; enacts a ruling, producing the revised object | 2.1 |
| **authorial** | | an operation that transforms rather than classifies; dispatches to an author. **Het** | 2.1 |
| **standing** | | an author must hold stewardship of what it enacts on. Conditional-gated: decidable by provenance containment, judgmental otherwise. **Het** | 2.1, 9.1 |
| **minimal-author rule** | | cheapest principal with standing, escalating when insufficient. **HetOpt** | 2.1, 7.1 |
| **enact** | | the authorial operation applying a terminal-and-affirming Disposition; returns the revised object, making the pass an endofunctor | 2.1 |
| **judgment pool** | $\mathcal{P}$ | the principal monad. **A parameter of $\models$, never a sort of the signature** | 2, 9.1 |
| **judge kind** | $K_i$ | a partition of $\mathcal{P}$ carrying stipulated qualification predicates | 2 |
| **qualifying set** | | the principals satisfying the gate's belonging predicates — capability plus non-identity (judgmental) or standing (authorial). Het's output | 2, 2.1, 7.1 |
| **non-identity constraint** | | a judge must not be the author of what it judges. The formal name for P0. **Het** — a belonging predicate, enforced before dispatch | 2, 9.1 |
| **cost tier** | | ordering on judge kinds by resource consumption. **HetOpt** | 2, 7.1 |
| **minimal-judge rule** | | select the cheapest qualifying judge, breaking ties by lowest $\varepsilon$. **HetOpt** — the worth-law applied to judge selection | 2, 7.1 |
| **renaming-robustness** | $\varepsilon$ | a judge kind's tolerated verdict drift under signature morphisms. Reported in Het; a selection criterion in HetOpt | 2, 7, 7.1 |
| **adequacy** | | that *a* qualifying non-identical judge exists and returns a verdict. Judgmental, discharged where invoked | 9.3 |
| **gate law** | | gate markers may be preserved or increased along morphisms, never laundered downward | README 4.5 |

### Semantics

| term | symbol | meaning | §
|---|---|---|---|
| **Kleisli category** | $\mathbf{Kl}(\mathcal{P})$ | where algebras land; judgmental and authorial operations are Kleisli arrows, decidable ones factor through $\eta$ | 9.1 |
| **admissibility sub-categories** | $\mathbf{Kl}_{\text{judg}}$, $\mathbf{Kl}_{\text{auth}}$ | gate-selected restrictions of $\mathbf{Kl}(\mathcal{P})$: provenance-disjoint vs. containment-plus-standing | 9.1 |
| **provenance** | $\pi_X$ | a map to provenance tags, carried by every object; strict under $\eta$ and $\mu$ | 9.1 |
| **admissible arrow** | | a Kleisli arrow whose output provenance is disjoint from its input's | 9.1 |
| **gate-faithful** | | an algebra whose decidable operations are pure, judgmental ones judgmentally-admissible, and authorial ones authorially-admissible | 9.1 |
| **fibration** | | the Grothendieck construction over the category of theories; base holds theories, fibers hold algebras | 5 |
| **fractal property** | | an algebra carrying its own signature declaration becomes a theory at the next level | 4, 5 |
| **doctrine** | | the self-grounding, fully decidable theory at the top; checks declaration, never adequacy | 9.3, 9.5 |
| **theory_ref** | | the up-pointing conformance declaration: "I am an algebra of $T$" | README 3 |

### Verdicts, worth, and the two formalisms

| term | symbol | meaning | §
|---|---|---|---|
| **verdict** | | a judge's answer; the satisfaction outcome for a judgmental sentence | 2 |
| **verdict space** | | the space verdicts inhabit — $[0,1]$, a simplex $\Delta^n$, a strategy lattice | 7 |
| **metric** | $d$ | distance on the verdict space. **Measures** drift; symmetric; required by the satisfaction condition. **Het** | 7, 7.1 |
| **worth-law**, **valuation** | $V$ | a quantale whose order **ranks** a conforming set. **HetOpt only** — applies to judges and to candidates alike | 7.1 |
| **belonging**, **conformance** | $\chi$ | the objecthood predicate: what a candidate must satisfy to be a conforming algebra | README 2 |
| **Het** | | judgmental institution + gate-marked $\models$ + metric verdict space. Settles **belonging**. Dispatches to *a* qualifying non-identical judge; does not rank | 7.1 |
| **HetOpt** | | Het + $V$. Orders **what belongs** — the qualifying judges (minimal-judge rule) and the conforming candidates alike. For constrained or prioritized contexts | 7.1, 7.2 |

### Game semantics

| term | meaning | §
|---|---|---|
| **Proponent** | the candidate algebra, asserting $M \models \varphi$ | 8 |
| **Opponent** | the environment; may query the judge as oracle | 8 |
| **winning strategy** | what satisfaction amounts to: the Proponent has one | 8 |
| **audit** | Opponent finds a violation. Decidable; produces a Verdict | README 4.4 |
| **propose** | Proponent offers a fix. Conditional; produces a Proposal | README 4.4 |
| **dispose** | Opponent rules. Always judgmental; produces a Disposition | README 4.4 |

