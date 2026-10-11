import { test } from "node:test";
import assert from "node:assert/strict";
import { SseParser } from "../src/live/sse.ts";

test("events, however the bytes are cut", () => {
  const wire = ": keepalive\n\nid: 5\nevent: record\ndata: {\"seq\":5}\n\nevent: delta\ndata: {\"turn\":1}\n\n";
  const whole = new SseParser().push(wire);
  assert.deepEqual(whole, [
    { event: "record", data: '{"seq":5}', id: "5" },
    { event: "delta", data: '{"turn":1}', id: undefined },
  ]);
  for (let step = 1; step < 9; step++) {
    const p = new SseParser();
    const got = [];
    for (let i = 0; i < wire.length; i += step) got.push(...p.push(wire.slice(i, i + step)));
    assert.deepEqual(got, whole, `chunks of ${step}`);
  }
});

test("a delta's missing id is not filled from the event before it", () => {
  const got = new SseParser().push("id: 9\nevent: record\ndata: x\n\nevent: delta\ndata: y\n\n");
  assert.equal(got[1].id, undefined);
});

test("CRLF, multi-line data, a comment-only frame, and a frame cut off", () => {
  const p = new SseParser();
  assert.deepEqual(p.push("event: a\r\ndata: one\r\ndata: two\r\n\r\n: ka\r\n\r\n"), [{ event: "a", data: "one\ntwo", id: undefined }]);
  assert.deepEqual(p.push("id: 3\ndata: half"), [], "a cut-off frame is not delivered");
});
