# Q5 — Fork-join / concurrency *(open)*

**Status:** OPEN

**Question.** Can a transition split one token into N independent linear
sub-tokens, run them concurrently, and join by consuming all of them exhaustively?

**Why it's open.** This deliberately bends the one-token contract (one → many →
one) that everything else rests on. The hard part is the *type* of a split rung:
how to yield a set of independent linear tokens and force the join to consume every
one. Genuinely unsolved, and where real workflows live.
