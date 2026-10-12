// Sending: one press is one write; a repeat of the same press adds once; the page waits for the host's answer (S1, page side).
import { test } from "node:test";
import assert from "node:assert/strict";
import { Sender, refusalWords } from "../src/queue/send.ts";

interface Call { url: string; method: string; body: { key: string; text: string; intent: string }; headers: Record<string, string> }
function fake(answers: (Response | Error | Promise<Response>)[]) {
  const calls: Call[] = [];
  const fetchImpl = (async (url: string, init: RequestInit) => {
    calls.push({ url, method: init.method ?? "GET", body: JSON.parse(String(init.body)), headers: Object.fromEntries(Object.entries((init.headers ?? {}) as Record<string, string>)) });
    const a = answers.shift();
    if (a === undefined) throw new Error("no answer scripted");
    if (a instanceof Error) throw a;
    return a;
  }) as unknown as typeof fetch;
  return { calls, fetchImpl };
}
const json = (status: number, body: unknown) => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
let n = 0;
const sender = (f: ReturnType<typeof fake>) => new Sender({ url: "api/i/x/v1/queue", fetchImpl: f.fetchImpl, newKey: () => `key-${++n}` });

test("a send writes once, with the text whole, the intent and a key, to the queue door", async () => {
  const f = fake([json(201, { id: "key-1", seq: 40, repeat: false })]);
  const text = "  keep the spaces\nand the line break, and a very long tail ".repeat(200);
  const r = await sender(f).send(text, "next");
  assert.deepEqual(r, { ok: true, id: "key-1", repeat: false });
  assert.equal(f.calls.length, 1);
  assert.equal(f.calls[0].url, "api/i/x/v1/queue");
  assert.equal(f.calls[0].method, "POST");
  assert.equal(f.calls[0].body.text, text, "the text goes exactly as typed");
  assert.equal(f.calls[0].body.intent, "next");
  assert.equal(typeof f.calls[0].body.key, "string");
  assert.equal(f.calls[0].headers["content-type"], "application/json");
});

test("a second press while the first is in flight is the same write, not a second one", async () => {
  let release!: (r: Response) => void;
  const f = fake([new Promise<Response>((ok) => { release = ok; })]);
  const s = sender(f);
  const a = s.send("do it", "next");
  const b = s.send("do it", "next");
  assert.equal(f.calls.length, 1);
  release(json(201, { id: "key-x", seq: 1, repeat: false }));
  assert.deepEqual(await a, await b);
});

test("a repeat after a lost answer carries the same key; edited text or a new intent is a new press", async () => {
  const f = fake([new Error("network"), json(200, { id: "k", seq: 5, repeat: true }), json(201, { id: "k2", seq: 6, repeat: false }), json(201, { id: "k3", seq: 7, repeat: false })]);
  const s = sender(f);
  const first = await s.send("same", "next");
  assert.deepEqual(first, { ok: false, kind: "unreachable", message: refusalWords("unreachable", "") });
  const second = await s.send("same", "next");
  assert.equal(f.calls[1].body.key, f.calls[0].body.key, "the repeat is the same press");
  assert.deepEqual(second, { ok: true, id: "k", repeat: true });
  await s.send("same", "next");
  assert.notEqual(f.calls[2].body.key, f.calls[0].body.key, "after it was written, the next press is a new one");
  const f2 = fake([new Error("network"), json(201, { id: "k", seq: 1, repeat: false })]);
  const s2 = sender(f2);
  await s2.send("one", "next");
  await s2.send("two", "next");
  assert.notEqual(f2.calls[1].body.key, f2.calls[0].body.key, "other text is another message");
});

test("an empty text is not sent", async () => {
  const f = fake([]);
  const r = await sender(f).send("  \n ", "next");
  assert.equal(r.ok, false);
  assert.equal(f.calls.length, 0);
});

