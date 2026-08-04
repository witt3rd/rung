# Bootstrap — stating the theory in the theory

> **Status: proposal. Not normative, not yet doctrine.** It argues for a change
> that has not been made. Where it disagrees with any `*-props.md`, that
> document governs and this one is wrong.

## 0. The proposal in one paragraph

`docs/*-props.md` are prose documents whose integrity is checked by 2,195 lines
of Python. The propositions are claims about artifacts; the Python is an
unstated theory of what those claims mean; `conformance.md` is its satisfaction
record. Every piece of that already has a first-class form in this repo:
`theory!` declares gate-marked sentences, `Pool` mints the tokens that settle
judgmental ones, `ladder!`'s pass carries audit → propose → dispose → enact, and
`rung-std::questions` proves the whole shape works on a body of markdown files
read off disk. This proposes that **the propositions become a theory in
`rung-std`, this repository becomes one of its carriers, and the Python shrinks
to a parser.** Not all of them. The proposal is explicit about which
propositions can never make the trip and why.

---

## 1. Why now

Four things exist that did not when the Python was written.

| piece | what it supplies |
|---|---|
| `rung::theory!` | the sentence surface — `decidable` bodies, `judgmental` sentences with roles |
| `rung::Pool` + `Qualified` / `Authorized` | the tokens; no public constructor, so a verdict cannot be typed |
| `rung_het::het_pass!` | audit → propose → dispose → enact, as a `ladder!` |
| `rung_std::questions` | **a theory over markdown files in this repo, with two carriers** |

The fourth is the one that matters. `rung-het/tests/questions_of_rung.rs` reads
`docs/questions/**/*.md` off disk at test time and evaluates every decidable
sentence against all twelve. The method is not speculative: it is running, in
CI, over this repository's own documents.

`rung_std::principals` supplies the other half. `Kind` already declares `Llm`,
`Agent`, `RelationalBeing`, `Human` — so a model *is* an admissible principal by
the theory's own reckoning. What is missing is one adapter, not a design.

---

## 2. The precedent, and what it fixes about the current arrangement

`questions` is the template, and its structure is the argument:

```
rung-std/src/questions.rs           the theory   — sorts, sentences, roles, edits, lifecycle
rung-het/tests/questions_of_rung.rs carrier one  — rung's own docs/questions/, read off disk
rung-std/tests/questions_theory.rs  carrier two  — a synthetic docket, disjoint id space
```

Three files because "a theory with one carrier is a domain model wearing a
library's name." The same split applies here: a `props` theory, this repository
as carrier one, a synthetic corpus as carrier two.

What it fixes: today `_ledger.py` decides what conformance *means* — that an
`enforced` row must name a test whose `fn` exists, that `collides` must be
empty, that a bare decimal in a mechanism string is an error. Those are
sentences of a theory nobody declared. They live in a Python file that is not in
`DOC_NAMES`, is not checked by anything, and states its rules only by enforcing
them. That is the eighth-instance pattern — *a rule stated in prose that nothing
enforces* — inverted: **a rule enforced by code that nothing states.**

---

## 3. What "getting off prose" can and cannot mean

Gate-marking the 379 propositions by hand was the obvious move and it is wrong,
for a reason this repo already states:

> **11.33** Mis-marking is likewise **not a claim that could be false**. Marking
> a sentence `decidable` gives it the decidable signature. A body needing an
> outside will not typecheck in that position.

A `data-gate="decidable"` attribute in a markdown anchor is precisely a promise
someone keeps. Nothing goes red when it is wrong. Adding one to every
proposition would manufacture 379 new unenforced claims in the name of removing
unenforced claims.

**So there is no marker.** The design instead is:

> A proposition's gate is **which arm of `theory!` declared it**. Membership in
> the theory is the marker, and membership is a fact about compiled code.

