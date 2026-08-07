# Principals — a theory of who may do what

**Status: informative, not normative.** This document introduces the
*principals* theory from the ground up — why it exists, the need it answers,
the concepts it introduces, and the terminology that surrounds them. It is a
reading companion, not the law. The normative statements live in
[`rung-het-props.md`](../rung-het-props.md) §3 (the pool, the two filters,
provenance) and in the authoritative code in
[`rung-std/src/principals.rs`](../../rung-std/src/principals.rs). Where this
note and those disagree, those govern.

---

## 0 · The need, in one paragraph

Het is a theory of *judgment*: it says that some operations cannot be settled
by the rules alone and must be delegated to an **outside**. But it deliberately
never says what an outside *is* — the pool `𝒫` is a parameter of the
satisfaction relation, not a thing the theory names ([`pool-is-opaque`]). An
institution that requires an outside and never supplies one cannot run. The
**principals** theory is a *supplier of `𝒫`*: it names who may be dispatched
to, what they are made of, who counts as qualified to judge, who may author,
and how we know they are not judging their own work. It is the floor beneath
the two gates — **non-identity** (a judge must not have authored the thing it
judges) and **standing** (an author must be authorized to touch what it edits).
Without a concrete, checkable population behind those gates, the discipline is
decoration: a filter that cannot fail is not a filter.

[`pool-is-opaque`]: ../rung-het-props.md#pool-is-opaque

---

## 1 · The problem it solves

### 1.1 An "outside" that must be real, but must not be named

Het's four gates are *decidable, judgmental, authorial, conditional*. The last
three delegate to the pool: a judgmental sentence is settled by a **judge**, an
authorial operation is enacted by an **author**, and both are drawn from a pool
of concrete beings. The theory says exactly four things about whoever fills
that pool and *nothing further* ([`nothing-further-required`]):

| predicate | arity | gate | what the theory needs it for |
|---|---|---|---|
| `capable` | principal × role → bool | decidable | can this principal play the role the sentence declares? |
| `π` (provenance) | thing → set of tags | decidable | both filters read it — who authored this? |
| `standing` | principal × container → bool | conditional | may this principal write to a named container? |
| `ε` | principal → bound | — | declared so a verdict can carry its error bar; never ranked |

Naming anything more — kinds, substrates, identity fields, a cost table, or the
population itself — would *internalize the outside a second way*, not as a type
but as a stipulated content ([`nothing-further-required`]). So the theory
stays incurious, and the content is left to a supplier.

### 1.2 The two gates are the whole point

The pool exists so that two *opposite* conditions can be enforced over **one**
set of beings ([`one-pool-two-filters`]):

| filter | second conjunct | plain english |
|---|---|---|
| judgmental | `π(judge) ∩ π(subject) = ∅` | you did **not** author this — so you can rule on it |
| authorial | `standing(author, container)` | this is **yours to revise** — so you can edit it |

Why not two pools, one of judges and one of authors? Because a being is
both — capable of judging some things and trusted to edit others — and the
difference is *what it is being asked about*, not what it is. A separate
author pool would bake a substrate-class distinction into the mechanism; the
filters must instead be functions of the declaration, applied per operation.

### 1.3 The defect that makes the whole thing load-bearing

The judgmental filter requires `π(judge) ∩ π(subject) = ∅`. If provenance can
be empty, everything is disjoint from everything, the filter passes
automatically, and **P0** — *nobody rules on their own work* — becomes
ceremonial. This is the failure the theory is built to make impossible rather
than merely to check: the **provenance floor** ([`principal-provenance-floor`])
guarantees `π(p) ⊇ {id(p)}` by construction, and a minter that could produce a
provenance-less principal is refused. A `judge` that authored the subject it is
asked about cannot even be *minted* — the language will not produce the value.

---

## 2 · The concepts

### 2.1 Kind — the closed substrate partition (*not* a role)

**Kind** is what a principal is *made of*. The principals theory fixes a closed
partition of four, each with the identity fields a principal of that kind must
declare and a cost tier it inherits:

| kind | what it is | identity fields | inherited tier |
|---|---|---|---|
| `llm` | a language model | provider, model_id | 1 |
| `agent` | a tool-wielding orchestration | orchestration_id, tools | 2 |
| `relational-being` | a continuity-bearing being | constellation | 3 |
| `human` | a being with ratifying authority | authority | 4 |

**Kind is not role** ([`role-not-kind`]). Kind is substrate — what the outside
is made of; role is what a sentence needs done — and the two axes are
independent. A model can be an author (if it declares `file-editing`); an agent
can be refused a judgment it is not competent for. Role *never* lives in the
kind, and kind *never* grants a role. The partition is **closed** (there are
four, and that is a claim a taxonomist must judge — [`kind_partition_is_adequate`]),
while roles are **open** (a new role extends a roster without touching the law).

