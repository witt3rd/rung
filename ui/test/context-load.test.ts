import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { loadContext, contextUrl } from "../src/context/load.ts";
import { parseRecord } from "../src/record/parse.ts";
import { parseHash, href } from "../src/route.ts";

const FIX = join(import.meta.dirname, "fixtures/context/ctx-demo");
const body = readFileSync(join(FIX, "4-2.json"), "utf8");
const lines = parseRecord(readFileSync(join(FIX, "record.ndjson"), "utf8"));
const answer = (status: number, text = body) => async () => new Response(text, { status, headers: { "content-type": "application/json" } });

test("the door's address, live and recorded", () => {
  assert.equal(contextUrl("live", "ctx-demo", 4, 2), "api/i/ctx-demo/v1/turns/4/context?call=2");
  assert.equal(contextUrl("static", "ctx-demo", 4, 2), "data/ctx-demo/context/4-2.json");
  assert.equal(contextUrl("live", "a b", 4, 2), "api/i/a%20b/v1/turns/4/context?call=2");
});

test("the host's door answers: its layers are shown, whole", async () => {
  const v = await loadContext({ mode: "live", id: "ctx-demo", turn: 4, call: 2, lines, fetchImpl: answer(200) as typeof fetch });
  assert.equal(v!.source, "door");
  assert.equal(v!.doorNote, null);
});

test("no door (404 with no error body, as a gateway without it answers): the record's own strata, and the page is told why", async () => {
  const v = await loadContext({ mode: "live", id: "ctx-demo", turn: 4, call: 2, lines, fetchImpl: answer(404, '{"error":"not_found","message":"no such door"}') as typeof fetch });
  assert.equal(v!.source, "record");
  assert.match(v!.doorNote!, /does not serve/i);
});

test("the door's own refusals are said in its words: a turn not yet logged, a call it does not have", async () => {
  const a = await loadContext({ mode: "live", id: "ctx-demo", turn: 4, call: 2, lines, fetchImpl: answer(409, '{"error":"turn_not_logged","message":"turn 4 has not ended: its messages are written when it does"}') as typeof fetch });
  assert.equal(a!.source, "record");
  assert.match(a!.doorNote!, /turn 4 has not ended: its messages are written when it does/);
  const b = await loadContext({ mode: "live", id: "ctx-demo", turn: 4, call: 2, lines, fetchImpl: answer(404, '{"error":"no_such_call","message":"turn 4 has 4 recorded call(s); call 9 is not one of them"}') as typeof fetch });
  assert.match(b!.doorNote!, /call 9 is not one of them/);
});

test("a door that fails (500) or answers nonsense is not hidden: the record is shown with the reason", async () => {
  const a = await loadContext({ mode: "live", id: "ctx-demo", turn: 4, call: 2, lines, fetchImpl: answer(500, '{"error":"internal","message":"the read failed"}') as typeof fetch });
  assert.equal(a!.source, "record"); assert.match(a!.doorNote!, /500/); assert.match(a!.doorNote!, /the read failed/);
  const b = await loadContext({ mode: "live", id: "ctx-demo", turn: 4, call: 2, lines, fetchImpl: answer(200, '{"bands":3}') as typeof fetch });
  assert.equal(b!.source, "record"); assert.match(b!.doorNote!, /bands/i);
  const c = await loadContext({ mode: "live", id: "ctx-demo", turn: 4, call: 2, lines, fetchImpl: (async () => { throw new Error("offline"); }) as typeof fetch });
  assert.equal(c!.source, "record"); assert.match(c!.doorNote!, /did not answer/i);
  const d = await loadContext({ mode: "live", id: "ctx-demo", turn: 4, call: 2, lines, fetchImpl: answer(200, '{"turn":4,"call":2,"calls":4,"bands":[') as typeof fetch });
  assert.equal(d!.source, "record"); assert.match(d!.doorNote!, /could not be read/i);
});

test("an answer for another turn or call than the one asked is refused, not drawn", async () => {
  const v = await loadContext({ mode: "live", id: "ctx-demo", turn: 4, call: 1, lines, fetchImpl: answer(200) as typeof fetch });
  assert.equal(v!.source, "record");
  assert.match(v!.doorNote!, /another call/i);
});

test("no such turn: nothing to show", async () => {
  assert.equal(await loadContext({ mode: "static", id: "ctx-demo", turn: 999, call: 1, lines, fetchImpl: answer(404, "") as typeof fetch }), null);
});

test("the route: a turn's context page, with the call in the address", () => {
  assert.deepEqual(parseHash("#/i/ctx-demo/turns/3/context"), { page: "context", id: "ctx-demo", turn: 3, call: null });
  assert.deepEqual(parseHash("#/i/ctx-demo/turns/3/context?call=2"), { page: "context", id: "ctx-demo", turn: 3, call: 2 });
  assert.deepEqual(parseHash("#/i/ctx-demo/turns/x/context"), { page: "turns", id: "ctx-demo", turn: null });
  assert.equal(href.context("ctx-demo", 3), "#/i/ctx-demo/turns/3/context");
  assert.equal(href.context("ctx-demo", 3, 2), "#/i/ctx-demo/turns/3/context?call=2");
  // the turns page still reads its own address
  assert.deepEqual(parseHash("#/i/ctx-demo/turns?turn=3"), { page: "turns", id: "ctx-demo", turn: 3 });
});
