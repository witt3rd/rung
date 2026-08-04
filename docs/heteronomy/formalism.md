# Het — The Formalism

**Status: normative.** This document is Het. It is self-contained: it
depends on no other document, cites no artifact, and records no history.
Every claim is stated once, in one place, and referred to elsewhere by
number.

The numbering is a tree. A proposition `n.m` is a remark on `n`; `n.mm`
is a remark on `n.m`. Interior propositions are the conjunction of their
children. Leaves are single checkable claims.

**Scope.** Propositions 1–7 and 9–12 specify Het. Proposition 8 specifies
the cut between Het and HetOpt, and states of HetOpt only what the cut
requires. Proposition 13 states the limit Het does not close.

---

## 1 · The relation

**1** There is one relation:

$$M \models_\Sigma \varphi$$

A model $M$ satisfies sentence $\varphi$ under signature $\Sigma$.

**1.1** The ambient structure is an institution — a quadruple
$(\mathbf{Sign}, \mathsf{Sen}, \mathsf{Mod}, \models)$.

**1.11** $\mathbf{Sign}$ is a category. Its objects are signatures; its
morphisms are signature morphisms.

**1.12** $\mathsf{Sen} : \mathbf{Sign} \to \mathbf{Set}$ assigns to each
signature its sentences.

**1.13** $\mathsf{Mod} : \mathbf{Sign}^{\text{op}} \to \mathbf{Cat}$
assigns to each signature its algebras.

**1.14** $\models_\Sigma \;\subseteq\; \lvert\mathsf{Mod}(\Sigma)\rvert \times \mathsf{Sen}(\Sigma)$.

**1.2** The institution's single axiom is the satisfaction condition:
truth is invariant under change of notation.

