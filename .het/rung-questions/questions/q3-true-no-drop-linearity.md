---
id: q3
status: blocked
depends_on:
  - {on: rust-linear-types, kind: gate}
affects:
  - {target: RUNG-CT§7, kind: citation}
---

# Q3 — True no-drop / linearity *(blocked)*

**Status:** BLOCKED

**Question.** `#[must_use]` is escapable (`mem::forget`, `let _ = token;`, burying
the token in a dropped container). Rust is affine, not linear. Can a live token be
made truly impossible to drop?

**Why it's blocked.** This needs *linear* types — a language feature Rust does not
have and a lint cannot emulate. It is not ours to close.

**Adjacent angle.** A different substrate — a linear language (Austral, Idris,
Lean, or a future Rust with linear types) — would host the ladder faithfully.
Because the ladder is a morphism in a linear/dagger category (see the CT map
below), a linear host would make that correspondence *exact* and close no-drop for
free. The experiment: port one ladder to a linear substrate and measure what
closes.