test("an instance that is not running is refused with 'Start the instance first.' (409 not_running)", async () => {
  const f = fake([json(409, { error: "not_running", message: "the instance is not running: start it first (the gateway has no start door); nothing was sent" })]);
  const r = await sender(f).send("hello", "next");
  assert.deepEqual(r, { ok: false, kind: "not_running", message: "Start the instance first." });
});

test("a key that names another message (409 key_reused) reads as already added, and the next press is a new one", async () => {
  const f = fake([json(409, { error: "key_reused", message: "the key already names a different message" }), json(201, { id: "k2", repeat: false, item: {} })]);
  const s = sender(f);
  const r = await s.send("hello", "next");
  assert.deepEqual(r, { ok: false, kind: "key_reused", message: "That press was already added. Nothing new was written." });
  await s.send("hello", "next");
  assert.notEqual(f.calls[1].body.key, f.calls[0].body.key, "the same text again is a new press with a new key");
});

test("a host that did not answer in time keeps the press: its own words, and the repeat carries the same key", async () => {
  const f = fake([json(503, { error: "instance_not_answering", message: "the message may or may not be queued; look at the queue, or send it again with the same key (a repeat adds once)" }), json(200, { id: "k", repeat: true, item: {} })]);
  const s = sender(f);
  const r = await s.send("hello", "next");
  assert.equal(r.ok, false);
  if (!r.ok) { assert.equal(r.kind, "unreachable"); assert.match(r.message, /may or may not be queued/); }
  const again = await s.send("hello", "next");
  assert.deepEqual(again, { ok: true, id: "k", repeat: true });
  assert.equal(f.calls[1].body.key, f.calls[0].body.key);
});

test("a gateway with no key for the instance (503 no_owner_key) is a refusal with its words, not a retry", async () => {
  const f = fake([json(503, { error: "no_owner_key", message: "name it in the gateway's own config (instances[].key_env)" }), json(201, { id: "k2", repeat: false, item: {} })]);
  const s = sender(f);
  const r = await s.send("hello", "next");
  assert.equal(r.ok, false);
  if (!r.ok) { assert.equal(r.kind, "refused"); assert.match(r.message, /key_env/); }
  await s.send("hello", "next");
  assert.notEqual(f.calls[1].body.key, f.calls[0].body.key);
});

test("a message of nothing visible (a zero-width space) is empty and is not sent", async () => {
  const f = fake([]);
  for (const t of ["\u200b", " \u200b\ufeff \u2060"]) { const r = await sender(f).send(t, "next"); assert.equal(r.ok === false && r.kind, "empty"); }
  assert.equal(f.calls.length, 0);
});

test("the other refusals read in plain words and keep the host's own reason when it gives one", async () => {
  const cases: [number, unknown, string, RegExp][] = [
    [403, { error: "read_only", message: "this token can only read" }, "read_only", /only read/i],
    [404, { error: "not_found", message: "no such door" }, "not_there", /cannot take/i],
    [400, { error: "bad_request", message: "intent must be next, free or steer" }, "refused", /intent must be next, free or steer/],
    [409, { error: "key_in_use", message: "that key holds other text" }, "refused", /other text/],
    [502, { error: "instance_unreachable", message: "the instance could not be reached: nothing was queued" }, "unreachable", /could not be reached/i],
  ];
  for (const [status, body, kind, words] of cases) {
    const r = await sender(fake([json(status, body)])).send("x", "next");
    assert.equal(r.ok, false);
    if (!r.ok) { assert.equal(r.kind, kind, String(status)); assert.match(r.message, words, String(status)); }
  }
});

test("a reason from the host that holds a key is redacted before it is shown", async () => {
  const r = await sender(fake([json(400, { error: "bad_request", message: "bad text sk-or-v1-0123456789abcdef0123456789abcdef" })])).send("x", "next");
  assert.equal(r.ok, false);
  if (!r.ok) assert.ok(!r.message.includes("0123456789abcdef"));
});

test("an answer that is not JSON is a refusal with words, not a crash", async () => {
  const r = await sender(fake([new Response("<html>bad gateway</html>", { status: 502 })])).send("x", "next");
  assert.equal(r.ok, false);
});