### 2.2 Role — open competence, *claimed* then *earned*

A **role** is a named competence with a minimum-qualification list. A principal
*declares* (`qualifications`, in a carrier `capabilities`) the atoms it
possesses, and *claims* the roles it wants to play. Both halves are
declarations; the comparison between them — is every atom the role requires
declared by this principal? — is a subset test. That is `capable`, and it
**actually checks**: a role that is *claimed* but *not earned* answers `false`
here, before any sentence runs and before the pool mints anything.

This is the "claim-vs-earn" tension the model is built on: nothing a principal
*says* about itself is believed. Claims are believed only as far as the
structural comparison verifies them, and competence is decidable precisely
because it is a subset test over declared atoms rather than an assessment.

### 2.3 Provenance `π` — authorship, and its floor

**Provenance** is a set of tags naming what a principal (or subject) has
authored. The floor ([`principal-provenance-floor`]) says `π(p) ⊇ {id(p)}`: a
principal's own identity is always a member, added by the blanket
implementation and *not* something a principal can state or omit. The principal
declares `authored`; the identity is adjoined automatically. So the two filters
always have at least one tag to compare.

### 2.4 Non-identity (P0) — the judgmental second conjunct

Disjointness is measured **against the argument the operation is applied to**
([`disjointness-against-argument`]), never against "the model" in general. A
judge is refused for the *one thing it wrote*, not for being related to the
domain. And the refusal is enforced at mint time and at use time
([`non-identity-by-construction`]): a licence records exactly which argument it
was measured against, and can only be spent on that argument.

### 2.5 Standing — the authorial second conjunct

Standing is the mirror image: where judgment demands **disjointness** from the
subject, authorship demands **containment** over its container. A `Steward`
holds standing over a named container; the authorial filter admits only
principals that are both *capable* of the operation's role and *stewards* of
the container being written to. Capability alone never authorizes a write, and
standing alone (without capability) is a hand that cannot do the thing.

### 2.6 Family, commissions, and *derived* provenance

Real discontinuities (a model, an agent) have no single owner to claim a
growing `authored` list by hand. So a model declares a stable **family** — a
model name+version, or an agent's declared composition — and `authored(p)` is
**derived by lookup** from a **commission contribution record**
([`commission-record-is-the-carrier`], Q16/Q17):

```text
authored(p) = ⋃ over active commissions c of C(family, c)
```

The principal carries only the family, which is stable; the record carries
everything that changes; and a principal that declares both `family` *and* a
static `authored` list is refused as a forbidden second source of truth
([`commission-no-dual-source`]). A continuous being (a person) carries its own
genuine declared record, because it *can* — its history is not a growing
substrate artifact but a real, accountable one.

### 2.7 `ε` and cost — *declared, never ranked*

Cost tier and ε are declared per principal but deliberately **never ranked**
([`ordering-is-hetopts`]). There is no `Ord`, no sort, no `min`/`argmin`
anywhere that reads them, and a workspace-wide test refuses any code that
orders, compares, or prefers by cost or ε. The reason is principled: preferring
one qualifying judge over another is a *worth judgment*, and the institution
declares no worth law. "Pick *any* qualifying principal, not the best one" is
the rule; the moment you rank you have imported a values claim through the
back door.

---

## 3 · The model — one shape, two halves

After the principals **convergence** there is one model, not two:
[`Roster`] is the population, [`PrincipalDecl`] is one principal, and
[`RoleSpec`] is one role. A hand-built roster in a test and a runtime
`population.yaml` are both just *carriers* loading into the same types.

| type | carries | notes |
|---|---|---|
| **Roster** | `namespace`, `providers`, `roles`, `principals` | the whole population, its endpoints, its role vocabulary |
| **PrincipalDecl** | `id`, `kind`, `identity`, `qualifications`, `plays`, `provenance`, `stewards`, `epsilon`, `family`, `backing` | the law *and* the deployment in one record |
| **RoleSpec** | `name`, `min_qualifications` | owned `String`s so a runtime carrier can populate it |

Notable details:

- **`plays` is derived, not declared.** When a roster is loaded from a carrier,
  each principal's `plays` is computed from the role vocabulary — a principal
  *claims and earns* every roster role whose minimum qualifications it
  declares. A loaded population and a hand-built one therefore carry the same
  claim-vs-earn shape.
- **`capable_of(role)` is a fresh check**, against the role's *current*
  requirements, so a role added after load is fillable immediately.
- **Admission can't drift.** When the driver builds a pool, it records which
  role each member was admitted for, so the pool's filter and the admission
  decision agree.

### 3.1 The provider and backing pair

