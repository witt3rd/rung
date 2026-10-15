# The live page: what grows with the record, and what redaction costs

Status: a reading of the code as it stands (`ui/src/live/store.ts`, `ui/src/data/hooks.ts`,
`ui/src/record/folds.ts`, `ui/src/record/redact.ts`, `ui/src/live/wire.ts`). **No cost here is
measured.** Measuring needs a build and a browser run on a build lane; a specialist
sandbox has neither. Every statement below is a count of passes over data, derived
from the code. Nothing is capped or cut, and this note proposes no cap. Where the
numbers matter, the next step is to measure (see the end).

Notation: *N* is the number of lines in the record held by the page, *B* the bytes
of those lines, *T* the length of the text of the turn being streamed, *p* the
number of pieces that text arrived in.

## 1. What the page does when one line arrives

For each line the events stream delivers, `LiveStore.onRecord` redacts that line
once (cost proportional to its own size) and queues it. A timer (20 ms) later
calls `flush`, which does the following for the whole batch:

1. `this.state.lines.concat(this.pending)` builds a new array of all *N* lines (the
   state is replaced, not appended to, so React sees a new value). Cost: *O(N)*
   per flush, in references, not bytes.
2. `set` notifies subscribers; `useInstance` sees a new `lines` and its `useMemo`
   runs every fold again over all *N* lines: `summarize` (which itself calls
   `foldTurns`, `foldDecisions`, `foldSpend`, `foldPack`), then `foldTurns`,
   `foldDecisions`, `foldPack` and `foldSpend` again for the returned value. About
   eight full passes over *N* lines per flush, several of them building objects
   per turn and per call; `foldTurns` also filters and searches per turn.
3. Each page reads those folds; `ContextPage` also scans all lines
   (`lines.some(...)`) to build a query key.

So one flush costs *O(N)*. A flush happens at most once per 20 ms while lines are
arriving, and a record that is followed for a long time arrives continuously.
Following a record from line 1 to line *N* therefore costs on the order of
*N* × (number of flushes), and the number of flushes grows with the length of the
run: the total is **quadratic in the length of the followed record** (each new
flush re-reads everything before it). The constant is the number of fold passes
(about eight) times the per-line work in them, plus the array copy.

The batching helps by a factor of the burst size (a burst of *k* lines is one
flush, not *k*), so a burst such as the replay on opening costs one pass. The
steady case of one line every few hundred milliseconds does not benefit.

`runningTurn()` also walks `[...state.lines, ...pending]` (a copy plus a scan,
*O(N)*). It runs when the replay is declared caught up and when a "behind" frame
arrives, not per line, so it does not add to the per-line growth.

### First load

`loadRecord` reads the record in pages of `RECORD_PAGE` lines by following `next`,
and does `lines = lines.concat(page.lines)` for each page: each page copies the
array built so far. For *P* pages that is *O(N·P)* reference copies, which is
quadratic in *N* with a small constant (one reference per line per page). The
host side of the same cursor is linear (see the changelog entry on the linear
record cursor); the quadratic part here is the page's own accumulation. The
whole record is then parsed and redacted once (`parseRecordLines`), which is
linear in *B* (section 2).

## 2. What redaction costs

Two redactors run on the page, both on text the host already redacted:

- **Whole lines** (`redact`, from `makeRedactor`): applied to every string value
  and every key of a decoded line, on each line once when it arrives or is loaded.
  For a string it removes exact secrets it was given (none are given on the
  page: `makeRedactor([])`), then `redactShapes`.
- **Streamed text** (`redactStreaming` = `redact` then `hideUnfinished`): applied
  to the **whole text so far** every time a piece arrives (`setDelta` is called on
  each `text` piece with the text so far, since a key may be split across pieces
  and redacting a piece alone would leave the tail of a key).

`redactShapes` is a single pass over a string: the scan for key prefixes uses one
regular expression, and every forward look (end of a token run, next dot) is
memoised in `Next`, so a hostile text such as a million `eyJ-` repeats is scanned
once and not once per candidate. The bearer scan and the private key scan are each
one pass. A test in the host (`rung-host/tests/redactor.rs`) pins the host
redactor's linearity with a ratio; the page's port has a matching design but is
the part with the less direct evidence. Cost for one string of length *L*: *O(L)*
with a constant that includes a regular expression pass, a `slice` per candidate
and a sort of the found ranges (*O(r log r)* for *r* hits).

Consequences:

- **Per line**: linear in the line's bytes. Over the record, linear in *B*, paid
  once per line. This is not quadratic.
- **Streaming**: each of the *p* pieces re-redacts the text so far, so one turn
  costs about *p* × *T* / 2 character-visits: **quadratic in the length of the
  streamed text** (for a fixed piece size, *p* is proportional to *T*). The
  streamed text is dropped when the turn's end is on the record, and the line
  that replaces it is redacted once, so the quadratic cost is bounded by one turn's
  text, not by the record. It is the cost to expect when one turn writes a very
  long answer.
- **Shapes and `UNFINISHED`**: `hideUnfinished` runs two regular expressions that
  are anchored at the end of the text (`$`). A regular expression anchored at the
  end can still try each start position; with the look-behinds and the character
  class `[A-Za-z0-9_.-]*$` a long run of token characters followed by a character
  that is not one can make each candidate start scan to the end of the run. This
  has not been shown to be a problem or ruled out: it is a place to test with a
  long run of token characters at the end of a streamed text.
- **Very long single lines** are dominated by the layout, not by redaction (see the
  separate note on the strata page and long lines in `ui/README.md`).

## 3. What is not quadratic

- Per-line redaction on arrival (linear in the line, once).
- The host's record door: linear since the cursor change.
- Burst handling: a burst is one flush.
- The summary request: one small request, after the record moves, at most once
  per 250 ms.

## 4. If a fix is wanted (not done here)

Stated as options, to be chosen after measuring; none is a cap or a cut:

- Keep folds incremental: fold only the lines added since the last flush into the
  previous result (turns are keyed and appended, so most folds are a tail update),
  and rebuild fully only when the record is replaced (`onReset`).
- Append to a growable store and publish a version number plus a view, instead of
  copying the array each flush; accumulate pages in a list and join once in
  `loadRecord`.
- For streaming text: redact incrementally from the last point at which no key
  could still be open (everything before the last separator is final), and only
  re-scan the tail. This keeps the rule that a key split across pieces is caught,
  because the tail that could still become a key is always re-read.

## 5. What to measure on a lane

With the page built and a synthetic record (generated, no real content):

1. Time one `flush` and the folds `useMemo` at *N* = 1 000, 10 000, 100 000 lines;
   the ratio between sizes shows the exponent. Count renders per second.
2. Time `loadRecord` at the same sizes and with different `RECORD_PAGE` values,
   in a browser and against a local gateway.
3. Time `redactStreaming` on a synthetic text of 10 kB, 100 kB, 1 MB, 10 MB,
   written in 100 pieces and in 10 000 pieces; and with a long token-character run
   at the end of the text, to settle the `hideUnfinished` question above.
4. Record heap use while following for an hour on a synthetic stream.

Record the numbers in this file when they exist, replacing the derivations that
they confirm or refute.