$$M \models_{\Sigma'} \mathsf{Sen}(\sigma)(\varphi) \iff \mathsf{Mod}(\sigma)(M) \models_\Sigma \varphi$$

**1.3** A signature declares sorts, operation symbols with arities, gate
markers, and the laws the theory declares.

**1.31** The signature layer is standard. Het's entire extension is in
$\models$.

**1.32** There is no layer above $\Sigma$. There is $\Sigma$, there is
$M$, and there is one gate-dispatched $\models$.

**1.4** Every other structure named in this document — the gates, the
pool, the tower, the game — is bookkeeping around **1**.

---

## 2 · The gate

**2** Every sentence and every operation carries a **gate marker**, which
fixes how its satisfaction is computed.

**2.1** The marker is one of exactly four.

| gate | satisfaction mechanism |
|---|---|
| `decidable` | $M \models \varphi$ is machine-checked. Standard equational logic. |
| `judgmental` | $M \models \varphi$ dispatches to a **judge** — an inhabitant of the principal pool $\mathcal{P}$. The judge's verdict *is* the satisfaction outcome. |
| `authorial` | The operation *transforms* the object rather than classifying it, or produces new content about it. It dispatches to an **author**, also from $\mathcal{P}$, holding standing over the object. |
| `conditional` | Whether satisfaction is decidable depends on the specific algebra. The condition is classified one level up (2.5). |

**2.11** No other value is well-formed.

**2.2** An operation without a gate marker is not a well-formed
declaration.

**2.3** A judgmental operation declares the **competence role** required
to discharge it.

**2.31** A role, not a kind. Kind is what a principal is made of, and
belongs to whatever supplies $\mathcal{P}$ (3.23). Role is what the
sentence needs done, and only the sentence's own theory knows that.

**2.32** The declaration is pointwise. There is no global map from
sentences to competences, and none is needed: the pointwise declaration
is what lets $\models$ resolve a judge.

**2.4** An authorial operation declares a **standing predicate**.

**2.5** A conditional operation names a **classifying sentence**.

**2.51** The classifying sentence is not itself judgmental. A judgmental
classifier reopens the regress 6.4 closes.

**2.52** A conditional gate partitions the fiber $\mathsf{Mod}(\Sigma)$:

$$\mathsf{Mod}_{\mathsf{dec}}(\Sigma, \varphi) \quad\text{and}\quad \mathsf{Mod}_{\mathsf{jud}}(\Sigma, \varphi)$$

**2.53** For every conditional sentence $\varphi$ of $\Sigma$ there exists
a classifying sentence in the theory one level up,

$$\mathsf{Decidable}_\Sigma(\varphi) \in \mathsf{Sen}(\Sigma^\uparrow)$$

such that

$$M \in \mathsf{Mod}_{\mathsf{dec}}(\Sigma, \varphi) \iff M \models_{\Sigma^\uparrow} \mathsf{Decidable}_\Sigma(\varphi)$$

**2.54** The predicate *"$\varphi$ is decidable in this algebra"* is
therefore expressible inside the ambient institution. The two sub-classes
are ordinary sub-fibers defined by satisfaction of a higher sentence.
Re-indexing transports that higher sentence, and fiber-wise uniformity is
restored.

---

## 3 · The pool

**3** $\mathcal{P}$ is a **parameter of the satisfaction relation**, not a
sort of the signature.

> The theory declares *what* must be judged; $\models$ determines *how* —
> mechanically or by delegation.

**3.1** $\mathcal{P}$ does not appear as a sort in any signature.

**3.11** A signature that declares $\mathcal{P}$ as a sort has
internalized the outside. The ontological separation collapses and
non-identity becomes unenforceable: if the judge is an element of the
algebra, what judges the judge?

**3.2** $\mathcal{P}$ is **opaque**. Het never names a principal
substrate, never enumerates kinds, and never inspects an inhabitant.

**3.21** Het requires only that whatever supplies $\mathcal{P}$ exposes
four predicates.

| predicate | arity | gate | what $\models$ needs it for |
|---|---|---|---|
| $\mathsf{capable}$ | $\mathcal{P} \times \mathsf{Role} \to \mathsf{Bool}$ | decidable | competence filter — can this principal play the role the sentence declares (2.3)? |
| $\pi$ | $X \to \mathsf{Prov}$, for $X$ a principal or an object | decidable | provenance tags; both filters read it |
| $\mathsf{standing}$ | $\mathcal{P} \times S \to \mathsf{Bool}$ | conditional | authorial filter (3.6); classified one level up |
| $\varepsilon$ | $\mathcal{P} \to [0,\infty)$ | decidable | renaming-drift bound reported with the verdict (4.6) |

**3.22** A theory that supplies $\mathcal{P}$ declares all four at these
arities. Conformance is signature inspection — decidable, and requiring
no edge machinery beyond reading the declaration.

**3.23** Het requires nothing further of a supplier. Kinds, substrate
partitions, identity fields, cost tiers, and the population itself are
the supplier's. Naming any of them here would internalize the outside a
second way — not as a sort, but as a stipulated content.

**3.24** $\mathsf{capable}$ is used at exactly one arity,
$\mathcal{P} \times \mathsf{Role}$, everywhere in Het. Its second
argument is $\mathsf{role}(\varphi)$ or $\mathsf{role}(o)$ — the role the
*sentence* or *operation* declares — never the sentence or object itself.
A supplier of $\mathcal{P}$ cannot be asked to inspect Het's sentences; it
does not have them.

**3.3** Three of the four are **belonging predicates**: capability,
non-identity, and standing. They decide whether a principal qualifies at
all. All three are Het's.

**3.31** $\varepsilon$ and cost tier support **ordering** among those
that qualify. Ordering is HetOpt's (8).

**3.32** Het requires $\varepsilon$ be declared so the verdict can carry
its error bar. Het never reads it as a preference.

**3.4** There is **one pool and two filters**. The gate marker selects
which qualification predicate applies, not which pool is consulted.
Distinct pools are not licensed.

### The judgmental filter — non-identity

**3.5** A judgmental sentence dispatches to a judge drawn from its
qualifying set:

$$\mathcal{P}_{\text{judg}}(\varphi, a) = \{\, p \in \mathcal{P} : \mathsf{capable}(p, \mathsf{role}(\varphi)) \wedge \pi(p) \cap \pi(a) = \emptyset \,\}$$

**3.51** Disjointness is measured against **the argument the operation is
applied to**, not against the model in general.

**3.52** Where the argument is the object under audit, $\pi(a) = \pi(M)$
and the two readings coincide. Where the argument is a Proposal, its
provenance is its author's (7.24) and the author need not be the model.
The argument governs.

**3.53** Non-identity is enforced before any judgmental dispatch. It is
decidable — disjointness of finite provenance-tag sets — and belongs to
the decidable fragment.

**3.54** Non-identity is not deferrable to valuation. It is a belonging
predicate, not a preference. A system that dispatches without it is
self-certifying, which is the failure this formalism exists to refuse.

**3.55** Het dispatches to *a* qualifying judge. It does not tier, cost,
or prefer among qualifying judges. Any of them yields a well-formed
verdict, reported with its own $\varepsilon$.

### The authorial filter — standing

**3.6** An authorial operation dispatches to an author drawn from its
qualifying set:

$$\mathcal{P}_{\text{auth}}(o, M) = \{\, p \in \mathcal{P} : \mathsf{capable}(p, \mathsf{role}(o)) \wedge \mathsf{standing}(p, M) \,\}$$

**3.61** Judgment classifies; authorship transforms. Both require an
outside, in opposite directions.

> **Judgment refuses the audited party. Authorship requires standing over it.**

**3.62** Non-identity excludes exactly the arrows authorship needs: the
author of a candidate *is* the party under audit, and enacting a remedy
means revising one's own text. Provenance overlap is the point, not the
defect.

**3.63** Standing is conditional-gated. It is **decidable** when
provenance containment settles it, $\pi(\text{outcome}) \subseteq \pi(p)$,
and **judgmental** otherwise.

**3.64** Standing-judgment terminates at depth one. The standing-judge's
own qualification is plain non-identity, decidable by
provenance-disjointness.

**3.65** That disjointness is relative to the **author**, not to the
audited object. The judge ruling *"does this principal have standing over
that object?"* must not be that principal.

**3.66** Two escalation triggers exist and are not the same.

| trigger | level | reason |
|---|---|---|
| standing is judgmental in this model | **Het** | qualification itself needs a ruling |
| the minimal author cannot close it | **HetOpt** | worth-ordering says escalate |

**3.67** Standing-escalation happens before any valuation is applied.

---

## 4 · The verdict

**4** Satisfaction is quantitative. Every theory declares a **verdict
space** carrying a **metric** $d$.

**4.1** Judges are stochastic. Verdicts carry confidence, distributional
information, and sensitivity to surface features such as naming.

**4.11** Under Boolean satisfaction the satisfaction condition (1.2)
breaks: renaming a sort changes the verdict.

**4.2** Typical verdict spaces are $[0,1]$, a probability simplex
$\Delta^n$, or a strategy lattice.

**4.3** The satisfaction condition is relaxed from strict equivalence to
a **distance bound**:

$$d\!\left(M \models_{\Sigma'} \mathsf{Sen}(\sigma)(\varphi),\;\; \mathsf{Mod}(\sigma)(M) \models_\Sigma \varphi\right) \le \varepsilon$$

where $\varepsilon$ bounds acceptable naming-induced drift.

**4.31** A judge whose confidence shifts from 0.92 to 0.81 under renaming
is within tolerance if $\varepsilon = 0.15$.

**4.4** $d$ is carried by the verdict space the theory declares, not
bolted on. Without $d$ there is nothing for $\varepsilon$ to bound, and
satisfaction falls back to Boolean.

**4.5** $d$ **measures**. It is symmetric. It states how far two verdicts
lie apart under renaming, and nothing about which is better.

**4.51** Reading an order on the verdict space as preference is
valuation, and belongs to HetOpt (8).

**4.6** $\varepsilon$ is reported alongside the verdict — an honest error
bar.

**4.7** Translation-invariance is the **candidate's** burden. A candidate
that adopts obscure naming bears the cost of the judge's drift. The
Proponent must name its structures clearly enough that its strategy
survives renaming (7).

---

## 5 · The semantics

**5** An algebra is a functor into the Kleisli category of the principal
monad:

$$M : T \to \mathbf{Kl}(\mathcal{P})$$

| gate | interpretation |
|---|---|
| `decidable` | an ordinary pure morphism — an actual function on the carrier; factors through $\eta$ |
| `judgmental` | a morphism in $\mathbf{Kl}_{\text{judg}}(\mathcal{P})$ — a computation that may consult the outside |
| `authorial` | a morphism in $\mathbf{Kl}_{\text{auth}}(\mathcal{P})$ — an enactment by a principal with standing. Never pure. |

**5.1** An algebra cannot be a functor into $\mathbf{Set}$.

**5.11** A functor to $\mathbf{Set}$ assigns every operation — including
judgmental ones — to an actual function, that is, to a decision
procedure.

**5.12** Such an algebra would *decide* the judgmental operations,
computing the very judgments the gate marker says no closed system can
discharge on itself. That is 3.54 violated in the semantic dimension.

**5.2** $\mathcal{P}(X)$ is *"an $X$, possibly obtained by a call on a
principal."*

**5.21** The unit $\eta : X \to \mathcal{P}(X)$ is *"no outside needed"*;
decidable data embeds.

**5.22** A judgmental operation is a Kleisli arrow
$A \to \mathcal{P}(B)$.

**5.23** The monad is exactly *what the trip through the outside adds
that the algebra could not generate alone.*

**5.24** Composing pure morphisms with judgmental ones is Kleisli
composition. This is why the fragments interleave without collapsing.

### Provenance

**5.3** The base category carries a **provenance structure**: every
object $X$ is equipped with a provenance map

$$\pi_X : X \to \mathsf{Prov}$$

to a discrete category of provenance tags.

**5.31** Morphisms preserve or strictly externalize provenance.

**5.32** $\mathcal{P}$ is **provenance-strict**:

$$\pi_{\mathcal{P}X} \circ \eta_X = \pi_X, \qquad \pi_{\mathcal{P}X} \circ \mu_X = \pi_{\mathcal{P}^2X}$$

$\eta$ never invents a new author; $\mu$ propagates the outermost author.

### Admissibility

**5.4** Nothing in the plain Kleisli construction prevents $M$ from
sending a judgmental operation to a **constant** arrow
$c_j : a \mapsto \eta(j)$ whose value $j$ is drawn from $M$'s own carrier.
The selection rule never fires; self-reference has been hard-coded into
the interpretation.

**5.41** Judgmental and authorial arrows therefore inhabit their
respective admissibility sub-categories:

$$\mathbf{Kl}_{\text{judg}}(\mathcal{P}) = \{\, f : \pi(f(a)) \cap \pi(a) = \emptyset \,\} \qquad \text{(the outside)}$$

$$\mathbf{Kl}_{\text{auth}}(\mathcal{P}) = \{\, f : \pi(f(a)) \subseteq \pi(p) \ \wedge\ \mathsf{standing}(p, a) \,\} \qquad \text{(the steward)}$$

**5.42** Authorial admissibility is **stronger, not weaker** — not
"anything goes," but "only the principal who holds stewardship may enact
on it." Where judgmental demands disjointness, authorial demands
containment plus standing.

**5.43** Both are sub-categories of the **same** $\mathbf{Kl}(\mathcal{P})$.
Distinct monads would mean distinct principal pools, which 3.4 does not
license.

**5.44** Admissibility is gate-relative, and this is licensed.
Decidability is already fiber-relative and classified one level up
(2.53); gate-relative admissibility is the same pattern applied to
provenance instead of decidability. The institution's uniformity lives in
*one $\models$, gate-dispatched* — not in having one admissibility
predicate.

### Gate-faithfulness

**5.5** An algebra is **gate-faithful** when every `decidable` operation
factors through $\eta$, every `judgmental` operation is a
judgmentally-admissible Kleisli arrow, and every `authorial` operation is
an authorially-admissible Kleisli arrow.

**5.51** $\mathsf{Mod}(\Sigma)$ consists **only** of gate-faithful
algebras.

**5.52** A gate-faithful algebra cannot launder a judgmental operation
into a decidable one, and cannot dispatch judgment to itself. The refusal
is enforced at the level of the model category, not as a post-hoc
selection rule.

**5.53** Because provenance re-indexes along signature morphisms, the
condition propagates through the fibration. Re-indexing cannot invent a
common author that did not already exist.

### Objects

**5.6** An **object** is an inhabitant of a carrier set $M(S)$ — a
specific datum, an element sitting in the algebra's interpretation of a
sort.

**5.61** A decidable operation on an object runs as a pure morphism: its
result is computed inside the algebra, with no outside.

**5.62** A judgmental operation on an object runs as a Kleisli morphism:
it emits an outside call, and the outcome is obtained only when the
outside answers.

**5.63** An object is therefore **self-governing** — its own algebra runs
its decidable audit — but **not self-closing**: its judgmental
dispositions require the monad's outside.

**5.64** That is autopoiesis without self-loop degeneracy, made precise.

---

## 6 · The tower

**6** An algebra whose carrier contains objects that themselves carry
signature declarations **becomes a theory at the next level**, with its
own fiber of algebras below.

**6.1** The tower is a **fibered category** — the Grothendieck
construction over the category of theories.

| level | role in fibration |
|---|---|
| theory $T$ | object in the base category $\mathbf{B}$ |
| $\mathsf{Mod}(T)$ | fiber over $T$ — the category of $T$-algebras |
| $\sigma : T_1 \to T_2$ | base morphism — a signature morphism |
| $\mathsf{Mod}(\sigma)$ | re-indexing — restricts $T_2$-algebra views to $T_1$ |
| $\models_T$ | fiber-wise relation: algebra × sentence → verdict |

**6.11** The satisfaction relation is the same at every level. What
changes is which theory's $\models$ is invoked and which principal pool
is available.

**6.12** The Kleisli construction iterates: the same algebra becomes the
theory whose satisfaction relation tests algebras one level below.

**6.13** The tower is **semantic at every level**. The fibration carries
the Kleisli structure through re-indexing, and gate-faithfulness is
preserved by signature morphisms.

### Two kinds of pointing

**6.2** Two distinct relations both look like "pointing," and run in
opposite directions.

| | direction | what it is |
|---|---|---|
| **conformance declaration** | up (concrete → abstract) | a *model* declares the theory it interprets. This is what a checker walks. |
| **signature morphism** | down (abstract → concrete) | the arrow selecting the structure a theory's algebras must carry — the semantic map whose existence the declaration asserts. |

**6.21** The two are duals of one edge. The up-pointing declaration is
what the satisfaction-checker walks to find the theory to test against;
the down-pointing morphism is the truth-condition the declaration claims
to satisfy.

**6.22** A conformance declaration is carried by **models only**. A
theory does not carry one.

**6.23** A model with no declared theory is a set of records with no law
to be measured against. There is nothing for $\models$ to evaluate.

**6.24** A conformance declaration is not a signature morphism and cannot
serve as one. Theory-to-theory morphisms are the arrows of
$\mathbf{Sign}$ (1.11) and are constitutive of the institution.

**6.25** Three relations must not be conflated: a population interprets a
law (6.22); a theory supplies $\mathcal{P}$ (3.21, checked by signature
inspection); a theory extends another (a morphism in $\mathbf{Sign}$).

### The gate law

**6.3** Gate markers may be preserved or increased along morphisms —
`decidable` → `decidable` or `judgmental`; `judgmental` → `judgmental`.

**6.31** No morphism may launder a judgmental predicate into a decidable
one. This is 3.54 at the morphism level.

### Termination

**6.4** The tower terminates on a **decidable well-formedness predicate**
$W$ on signatures.

**6.41** $W(\Sigma)$ holds when: $\Sigma$ declares at least one sort and
at least one operation; every operation carries a gate marker (2, 2.2);
every judgmental operation declares a competence role (2.3); every
authorial operation declares a standing predicate (2.4); every
conditional operation names a classifying sentence (2.5); and, if
$\Sigma$ supplies $\mathcal{P}$, it declares the four predicates of 3.21
at their stated arities.

**6.42** Each clause is decidable by inductive inspection of the
declaration. $W$ invokes no judge.

**6.43** $W$ is the floor the regress terminates on. It is not
gate-marked and is not itself a Het theory; asking it to be one would ask
the floor to stand on itself.

**6.44** $W$ checks **declaration, never adequacy**. It never asserts
that any concrete principal satisfies its own predicates, nor that the
pool is non-empty.

**6.5** Adequacy lives one level below, inside the theories that actually
invoke judges. For a judgmental sentence $\varphi$ of a theory $T$:

$$\mathsf{Adequate}_T(\varphi) \equiv \text{“a qualifying non-identical judge for } \varphi \text{ exists and returns a verdict”}$$

**6.51** That sentence is itself **judgmental**, discharged by an outside
call exactly when an algebra of $T$ attempts to interpret $\varphi$.

**6.52** Failure of adequacy is an ordinary judgmental failure at the
level where the judge is required. It is not a defect in $W$.

**6.53** Adequacy asks for *a* qualifying judge, not the minimal one.

**6.54** Adequacy is **local, not global**. There is no infinite regress
and no global fixed-point proof.

### Self-grounding

**6.6** Self-grounding is a property of a **pair**, never of one object
alone.

**6.61** Het is self-grounding when its own signature satisfies $W$, and
$W$ is decidable.

**6.62** Neither member stands on itself: the signature is grounded by
satisfying a predicate that is not gate-marked, and the predicate is
grounded by being an ordinary shape-check.

**6.63** The first question is therefore not whether some domain conforms
to Het, but whether Het's own signature satisfies $W$. Answering it
**demonstrates** self-grounding rather than asserting it.

### Signature-claims are not sentences

**6.7** A theory's claims about **its own signature** — that a type is
closed, that two axes are orthogonal, that the theory declares no
population — are clauses of $W$, not sentences.

**6.71** A sentence is evaluated as $M \models \varphi$ against
inhabitants of a carrier. A signature-claim has no such inhabitant to
test, and walking a population cannot check it.

**6.72** Such a claim carries no equation because there is nothing for
$\models$ to compute. **A decidable sentence with no equation is a
mis-filing, not an omission** — the emptiness is the diagnostic.

---

## 7 · The game

**7** Satisfaction is a two-player game. A sentence is satisfied iff the
**Proponent** has a winning strategy.

**7.1** The Proponent is the candidate algebra, asserting
$M \models \varphi$. The **Opponent** is the environment, which may query
an oracle — the judge.

**7.11** Decidable predicates are games with finite, mechanizable winning
strategies: the tree is bounded and the strategy is a decision procedure.

**7.12** Judgmental predicates are games where the Opponent has oracle
access: the tree may be unbounded, and the strategy involves querying the
oracle at specific nodes.

**7.13** Static satisfaction cannot say who is right when judge and
candidate disagree. The game can: the Proponent may contest, and the
contest is itself a move (7.3).

### The pass

**7.2** The audit-rectify pass is the game in operation — a chain of
principals, each acting on what the previous one produced. The gate says
*how* each move is settled; the table says *by whom*, and relative to
whose authorship.

| game move | operation | gate | acts | result |
|---|---|---|---|---|
| a violation is found | `audit` | decidable, or judgmental per $\varphi$ | nobody, or a judge disjoint from $M$ | Verdict |
| the Proponent answers | `propose` | **authorial** | an author with standing over $x$ | Proposal |
| the Opponent rules | `dispose` | judgmental | a judge disjoint from **the Proposal** | Disposition |
| the Proponent applies it | `enact` | authorial | an author with standing over $x$ | the revised object |

**7.21** `propose` is **authorial**. Answering a verdict is the
Proponent's move, and producing content about an object is authorship,
which requires standing over it (3.6).

**7.22** A judgmental gate on `propose` would dispatch under the
disjointness filter (3.5), that is, to the Opponent's side — making the
Opponent play the Proponent's move.

**7.23** That the remedy is sometimes mechanically determined and
sometimes requires assessment is a statement about the author's
difficulty, not about whether an outside is needed. Authorship is
required either way.

**7.24** A Proposal's provenance is its **author's**:
$\pi(\mathsf{propose}(x, v)) \subseteq \pi(p)$ for the authoring
principal $p$. Without this, 3.5 cannot be evaluated at `dispose`.

### The Proposal vocabulary

**7.3** A Proposal is one of exactly two.

| | means | licenses |
|---|---|---|
| `remedy` | *"the verdict stands; here is the fix"* | `enact` on acceptance |
| `dispute` | *"the verdict is wrong; the object stands as authored"* | nothing to enact |

**7.31** A `dispute` is still judged. The Opponent rules on it exactly as
on a `remedy`; an author does not overturn a verdict by asserting it.

**7.32** `dispute` is the only path to contest a verdict. `propose` is
defined only on a non-conforming verdict, so without it an author who
believed the audit wrong would have to author a remedy for a diagnosis
they dispute, in order to obtain a vehicle for disputing it.

### The Disposition vocabulary

**7.33** A `remedy` carries an **edit** — what would be done to the object.
The edits are the theory's, not Het's (11.12); Het requires only that a
remedy name one, and that `enact` apply it.

**7.4** A Disposition is one of exactly five.

| | terminal | affirming | who acts next |
|---|---|---|---|
| `accept` | ✓ | ✓ | the author enacts |
| `reject-diagnosis` | ✓ | ✗ | nobody — the audit was wrong; the object stands |
| `reject-remedy` | ✗ | ✗ | the author re-proposes, carrying the reason |
| `defer` | ✗ | ✗ | a prerequisite is required first |
| `raises-questions` | ✗ | ✗ | the auditor clarifies; the object re-enters |

**7.41** A Disposition is a **ruling, not a revision**. Something must
apply it, and that something is an author with standing (3.6).

**7.42** No Disposition amends a Proposal. A judge that amends is
*transforming*, not classifying; and being provenance-disjoint from the
object (3.5), it cannot hold standing over a modification it has just
authored (3.6). Any amending variant would require one principal to
satisfy two opposite conditions on one object.

**7.43** `reject-remedy` may carry a **reason**, which is advisory prose
and **not an edit**. Stating why a remedy fails is classification;
supplying the replacement is authorship. The author re-proposes with the
reason in hand.

**7.44** A re-proposal carries the chain of prior dispositions and their
reasons. Without them an author can cycle indefinitely on the same
objection, and nothing downstream could detect it.

### Enactment

**7.5** `enact` is what makes the pass an **endofunctor** rather than a
one-way funnel into a verdict.

**7.51** A terminal-and-affirming Disposition licenses `enact`; it does
not guarantee the edit lands.

**7.52** Where the revised object enters another governed container,
**that container's own $\models$ runs** — the pass composed with itself
under 6 — and may refuse it.

**7.53** An authorization to edit is not a licence to violate the
target's law. `enact` has two failure points: the Disposition may
withhold it, and the target may refuse it.

### Panels

**7.6** Panels are $\models$ with more than one judge — the game with an
enlarged oracle-move set. They are not a separate construction.

**7.61** A Proponent winning strategy in the original game remains
winning in the composite; additional oracle answers can only strengthen
the Opponent.

---

## 8 · The cut

**8** **Het settles belonging. HetOpt orders what belongs.**

$$\textbf{Het} = \text{judgmental institution} + \text{gate-marked } \models + \text{metric verdict space}$$

$$\textbf{HetOpt} = \textbf{Het} + V$$

**8.1** Metric and preference are the same categorical furniture read two
ways. A metric space *is* a category enriched over
$([0,\infty], \ge, +)$, and quantale-enrichment is the general form. They
are not the same **role**.

| | what it does | where it lives |
|---|---|---|
| $d$ — verdict metric | symmetric; how far two verdicts lie apart under renaming | **Het** |
| $V$ — worth-law | orders a conforming set by preference | **HetOpt** |

**8.2** The cut is drawn at **valuation itself**, not at any one
application of it.

**8.21** A Het theory declares no worth-law $V$, and does not declare the
minimal-judge rule.

**8.22** $V$ applies wherever Het has produced a conforming set.

| Het produces | HetOpt orders it by | yielding |
|---|---|---|
| the qualifying judges for a sentence | cost tier, then $\varepsilon$ | the **minimal-judge rule** |
| the qualifying authors for an operation | cost tier | the **minimal-author rule** |
| the conforming algebras of a theory | the declared worth-law | ranked candidates |

**8.23** One piece of machinery, two levels — 6 applied to valuation.
Judge selection and candidate ranking are not two features but one:
*conformance, then valuation*, instantiated twice.

**8.3** The cut lands here because the order is filter first, then
optimize. Non-identity is enforced at the model-category level as an
admissibility restriction on Kleisli arrows (5.41); the minimal-judge
rule optimizes only among arrows that have already survived that filter.
Het is the filter; HetOpt is the optimization.

**8.31** It lands no later: non-identity cannot move to HetOpt (3.54).

**8.32** It lands no earlier: Het has no $V$ anywhere, HetOpt has $V$
everywhere. Keeping a valuation in Het for judges while withholding one
for candidates would leave *"why judges and not candidates?"* with no
answer beyond stipulation.

**8.4** HetOpt is a theory extension in the ordinary sense.
$\mathbf{Sign}_{\textbf{HetOpt}}$ extends $\mathbf{Sign}_{\textbf{Het}}$
with the declaration of $V$, and
$\textbf{Het} \hookrightarrow \textbf{HetOpt}$ carries Het-algebras into
the HetOpt fiber by re-indexing.

**8.41** In HetOpt the enrichment base $V$ **is** the metric $d$, and the
fibers become $V$-enriched. In Het the verdict space carries $d$ alone.

---

## 9 · Composition

**9** When two theories are combined, their principal pools combine, and
the composite is again a judgmental institution.

**9.1** $\mathcal{P}_{1+2} = \mathcal{P}_1 + \mathcal{P}_2$, provenance
preserved componentwise.

**9.11** The non-identity restriction extends to the composite Kleisli
category.

**9.12** The qualifying set of the composite is the union of the
component qualifying sets, each still filtered by non-identity.

**9.2** Kinds form the disjoint union $K_1 \sqcup K_2$.

**9.3** The composite qualifying set is non-empty whenever either
component's was. Adequacy composes.

**9.4** Theory combination is closed.

---

## 10 · Evaluation

**10** $\models$ is defined by dispatch on the gate marker:

$$
M \models \varphi \;=\;
\begin{cases}
\mathsf{check}(M, \varphi) & \varphi\text{ decidable} \\[2pt]
\mathsf{dispatch}(\varphi, a, \mathcal{P}_{\text{judg}}) & \varphi\text{ judgmental} \\[2pt]
\mathsf{dispatch}(\varphi, a, \mathcal{P}_{\text{auth}}) & \varphi\text{ authorial} \\[2pt]
M \models_{\Sigma^\uparrow} \mathsf{Decidable}_\Sigma(\varphi) \;?\; \mathsf{check} : \mathsf{dispatch} & \varphi\text{ conditional}
\end{cases}
$$

**10.1** $\models$ is run over every $\varphi \in \mathsf{Sen}(\Sigma)$
against $M$.

**10.2** Dispatch is two operations, and the first is decidable:

$$\text{qualifying} = \{\, p \in \mathcal{P} : \mathsf{capable}(p, \mathsf{role}(\varphi)) \wedge \pi(p) \cap \pi(a) = \emptyset \,\}$$

$$\mathsf{dispatch} = \text{any member of } \text{qualifying}$$

**10.21** $a$ is **the argument the operation is applied to** (3.51) —
the object at `audit`, the Proposal at `dispose`. Reading $\pi(M)$ in its
place is the error 3.52 excludes.

**10.22** Both conjuncts read only the four predicates of 3.21. The
conformance half requires no judge to test: it is set operations over
declared predicates.

**10.23** Returning *any* qualifying judge is not a decision deferred; it
is what Het specifies (3.55). The minimal-judge rule replaces
*any* with *argmin*, and that substitution is the seam where HetOpt lands
(8.22).

---

## 11 · The surface

**11** A theory written *in* Het declares four things and nothing else: its
sorts, its edits, its sentences with their gates, and a role for each
judgmental sentence.

**11.1** Het declares the **slots**. The theory fills them. This is the
division that runs through the whole document: Het says what must be
declared and under what condition it is settled; it never says what the
content is.

**11.11** Het requires that a judgmental sentence declare a role (2.31). It
does not enumerate roles. `taxonomist`, `triager`, `chord-reader` are the
theory's.

**11.12** Het requires that a `remedy` carry an **edit** (7.33) and that
`enact` apply one (7.5). It does not enumerate edits. Whether the domain's
edits are `amend | remove | relocate`, or `fix | won't-fix | duplicate |
reprioritize`, is the theory's.

**11.13** Het requires a verdict space carrying a metric (4.1). It does not
say what the space is.

**11.14** Het requires that a supplier of $\mathcal{P}$ expose four
predicates (3.21). It does not say what a principal is made of (3.22).

**11.2** Consequently `enact` is **generic over the theory's edit type**. Het
cannot apply an edit it did not name. The theory supplies the application;
Het governs only who may perform it (3.4) and whether the result is admitted
(7.52).

**11.21** This is not a limitation worked around. Het governs *who may act,
and under what condition*. What the act **is** belongs to the domain, and a
formalism that enumerated edits would be legislating domains it does not
know.

### The decidable fragment

**11.3** A decidable sentence is **any total predicate of the host language
on the model**. Het names no logical fragment.

**11.31** The two gates are not two fragments. They are two **signatures**,
and the host language's type system separates them:

| gate | the sentence's form |
|---|---|
| `decidable` | $M \to \mathsf{Bool}$ — the model alone |
| `judgmental` | $M \times \mathsf{Qualified}\langle \mathsf{role}(\varphi) \rangle \to \mathsf{Verdict}$ |

**11.32** A decidable sentence therefore *cannot* consult $\mathcal{P}$: no
parameter admits a principal, and the qualifying token has no constructor
outside 3.5. The prohibition is not a rule the author is asked to respect;
it is a term that cannot be written.

**11.33** Mis-marking is likewise not a claim that could be false. Marking a
sentence `decidable` gives it the decidable signature. A body needing an
outside will not typecheck in that position.

**11.34** This replaces fragment-membership as the mechanism of
gate-honesty. A chosen fragment is a constraint someone must check; a
signature is checked by the host language's compiler, which does not know
Het exists and cannot be persuaded.

**11.4** Two properties the signature does **not** secure. Both are stated
as limits.

**11.41** **Termination.** A host language admitting non-termination admits
a `decidable` sentence that does not terminate. Het does not check this. The
type proves the sentence was *evaluated as* a machine check, not that the
check *halts*.

**11.42** **Purity.** The decidable signature excludes $\mathcal{P}$. It does
not exclude the world: a predicate on the model may still reach a network, a
clock, a file. "Consults no outside" is exact about **Het's** outside — the
principal pool — and silent about every other.

**11.43** Neither is closed here. Closing 11.41 requires a total language;
closing 11.42 requires an effect system. Het requires neither, and a Het
built on a host that supplies them inherits the guarantee for free.

---

## 12 · Vocabulary

Terms not listed here are not part of the formalism. An encoding that
introduces one has drifted.

### The institution

| term | symbol | meaning | prop |
|---|---|---|---|
| **signature** | $\Sigma$ | a theory declaration: sorts, operation symbols with arities, gate markers, and the laws the theory declares | 1.3 |
| **signature category** | $\mathbf{Sign}$ | the category of signatures; objects are theories, morphisms are signature morphisms | 1.11 |
| **sentence** | $\varphi$ | an element of $\mathsf{Sen}(\Sigma)$; a claim over the signature, carrying a gate marker | 1.12, 2 |
| **sentence functor** | $\mathsf{Sen}$ | $\mathbf{Sign} \to \mathbf{Set}$ | 1.12 |
| **algebra**, **model** | $M$ | an interpretation of a signature; here a functor $T \to \mathbf{Kl}(\mathcal{P})$ | 1.13, 5 |
| **model functor** | $\mathsf{Mod}$ | $\mathbf{Sign}^{\text{op}} \to \mathbf{Cat}$ | 1.13 |
| **satisfaction relation** | $\models$ | the mechanism testing an algebra against a sentence; the locus of the entire extension | 1, 1.31 |
| **satisfaction condition** | | truth is invariant under change of notation; the institution's only axiom | 1.2, 4.3 |
| **signature morphism** | $\sigma$ | a structure-preserving map of signatures; translates sentences forward and algebras backward | 1.11, 6.24 |
| **re-indexing** | $\mathsf{Mod}(\sigma)$ | transport of algebras along a signature morphism | 6.1 |
| **sort** | $S$ | a type declared by the signature, interpreted as a carrier $M(S)$ | 1.3 |
| **object** | $x : M(S)$ | an inhabitant of a carrier — a specific datum under judgment | 5.6 |
| **conformance declaration** | | the up-pointing edge on a model: "this population interprets that law" | 6.2, 6.22 |

### Judgment

| term | symbol | meaning | prop |
|---|---|---|---|
| **gate marker** | | the annotation fixing a sentence's or operation's satisfaction mechanism | 2 |
| **decidable** | | satisfaction is machine-checked by standard equational logic | 2.1 |
| **judgmental** | | satisfaction dispatches to a judge; the verdict *is* the outcome | 2.1 |
| **authorial** | | the operation transforms rather than classifies; dispatches to an author | 2.1, 3.6 |
| **conditional** | | decidability depends on the algebra; classified one level up | 2.1, 2.5 |
| **competence role** | $\mathsf{Role}$ | what a judgmental sentence needs done; declared pointwise by the sentence | 2.3 |
| **principal pool** | $\mathcal{P}$ | the pool dispatched to by non-decidable gates. **A parameter of $\models$, never a sort** | 3 |
| **judge** | | a principal filtered by capability and non-identity; renders a verdict | 3.5 |
| **author** | | a principal filtered by capability and standing; enacts a ruling | 3.6 |
| **standing** | | an author holds stewardship of what it enacts on. Conditional-gated | 3.6, 3.63 |
| **non-identity** | | a judge must not be the author of what it judges. Decidable; enforced before dispatch | 3.5, 3.53 |
| **belonging predicate** | | a predicate deciding whether a principal qualifies at all: capability, non-identity, standing | 3.3 |
| **qualifying set** | | the principals surviving the gate's belonging predicates. Het's output | 3.5, 3.6, 10.2 |
| **kind** | $K_i$ | a partition of $\mathcal{P}$ by substrate. The supplier's, not Het's | 3.23, 9.2 |
| **cost tier** | | ordering on principals by resource consumption. **HetOpt** | 3.31, 8.22 |
| **minimal-judge rule** | | select the cheapest qualifying judge, breaking ties by lowest $\varepsilon$. **HetOpt** | 8.22 |
| **minimal-author rule** | | select the cheapest principal with standing, escalating when it cannot close. **HetOpt** | 3.66, 8.22 |
| **renaming-robustness** | $\varepsilon$ | tolerated verdict drift under signature morphisms. Reported in Het; a criterion in HetOpt | 3.32, 4.6 |
| **adequacy** | | that *a* qualifying non-identical judge exists and returns a verdict. Judgmental, discharged where invoked | 6.5 |
| **gate law** | | gate markers may be preserved or increased along morphisms, never laundered downward | 6.3 |

### Semantics

| term | symbol | meaning | prop |
|---|---|---|---|
| **Kleisli category** | $\mathbf{Kl}(\mathcal{P})$ | where algebras land; judgmental and authorial operations are Kleisli arrows, decidable ones factor through $\eta$ | 5 |
| **admissibility sub-categories** | $\mathbf{Kl}_{\text{judg}}$, $\mathbf{Kl}_{\text{auth}}$ | gate-selected restrictions: provenance-disjoint versus containment-plus-standing | 5.41 |
| **provenance** | $\pi_X$ | a map to provenance tags, carried by every object; strict under $\eta$ and $\mu$ | 5.3, 5.32 |
| **gate-faithful** | | an algebra whose decidable operations are pure, judgmental ones judgmentally-admissible, authorial ones authorially-admissible | 5.5 |
| **fibration** | | the Grothendieck construction over the category of theories | 6.1 |
| **fractal property** | | an algebra carrying its own signature declaration becomes a theory at the next level | 6 |
| **well-formedness predicate** | $W$ | the decidable shape-check on signatures on which the tower terminates | 6.4 |

### Verdicts, worth, and the two formalisms

| term | symbol | meaning | prop |
|---|---|---|---|
| **verdict** | | a judge's answer; the satisfaction outcome for a judgmental sentence | 2.1 |
| **verdict space** | | the space verdicts inhabit — $[0,1]$, a simplex $\Delta^n$, a strategy lattice | 4, 4.2 |
| **metric** | $d$ | distance on the verdict space. **Measures** drift; symmetric | 4, 4.5 |
| **worth-law**, **valuation** | $V$ | a quantale whose order **ranks** a conforming set. **HetOpt only** | 8, 8.22 |
| **belonging**, **conformance** | $\chi$ | the objecthood predicate: what a candidate must satisfy to be a conforming algebra | 8 |
| **Het** | | judgmental institution + gate-marked $\models$ + metric verdict space. Settles belonging | 8 |
| **HetOpt** | | Het + $V$. Orders what belongs — qualifying judges and conforming candidates alike | 8 |

### The game

| term | meaning | prop |
|---|---|---|
| **Proponent** | the candidate algebra, asserting $M \models \varphi$ | 7.1 |
| **Opponent** | the environment; may query the judge as oracle | 7.1 |
| **winning strategy** | what satisfaction amounts to: the Proponent has one | 7 |
| **audit** | a violation is found; produces a Verdict | 7.2 |
| **propose** | the Proponent answers; authorial; produces a Proposal | 7.2, 7.21 |
| **dispose** | the Opponent rules; judgmental; produces a Disposition | 7.2 |
| **enact** | the Proponent applies a terminal-and-affirming Disposition; produces the revised object | 7.2, 7.5 |
| **panel** | $\models$ with more than one judge; the game with an enlarged oracle-move set | 7.6 |

---

## 13 · The limit

**13** Het places **no bound on re-entry**.

**13.1** If no acceptable remedy exists, `reject-remedy` re-enters
forever (7.4) and the object never leaves the loop.

**13.2** Het cannot close this. The available answers — evict the object,
bound the attempts, or accept non-conformance as declared debt — are all
worth-shaped, and 8.21 forbids a Het theory from declaring a worth-law.

**13.3** This is the one state that belonging alone produces and cannot
exit. The bound belongs to HetOpt.

**13.4** It is stated as a limit rather than closed by an eviction rule,
which would be a worth-law under another name.