A proposition encoded as `decidable foo = |m| ...` *is* decidable — the body
takes only the model, so there is no parameter an outside could enter through. A
proposition encoded as `judgmental bar: SpecReader` *is* judgmental — settling it
consumes a `Qualified<SpecReader>` that only the filter mints. Mis-marking is
not wrong, it is **unrepresentable**, which is the only form 11.33 accepts.

A proposition with no sentence is **unencoded**. That is the census, and it is
the honest replacement for a hand-typed verdict.

### The three-way split this forces

Attempting the encoding partitions the corpus, and the partition is worth more
than the encoding:

- **Sentences.** "Every rung and verdict MUST be `!Send + !Sync`." A real
  `M ⊨ φ`. These make the trip.
- **Signature.** "A rung is an **object**. An object is inert — data at rest, a
  point. It has no verbs" (`rungs-are-objects`). This is not a claim about a
  model; it *declares a sort*. It belongs in Σ, not among the sentences.
- **Rationale.** "The hiding is **not a convenience**…" (`hiding-is-not-optional`).
  An argument. It has no gate because it is not a sentence, and it belongs in
  `*-notes.md`.

Today all three are mixed and all three are called normative. Nothing about the
current arrangement can tell them apart. The encoding cannot help but tell them
apart, because only one kind compiles.

---

## 4. The theory

Two sorts, forced by the same constraint `questions` hit — a `theory!` declares
one sort per invocation, and the two sorts are the two things a principal is
ever handed.

```rust
// sort 1 — one proposition
theory!(proposition for Proposition {
    decidable  anchor_is_well_formed      = |p| p.slug.is_kebab() && p.follows_anchor();
    decidable  parent_exists              = |p| p.parent.map_or(true, |q| p.corpus_has(q));
    decidable  number_is_derived          = |p| p.rendered == p.recompute();
    decidable  vocabulary_is_current      = |p| !p.body.names_a_retired_term();
    decidable  cited_test_exists          = |p| p.cited_tests().all(Path::resolves_to_fn);

    judgmental establishes_what_it_cites  : SpecReader;   // does the test prove the claim?
    judgmental is_a_sentence_not_rationale: Editor;       // does it belong in props at all?
});

// sort 2 — the corpus, for what no single proposition can see
theory!(corpus for Corpus {
    decidable  slugs_are_unique           = |c| c.slugs().all_distinct();
    decidable  references_resolve         = |c| c.refs().all(|r| c.has(r.slug));
    decidable  numbering_is_current       = |c| c.recompute() == c.rendered();
    decidable  every_proposition_placed   = |c| c.props().all(|p| c.encoded(p) || c.unencoded(p));
    decidable  no_proposition_collides    = |c| c.collides().is_empty();

    judgmental partition_is_adequate      : Taxonomist;   // sentence / signature / rationale
});
```

Roles: `SpecReader`, `Editor`, `Taxonomist` for judgment; `Maintainer` for
authorship, with standing over `docs/`.

Edits — the vocabulary the pass may apply, which is the theory's and not
Het's: `AmendBody`, `Reparent`, `Retire`, `EncodeAsSentence`, `MoveToNotes`.

### Carriers

1. **This repository.** `Corpus` read from `docs/*-props.md` at test time, every
   decidable sentence evaluated over all 379. If a sentence is wrong about the
   real corpus, the suite says so rather than being relaxed until it agrees —
   the rule `questions_of_rung.rs` already follows.
2. **A synthetic corpus** with a disjoint slug space, in
   `rung-std/tests/props_theory.rs`. Without it this is a domain model wearing a
   library's name.

---

## 5. What the Python becomes

Not deleted. **Demoted from judge to parser**, which is the job it should have
had.

| today | after |
|---|---|
| `_props.py check` decides integrity | parses markdown → `Corpus`; the sentences decide |
| `_ledger.py` decides what conformance means | the theory states it; `conformance.md` is the satisfaction record |
| verdicts curated in a Python dict | derived from which arm declared the sentence, plus whether it holds |
| `_props.py cited` | stays — it is genuinely decidable plumbing and already correct |

