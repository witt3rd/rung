---
id: q13
status: open
depends_on:
  - {on: q2, kind: justification}
affects:
  - {target: q4, kind: premise}
---

# Q13 — Can a suspended run survive process death? *(open)*

**Status:** OPEN

**Question.** A judgmental dispatch is now suspendable
([`G16`](../../rung-props.md#g16-the-residual-channel)): a principal that cannot
answer raises a matter, the transition returns `Result<Next, Suspended<Prev>>`
with the argument unconsumed, and a resume edge revives the run when the raised
matter terminates. All of that happens **in one process** — a driver holds the
`Suspended<Prev>` in memory, and that is the whole of the claim
([`5.7`](../../rung-props.md#suspension-is-in-process-only)).

A raised matter that takes months is not an odd case; it is the motivating one.
So:

> **Can a suspended run be written to durable storage and reconstituted later —
> and if so, what is the reconstituted token?**

## The collision, stated precisely

[`G2`](../../rung-props.md#g2-sealed-construction) says a rung MUST NOT be
constructible outside its module, and says why: not as a fabrication guard but
because *a verb cannot occupy object-position*
([`the-law`](../../rung-ct-props.md#the-law)). A state is reached **only by
traversing an arrow**, never fabricated to hold an arrow's result.

Deserialization is fabrication by that definition. `serde` reading a
`Suspended<Posed>` back from bytes does not traverse the arrow that produced it;
it **asserts** the result. Every mid-ladder rung in the file was reached by an
arrow *in a process that no longer exists*, and the current process has no
access to that fact — only to a byte string claiming it. The bytes are a
proposition about history, and G2's whole content is that history is carried by
construction rather than asserted.

The three obvious moves each fail for the same reason:

| move | why it does not close it |
|---|---|
| `#[derive(Serialize, Deserialize)]` on the emitted rungs | `Deserialize` **is** a public constructor. It is `new` with a wire format, and it is `pub`, so G2 holds in name only. |
| a private `from_wire` inside the module, called by a driver | G2's boundary is the module, and the driver is outside it. Moving the call inside changes who types the line, not who traverses the arrow. |
| an `Authorized` pen on the reconstitution | This is the near miss, and it is why the question is filed rather than answered — see below. |

## What `resumption-is-authorial` does and does not answer

The resume edge is gated on an [`Authorized`](../../rung-props.md#resume-signature)
pen because reviving a suspended run *constructs a rung*, and G2 seals that from
outside the module
([`resumption-is-authorial`](../../rung-het-props.md#resumption-is-authorial)).
It is tempting to read that as already settling this question: put a pen on the
deserializer too, and reconstitution becomes just another authorial act.

**It does not, and the gap is exact.** Resumption-is-authorial answers **who
may** revive a run. It does not answer **what the revived token is**. In the
in-process case that second question does not arise, because the token is *the
same token* — it was never consumed, it was held, and the resume edge hands it
back. Nothing is reconstructed; the pen governs an act on an object that already
exists.

Across process death there is no such object. A pen would authorize a principal
to **assert** that a run reached rung *R*, with no arrow anywhere in the current
process that reached it. That is a stronger power than any authorial operation
Het describes: `enact` transforms a subject that exists
([`enact-makes-an-endofunctor`](../../rung-het-props.md#enact-makes-an-endofunctor)),
and standing is held *over a container*
([`authorial-qualifying-set`](../../rung-het-props.md#authorial-qualifying-set)),
not over the past.

So: **the authority half is answered and the representation half is open.** Any
proposed answer must say what a reconstituted token *is* — not merely who is
allowed to produce one.

## Relation to Q2

[Q2 (cross-crate provenance)](../parked/q2-cross-crate-provenance.md) is the
nearest existing question, and it is the same question weakened by one axis.

Q2 asks whether a token can be sealed once it crosses a **crate** boundary —
where the receiving code is trusted, like any Rust public API. This asks whether
it can be sealed once it crosses a **process** boundary, where there is no
receiving code at all: only bytes, and a later process willing to believe them.

The relation is useful in both directions. Q2's known mechanism — emit the
sealed types into a sub-crate the macro alone controls, so even the defining
crate cannot fabricate — is a *spatial* answer and does nothing here, because
the sub-crate in the new process is just as unable to witness the old one's
arrows. Conversely, anything that answers this question answers Q2 outright: a
token that carries its own traversal evidence is sealed across every boundary,
not just this one. That asymmetry is the reason to keep them apart rather than
merge them — Q2 is engineering-with-a-cost and parked for YAGNI, and this is
not.

## What would move it

Not a serialization format. The shape of an answer would have to be one of:

- **A witness that survives the wire.** Something a rung carries that constitutes
  evidence of the traversal — not a claim of it — and that a later process can
  check without having run the arrow. This is where a cryptographic reading of
  "the compiler is a signature for state transitions" would have to be made
  literal rather than metaphorical, and it is not obvious that it can be.
- **A ruling that reconstitution is a different act.** Perhaps a run read from
  bytes is not *the* run resumed but a **new** run entered at a declared rung,
  with the old one's terminal as its argument — which would make it an ordinary
  entry-constructor call and no G2 problem at all, at the cost of saying that
  identity across process death is not preserved and does not need to be. That
  is a change to a proposition, not an implementation, and belongs to whoever
  owns the doctrine.
- **A ruling that it is out of scope.** Het governs what passes through it, and a
  process boundary may simply be outside. Then the answer is that a driver
  persists *the theory's own* record — the `Raised` reference, which is opaque
  and already the theory's
  ([`raised-reference-is-opaque`](../../rung-het-props.md#raised-reference-is-opaque))
  — and re-enters the ladder from the top on restart, replaying rather than
  resuming.

**Not solved here, deliberately.** The in-process channel is the piece that was
missing from the language and it stands on its own; guessing at this one would
put a hole in G2 to save a driver some work.
