# questions — a theory of well-posed questions

**Status: informative, not normative.** This document introduces the
`questions` theory from the ground up — why it exists, the problem it solves,
the concepts it carries, and how it plugs an open-question docket into the
audit-rectify loop. It is a reading companion, not the law. The normative
statements live in [`rung-het-props.md`](../rung-het-props.md) and the
authoritative code in [`rung-std/src/questions.rs`](../../rung-std/src/questions.rs).
Where this note and those disagree, those govern.

This theory is, more than any other in rung, an **exemplar of Het** — it is the
place where the *decidable/judgmental* split stops being an abstract doctrine
and becomes a working, up-front design, and where the *theory / driver /
carrier* three-way separation is most concrete. Read it to see those ideas with
their clothes on.

---

## 0 · The need, in one paragraph

A body of open questions — an architecture decision log, a research docket, a
standards body's issues, a review queue — is not a folder of markdown. It is a
**registry with a law**: who may file, what counts as a real question (rather
than a decision or a wish), what one question "responding to" another means,
how a question moves from *open* to a verdict, and — when every judgement in it
grows contentious — who gets to decide and on whose authority. Untended, such a
registry rots: it silently absorbs *decisions wearing questions' clothes*, its
edges become decorative, and its "verdicts" get recorded by whoever answered.
`questions` is a complete Het theory over just such a body: it names the sorts,
declares the gate-marked sentences that judge them, fixes the typed edges and
what each propagates, and provides the lifecycle and the edits. It knows no
particular body of questions; every deployment supplies its own ids, files and
edges as parameters.

---

## 1 · The problem it solves

### 1.1 A folder is not a theory

You can put question-shaped files in a directory and nothing stops the rot,
because nothing is *enforced*. Het's whole wager is that governance should live
in the type-and-sentence layer, not in a policy somebody remembers to follow.
`questions` fills Het's declared slots ([`theory-declares-four-things`]) with a
body of open questions: two sorts, a typed edit vocabulary, gate-marked
sentences, and a role for each judgmental sentence.

It earns its place in `rung-std` (rather than being a one-off domain model) by
a single test: **two carriers already fill it** — rung's own `questions` tree
(read off disk by `rung-het/tests/questions_of_rung.rs`) and a synthetic
decision docket with a disjoint id space, a disjoint edge set, and a different
lifecycle path (`rung-std/tests/questions_theory.rs`). A theory with one
carrier is a domain model wearing a library's name.

### 1.2 The membership problem

The sharpest problem a registry faces is **intake**: is this thing a question
at all, and is it *well-posed*? Most of what arrives asking to be filed is one
of three things wearing a question's clothes — a decision awaiting a ruling, a
definitional commitment, or a work item. Left unchecked, intake makes the
registry a decision log in disguise. The theory's answer is the **membership
criterion**, and it is the heart of this document (§3) — because it is where
the decidable/judgmental split first bites for real.

---

## 2 · The concepts

### 2.1 Two sorts, and why

`questions` declares **two sorts** of one theory:

| sort | states what | examples |
|---|---|---|
| **`Question`** | one question must satisfy | id matches its file, status is declared, is well-posed, the resolution answers it |
| **`Questions`** | a *relation between* questions must satisfy | every dependency resolves, ids are unique, every declared edge kind is lived |

