import { test } from "node:test";
import assert from "node:assert/strict";
import { EventsClient, type Delta, type Status } from "../src/live/client.ts";
import { startMock } from "../mock/gateway.ts";
import { frame } from "../mock/host.ts";
import type { Line } from "../src/record/types.ts";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until(cond: () => boolean, ms = 5000) { const end = Date.now() + ms; while (!cond()) { if (Date.now() > end) assert.fail("timed out"); await sleep(10); } }

/** A fetch that answers from a script: each call gets the next body (frames to send, then the stream ends) or a status. */
function scripted(steps: (string[] | number)[], seenAfter: string[] = []): typeof fetch {
  let i = 0;
  return (async (_url: string, init?: RequestInit) => {
    seenAfter.push((init?.headers as Record<string, string>)["last-event-id"]);
    const step = steps[Math.min(i++, steps.length - 1)];
    if (typeof step === "number") return new Response("{}", { status: step });
    const enc = new TextEncoder();
    return new Response(new ReadableStream({ start(c) { for (const f of step) c.enqueue(enc.encode(f)); c.close(); } }), { status: 200, headers: { "content-type": "text/event-stream" } });
  }) as typeof fetch;
}
const rec = (seq: number) => frame("record", { seq, at: seq, kind: "x" }, seq);

test("against the mock: cut by the host, cut by the client, and a host that was written to while nobody listened", async () => {
  const m = await startMock();
  try {
    const host = m.hosts.get("alpha")!;
    const got: number[] = [];
    const statuses: Status[] = [];
    const c = new EventsClient({
      url: (a) => `${m.url}/api/i/alpha/v1/events?after=${a}`, after: host.lastSeq,
      onRecord: (l) => got.push(l.seq), onStatus: (s) => statuses.push(s), backoffMs: 20,
    });
    const from = host.lastSeq;
    c.start();
    await until(() => statuses.includes("following"));
    await host.poke(3);
    await until(() => got.length >= 3);
    host.cutAll(); // the host side drops us
    await host.poke(3);
    await until(() => got.length >= 6);
    c.cut(); // our side drops
    await host.poke(2);
    await until(() => got.length >= 8);
    c.stop();
    assert.deepEqual(got, Array.from({ length: 8 }, (_, i) => from + 1 + i), "every line once, in order");
    assert.ok(statuses.includes("reconnecting"));
    assert.equal(statuses.at(-1) === "reconnecting" || statuses.at(-1) === "following", true);
  } finally { await m.stop(); }
});

test("a line seen twice is applied once", async () => {
  const got: number[] = [];
  const c = new EventsClient({ url: () => "x", after: 0, onRecord: (l) => got.push(l.seq), backoffMs: 5,
    fetchImpl: scripted([[rec(1), rec(2), rec(2), rec(1), rec(3), frame("caught_up", { last_seq: 3 })]]) });
  c.start(); await until(() => got.length >= 3); await sleep(30); c.stop();
  assert.deepEqual(got.slice(0, 3), [1, 2, 3]);
});

test("a skipped number is a broken stream: nothing after the gap is applied, and the client resumes from its last", async () => {
  const got: number[] = [];
  const asked: string[] = [];
  const c = new EventsClient({ url: () => "x", after: 0, onRecord: (l) => got.push(l.seq), backoffMs: 5,
    fetchImpl: scripted([[rec(1), rec(2), rec(4), rec(5)], [rec(3), rec(4), rec(5), frame("caught_up", { last_seq: 5 })]], asked) });
  c.start(); await until(() => got.length >= 5); c.stop();
  assert.deepEqual(got, [1, 2, 3, 4, 5]);
  assert.deepEqual(asked.slice(0, 2), ["0", "2"]);
});

test("a host that does not know our number: the record must be loaded again", async () => {
  let reset = 0;
  const c = new EventsClient({ url: () => "x", after: 99, onRecord: () => {}, onReset: () => { reset++; }, fetchImpl: scripted([400]) });
  c.start(); await until(() => reset === 1); await sleep(30); c.stop();
  assert.equal(reset, 1);
});

test("failures back off and then follow; deltas pass through and a cut stream's half frame is dropped", async () => {
  const statuses: Status[] = [];
  const deltas: Delta[] = [];
  const got: number[] = [];
  const c = new EventsClient({ url: () => "x", after: 0, onRecord: (l: Line) => got.push(l.seq), onDelta: (d) => deltas.push(d), onStatus: (s) => statuses.push(s), backoffMs: 5,
    fetchImpl: scripted([503, ["id: 1\nevent: record\ndata: {\"seq\":1,\"at\":1,\"ki"], [rec(1), frame("delta", { turn: 1, kind: "text", text: "hi " }), frame("caught_up", { last_seq: 1 })]]) });
  c.start(); await until(() => statuses.includes("following")); c.stop();
  assert.deepEqual(got, [1], "the half frame was not applied, the whole one was");
  assert.ok(deltas.length >= 1, "a delta passed through");
  for (const d of deltas) assert.deepEqual(d, { turn: 1, kind: "text", text: "hi " });
  assert.ok(statuses.filter((s) => s === "reconnecting").length >= 2);
});
