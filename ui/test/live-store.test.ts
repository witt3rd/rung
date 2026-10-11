import { test } from "node:test";
import assert from "node:assert/strict";
import { LiveStore } from "../src/live/store.ts";
import { overlayRunning } from "../src/live/overlay.ts";
import { foldTurns } from "../src/record/folds.ts";
import { startMock } from "../mock/gateway.ts";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until(cond: () => boolean, ms = 5000) { const end = Date.now() + ms; while (!cond()) { if (Date.now() > end) assert.fail("timed out"); await sleep(10); } }

test("the store loads the record, follows it, shows deltas while a turn runs and drops them at its end", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  const store = new LiveStore(`${m.url}/api/i/alpha`);
  try {
    await store.start();
    assert.equal(store.getState().loaded, true);
    assert.equal(store.getState().lines.length, host.lastSeq);
    await until(() => store.getState().status === "following");
    await until(() => store.getState().host !== null);
    const before = store.getState().lines.length;
    const turn = host.runTurn({ pace: 20 });
    // While it runs: the record has its start, the page has the text so far and the tool in flight.
    await until(() => (store.getDelta()?.text.length ?? 0) > 20);
    await until(() => foldTurns(store.getState().lines, { live: true }).at(-1)!.status === "running");
    const running = foldTurns(store.getState().lines, { live: true }).at(-1)!;
    assert.equal(running.status, "running");
    const shown = overlayRunning(foldTurns(store.getState().lines, { live: true }), store.getDelta()).at(-1)!;
    assert.ok(shown.text.length > 0 && shown.text === store.getDelta()!.text, "the streamed text is what the turn shows");
    await turn;
    await until(() => store.getState().lines.length === host.lastSeq);
    assert.equal(store.getDelta(), null, "the finished text is on the record; the stream's pieces are gone");
    const done = foldTurns(store.getState().lines, { live: true }).at(-1)!;
    assert.equal(done.status, "completed");
    assert.ok(done.text.length > 100);
    assert.ok(store.getState().lines.length > before);
    // The host's summary follows the record.
    await until(() => store.getState().host?.last_seq === host.lastSeq);
  } finally { store.stop(); await m.stop(); }
});

test("a cut mid-turn: the page shows no torn text, catches up, and holds the same record as the host", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  const store = new LiveStore(`${m.url}/api/i/alpha`);
  try {
    await store.start();
    await until(() => store.getState().status === "following");
    const turn = host.runTurn({ pace: 15 });
    await until(() => (store.getDelta()?.text.length ?? 0) > 10);
    host.cutAll();
    await until(() => store.getDelta() === null || store.getState().status === "reconnecting");
    await turn;
    await until(() => store.getState().lines.length === host.lastSeq && store.getState().status === "following", 8000);
    assert.deepEqual(store.getState().lines.map((l) => l.seq), host.lines.map((l) => l.seq), "nothing missing, nothing twice");
    assert.deepEqual(store.getState().lines, JSON.parse(JSON.stringify(host.lines)), "line for line");
  } finally { store.stop(); await m.stop(); }
});

test("the overlay puts the streamed text and the tool in flight on the running turn only", () => {
  const lines = [
    { seq: 1, at: 1, kind: "turn.started", turn: 1, turn_kind: "free", epoch: 1, pack_tokens: 1 },
    { seq: 2, at: 2, kind: "turn.ended", turn: 1, status: "completed", final_text: "done", cost: {} },
    { seq: 3, at: 3, kind: "turn.started", turn: 2, turn_kind: "free", epoch: 1, pack_tokens: 1 },
  ];
  const turns = foldTurns(lines, { live: true });
  const shown = overlayRunning(turns, { turn: 2, text: "so far", tool: "ws_write" });
  assert.equal(shown[1].text, "so far");
  assert.deepEqual(shown[1].calls.map((c) => [c.name, c.ok]), [["ws_write", null]], "a tool in flight has no result yet");
  assert.equal(shown[0].text, "done", "a finished turn is as recorded");
  assert.equal(overlayRunning(turns, { turn: 1, text: "stale", tool: null })[0].text, "done", "a delta for a turn that is not running is ignored");
  assert.equal(overlayRunning(turns, null), turns);
});

test("a key shape in a live line or a delta never reaches what the page holds", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  const store = new LiveStore(`${m.url}/api/i/alpha`);
  const KEY = "sk-or-v1-0123456789abcdef0123456789abcdef";
  try {
    await store.start();
    await until(() => store.getState().status === "following");
    host.delta({ turn: 99, kind: "text", text: `the key is ${KEY} ok` });
    host.append("note.written", { turn: 99, text: `printed ${KEY}` });
    await until(() => store.getState().lines.at(-1)?.kind === "note.written");
    await until(() => store.getDelta() !== null);
    assert.equal(JSON.stringify(store.getState().lines).includes(KEY), false, "not in a live line");
    assert.equal(store.getState().lines.at(-1)!.text, "printed [redacted]");
    assert.equal(store.getDelta()!.text, "the key is [redacted] ok", "not in a delta");
  } finally { store.stop(); await m.stop(); }
});

test("a reset (the host does not know our number) reloads the record and applies no line queued before it", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  const store = new LiveStore(`${m.url}/api/i/alpha`);
  try {
    await store.start();
    await until(() => store.getState().status === "following");
    // Queue lines, then reset before they are flushed: they are on the record the reload reads, and must not be added twice.
    const n = host.lastSeq;
    await host.poke(5);
    (store as unknown as { client: { o: { onReset: () => void } } }).client.o.onReset();
    await until(() => store.getState().loaded && store.getState().lines.length === n + 5 && store.getState().status === "following", 8000);
    await sleep(100);
    assert.deepEqual(store.getState().lines.map((l) => l.seq), Array.from({ length: n + 5 }, (_, i) => i + 1), "each number once, in order");
  } finally { store.stop(); await m.stop(); }
});
