# `judgments/`

**Operational, not documentation.** These records are what settles the
judgmental fragment of rung's doctrine — the propositions no test can decide.
A `Kind::Judgmental { ruling }` in `rung-doctrine/` names a file here, and
`cargo run -p rung-doctrine --bin audit` reads it. They sit at the repository
root for the same reason `rung/` and `rung-doctrine/` do: losing one changes
what the project is known to hold.

## What a judgment is for

47 propositions declare that only a principal can settle them. Most are
categorical identifications — *"a transition is a Prism"*, *"the tower is a
fibered category"* — and the precedent for taking them seriously is that
**Q7's ruling overturned the account that preceded it**. A judgment here is
not a blessing; it is a finding that may go either way and may require the
doctrine to change.

## The record

```yaml
---
proposition: transition-is-a-prism      # must be judgmental, and unsettled until this exists
role: category-theorist                 # must match what the proposition declares
tier: attested | dispatched
judges:
  - id: a-name
    provenance: [what-they-authored]    # must be disjoint from the proposition
    verdict: conforming | non-conforming
    epsilon: 0.15                       # optional; an honest error bar
    on: 2026-08-05
---

The reasoning. Not optional, and not a summary of the verdict — the argument
that would let a later reader disagree.
```

### `judges` is a list, and nothing here decides how long

A panel is `⊨` with more than one judge (`panels`), and the habit in
`questions/resolved/_evidence/` is two independent reviews. The schema
carries any number.

**What it does not carry is a rule about how many are enough.** Deciding that a
claim warrants a deep panel rather than one reasoning model is a judgment about
*worth*, and Het declares no worth law (`het-declares-no-worth-law`) — it
belongs to HetOpt, which does not exist. So records report the count and
nothing ranks, requires or prefers.

### `tier` is the honest part

| tier | what it is |
|---|---|
| `dispatched` | produced by `rung-driver` from an actual `Pool::consult`. The judge's provenance comes out of a sealed `Judgment`, not out of a field someone typed. |
| `attested` | a judgment that happened out of band, written down afterwards. |

**An `attested` record is a receipt, not a judgment.** Anyone with commit
access can write one, and nothing here can tell a faithful transcription from
an invention. That is the same gap `composition-notes.md` §3 names — *the
ruling arrived out of band* — and it is why an attested record should include
the exchange it is transcribing, so a reader can audit the receipt.

`dispatched` closes it, and is what `rung-driver` was built for.

## What is checked

- the proposition exists, is judgmental, and names this record
- the role matches the one the proposition declares
- every judge's provenance is **disjoint** from the proposition — non-identity,
  which is the whole point of asking someone else
- a verdict and non-empty reasoning are present

What is not checked, and cannot be: that the judge said it, and that the
reasoning is sound.

## There are none yet

Zero records, 47 unsettled. That is the honest state, and an empty directory
with a schema is a better account of it than a plausible-looking example would
be — a fabricated judgment is exactly the thing this collection exists to make
impossible to pass off.
