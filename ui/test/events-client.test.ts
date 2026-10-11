import { test } from "node:test";
import assert from "node:assert/strict";
import { EventsClient, type Delta, type End, type Open, type Status } from "../src/live/client.ts";
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
const open = (after = 0, live = true) => frame("open", { contract: 1, after, live, partials: true, redaction: { env_unset: [] } });
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
    await until(() => statuses.includes("catching up"));
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
    assert.ok(statuses.filter((x) => x === "catching up").length >= 3, "the stream opened again after each cut");
  } finally { await m.stop(); }
});

test("a line seen twice is applied once", async () => {
  const got: number[] = [];
  const c = new EventsClient({ url: () => "x", after: 0, onRecord: (l) => got.push(l.seq), backoffMs: 5,
    fetchImpl: scripted([[open(), rec(1), rec(2), rec(2), rec(1), rec(3), frame("end", { reason: "stopped", last_seq: 3, problem: null })]]) });
  c.start(); await until(() => got.length >= 3); await sleep(30); c.stop();
  assert.deepEqual(got.slice(0, 3), [1, 2, 3]);
});

test("a skipped number is a broken stream: nothing after the gap is applied, and the client resumes from its last", async () => {
  const got: number[] = [];
  const asked: string[] = [];
  const c = new EventsClient({ url: () => "x", after: 0, onRecord: (l) => got.push(l.seq), backoffMs: 5,
    fetchImpl: scripted([[open(), rec(1), rec(2), rec(4), rec(5)], [open(2), rec(3), rec(4), rec(5), frame("end", { reason: "stopped", last_seq: 5, problem: null })]], asked) });
  c.start(); await until(() => got.length >= 5); c.stop();
  assert.deepEqual(got, [1, 2, 3, 4, 5]);
  assert.deepEqual(asked.slice(0, 2), ["0", "2"]);
});

test("a cursor from another instance (409 cursor_ahead): the record must be loaded again", async () => {
  let reset = 0;
  const c = new EventsClient({ url: () => "x", after: 99, onRecord: () => {}, onReset: () => { reset++; }, fetchImpl: scripted([409]) });
  c.start(); await until(() => reset === 1); await sleep(30); c.stop();
  assert.equal(reset, 1);
});

test("failures back off and then follow; deltas pass through and a cut stream's half frame is dropped", async () => {
  const statuses: Status[] = [];
  const deltas: Delta[] = [];
  const got: number[] = [];
  const c = new EventsClient({ url: () => "x", after: 0, onRecord: (l: Line) => got.push(l.seq), onDelta: (d) => deltas.push(d), onStatus: (s) => statuses.push(s), backoffMs: 5,
    fetchImpl: scripted([503, ["id: 1\nevent: record\ndata: {\"seq\":1,\"at\":1,\"ki"], [open(), rec(1), frame("delta", { turn: 1, call: 0, kind: "text", text: "hi " })]]) });
  c.start(); await until(() => deltas.length >= 1); c.stop();
  assert.deepEqual(got, [1], "the half frame was not applied, the whole one was");
  assert.ok(deltas.length >= 1, "a delta passed through");
  for (const d of deltas) assert.deepEqual(d, { turn: 1, call: 0, kind: "text", text: "hi " });
  assert.ok(statuses.filter((s) => s === "reconnecting").length >= 2);
});

test("a gateway with no event stream for the instance (404): said once, not retried", async () => {
  const statuses: Status[] = [];
  let calls = 0;
  const f = (async () => { calls++; return new Response("{}", { status: 404 }); }) as typeof fetch;
  const c = new EventsClient({ url: () => "x", after: 3, onRecord: () => {}, onStatus: (s) => statuses.push(s), backoffMs: 5, fetchImpl: f });
  c.start(); await sleep(150); c.stop();
  assert.equal(calls, 1, "no retry loop against a door that is not there");
  assert.equal(statuses.at(-1), "unavailable");
});

test("an end frame: the host stopped. The status says so, nothing is retried at once, and the host is asked again later", async () => {
  const statuses: Status[] = [];
  const ends: End[] = [];
  const opens: Open[] = [];
  let calls = 0;
  const f = (async () => {
    calls++;
    const frames = calls === 1 ? [open(0, false), rec(1), frame("end", { reason: "stopped", last_seq: 1, problem: null })] : [open(1, true), rec(2)];
    const enc = new TextEncoder();
    return new Response(new ReadableStream({ start(c) { for (const x of frames) c.enqueue(enc.encode(x)); c.close(); } }), { status: 200 });
  }) as typeof fetch;
  const got: number[] = [];
  const c = new EventsClient({ url: () => "x", after: 0, onRecord: (l) => got.push(l.seq), onStatus: (x) => statuses.push(x), onEnd: (e) => ends.push(e), onOpen: (o) => opens.push(o), fetchImpl: f, backoffMs: 5, endedRetryMs: 60 });
  c.start();
  await until(() => statuses.includes("ended"));
  assert.equal(calls, 1, "not asked again at once");
  assert.deepEqual(ends, [{ reason: "stopped", last_seq: 1, problem: null }]);
  assert.equal(statuses.filter((x) => x === "reconnecting").length, 0, "an end is not a cut");
  await until(() => got.length >= 2, 3000);
  c.stop();
  assert.deepEqual(got, [1, 2], "when the host is back its new lines follow, once");
  assert.equal(opens[1].live, true);
});

test("a bad request (400) is not a reset: it is retried like any refusal", async () => {
  let resets = 0, calls = 0;
  const f = (async () => { calls++; return new Response("{}", { status: 400 }); }) as typeof fetch;
  const c = new EventsClient({ url: () => "x", after: 0, onRecord: () => {}, onReset: () => { resets++; }, backoffMs: 5, maxBackoffMs: 10, fetchImpl: f });
  c.start(); await sleep(120); c.stop();
  assert.equal(resets, 0);
  assert.ok(calls >= 2, "retried");
});