The split is not decoration. `ids_are_unique` cannot live on a single
`Question` (no one question can see the whole set), and `is_well_posed` cannot
live on `Questions` (a judge rules on *one* question, and the licence is minted
against that one question's provenance). The two sorts are exactly the two
things a principal is ever handed.

### 2.2 The sentences, and the gate split (the exemplar)

Het gives every sentence a **gate marker** — *decidable* or *judgmental* —
which fixes *how* it is satisfied. `questions` leans on this distinction as
hard as any theory in the corpus:

- **decidable** — satisfied by a machine-checkable predicate over the
  declaration, with **no outside**. E.g. `id_matches_the_filename`,
  `status_is_declared`, `edge_kinds_are_declared`, and — crucially —
  `answerable_is_declared` (the cold first cut of well-posedness).
- **judgmental** — satisfied only by dispatching to an **outside principal**
  who renders a verdict; no predicate settles it. E.g. `is_well_posed`,
  `resolution_answers_the_question`, and (between questions)
  `survives_the_change`.

The rule of thumb the theory is built on: **if a genuine sentence can be
checked by reading the declaration, it is decidable; if it requires assessing
the substance or rephrasing, it is judgmental — and it is a defect to fake one
as the other.** (§3 shows a sentence that is *deliberately* a conjunction of
both.)

### 2.3 The typed edges, and what each propagates

`questions` declares a **closed edge taxonomy** of seven kinds, each with a
propagation that says what a change along that edge does to the dependent:

| edge | meaning | propagation |
|---|---|---|
| `premise` | the dependent rests on this; was *wrong* until folded in | **strict** (an obligation) |
| `justification` | the dependent was motivated by this and stands alone | advisory (review required / survive) |
| `spawn` | the dependent exists only because this resolved | generative (existence) |
| `gate` | a blocker; whether it has lifted is *settled per model* | conditional |
| `citation` | a reference to fix | mechanical |
| `evidence` | inbound support | inert |
| `related` | see-also | inert |

The taxonomy is **the theory's**, exactly as an edit vocabulary is
([`edge-taxonomy-is-the-theorys`]) — `rung` and `rung-het` have never heard of
`premise`. It is a closed, lived vocabulary: a declared kind with no instance
is reported, not silently accepted.

### 2.4 The roles

Three principals are named by the sentence `role(φ)`:

| role | the sentence it settles | kind |
|---|---|---|
| **Curator** | files, moves, rewrites (the author of the docket) | authorial |
| **Interrogator** | `is_well_posed` | judgmental |
| **Adjudicator** | `resolution_answers_the_question`, `survives_the_change` | judgmental |

Each role is *declared*, never enumerated — `rung` requires that a judgmental
sentence name a role, and a theory supplies them (`role-declared-not-enumerated`).

---

## 3 · Well-posedness — the membership criterion

This is the exemplar. Well-posedness (the term is **Hadamard's**, from the
theory of PDEs, transplanted) is the answer to *"is this a question at all?"* —
and `questions` formalizes it as a **compound of four cuts**, whose answer
being **found by the structure rather than made by the asker**:

```
is_well_posed  :=  existence ∧ unique ∧ stable ∧ authentic
```

| cut | Hadamard ↔ question | gate |
|---|---|---|
| **existence** | a solution exists ↔ an answer is reachable in principle | judgmental |
| **unique** | the solution is unique ↔ one answer, not a family of framings | judgmental (+ cold screen) |
| **stable** | continuous dependence ↔ the answer survives rephrasing | judgmental |
| **authentic** | — ↔ a question, not a decision / definitional commitment / work item | judgmental (+ cold screen) |

The rigor is in *which* parts are decidable and which are judgmental, and in
not pretending otherwise:

- **The cold first cut is decidable.** A question must declare `answerable:` —
  what would count as an answer. Reading the declaration is machine-checkable.
- **`unique` and `authentic` also carry cold screens** — they have recognizable
  footprints (an unpinned equivalence relation, a decision mask), so the theory
  can flag them without an outside.
- **`existence` and `stable` are irreducibly judgmental** — reachability from
  the substance and survive-rephrasing cannot be reduced to a heuristic. **This
  asymmetry is correct and is protected.** A uniform table — one that "closed"
  existence with a heuristic for shapeliness — would be prettier and wrong.

### 3.1 Two filing modes

The sharpest move separates *whether a question claims well-posedness* from
*whether it is well-posed*:

- **Mode A (`filing: well-posed`)** declares `answerable:` and **claims** the
  four cuts. No exceptions — a question that claims well-posedness owes a named
  resolution condition, because that conditioning is what makes the judgment
  non-vacuous (an unanchored judge is a free-floating judge).
- **Mode B (`filing: ill-posed`)** is the honest escape hatch: `answerable` is
  absent **on purpose**, the **ill-posed condition is named**, and the question
  makes **no false audit claim** — it is not audited for a well-posedness it
  never asserted.

Mode B *is* the gentle onramp (a newcomer can file something incomplete without
faking well-posedness); Mode A is what the audit actually judges. The escape
hatch is not a silent opt-out: `ill_posed_filings_name_their_condition` is a
decidable sentence that refuses an ill-posed filing which doesn't name why it
isn't a question yet. **And `answerable:` is the single source of the
resolution condition** — a Mode A body defers to it, or there are two things
claiming to be the answer.

---

## 4 · The carrier, the theory, and the driver

The three-way separation is where `questions` plugs into the rest of rung:

| layer | what it is | where |
|---|---|---|
| **the theory** | the law: sorts, sentences, edges, roles, edits, lifecycle | `rung_std::questions` |
| **the carrier** | the concrete subjects: the flat docket of question files | a directory, e.g. `.het/rung-questions/questions/` |
| **the driver** | theory-blind machinery: audit → propose → judge → enact → verify | `rung-driver` |

### 4.1 The carrier is flat and self-describing

Structure is **metadata, not folder**. A question's *status* lives in its own
frontmatter (`status: open`), not in which directory it sits; the flat set of
`*.md` files is the carrier, and `dir` mirrors `status` for standing. Each
instance declares its [`Scheme`] — provenance namespace, the container it sits
in (`root`), and the id prefix that marks an id as internal — plus its
population, commission record, and state home, all in a `.het/` instance (see
[`rung-driver`]: the `Instance` reads `config.yaml` → carrier → theory).

### 4.2 The model

- **`Question`** — id, status, `filing` (Mode A/B), `answerable:` (the
  resolution criterion), `ill_posed:` (Mode B's named condition), the
  `depends_on` / `affects` edges. Its provenance is `{namespace, id}` — coarse
  but not vacuous: anyone tagged with the set is refused as a judge of it.
- **`Questions`** — the whole set; supplies the whole-set sentences and the
  edits.
- **The lifecycle** — `open → blocked / parked / resolved / dissolved`, with a
  write-guard on the `resolved` done-pile: the destination runs its own law at
  its boundary.

---

## 5 · The audit-rectify cycle

The driver runs one composed cycle over a `Pass`-implementing theory
([`run_cycle`]):

```
       audit ──▶ propose ──▶ judge ──▶ enact ──▶ verify ──▶ record
   (find a      (the        (the       (the      (an        (write a
    defect)      author      theory     author    observer   dispatched
                 proposes    weighs     applies   reads the  record)
                 a remedy)   in)        the edit)  post-state)
```

### 5.1 Judging is abstract — one judge or a panel is the same step

The cycle treats **judging** as a single step; whether its judgmental seat is
one principal or a panel is conceptually irrelevant to the loop. The driver
mints the whole qualifying set, each judge's sealed verdict is read, and the
**theory's** [`Pass::combine`] decides the effective disposition — so a panel
whose members agree affirms, and a panel that diverges **surfaces the dissent
to the author as reasons** (reject-remedy → re-propose). This is the project's
answer to "we need multiple outside experts (category theorists, reviewers) to
weigh in": one judge is just a panel of one, and `panels-cannot-weaken-the-opponent`
holds — a panel never *grants* affirmation a judge would not.

### 5.2 The questions pass

`Questions` implements the `Pass`: its audit finds real, pinned drift
(`affects_mirrors_inbound` — outbound edges unacknowledged by their source),
its `remedy` proposes mirroring the first drift edge, its `combine` is
consensus, and its `Verify` reads the enacted edge back. This is the loop, run
on rung's own questions, by the real driver — the thing the bootstrapped
self-hosting arc is working toward.

### 5.3 Why the split matters

The audit is **only as honest as the judgmental/decidable split**, and this
theory won't let it blur: a decidable sentence that secretly called an outside
would be caught; a judgmental sentence that pretended a predicate settled it
would be a fake. Well-posedness is the case that proves the discipline is worth
it — it would be *unusable* if its judgmental cuts were silently "closed" into
heuristics.

---

## 6 · First-principles that shape it

1. **Governance lives in the sentence, not the policy.** The rot that kills a
   question registry (hidden decisions, decorative edges, verdicts recorded by
   whoever answered) is what the two sorts, the gate markers, and the closed
   edge taxonomy exist to make impossible rather than to remind you to avoid.
2. **Two sorts because a principal is handed one of two things.** Split the
   sentence so that what a judge rules on is exactly one question.
3. **The decidable/judgmental split is load-bearing, and it is protected at its
   edges.** Only recognize-able footprints get cold screens; the irreducible
   judgments stay judgmental.
4. **Claiming well-posedness is optional; faking it is not.** Mode B makes the
   onramp honest without letting Mode A dodge its anchor.
5. **The theory, driver, and carrier are separable.** A new body of questions is
   a new carrier + a `Scheme` + a `.het` instance — not new machinery.

---

## 7 · Where things live

| you want | go to |
|---|---|
| the theory and the model | [`rung-std/src/questions.rs`](../../rung-std/src/questions.rs) |
| the well-posedness doctrine (Mode A/B, four cuts) | §3 here, and the `theory!(question)` block in the source |
| the audit-rectify engine (abstract judging) | [`rung-driver/src/pass.rs`](../../rung-driver/src/pass.rs) |
| the theory-blind driver + carrier + instance | [`rung-driver/`](../../rung-driver/) (carrier, instance, config) |
| rung's own questions docket | `.het/rung-questions/questions/` |
| the law's tests over a synthetic docket | [`rung-std/tests/questions_theory.rs`](../../rung-std/tests/questions_theory.rs) |
| the theory over rung's real docket | [`rung-het/tests/questions_of_rung.rs`](../../rung-het/tests/questions_of_rung.rs) |