Rough shape: ~2,195 lines of Python become ~400 of parser plus a theory in
`rung-std`. The line count is not the point; **the point is that the parser
makes no claims.** Everything it does today that constitutes a claim moves
somewhere a mutation can reach it.

---

## 6. The rectify loop

`het_pass!` already has the shape: `Auditing → Audited(Verdict) → Proposing →
#[authorial] Proposed(Proposal<Edit>) → #[judgmental] { … }`.

Pointed at the corpus, a non-conformance produces a `Proposal` in one of two
directions, both already in the vocabulary:

- **`remedy`** — the corpus is wrong. Amend the proposition, retire it, reparent
  it, move it to notes.
- **`dispute`** — the *sentence* is wrong. The proposition is right and the
  encoding misread it. Still judged: a dispute goes to `dispose` exactly as a
  remedy does.

That second direction is what makes this a loop rather than a linter, and it is
the one that needs the authorial gate: amending a normative document is an
enactment by a principal holding standing over `docs/`, not a classification.

---

## 7. Obstacles, named

Each with what it blocks, so none is discovered late.

1. **`#[conditional(..)]` is a parse-time refusal.** Most propositions about the
   macro are universally quantified over *all possible ladders* — "the macro MUST
   reject any ladder in which two carry fields share a name." That is not
   decidable by inspecting a model; it is decidable per algebra, which is exactly
   Het's conditional gate (`classifier-one-level-up`). **Blocks:** most of §1 and
   §2 of `rung-props.md`. **Mitigation:** encode the instance-level claim
   (this corpus, this emitted module) and leave the universal claim to
   `trybuild`, marking it unencoded rather than pretending. This is Q11's
   remaining half and the proposal does not close it.

2. **Verdicts are Boolean.** No metric `d`, no `ε`. A ruling on
   `establishes_what_it_cites` cannot say *"probably, at 0.7."* **Blocks:**
   graded confidence in judgmental rows. **Mitigation:** none; record it, as
   `questions` did.

3. **A decidable body returns `bool`.** A sentence failing over five propositions
   cannot say which five. **Blocks:** useful failure messages. **Mitigation:**
   the `questions` workaround — a plain method computes the detail, the sentence
   supplies only the verdict.

4. **One sort per `theory!`.** `Sen(Σ)` becomes a hand-written concatenation of
   two modules' `SENTENCES`. **Blocks:** nothing real. Recorded as a DSL limit.

5. **Non-identity will disqualify most available judges.** `establishes_what_it_cites`
   needs a judge provenance-disjoint from whoever wrote the proposition *and* the
   test. In a repository authored by one human and a small set of agents, that
   set is often empty. **Blocks:** settling the judgmental fragment at all.
   **This is the interesting one** — see §8, stage 4.

6. **Self-reference.** Answered, not open. Het states it:

   > **5.63** An subject is therefore **self-governing** — its own algebra runs
   > its decidable audit — but **not self-closing**: its judgmental dispositions
   > require the monad's outside.

   And the regress terminates on `tower-floor` (6.4): a **decidable**
   well-formedness predicate `W` on signatures, whose clauses (6.41) are
   syntactic and checkable without the theory. `W` is the trusted base. Stage 0
   below is exactly "check `W` by hand, once."

---

## 8. Stages, each with the mutation that must go red

No stage lands without a falsifier that was actually run and restored.

**Stage 0 — `W` by hand.** Verify the props theory satisfies `tower-floor`'s
clauses: at least one sort and one operation, every sentence gate-marked, every
judgmental sentence naming a role. One reading, recorded.
*Falsifier:* remove a role from a judgmental sentence → `theory!` refuses to
compile (the `judgmental $s:ident = $body` arm is a hard error: *a judgmental
sentence is settled, not computed*).

**Stage 1 — parser, no claims.** Markdown → `Corpus`. `_props.py` keeps working
unchanged; the parser is proved against it by agreeing on all 379.
*Falsifier:* corrupt one anchor → the two disagree → the test that asserts
agreement goes red.

