> **Archived 2026-08-05. Right about its question, wrong about the question.**
>
> This measured the 118 enforced and expressible ledger rows and found that
> **none is a claim about a document** — so none could move into a theory whose
> model is the corpus. That finding was correct and it corrected the proposal.
>
> It then drew the wrong conclusion: that the real target was the Python's
> integrity rules. Also too small. The right question is not *what can be
> computed from a data structure* but *what has a proof that fails when it is
> violated* — under which the decidable fragment went from 6 to 123, and most of
> what this census filed as unreachable turned out to be already proven.
>
> Kept as the record of a measurement that was sound and a conclusion that was
> not.

---

# Shape census — what the 118 rows are actually made of

**Status: informative.** `rung-props.md`, `rung-ct-props.md` and
`rung-het-props.md` govern. This note reports a measurement taken against
`2026-08-04_bootstrap-proposal.md` §9, which proposes that *"the honest target is the
105 enforced rows plus the 13 expressible ones, not 379."* It states no
proposition and imposes no obligation.

Measured 2026-08-04 at `881df9f`, over all 118 `enforced` + `expressible` rows of
`conformance.md`. Method: each row's cited test is classified by **what
establishes it** — whether the test drives a run, refuses a compile, or evaluates
a sentence over a model.

---

## 1 · The result

| bucket | rows | share | what establishes the claim |
|---|---:|---:|---|
| **run-witnessed** | 67 | 56% | a ladder is driven and the claim is read off the execution |
| **compile/universal** | 40 | 33% | `trybuild` refusal, or the compiler directly (`(rustc)`) |
| **already a theory's** | 9 | 7% | a sentence of an existing `theory!`, evaluated by a carrier |
| **Python-enforced** | 2 | 1% | `_ledger.py` is both the enforcer and the subject |

By document:

| bucket | `rung-props` | `rung-het-props` | `rung-ct-props` |
|---|---:|---:|---:|
| run-witnessed | 27 | 32 | 8 |
| compile/universal | 21 | 10 | 9 |
| already a theory's | 0 | 7 | 2 |

The 33% compile/universal share is the direct measurement of bootstrap's
obstacle 1. It is somewhat better than the ⅓-of-`rung-props` the proposal
implies — but §2 concentrates it: **21 of `rung-props.md`'s 48 rows are
compile-refusals**, so the macro spec is where the obstacle actually bites, as
predicted.

## 2 · The result that matters more

**None of the 118 is corpus-decidable in bootstrap's sense.**

The nine rows in the third bucket are not propositions a props theory would
carry; they are propositions *already carried* by `rung_std::questions` and
`rung_std::principals`, whose models are questions and principals, not
propositions. The two in the fourth are propositions **about the conformance
apparatus itself** — `conformance-suite` and
`no-guarantee-cites-a-compile-fail-doctest` — where `_ledger.py` is
simultaneously the enforcement and the thing being claimed about.

The other 107 are claims about rung's machinery: what the macro emits, what a
token permits, what a run does. A model built by parsing `docs/*-props.md`
cannot evaluate any of them, because none of them is a claim about a document.

## 3 · The corollary for bootstrap §4

Read bootstrap's ten proposed sentences against the corpus and the same result
arrives from the other direction:

```
anchor_is_well_formed   parent_exists          number_is_derived
vocabulary_is_current   cited_test_exists      slugs_are_unique
references_resolve      numbering_is_current   every_proposition_placed
no_proposition_collides
```

**Not one of these encodes any of the 379 propositions.** No proposition in any
of the three documents is about anchors, slugs, parents, numbering, or reference
resolution. Those are the *Python's* rules — which is precisely §2's argument,
stated correctly there:

> a rule enforced by code that nothing states.

So the encoding project bootstrap actually describes in §4 is: **state the
Python's unstated theory as sentences.** That is a real and worthwhile project,
and it is the one §2 argues for. It is not the project §9 names.

### What §9 should say instead

Not *"the honest target is the 105 enforced rows plus the 13 expressible ones."*
Those 118 are already enforced by runs and by the compiler, and moving them into
a props theory would weaken them — a decidable sentence over a parsed document
is a strictly worse witness for "a `Qualified` token cannot be forged" than a
`trybuild` snapshot is.

The honest target is the complement: the integrity rules currently living only
in 2,195 lines of Python, of which **two** have a proposition today and the rest
have none. The measure of success is not *"N of 379 encoded"* but *"the Python
makes no claims"* — which §5 already states correctly, and which does not need
the §9 framing at all.

## 4 · What this does and does not change

**Does not change:** stages 0–3 are unaffected in substance. Stage 1's parser,
stage 2's sentences, and stage 3's kill criterion — *the deletions must not
change CI's verdict on a corrupted corpus* — all describe the Python's rules and
are correct as written.

**Does change:** the scope claim, and with it the expected size. Stage 2 is
smaller and more tractable than §9 implies, because it is not attempting 118
propositions; it is attempting roughly ten sentences that no proposition
currently states. Stage 6 (`rectify`) then operates over a corpus governed by
those ten, not over the conformance ledger.

**Also worth stating plainly:** obstacle 1 is not an obstacle to the project
bootstrap should be doing. `#[conditional(..)]` blocks encoding the *macro*
propositions, and those are the 40 that should stay where they are. The
proposal's most-feared blocker turns out to sit entirely outside its real scope.

## 5 · Method, and its limits

Classification is by cited test, mechanically:

- cited fn body contains `trybuild`, or the citation is `(rustc)` →
  compile/universal
- cited path is `compile_pass.rs` (static assertions over emitted types) →
  compile/universal
- cited path is a `*_theory.rs` carrier or `questions_of_rung.rs` → already a
  theory's
- cited path is `docs/_ledger.py` → Python-enforced
- otherwise → run-witnessed

**Limits.** The buckets are shapes of *evidence*, not of propositions, and a
proposition can in principle be established by a different shape of evidence than
the one it currently cites. Two rows citing `compile_pass.rs` are borderline —
they assert properties of emitted types rather than refusing a malformed ladder —
and moving them would shift the compile/universal share by under 2%. The
`run-witnessed` bucket is heterogeneous by construction: it is the residue, and
it holds everything not established by the compiler or by a theory.

None of that touches §2 or §3, which are the claims that decide the count.
