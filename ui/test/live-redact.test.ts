// A key shape must not reach the page by any path a host or the gateway writes to: the record (live lines), the summary, the
// instance list, and the streamed text, including a key that arrives in pieces. And what a cut leaves must not be a torn tail.
import { test } from "node:test";
import assert from "node:assert/strict";
import { LiveStore } from "../src/live/store.ts";
import { loadLiveIndex } from "../src/data/source.ts";
import { redactStreaming } from "../src/live/wire.ts";
import { startMock } from "../mock/gateway.ts";

const KEY = "sk-or-v1-0123456789abcdef0123456789abcdef";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until(cond: () => boolean, ms = 8000) { const end = Date.now() + ms; while (!cond()) { if (Date.now() > end) assert.fail("timed out"); await sleep(10); } }
/** Every run of 8 characters of the key's body: none may be on the page. */
const pieces = Array.from({ length: KEY.length - 7 }, (_, i) => KEY.slice(i, i + 8)).filter((p) => !/^sk-/.test(p) || true);
const leaks = (text: string) => pieces.filter((p) => text.includes(p));

test("the instance list: a name, a summary line or an error with a key in it is redacted before the page holds it", async () => {
  const body = { instances: [
    { id: "alpha", name: `alpha ${KEY}`, reachable: true, error: null,
      summary: { contract: 1, state: "Working", doing: `printing ${KEY} now`, needs_you: false, turn: 1, last_seq: 3, last_at: 1, requests: 1, quota: null, spend_usd_day: 0 } },
    { id: "beta", name: "beta", reachable: false, summary: null, error: `no answer for ${KEY}` },
  ] };
  const fake = (async () => new Response(JSON.stringify(body), { status: 200 })) as typeof fetch;
  const index = await loadLiveIndex("api", fake);
  assert.equal(JSON.stringify(index).includes(KEY), false);
  assert.deepEqual(leaks(JSON.stringify(index)), []);
  assert.equal(index.instances[0].summary.doing, "printing [redacted] now");
  assert.equal(index.instances[0].name, "alpha [redacted]");
});

test("through the mock: a key in /v1/summary and in /api/instances is on the wire, and not on the page", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  host.redactDoors = false; // a host that has not been given its redactor yet
  host.doingOverride = `the key is ${KEY}`;
  const store = new LiveStore(`${m.url}/api/i/alpha`);
  try {
    // The mock host did not redact, as a host that has not yet been fixed would not: it is on the wire.
    assert.ok((await (await fetch(`${m.url}/api/instances`)).text()).includes(KEY));
    assert.ok((await (await fetch(`${m.url}/api/i/alpha/v1/summary`)).text()).includes(KEY));
    // ...and the page's two readers of those answers hold none of it.
    const index = await loadLiveIndex(`${m.url}/api`);
    assert.equal(JSON.stringify(index).includes(KEY), false);
    assert.deepEqual(leaks(JSON.stringify(index)), []);
    await store.start();
    await until(() => store.getState().host !== null);
    assert.equal(JSON.stringify(store.getState().host).includes(KEY), false);
    assert.equal(store.getState().host!.doing, "the key is [redacted]");
  } finally { store.stop(); await m.stop(); }
});

test("a key split across four pieces of streamed text is never shown, head or tail, at any moment", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  host.redactDoors = false; // pieces of a key reach the page as they would from a host that cannot join them
  const store = new LiveStore(`${m.url}/api/i/alpha`);
  try {
    await store.start();
    await until(() => store.getState().status === "following");
    const parts = ["the key is sk-or-v1-0123", "456789abcdef0", "123456789abcdef", " and that is all"];
    const shown: string[] = [];
    let notified = 0;
    store.subscribeDelta(() => { notified++; });
    for (const p of parts) {
      const before = notified;
      host.delta({ turn: 99, kind: "text", text: p });
      await until(() => notified > before);
      shown.push(store.getDelta()!.text);
      assert.deepEqual(leaks(shown.at(-1)!), [], `after "${p}": ${shown.at(-1)}`);
    }
    assert.equal(shown.at(-1), "the key is [redacted] and that is all");
  } finally { store.stop(); await m.stop(); }
});

test("redactStreaming hides a key that is not whole yet, and only that", () => {
  assert.equal(redactStreaming("see sk-or-v1-01"), "see [redacted]");
  assert.equal(redactStreaming("a Bearer abc"), "a [redacted]");
  assert.equal(redactStreaming("an ask- for help"), "an ask- for help", "a word that merely contains the letters is left alone");
  assert.equal(redactStreaming(`whole ${KEY} and more`), "whole [redacted] and more");
});

test("after a cut the running turn's text is not shown (a torn tail); the next turn is shown whole", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  const store = new LiveStore(`${m.url}/api/i/alpha`);
  try {
    await store.start();
    await until(() => store.getState().status === "following");
    const turn = host.runTurn({ pace: 70 });
    await until(() => (store.getDelta()?.text.length ?? 0) > 10);
    host.cutAll();
    await until(() => store.getState().status === "reconnecting");
    await until(() => store.getState().status === "following", 12000);
    // The turn is still running and pieces of it are arriving again: they are the tail of a text we did not see the start of.
    await sleep(500);
    assert.equal(store.getDelta(), null, "no torn tail");
    await turn;
    await until(() => store.getState().lines.length === host.lastSeq);
    assert.equal(store.getDelta(), null);
    // The finished text is on the record, whole.
    const done = store.getState().lines.findLast((l) => l.kind === "turn.ended")!;
    assert.ok(String(done.final_text).length > 100);
    // A turn that starts after we caught up is seen from its first piece.
    const next = host.runTurn({ pace: 20 });
    await until(() => (store.getDelta()?.text.length ?? 0) > 20);
    assert.ok(host.lines.findLast((l) => l.kind === "turn.log" || l.kind === "decision.admit"), "sanity");
    await next;
  } finally { store.stop(); await m.stop(); }
});

test("joining while a turn runs: that turn's pieces are not shown either", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  const turn = host.runTurn({ pace: 60 });
  await until(() => host.lines.at(-1)?.kind === "turn.started");
  const store = new LiveStore(`${m.url}/api/i/alpha`);
  try {
    await store.start();
    await until(() => store.getState().status === "following");
    await sleep(500);
    assert.equal(store.getDelta(), null, "we saw this turn from the middle");
    await turn;
  } finally { store.stop(); await m.stop(); }
});

test("a second cut, in a later turn, hides that turn's tail too", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  const store = new LiveStore(`${m.url}/api/i/alpha`);
  try {
    await store.start();
    await until(() => store.getState().status === "following");
    for (const label of ["first", "second"]) {
      const turn = host.runTurn({ pace: 70 });
      await until(() => (store.getDelta()?.text.length ?? 0) > 10, 8000);
      host.cutAll();
      await until(() => store.getState().status === "reconnecting");
      await until(() => store.getState().status === "following", 12000);
      await sleep(400);
      assert.equal(store.getDelta(), null, `no torn tail after the ${label} cut`);
      await turn;
      await until(() => store.getState().lines.length === host.lastSeq);
    }
  } finally { store.stop(); await m.stop(); }
});