**Stage 2 — the decidable fragment.** Encode the five corpus sentences and the
five proposition sentences. Both carriers.
*Falsifier:* for each sentence, mutate the corpus so it should fail, confirm it
does, restore. A sentence with no run mutation is not promoted.

**Stage 3 — retire the Python's claims.** Delete from `_props.py`/`_ledger.py`
every check now stated as a sentence. The Python keeps only parsing and `cited`.
*Falsifier:* the deletions must not change CI's verdict on a corrupted corpus —
if removing a Python check loses a failure the theory does not catch, the
sentence was incomplete and stage 2 was not finished.

**Stage 4 — the judgmental fragment, honestly.** Declare
`establishes_what_it_cites` and leave it **unsettled**. Ship the count.
*Falsifier:* attempt to settle one with a token minted against a different
proposition → G13's injected prologue refuses it.

This stage is where the proposal earns or loses its keep. Today the ledger says
105 rows are `enforced`; what it cannot say is how many were independently judged
versus asserted by the person who wrote both the claim and the test. Stage 4
makes that number exist. **It will be large and it will be uncomfortable, and
that is the entire point** — it is the same discovery as *"a mutation that came
back green because a test's generic bound was never solved,"* one level up.

**Stage 5 — a model as judge.** `Kind::Llm` is already an admissible principal
and `rung_std::llm` already puts a blocking outside call on an arrow. The
missing piece is an adapter: a `Principal` whose settle drives `LlmCall`. That
makes the judgmental fragment tractable at corpus scale.
*Falsifier:* a model whose provenance tag overlaps the proposition's author must
fail `qualify`. If a model judge can rule on text it helped write, non-identity
is decorative and the stage is void.
*Open question this raises and does not answer:* what `π` a model instance
carries. If every model shares one tag, no model can judge any document a model
touched — which in this repository is most of them.

**Stage 6 — rectify.** Point `het_pass!` at the corpus. Only after 2–5.

---

## 9. What stays prose, permanently

Not a phase-out list. These have no encoded form and should not acquire one:

- **Signature propositions** — `rungs-are-objects`, `transitions-are-morphisms`.
  They declare sorts. Encoding them as sentences would be a category error.
- **Rationale** — `hiding-is-not-optional`, the corrections appendix. Arguments
  and history. `*-notes.md` is their home and this proposal moves some of them
  there.
- **Design judgments** — J1, J2. Explicitly the half no machine decides.
  Encodable as `judgmental`, but their subject is a design decision that does not
  exist as an artifact to be handed to a judge.
- **The non-guarantees** — §5. A claim that something is *not* enforced has no
  satisfying model.

A rough census against the ledger as of this writing: of 379 propositions, 254
are `out-of-scope` and most are signature or rationale. **The honest target is the
105 enforced rows plus the 13 expressible ones, not 379.** Any framing that
promises "no more prose normative" is overselling; the correct framing is *the
normative claims that a run can check stop being prose.*

---

## 10. Kill criteria

Abandon, and say so, if:

- Stage 2 cannot produce a run mutation for a sentence — it was never decidable.
- Stage 3 loses a failure the Python caught — the theory is weaker than what it
  replaced, and shipping it would be a net loss of enforcement dressed as a gain.
- Stage 5's provenance question has no answer that keeps non-identity real — then
  the judgmental fragment stays declared-and-unsettled indefinitely, which is
  honest but is not what this proposal promised.
- The theory begins stating things the propositions do not. It may say which
  sentences are decidable and who may judge the rest. It may not add a claim.
  A second source of truth beside the document it governs is the failure mode
  that would make all of this worse than the Python.

---

## 11. What it does not fix

Q11 stands. `establishes_what_it_cites` constrains what a judge is handed, not
what the judge returns — admissibility is a condition on an arrow's **output**,
and every mechanism here, as everywhere else in this repo, constrains its
**input**. Encoding the corpus does not close that; it relocates it.
