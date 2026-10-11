import { test } from "node:test";
import assert from "node:assert/strict";
import { parseRecordLines } from "../src/live/wire.ts";
import { LiveStore } from "../src/live/store.ts";

const L = (seq: number) => ({ seq, at: seq, kind: "x" });

test("a record with a gap in its numbers is refused, not shown", () => {
  assert.equal(parseRecordLines({ lines: [L(1), L(2), L(3)] }).length, 3);
  assert.throws(() => parseRecordLines({ lines: [L(1), L(2), L(4)] }), /gap between line 2 and 4/);
});

test("a store that is given a record with a gap says so and shows nothing of it", async () => {
  const f = (async () => new Response(JSON.stringify({ lines: [L(1), L(3)], offset: 0, limit: 10, total: 2, next: null }), { status: 200 })) as typeof fetch;
  const store = new LiveStore("api/i/x", f);
  await store.start();
  store.stop();
  assert.match(store.getState().error ?? "", /gap/);
  assert.deepEqual(store.getState().lines, []);
});
