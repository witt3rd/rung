**RULING (Q16):** What carrier makes `authored(p)` a derived fact?

**JUDGED BY:** an outside expert, out of band, on the strength of Q14's ruling and
the doctrine. Recorded here afterwards — *tier: attested*, in the terms of
`judgments/README.md`: an exchange a reader can audit, because nothing here can
tell a faithful transcription from an invention.

**THE VERDICT:** the carrier is a **commission contribution record**. The three
conditions Q16 required are met, the two forbidden shapes are refused, and the
implementation — not the definition — is what remains open.

## The question, restated

Q14 fixed the map

$$\pi(p)=\mathsf{authored}(p)\cup\{\mathsf{id}(p)\}$$

with `id(p)` the family identifier for discontinuous kinds. Q16 asks for the
*carrier* of the first summand: a source from which the pool can evaluate
`authored(p)` as a derived fact rather than a static declaration. Without it the
floor remains formally correct but operationally vacuous for every model
principal.

## Refusal of the two forbidden shapes

- **A guessed static list in `population.yaml`** manufactures the tags the floor
  exists to protect. Refused at the meta-level (Q14's own (B)).
- **Any source that itself requires a judgmental dispatch** re-opens the regress
  the doctrine closes. Refused.

## The ruling

The carrier is the **commission contribution record**:

1. The harness maintains a finite map

   $$C:\mathsf{Family}\times\mathsf{CommissionId}\to\mathcal{P}_{\mathrm{fin}}(\mathsf{ArtifactId}).$$

   Entry `C(f,c)` is the set of artifacts produced by family `f` under
   commission `c`.

2. For a principal `p` of family `f` under the active commission set `S` (the
   current commission, plus any prior commissions the supplier has explicitly
   carried forward),

   $$\mathsf{authored}(p)\;=\;\bigcup_{c\in S}C(f,c).$$

3. A newly opened commission begins with `C(f,c)=∅` for every family. Prior
   commissions enter `S` only by explicit supplier decision; they are never
   automatic.

4. The pool evaluates `authored(p)` by reading `C` and `S` at qualification
   time. Both are finite declared facts; no network call and no judgmental
   sentence is required. Disjointness remains ordinary finite-set comparison.

5. Until the record `C` exists and is wired into the pool, the static empty
   field remains the only value the pool can read. **Stage 5 therefore stays
   gated.** No guessed entries are written into `population.yaml`.

## Verification against the three conditions

- **Decidable.** `C` and `S` are finite data the pool already possesses at
  dispatch (once the log is present).
- **Non-vacuous.** Inside an open commission a model cannot judge any artifact
  its family produced under that commission. The constant-arrow reading is
  closed.
- **Not total.** Artifacts belonging only to closed commissions that have not
  been carried forward remain open to later instances of the same family.

## Equivalence note

Per-artifact attribution (each artifact carries the family that produced it;
`authored(p)` is the inverse image) is informationally dual. Either indexing may
be chosen; the commission-indexed form matches the language of Q14's ruling and
is preferred for that reason alone.

## Status

- **Q16 is resolved on the definitional question:** the carrier *is* the
  commission contribution record described above.
- **The implementation of the record remains open work.** Until it exists, model
  principals remain capable in role but cannot be meaningfully dispatched under
  P0, and the placeholder empty field continues under the same test pin that
  prevents it from shipping as a working configuration.
- **Stage 5 of the bootstrap stays gated** by the absence of the carrier, not by
  any remaining definitional ambiguity.

This keeps non-identity a belonging predicate rather than a decorative check,
keeps the gap visible rather than papered, and supplies a precise interface the
pool can later read.
