import { test } from "node:test";
import assert from "node:assert/strict";
import { markBands, cachedLine } from "../src/context/cache.ts";
import { WIRE_ORDER, type BandId } from "../src/context/types.ts";

const est = (o: Partial<Record<BandId, number | null>>) => WIRE_ORDER.map((id) => ({ id, estimate: o[id] === undefined ? null : o[id]! }));
// The design note's example: bands in wire order, the provider counted 41,880, of which 39,400 were cached.
const NOTE = est({ tools: 2900, system: 2400, epoch: 9100, history: 28300, recall: 1100, message: 90, this_turn: 400 });

test("the note's example: the stable layers and the epoch are cached, history is partly, the rest is new", () => {
  const m = markBands(NOTE, { prompt: 41880, cached: 39400, written: 0 });
  assert.deepEqual(m, ["cached", "cached", "cached", "partly", "new", "new", "new"]);
});

test("nothing cached: every band is new; everything cached: every band is cached", () => {
  assert.deepEqual(markBands(NOTE, { prompt: 41880, cached: 0, written: 0 }), Array(7).fill("new"));
  assert.deepEqual(markBands(NOTE, { prompt: 41880, cached: 41880, written: 0 }), Array(7).fill("cached"));
});

test("the estimates are scaled to the provider's count, so a band's place follows the truth, not the estimate", () => {
  // estimates add up to 100, the provider counted 200: the cut at 120 provider tokens is at 60 estimated, inside the second band
  const m = markBands([{ id: "tools", estimate: 50 }, { id: "system", estimate: 50 }], { prompt: 200, cached: 120, written: 0 });
  assert.deepEqual(m, ["cached", "partly"]);
});

test("a band the host did not record has no size: only the two certain cases are marked, the rest say they are not known", () => {
  const gap = est({ tools: null, system: null, epoch: 81, history: null, recall: null, message: null, this_turn: null });
  assert.deepEqual(markBands(gap, { prompt: 3000, cached: 0, written: 0 }), Array(7).fill("new"));
  assert.deepEqual(markBands(gap, { prompt: 3000, cached: 3000, written: 0 }), Array(7).fill("cached"));
  assert.deepEqual(markBands(gap, { prompt: 3000, cached: 2055, written: 0 }), Array(7).fill("unknown"));
});

test("no provider count yet (a call not answered): every band is unknown", () => {
  assert.deepEqual(markBands(NOTE, null), Array(7).fill("unknown"));
});

test("a band with nothing in it is marked empty, wherever the cut falls", () => {
  const m = markBands(est({ tools: 100, system: 100, epoch: 0, history: 100, recall: 0, message: 10, this_turn: 0 }), { prompt: 310, cached: 200, written: 0 });
  assert.deepEqual(m, ["cached", "cached", "empty", "new", "new", "empty", "empty"]);
});

test("the bottom line: cached N of M provider tokens and the percent, or nothing when there is no count", () => {
  assert.deepEqual(cachedLine({ prompt: 41880, cached: 39400, written: 0 }), { cached: 39400, prompt: 41880, pct: 94 });
  assert.deepEqual(cachedLine({ prompt: 0, cached: 0, written: 0 }), { cached: 0, prompt: 0, pct: null });
  assert.equal(cachedLine(null), null);
});

test("a band known to be empty is marked empty even when the other sizes are not known", () => {
  const m = markBands(est({ tools: null, system: null, epoch: 81, history: null, recall: 0, message: null, this_turn: 0 }), { prompt: 3000, cached: 2055, written: 0 });
  assert.deepEqual(m, ["unknown", "unknown", "unknown", "unknown", "unknown", "empty", "empty"]);
});
