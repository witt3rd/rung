# Het — The Formalism

**Status:** normative. This document is the prose statement of Het. It is
the provenance source for the encoding (`het/theory.yaml`) and the
conformance target for the enforcement (`het-rs`).

Where this document and any other prose in this repository disagree, this
document governs. `docs/institutional_judgment.md` is development
archaeology — the record of how the formalism was derived — and is not
normative. `README.md` orients; it does not specify.

**Scope.** This document specifies **Het**: the judgmental institution,
gate-marked satisfaction, the principal pool, quantitative satisfaction,
Kleisli semantics, and the tower. It specifies **HetOpt** only at its
seam — what HetOpt adds and where. HetOpt's own content is out of scope
until it ships.

---

## 1. The institution

Het is built on institution theory (Goguen & Burstall). An **institution**
is a quadruple $(\mathbf{Sign}, \mathsf{Sen}, \mathsf{Mod}, \models)$:

| component | type | meaning |
|---|---|---|
| $\mathbf{Sign}$ | category | signatures — theory declarations |
| $\mathsf{Sen}$ | $\mathbf{Sign} \to \mathbf{Set}$ | sentences over each signature |
| $\mathsf{Mod}$ | $\mathbf{Sign}^{\text{op}} \to \mathbf{Cat}$ | algebras over each signature |
| $\models_\Sigma$ | $\subseteq \lvert\mathsf{Mod}(\Sigma)\rvert \times \mathsf{Sen}(\Sigma)$ | the satisfaction relation |

The institution's single axiom is the **satisfaction condition** — truth is
invariant under change of notation:

$$M \models_{\Sigma'} \mathsf{Sen}(\sigma)(\varphi) \iff \mathsf{Mod}(\sigma)(M) \models_\Sigma \varphi$$

**The signature layer is standard.** Het's entire extension is in $\models$.

### 1.1 The fundamental expression

One relation:

$$M \models_\Sigma \varphi$$

A model $M$ satisfies sentence $\varphi$ under signature $\Sigma$. Every
other structure in Het — the tower, the layers, the addressing — is
bookkeeping around this one relation.

**There is no encoding layer above $\Sigma$.** There is $\Sigma$ (a gate-marked
sentence set), $M$ (an interpretation), and one gate-dispatched evaluator.

---

## 2. Gate-marked satisfaction

Every sentence and every operation carries a **gate marker** fixing how
satisfaction is computed.

| gate | satisfaction mechanism |
|---|---|
| `decidable` | $M \models \varphi$ is machine-checked. Standard equational logic. |
| `judgmental` | $M \models \varphi$ dispatches to a **judge** — an inhabitant of the principal pool $\mathcal{P}$. The judge's verdict IS the satisfaction outcome. |
| `authorial` | The operation *transforms* the object rather than classifying it — or produces new content about it. It dispatches to an **author**, also from $\mathcal{P}$, holding standing over the object. Both `propose` and `enact` are authorial (§6.1.1). |
| `conditional` | Decidability depends on the specific algebra. The condition is classified by a sentence of the theory one level up (§2.5). |

### 2.1 Normative requirements

**N1.** Every operation in a signature MUST carry an explicit gate marker.
An operation without one is not a well-formed declaration.

**N2.** The gate marker MUST be one of exactly `decidable`, `judgmental`,
`authorial`, `conditional`. No other value is well-formed.

**N3.** Every `judgmental` operation MUST declare the **competence role**
required to discharge it. A role, not a kind: kind is what a principal is
made of and belongs to whichever theory supplies $\mathcal{P}$ (§2.2); role
is what the sentence needs done, and only the sentence's own theory knows
that. This pointwise declaration is what lets $\models$ resolve a judge —
there is no global map from sentences to competences, and none is needed.

**N4.** Every `authorial` operation MUST declare a standing predicate.

**N5.** Every `conditional` operation MUST name a classifying sentence
(§2.5). That classifying sentence MUST NOT itself be `judgmental` — a
judgmental classifier reopens the regress §5.4 closes.

### 2.2 The principal pool $\mathcal{P}$

$\mathcal{P}$ is **not a sort of the signature.** It is a **parameter of
the satisfaction relation.**

> The theory declares *what* must be judged; $\models$ determines *how* —
> mechanically or by delegation.

**N6.** $\mathcal{P}$ MUST NOT appear as a sort in any signature. A
signature that declares it as a sort has internalized the outside; the
ontological separation collapses and non-identity becomes unenforceable
(if the judge is an element of the algebra, what judges the judge?).

#### The interface

$\mathcal{P}$ is **opaque to Het.** Het never names a principal substrate,
never enumerates kinds, and never inspects an inhabitant. It requires only
that whatever supplies $\mathcal{P}$ exposes four predicates:

| predicate | arity | gate | what $\models$ needs it for |
|---|---|---|---|
| $\mathsf{capable}$ | $\mathcal{P} \times \mathsf{Role} \to \mathsf{Bool}$ | decidable | competence filter — can this principal play the role the sentence declares (N3)? |
| $\pi$ | $X \to \mathsf{Prov}$, for $X$ a principal or an object | decidable | provenance tags; both filters read it |
| $\mathsf{standing}$ | $\mathcal{P} \times S \to \mathsf{Bool}$ | conditional | authorial filter (§2.4); classified one level up |
| $\varepsilon$ | $\mathcal{P} \to [0,\infty)$ | decidable | renaming-drift bound reported with the verdict (§3) |

**N6a.** A theory that supplies $\mathcal{P}$ MUST declare all four with
these arities. **Conformance is signature inspection** — decidable, and
requiring no edge machinery beyond reading the declaration.

**N6b.** Het MUST NOT require anything further of a supplier. Kinds,
substrate partitions, identity fields, cost tiers, and the population itself
are the supplier's, not Het's. Naming any of them here would internalize the
outside a second way — not as a sort, but as a stipulated content.

**Capability, non-identity, and standing are belonging predicates.** They
decide whether a principal qualifies at all. All three are Het's.

$\varepsilon$ and cost tier support **ordering** among those that qualify.
Ordering is HetOpt's (§7). Het requires $\varepsilon$ be *declared* so the
verdict can carry its error bar; it never reads it as a preference.

### 2.3 The judgmental gate — non-identity

A judgmental sentence dispatches to **a** judge drawn from its qualifying
set. The sentence declares the role it needs (N3); the qualifying set is
those principals capable of that role whose provenance is disjoint from the
**argument** the operation is applied to (N6d):

$$\mathcal{P}_{\text{judg}}(\varphi, a) = \{\, p \in \mathcal{P} : \mathsf{capable}(p, \mathsf{role}(\varphi)) \wedge \pi(p) \cap \pi(a) = \emptyset \,\}$$

At `audit` the argument *is* the model, so this reads
$\pi(p) \cap \pi(M) = \emptyset$ — the familiar form.

**N6c.** $\mathsf{capable}$ is used at **exactly one arity** —
$\mathcal{P} \times \mathsf{Role}$ — everywhere in Het. Where earlier drafts
wrote $\mathsf{capable}(p, \varphi)$ or $\mathsf{capable}(p, o)$, the second
argument is $\mathsf{role}(\varphi)$ or $\mathsf{role}(o)$ respectively: the
role the *sentence* or *operation* declares, never the sentence or object
itself. A supplier of $\mathcal{P}$ cannot be asked to inspect Het's
sentences — it does not have them.

**N6d.** Disjointness is measured against **the argument the operation is
applied to**, not against the model in general:

$$\mathcal{P}_{\text{judg}}(\varphi, a) = \{\, p \in \mathcal{P} : \mathsf{capable}(p, \mathsf{role}(\varphi)) \wedge \pi(p) \cap \pi(a) = \emptyset \,\}$$

At `audit` the argument is the object, so $\pi(a) = \pi(M)$ and the two
readings coincide. **At `dispose` they do not**: the argument is a Proposal,
whose provenance is its author's (N32b), and the author need not be the model.

Earlier drafts stated this twice and inconsistently — N22 measured against the
argument, the dispatch rule against the model — leaving a hole exactly where
the two diverge. A judge that authored a Proposal is disjoint from the model
by construction, so under the model-relative reading it passes the filter and
**may dispose on its own proposal.** N6d closes it: the argument governs, and
N22's semantic condition and the operational dispatch rule now say the same
thing.

**N7 (P0).** Non-identity MUST be enforced before any judgmental dispatch.
It is decidable — provenance-tag disjointness over finite sets — and
belongs to the decidable fragment.

**N8.** Non-identity MUST NOT be deferred to HetOpt. It is a belonging
predicate, not a preference. A Het that dispatches without it is
self-certifying, which is the failure the formalism exists to refuse.

**N9.** Het dispatches to *a* qualifying judge. Het MUST NOT tier, cost,
or prefer among qualifying judges. Any of them yields a well-formed
verdict, reported with its own $\varepsilon$.

### 2.4 The authorial gate — standing

Judgment classifies; authorship **transforms**. Both require an outside,
in opposite directions:

> **Judgment refuses the audited party. Authorship requires standing over it.**

Non-identity excludes exactly the arrows authorship needs — the author of
a candidate *is* the party under audit, and enacting a remedy means
revising one's own text. **Provenance overlap is the point, not the defect.**

$$\mathcal{P}_{\text{auth}}(o, M) = \{\, p \in \mathcal{P} : \mathsf{capable}(p, \mathsf{role}(o)) \wedge \mathsf{standing}(p, M) \,\}$$

**N10.** There is **one pool, two filters.** The gate marker selects which
qualification predicate applies, not which pool is consulted. Distinct
pools are not licensed.

**N11.** Standing is **conditional-gated**:

- **decidable** when provenance containment settles it:
  $\pi(\text{outcome}) \subseteq \pi(p)$
- **judgmental** otherwise — the standing question dispatches to a judge

**N12.** Standing-judgment MUST terminate at depth one. The
standing-judge's own qualification is plain non-identity, decidable by
provenance-disjointness. Note the disjointness is relative to the
**author**, not to the audited object: the judge ruling *"does this
principal have standing over that object?"* must not be that principal.

**N13.** Two escalation triggers exist and are **not** the same:

| trigger | level | reason |
|---|---|---|
| standing is judgmental in this model | **Het** | qualification itself needs a ruling |
| the minimal author cannot close it | **HetOpt** | worth-ordering says escalate |

Standing-escalation happens before any valuation is applied.

### 2.5 The conditional gate

A conditional gate means the mode of satisfaction depends on the specific
algebra. The fiber $\mathsf{Mod}(\Sigma)$ is partitioned:

$$\mathsf{Mod}_{\mathsf{dec}}(\Sigma, \varphi) \quad\text{and}\quad \mathsf{Mod}_{\mathsf{jud}}(\Sigma, \varphi)$$

**N14.** For every conditional sentence $\varphi$ of $\Sigma$ there MUST
exist a classifying sentence

$$\mathsf{Decidable}_\Sigma(\varphi) \in \mathsf{Sen}(\Sigma^\uparrow)$$

in the theory one level up, such that

$$M \in \mathsf{Mod}_{\mathsf{dec}}(\Sigma, \varphi) \iff M \models_{\Sigma^\uparrow} \mathsf{Decidable}_\Sigma(\varphi)$$

The predicate *"$\varphi$ is decidable in this algebra"* is itself
expressible inside the ambient institution. The two sub-classes become
ordinary sub-fibers defined by satisfaction of a higher sentence.
Re-indexing transports that higher sentence; fiber-wise uniformity is
restored.

---

## 3. Quantitative satisfaction

Standard institutions have Boolean satisfaction. **Judges — particularly
LLMs — are stochastic.** Verdicts carry confidence, distributional
information, and sensitivity to surface features like naming. Under a
Boolean institution the satisfaction condition breaks: renaming a sort
changes the verdict.

**N15.** Every Het theory MUST declare a **verdict space** carrying a
**metric** $d$ — typically $[0,1]$, a probability simplex $\Delta^n$, or a
strategy lattice.

The satisfaction condition is relaxed from strict equivalence to a
**distance bound**:

$$d\!\left(M \models_{\Sigma'} \mathsf{Sen}(\sigma)(\varphi),\;\; \mathsf{Mod}(\sigma)(M) \models_\Sigma \varphi\right) \le \varepsilon$$

where $\varepsilon$ bounds acceptable naming-induced drift. A judge whose
confidence shifts from 0.92 to 0.81 under renaming is within tolerance if
$\varepsilon = 0.15$.

**N16.** $d$ is **carried by the verdict space the theory declares**, not
bolted on. Without $d$ there is nothing for $\varepsilon$ to bound and
satisfaction falls back to Boolean.

**N17.** $d$ **measures**. It is symmetric. It states how far two verdicts
lie apart under renaming and **nothing about which is better**. Reading an
order as preference is HetOpt's (§7).

**N18.** In Het, $\varepsilon$ is **reported alongside the verdict** — an
honest error bar. In HetOpt it becomes a selection criterion.

**Translation-invariance is the candidate's burden.** If a candidate
adopts obscure naming, the judge's verdict may drift and the candidate
bears the cost. The Proponent must name its structures clearly enough
that its strategy survives renaming (§6).

---

## 4. Semantics — algebras in the Kleisli category

### 4.1 Why not $\mathbf{Set}$

In ordinary algebra, an algebra of a theory $T$ is a structure-preserving
functor $M: T \to \mathbf{Set}$. **That cannot be the definition here.**

A functor to $\mathbf{Set}$ assigns every operation — including judgmental
ones — to an actual function, i.e. a **decision procedure**. A naive
Set-algebra would *decide* the judgmental operations, computing the very
judgments the gate marker says no closed system can discharge on itself.
That is P0 violated in the semantic dimension.

**N19.** An algebra MUST be a functor into the Kleisli category of the
principal monad:

$$M: T \to \mathbf{Kl}(\mathcal{P})$$

| gate | interpretation |
|---|---|
| `decidable` | an ordinary (pure) morphism — an actual function on the carrier; factors through $\eta$ |
| `judgmental` | a morphism in $\mathbf{Kl}_{\text{judg}}(\mathcal{P})$ — a computation that may consult the outside |
| `authorial` | a morphism in $\mathbf{Kl}_{\text{auth}}(\mathcal{P})$ — an enactment by a principal with standing. Never pure. |

$\mathcal{P}(X)$ is *"an $X$, possibly obtained by a call on a principal."*
The unit $\eta: X \to \mathcal{P}(X)$ is *"no outside needed"* — decidable
data embeds. A judgmental operation is a Kleisli arrow $A \to \mathcal{P}(B)$.

> The monad is exactly *what the trip through the outside adds that the
> algebra could not generate alone.*

Composing pure morphisms with judgmental ones is Kleisli composition —
which is why the fragments interleave without collapsing.

### 4.2 Provenance

**N20.** The base category MUST carry a **provenance structure**: every
object $X$ is equipped with a provenance map

$$\pi_X: X \to \mathsf{Prov}$$

to a discrete category of provenance tags. Morphisms preserve or strictly
externalize provenance.

**N21.** $\mathcal{P}$ MUST be **provenance-strict**:

$$\pi_{\mathcal{P}X} \circ \eta_X = \pi_X, \qquad \pi_{\mathcal{P}X} \circ \mu_X = \pi_{\mathcal{P}^2X}$$

$\eta$ never invents a new author; $\mu$ propagates the outermost author.

### 4.3 Admissibility — two sub-categories, one monad

Nothing in the plain Kleisli construction prevents $M$ from sending a
judgmental operation to a **constant** arrow $c_j: a \mapsto \eta(j)$ whose
value $j$ is drawn from $M$'s own carrier. The selection rule never fires;
self-reference has been hard-coded into the interpretation.

**N22.** Judgmental and authorial arrows MUST inhabit their respective
admissibility sub-categories:

$$\mathbf{Kl}_{\text{judg}}(\mathcal{P}) = \{\, f : \pi(f(a)) \cap \pi(a) = \emptyset \,\} \qquad \text{(the outside)}$$

$$\mathbf{Kl}_{\text{auth}}(\mathcal{P}) = \{\, f : \pi(f(a)) \subseteq \pi(p) \ \wedge\ \mathsf{standing}(p, a) \,\} \qquad \text{(the steward)}$$

Authorial admissibility is **stronger, not weaker** — not "anything goes,"
but "only the principal who holds stewardship may enact on it." Where
judgmental demands disjointness, authorial demands containment plus
standing.

**N23.** Both MUST be sub-categories of the **same** $\mathbf{Kl}(\mathcal{P})$.
Distinct monads would mean distinct principal pools, which the doctrine
does not license.

**Admissibility is gate-relative, and this is licensed.** Decidability is
already fiber-relative and classified one level up (§2.5). Gate-relative
admissibility is the same pattern applied to provenance instead of
decidability. The institution's uniformity lives in *one $\models$,
gate-dispatched* — not in having one admissibility predicate.

### 4.4 Gate-faithfulness

**N24.** An algebra is **gate-faithful** when:

- every `decidable` operation factors through $\eta$ (pure/internal)
- every `judgmental` operation is a judgmentally-admissible Kleisli arrow
- every `authorial` operation is an authorially-admissible Kleisli arrow

**N25.** $\mathsf{Mod}(\Sigma)$ for Het consists **only** of gate-faithful
algebras.

A gate-faithful algebra cannot launder a judgmental operation into a
decidable one, and cannot dispatch judgment to itself. **P0 is enforced at
the level of the model category, not as a post-hoc selection rule.**
Because provenance re-indexes along signature morphisms, the condition
propagates automatically through the fibration — re-indexing cannot invent
a common author that did not already exist.

### 4.5 Objects

An **object** is an inhabitant of a carrier set $M(S)$ — a specific datum,
a document, an element sitting in the algebra's interpretation of a sort.

Given an algebra $M$ and an object $x : M(S)$:

- `audit` (decidable) runs as a pure morphism → a Verdict computed inside
  the algebra. **No outside.**
- `propose` / `dispose` (judgmental) run as Kleisli morphisms → they emit
  an outside call, and the disposition is obtained only when the outside
  answers. **The algebra cannot close the loop on its own judgmental steps.**

This is the semantic meaning of the whole structure:

> **An object is self-governing (its own algebra runs its audit) but not
> self-closing (its judgmental dispositions require the monad's outside).**

That is autopoiesis without self-loop degeneracy, made precise.

---

## 5. The tower

### 5.1 Two kinds of pointing

The direction of the arrows looks contradictory until two different things
called "pointing" are separated.

| | direction | what it is |
|---|---|---|
| **reference edge** | UP (concrete → abstract) | a *model* declares `theory_ref` — a **conformance declaration**: "I interpret $T$." This is what the checker *walks*. |
| **categorical morphism** | DOWN (abstract → concrete) | the signature morphism selects the structure the theory's algebras must carry — the **semantic map** whose existence the declaration asserts. |

The two are duals of one edge. The up-pointing reference is what the
satisfaction-checker walks to find the theory to test against. The
down-pointing morphism is the truth-condition the declaration claims to
satisfy.

**N26a.** `theory_ref` is declared by **models only**. A theory does not
carry one.

Earlier drafts put `theory_ref` on theories as well, reading it as
"I extend $T$." That reading has been retired. It named three distinct
relations at once — parameter supply (§2.2), auditability under the pass
(§5.3), and signature extension — and the conflation is what let a theory
declare a parent with which it shared no operations and still validate.

The three are now separate:

| relation | carried by | checked how |
|---|---|---|
| a population interprets a law | `theory_ref` on the model | resolve, then walk the theory's sentences |
| a theory supplies $\mathcal{P}$ | the interface (N6a) | signature inspection |
| a theory is auditable by the pass | fractal closure (N26) | the pass runs *on* it; nothing to declare |
| a theory extends another | — | **unexpressed; see N31a** |

### 5.2 The fibration

The tower is a **fibered category** — the Grothendieck construction over
the category of theories.

| level | role in fibration |
|---|---|
| theory $T$ | object in the base category $\mathbf{B}$ |
| $\mathsf{Mod}(T)$ | fiber over $T$ — category of $T$-algebras |
| $\sigma: T_1 \to T_2$ | base morphism (signature morphism) — **not currently expressed in the encoding; see N31a** |
| $\mathsf{Mod}(\sigma)$ | re-indexing — restricts $T_2$-algebra views to $T_1$ |
| $\models_T$ | fiber-wise relation: algebra × sentence → verdict |

### 5.3 The fractal property

**N26.** An algebra whose carrier contains objects that themselves carry
signature declarations **becomes a theory at the next level**, with its own
fiber of algebras-below.

The satisfaction relation is the same at every level. What changes is which
theory's $\models$ is invoked and which principal pool is available. The
Kleisli construction iterates: the same algebra becomes the theory whose
satisfaction relation tests algebras one level below.

The tower is **semantic at every level.** The fibration carries the Kleisli
structure through re-indexing; gate-faithfulness is preserved by signature
morphisms.

### 5.4 The gate law and termination

**N27 (the gate law).** Gate markers MAY be preserved or increased along
morphisms — `decidable` → `decidable` or `judgmental`; `judgmental` →
`judgmental`. **No morphism may launder a judgmental predicate into a
decidable one.** This is P0 at the morphism level.

**N28.** The **doctrine** — what makes a theory well-formed — is the
encoding spec `spec/het-theory.schema.json`. It requires at minimum one
sort, at least one operation, gate markers on every operation, a declared
competence role for every judgmental operation, a declared standing
predicate for every authorial operation, a classifying sentence for every
conditional operation, and the interface (N6a) from any theory supplying
$\mathcal{P}$.

The doctrine is an ordinary JSON Schema — decidable, machine-closed, and
deliberately not Het-encoded (N40). **It is the floor the regress terminates
on.**

**N28a.** `het/theory.yaml` is **not** the doctrine. It is the theory of the
audit-rectify pass: its sentences are about `audit`, `propose`, `dispose`,
`enact`, and their composition. Earlier drafts claimed both roles for one
object — "the doctrine of what a valid Het theory is, *and* the $\Sigma$
conformance is checked against, same object, both roles." They are two
objects. What makes a theory well-formed was already encoded in the schema;
what the pass does is encoded in het's sentences. The claim was never true
of the artifact.

**N29.** *(retired.)* Formerly: "the doctrine MUST NOT carry a `theory_ref`;
its absence terminates the tower." Superseded by N28 — the schema terminates
the regress, and under N26a no theory carries a `theory_ref` at all, so
absence distinguishes nothing.

**N30.** The doctrine checks **declaration, never adequacy.** It decides
purely syntactic well-formedness, all decidable by inductive inspection. It
never asserts that any concrete principal satisfies its own predicates, nor
that the pool is non-empty.

**N30a.** A theory's claims about **its own signature** — that a type is
closed, that two axes are orthogonal, that the theory declares no population
— are checked the same way, by inductive signature inspection. They are
`well_formedness` rules, **not sentences.** A sentence is evaluated as
$M \models \varphi$ against inhabitants of a carrier; a signature-claim has
no such inhabitant to test and walking a population cannot check it.

This is why such a claim carries no equation: there is nothing for $\models$
to compute. **A decidable sentence with no equation is a mis-filing, not an
omission** — it is the diagnostic that the thing is not a sentence.

**Adequacy lives one level below**, inside the theories that actually invoke
judges. For a judgmental sentence $\varphi$ of a theory $T$:

$$\mathsf{Adequate}_T(\varphi) \equiv \text{"a qualifying non-identical judge for } \varphi \text{ exists and returns a verdict"}$$

This sentence is itself **judgmental**, discharged by an outside call
exactly when an algebra of $T$ attempts to interpret $\varphi$. Failure of
adequacy is an ordinary judgmental failure at the level where the judge is
required, **not a defect in the doctrine**.

**Adequacy asks for *a* qualifying judge, not the minimal one.**

**Termination.** The tower terminates at the schema — a decidable
shape-check that every theory validates against directly. Adequacy is
**local, not global**. No infinite regress; no global fixed-point proof.

### 5.5 What replaces the tree condition

**N31.** *(retired.)* Formerly the tree condition — exactly one root, every
other theory names a parent that exists, no cycles — checked over the whole
repository by `spec/check.py` CHECK 4.

It existed to catch a specific hazard: a fiber that silently dropped its
`theory_ref` would still validate and read as a doctrine. Under N26a no
theory carries a `theory_ref`, and under N28 doctrine-status is not
conferred by absence of a parent but by being the schema. **The hazard is
structurally gone, so the guard against it retires with it.** Multiple
theories with no parent are ordinary and unremarkable.

`theory_ref` remains **required on every model** (`spec/het-model.schema.json`).
A model with no theory is a set of records with no law to be measured
against; there is nothing for $\models$ to evaluate. That requirement is
unaffected by any of the above.

**N31a (open).** Retiring `theory_ref` from theories leaves the base
category $\mathbf{B}$ with objects and no declared arrows. The fibration
survives — models still fiber over theories — but **theory-to-theory
morphisms are not currently expressible in the encoding**, and C2/M2 (the
gate law at the morphism level, §5.4 N27) quantifies over exactly those.

`theory_ref` was never the right carrier: a conformance declaration is not a
signature morphism. But something must carry it. How the encoding expresses
a theory-morphism is **open**, and is deliberately left open rather than
resolved by letting a retired field keep pretending to cover it.

---

## 6. Game semantics

Static satisfaction misses something essential: **when the judge and the
candidate disagree, who is right?**

Satisfaction **is** a two-player game:

- **Proponent** — the candidate algebra, asserting $M \models \varphi$
- **Opponent** — the environment, which may query an oracle (the judge)

A sentence is satisfied iff the Proponent has a **winning strategy**.

| | game structure |
|---|---|
| **decidable** predicates | games with finite, mechanizable winning strategies. The tree is bounded; the strategy is a decision procedure. |
| **judgmental** predicates | games where the Opponent has oracle access. The tree may be unbounded; the strategy involves querying the oracle at specific nodes. |

### 6.1 The audit-rectify loop is the game in operation

The pass is a **chain of principals**, each acting on what the previous one
produced. Every operation carries a gate saying *how* it is settled; §6.1.1
says *by whom*, and relative to whose authorship.

| game move | operation | gate | acts | result |
|---|---|---|---|---|
| a violation is found | `audit` | decidable, or judgmental per $\varphi$ | nobody, or a judge disjoint from $M$ | Verdict |
| the Proponent answers | `propose` | **authorial** | an author with standing over $x$ | Proposal |
| the Opponent rules | `dispose` | judgmental | a judge disjoint from **the Proposal** | Disposition |
| the Proponent applies it | `enact` | authorial | an author with standing over $x$ | the revised object |

**N32.** A Disposition is a **ruling, not a revision.** Something must apply
it. That something is an author with standing (§2.4). `enact`'s output is
what makes the pass an **endofunctor** rather than a one-way funnel into a
verdict.

#### 6.1.1 `propose` is authorial

**N32a.** `propose` is **authorial**, not conditional. §6 defines the
Proponent as the candidate algebra — the audited party. Answering a verdict
is the Proponent's move, and producing content about an object is authorship,
which requires standing over it (§2.4).

Earlier drafts gated `propose` as `conditional`, which resolves to
`judgmental` in some algebras — and a judgmental operation dispatches under
the **disjointness** filter, i.e. to the Opponent's side. That made the
Opponent play the Proponent's move. The gate marker and the game role
contradicted each other in a single table row.

The observation the conditional gate was reaching for is real but is not about
*who acts*: sometimes the remedy is mechanically determined and sometimes it
requires assessment. That is a statement about the author's difficulty, not
about whether an outside is needed. **Authorship is required either way**, so
the gate is authorial either way.

**N32b.** A Proposal's provenance is its **author's**:
$\pi(\mathsf{propose}(x, v)) \subseteq \pi(p)$ for the authoring principal $p$.
Nothing previously stated this, and §2.3's dispatch condition cannot be
evaluated at `dispose` without it.

#### 6.1.2 The Proposal vocabulary — answering, and contesting

**N32c.** A Proposal is one of:

| | means | licenses |
|---|---|---|
| `remedy` | *"the verdict stands; here is the fix"* | `enact` on acceptance |
| `dispute` | *"the verdict is wrong; the object stands as authored"* | nothing to enact |

Without `dispute` there is no path to contest a verdict. `audit` yields
non-conforming, `propose` is defined only on a non-conforming verdict, and the
false-positive override lived at `dispose` — **downstream of propose**. An
author who believed the audit simply wrong had to first author a remedy for a
diagnosis they disputed, in order to obtain a vehicle for disputing it.

A `dispute` is still judged. The Opponent rules on it exactly as on a remedy;
the author does not get to overturn a verdict by asserting it.

#### 6.1.3 The Disposition vocabulary

**N32d.** A Disposition is one of exactly:

| | terminal | affirming | who acts next |
|---|---|---|---|
| `accept` | ✓ | ✓ | the author enacts |
| `reject-diagnosis` | ✓ | ✗ | nobody — the audit was wrong; the object stands |
| `reject-remedy` | ✗ | ✗ | the author re-proposes, carrying the reason |
| `defer` | ✗ | ✗ | a prerequisite is required first |
| `raises-questions` | ✗ | ✗ | the auditor clarifies; the object re-enters |

**`accept-with-mod` is retired.** A judge amending a proposal is
*transforming*, not classifying — and N32 says a Disposition is a ruling, not
a revision. The judge is provenance-**disjoint** from the object (§2.3), so it
cannot hold standing over a modification it just authored (§2.4). The variant
required a principal satisfying two opposite conditions on one object.

`reject-with-alternative` fails for the same reason and is not admitted.

**N32e.** `reject-remedy` MAY carry a **reason**, which is advisory prose and
**not an edit**. The distinction is what keeps the judge inside the judgmental
gate: stating why a remedy fails is classification; supplying the replacement
is authorship. The author re-proposes with the reason in hand.

**N32f.** A re-proposal MUST carry the chain of prior dispositions and their
reasons. An author re-proposing without them can cycle indefinitely on the
same objection, and nothing downstream could detect it.

**N32g (open — the unfixable item).** Het places **no bound on re-entry**. If
no acceptable remedy exists, `reject-remedy` re-enters forever and the object
never leaves the loop.

Het does not resolve this, and cannot: the available answers — evict the
object, bound the attempts, or accept non-conformance as declared debt — are
all **worth-shaped**, and N33 forbids a Het theory from declaring a worth-law.

This is the first case found in which $\chi$ alone produces a state it cannot
exit. It is stated as a limit rather than closed by inventing an eviction rule,
which would be a worth-law smuggled in under another name. **The bound belongs
in HetOpt** (§7); until HetOpt ships, an implementation must surface a
re-entering object to its outside rather than loop on it.

#### 6.1.4 Enactment can still be refused

**N32h.** A terminal-and-affirming Disposition licenses `enact`; it does not
guarantee the edit lands. Where the revised object enters another governed
container, **that container's own $\models$ runs** — the pass composed with
itself under fractal closure (N26) — and may refuse it.

An authorization to edit is not a licence to violate the target's law. `enact`
therefore has two failure points, not one: the Disposition may withhold it, and
the target may refuse it.

### 6.2 Panels

**Panels are the game semantics of $\models$ with more than one judge** —
the game with an enlarged oracle-move set. They are not a separate feature
to design.

A Proponent winning strategy in the original game remains winning in the
composite; additional oracle answers can only strengthen the Opponent.

---

## 7. The Het / HetOpt cut

Metric and preference are the same categorical furniture read two ways. A
metric space *is* a category enriched over $([0,\infty], \ge, +)$;
quantale-enrichment is the general form. They are not the same **role**.

| | what it does | where it lives |
|---|---|---|
| $d$ — verdict metric | symmetric; how far two verdicts lie apart under renaming | **Het** |
| $V$ — worth-law | orders a conforming set by preference | **HetOpt** |

$$\textbf{Het} = \text{judgmental institution} + \text{gate-marked } \models + \text{metric verdict space}$$

$$\textbf{HetOpt} = \textbf{Het} + V$$

> **Het settles belonging. HetOpt orders what belongs.**

**N33.** The cut is drawn at **valuation itself**, not at any one
application of it. A Het theory MUST NOT declare a worth-law $V$, and MUST
NOT declare the minimal-judge rule.

**N34.** $V$ applies wherever Het has produced a conforming set:

| Het produces | HetOpt orders it by | yielding |
|---|---|---|
| the qualifying judges for a sentence | cost tier, then $\varepsilon$ | the **minimal-judge rule** |
| the qualifying authors for an operation | cost tier | the **minimal-author rule** |
| the conforming algebras of a theory | the declared worth-law | ranked candidates |

**One piece of machinery, two levels** — the fractal property applied to
valuation. Judge selection and candidate ranking are not two features but
one: *conformance, then valuation*, instantiated twice.

**Why the cut lands here.** Filter first, then optimize. Non-identity is
enforced at the model-category level as an admissibility restriction on
Kleisli arrows; the minimal-judge rule is a subsequent optimization
performed only among arrows that have already survived that filter. **Het
is the filter. HetOpt is the optimization.**

**Why it lands no later.** Non-identity cannot move to HetOpt. It is P0.

**The symmetry that makes this principled.** Het has no $V$ anywhere;
HetOpt has $V$ everywhere. Keeping a valuation in Het for judges while
withholding one for candidates would leave *"why judges and not
candidates?"* with no answer beyond stipulation.

**N35.** HetOpt is a theory extension in the ordinary sense:
$\mathbf{Sign}_{\textbf{HetOpt}}$ extends $\mathbf{Sign}_{\textbf{Het}}$
with the declaration of $V$, and $\textbf{Het} \hookrightarrow
\textbf{HetOpt}$ carries Het-algebras into the HetOpt fiber by re-indexing.
In HetOpt the enrichment base $V$ **is** the metric $d$ and the fibers
become $V$-enriched; in Het the verdict space carries $d$ alone.

---

## 8. Composition

When two theories are combined, their principal pools must combine.

**N36 (composite monad).** $\mathcal{P}_{1+2} = \mathcal{P}_1 + \mathcal{P}_2$,
provenance preserved componentwise. The non-identity restriction extends to
the composite Kleisli category. The qualifying set of the composite is the
union of the component qualifying sets, each still filtered by non-identity.

**N37 (composite kinds).** Kinds form the disjoint union $K_1 \sqcup K_2$.

**N38 (adequacy composes).** The composite qualifying set is non-empty
whenever either component's was.

**Result: the composite institution is again a judgmental institution.
Theory combination is closed.**

---

## 9. Mechanization

The normative evaluator:

```
eval(M, φ):
  match φ.gate:
    decidable   → machine_check(M, φ)
    judgmental  → dispatch(M, φ, 𝒫_judg)     # a qualifying non-identical judge
                                             # (HetOpt: the minimal one)
    authorial   → dispatch(M, φ, 𝒫_auth)     # an author with standing
                                             # (HetOpt: the minimal one)
    conditional → if eval(M_up, φ.classifier) then machine_check else dispatch
```

The pass's own operations dispatch as: `audit` decidable (or judgmental per
the sentence), `propose` **authorial** (N32a), `dispose` judgmental, `enact`
authorial. `conditional` remains in the vocabulary for theories that need it
(§2.5); the pass no longer uses it.

Run `eval` over every $\varphi$ in $\Sigma$ against $M$.

**N39.** Dispatch is two operations and the first is decidable:

```
qualifying = { p ∈ 𝒫 : capable(p, role(φ)) ∧ π(p) ∩ π(a) = ∅ }   ← Het, testable cold
return any(qualifying)                                            ← Het
     # HetOpt: argmin cost(p) over qualifying
```

`a` is **the argument the operation is applied to** (N6d) — the object at
`audit`, the Proposal at `dispose`. Reading `π(M)` here is the hole N6d closes.

Both conjuncts read only the interface (N6a). **The conformance half needs
no LLM to test** — it is set operations over declared predicates.

A single qualifying judge is **not a hardcode deferred** — it is
Het-correct. `argmin` is the named seam where HetOpt lands.

### 9.1 Self-grounding

**N40.** The encoding spec (`spec/`) MUST remain a plain decidable schema.
It MUST NOT be Het-encoded — that would reopen the regress §5.4 closes. The
spec **is** the doctrine (N28); Het-encoding it would ask the floor to stand
on itself.

The bootstrap relationship is the good kind:

- `spec/` says what a valid gate-marked sentence-set looks like (decidable)
- `het/theory.yaml` **is** a gate-marked sentence-set — the theory of the pass
- **`het/theory.yaml` MUST validate against `spec/`**

**N41.** The **first** check is not "does some domain conform to Het" — it
is *"does Het's own encoding conform to Het's encoding-spec."* Passing that
**demonstrates** self-grounding rather than asserting it.

**N41a.** Self-grounding is a property of the **pair**, not of either
artifact alone. `het/theory.yaml` is not self-grounding by being its own
doctrine — it is not its own doctrine (N28a). It is grounded by validating
against a decidable schema that is not itself Het-encoded, and the schema is
grounded by being decidable. Neither stands on itself; the regress
terminates because one of the two is an ordinary shape-check.

---

## 10. Vocabulary

The official vocabulary is the Glossary at `docs/institutional_judgment.md`
§11. Terms not listed there are not part of the formalism; an encoding that
introduces one has drifted.

### 10.1 Retired terms

**N42.** The following are retired. They MUST NOT appear anywhere in a Het
theory or in the normative docs — not in structural positions, not in prose,
not in provenance. This table is the sole exception: a ban has to name what it
bans. `spec/check.py` CHECK 3 mirrors this list and gates on it.

| retired | use instead |
|---|---|
| **register** | a *theory* — an algebra carrying its own signature (§5.3) |
| **charter** | $\chi$, the belonging-law; declared *in* $\Sigma$, not pointed at |
| **element** | an *object* — an inhabitant of a carrier $M(S)$ (§4.5) |
| **finding** | an *object*; auditability is carried by the fractal property |
| **role**, **presentation** | — the fibration carries one-sort-many-views |

A claim that needs an earlier encoding to state itself has not been stated.
Where the substance is real, voice it positively from §§1–9; where it is only
lineage, cut it.

**Sunset.** This table exists so a reader arriving with the earlier vocabulary
can find the current term once. It is also the mechanism by which retired
nouns stay legible in a normative document, which is a cost that compounds.
**It is scheduled for deletion**, and `spec/check.py` CHECK 3 — which enforces
the ban from its own list — is what makes deletion safe. Nothing but the check
needs to survive it.

**N43.** `theory_ref` is the up-pointing conformance declaration on a
**model**: "this population interprets that law." The field is `theory_ref`,
not `charter_ref` — "charter" is retired. Theories do not carry it (N26a).

---

## Appendix — provenance map

Where the encoding cites this document.

| requirement | subject | cited by |
|---|---|---|
| N1–N5 | gate markers, declared role | `het/theory.yaml` operations |
| N6, N6a–N6c | $\mathcal{P}$ not a sort; the interface | any theory supplying $\mathcal{P}$ |
| N7–N9 | non-identity (P0) | supplier's non-identity predicate |
| N10–N13 | standing, authorial gate | supplier's standing predicate |
| N14 | conditional classifier | `het/theory.yaml` classifier |
| N15–N18 | verdict space, metric | `het/theory.yaml` satisfaction |
| N19–N25 | Kleisli semantics, gate-faithfulness | `het-rs` evaluator |
| N26, N26a | fractal property; `theory_ref` on models only | `spec/het-model.schema.json` |
| N27 | gate law at morphism level | — (see N31a) |
| N28, N28a, N30, N30a | the doctrine is the schema; signature-claims | `spec/het-theory.schema.json` |
| N29, N31 | **retired** | — |
| N31a | theory-morphism expression | **open** |
| N32 | `enact` / endofunctor | `het/theory.yaml` enact |
| N33–N35 | Het/HetOpt cut | `spec/check.py` CHECK 2 injections |
| N39–N41a | mechanization, self-grounding | `spec/check.py` CHECK 1 |
| N42–N43 | retired vocabulary | `spec/check.py` CHECK 3 |