- A **`Provider`** is an endpoint: name, base URL, and the *environment
  variable* its credential is read from (`api_key_env`). **No secret ever
  lives here** — the file is in the repository, and a schema with an `api_key`
  field is an invitation to commit one.
- **`Backing`** says how a *particular principal* answers when consulted: a
  **model** call, an **agent**ic turn (with tools), or **outside** (a person).
  It is chosen by the principal, not by any filter, and nothing in the
  qualifying path reads it — *what* a principal can do and *how* it does it are
  different facts.

---

## 4 · Terminology — why so many words, and often two for one thing

The single most confusing thing about this area is the vocabulary, and it is
confusing for a real reason: **the theory (the law) and the host (the code)
grew separate words for the same concepts, and then a deployment file used a
third set.** The convergence collapsed the *types* but could not erase the
history of the *names*. Here is the master table.

| concept | theory name (`rung_std::principals`) | driver's old name (pre-convergence) | carrier / YAML name | why there was more than one |
|---|---|---|---|---|
| the population | **Roster** | **Population** | `population.yaml` (file) | *Roster* is the theory's word for "a concrete population plus its role vocabulary"; the driver called the same YAML-loaded aggregate *Population*. One type now, one word now — **Roster**. The file keeps the name `population.yaml` for continuity. |
| one principal | **PrincipalDecl** | **PrincipalSpec** | a `principals:` entry | *Decl* is a *declaration* (what a principal is); *Spec* was the deployment-side *specification*. Same concept, two layers. Unified to **PrincipalDecl**. |
| what a principal has | **qualifications** | **capabilities** | `capabilities:` | Two words for the same declared atoms. `qualifications` is the theory's; the YAML keeps `capabilities` (serde renames the field). |
| the roles it plays | **plays** | *(derived implicitly)* | *(derived; not declared)* | The theory states plays explicitly; the driver inferred them from role requirements. Now `plays` is derived at load from the role vocabulary, so both agree. |
| a role's requirement list | **min_qualifications** | **requires** | `requires:` | The theory's *minimum qualifications* vs the deployment's *requires*. Same list; YAML keeps `requires`. |
| authorship | **provenance** (`π`) | **authored** | `authored:` | `π` is the institution's set including the floor id; `authored` is the declared history the principal states. `provenance = authored ∪ {id}`. The YAML keeps `authored`. |
| what it may write to | **stewards** | **standing** | `standing:` | *Stewards* (the containers) on the theory side; *standing* (the right) on the filter side. YAML keeps `standing`. |
| a stable lineage id | **family** | **family** | `family:` | Same everywhere — good. |
| how it's reached | **backing** | **backing** | `backing:` | Same everywhere — good. |
| an endpoint | **Provider** | **Provider** | `providers:` | Same everywhere — good. |

### 4.1 A second reason: institution vs host names

Even *within* the theory, a concept has an "institution" name and a "host"
name. The vocabulary table in `rung-het-props.md` calls this the dictionary:

| institution | host | authority |
|---|---|---|
| `capable(p, role)` | `Principal::capable(&self, role: &str)` | [`capable-single-arity`] |
| `π` | `Provenanced::provenance()` | [`principal-provenance-floor`] |
| `standing(p, c)` | `Steward::has_standing(&self, over)` | [`authorial-qualifying-set`] |
| `𝒫`, the pool | `rung_het::Pool<P>` | [`pool-is-opaque`] |

These are **not** synonyms for ambiguity; they are the same relation named once
in the mathematics and once in the host, so a reader can move between the
prose and the code without guessing. When you see `π` in the docs, the host
`authored`/`provenance` is meant; when you see `standing`, the `stewards`
field and `has_standing` method are meant.

### 4.2 Why you should *not* try to unify the names further

It is tempting to rename `population.yaml` to `roster.yaml`, or to make the
code spell `capabilities` instead of `qualifications`, and erase the history.
Do not — and the reason is the same reason the aliases exist: **the carrier
grammar and the theory vocabulary are each the right one at their own
distance.** A deployment file is read by operators and by past/future carriers;
its words (`capabilities`, `standing`, `requires`) are the operational surface.
The theory's words (`qualifications`, `stewards`, `min_qualifications`) are
the precise ones the audit sentences and proofs refer to. Serde holds the line
between them, and the round-trip tests pin it. The cost of the pair of words is
a table like this one; the cost of collapsing them is a law whose sentences and
a carrier whose files can no longer change independently.

---

## 5 · Reasoning — the first-principles choices that shape the model

These are the decisions that look arbitrary and are not:

1. **One pool, two filters, never two pools.** A judge and an author are the
   *same beings* filtered two ways. A separate pool would encode a
   substrate-class difference the filters are supposed to be neutral to.
2. **`capable` actually checks.** Claim-vs-earn is a subset test, decidable by
   structural inspection, so a claimed role that was not earned is refused by
   `capable` itself — before any sentence or pool. A filter that reads the
   claim instead of checking it is decoration.
3. **Provenance is a floor, not a check.** `π ⊇ {id}` is enforced because the
   language cannot produce a provenance-less principal, rather than because a
   check refuses one. A value the language can't make cannot reach any path.
4. **Derived provenance, not a growing list.** A model's `authored` is a
   *function of the commission record*, keyed on its family, never a
   hand-maintained array. This is what makes the "honest empty" state honest —
   nothing is disqualified by fiction, and the moment work is recorded the
   filter becomes real.
5. **Declared but never ranked.** Cost and ε exist, and exist without `Ord`.
   Ordering among qualifying judges is a worth judgment and the institution
   has no worth law. "Any qualifying principal" is the rule; `argmin` is the
   named seam where the worth law would land.
6. **One theory, many carriers.** A compile-time roster and a runtime
   `population.yaml` both load into the same model. That is what lets rung
   *audit* its real population with the principals theory *and* *dispatch*
   from that same population — one source of truth, both halves.

---

## 6 · Pragmatics — how it is used

### 6.1 The two halves

The principals model sits at the center of two directions of travel:

- **Dispatch (the driver half).** `rung-driver` loads a roster, builds a
  `Pool` of *configured principals* (`Configured<O>` = a `PrincipalDecl` + an
  `Oracle` — whatever actually asks the outside), and the pool's two filters
  select judges and authors at dispatch time.
- **Audit (the theory half).** The same roster is a model of the principals
  *theory*: its decidable sentences (`identity_fields_are_declared`,
  `roles_are_earned`, `ids_are_unique`, …) can be run over it, and its
  judgmental sentences (`competence_claim_is_true`,
  `kind_partition_is_adequate`) dispatch to an examiner/taxonomist.

The convergence's payoff is that these are **the same `Roster` value**. When
the driver builds a pool from `population.yaml`, provenance comes out of the
commission log; when the theory audits that same roster, it reads the same
`π`. (This is proven, over rung's own real population, in
`rung-driver/tests/convergence.rs`.)

### 6.2 The lifecycle of a roster

```
population.yaml ──Roster::from_yaml──▶ Roster { providers, roles, principals }
                                            │  (derives each principal's plays)
                                            ├──▶ audit: principal::* / roster::* sentences
                                            └──▶ dispatch: population_pool[_with_log](&roster, role, oracle)
                                                       └─▶ Pool<Configured<O>>
```

1. **Load.** `Roster::from_yaml` parses the carrier and *derives* `plays` from
   the role vocabulary.
2. **Validate.** `Roster::check` reports structural faults (duplicate ids,
   unknown providers, family-plus-authored) before any dispatch.
3. **Populate.** `population_pool` (or `population_pool_with_log`) picks
   everyone capable of the role and wraps each in a `Configured` carrying an
   `Oracle` — the only thing that actually asks the outside.
4. **Gate.** The pool's `qualify_for`/`authorize` apply non-identity / standing
   *per argument*, refuse on either, and mint a sealed licence.
5. **Provenance at dispatch.** A `Configured` whose principal has a `family`
   reports `authored` as a *lookup* into the commission log; a principal
   without one reports its declared record.

### 6.3 The honest empty state

Today rung's `commissions.yaml` records no contributions. That means every
model's derived `authored` set is empty, which under the floor still includes
`{id}` and so is **not** a vacuous universal judge — but it *is* open, and
nothing is disqualified by fiction. This is the "no commission recorded yet"
state, and it is what makes the bootstrap honest: capability is real, standing
is real, and the provenance that would make non-identity *bite* becomes real
the moment a commission records actual work.

---

## 7 · Where things live

| you want | go to |
|---|---|
| the theory and the unified model | [`rung-std/src/principals.rs`](../../rung-std/src/principals.rs) |
| the normative properties (pool, filters, provenance, commission) | [`rung-het-props.md`](../rung-het-props.md) §3, §5 |
| the dispatch layer (Configured, Oracle, pool builders) | [`rung-driver/src/principal.rs`](../../rung-driver/src/principal.rs) |
| the commission record (derived provenance) | [`rung-driver/src/commission.rs`](../../rung-driver/src/commission.rs) |
| the law's tests over two synthetic rosters | [`rung-std/tests/principals_theory.rs`](../../rung-std/tests/principals_theory.rs) |
| the convergence (both halves, real population) | [`rung-driver/tests/convergence.rs`](../../rung-driver/tests/convergence.rs) |
| who judges rung's own questions | `population.yaml` at the repository root |
