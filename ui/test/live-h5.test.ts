// The page follows H5's stream: an open frame, record frames, partial frames of several kinds, an end frame.
import { test } from "node:test";
import assert from "node:assert/strict";
import { LiveStore } from "../src/live/store.ts";
import { startMock } from "../mock/gateway.ts";
import { foldTurns } from "../src/record/folds.ts";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until(cond: () => boolean, ms = 8000) { const end = Date.now() + ms; while (!cond()) { if (Date.now() > end) assert.fail("timed out"); await sleep(10); } }
async function started() {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  const store = new LiveStore(`${m.url}/api/i/alpha`);
  await store.start();
  await until(() => store.getState().status === "following");
  return { m, host, store };
}

test("following means the replay is applied: catching up until then, and the replayed lines are applied once", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  await host.poke(40);
  // The page read the record some time ago: the last 25 lines were written since, and are the stream's replay.
  const stale = (async (url: string, init?: RequestInit) => {
    const r = await fetch(url, init);
    if (!String(url).includes("/v1/record")) return r;
    const j = await r.json();
    j.lines = j.lines.slice(0, j.lines.length - 25);
    return new Response(JSON.stringify(j), { status: 200 });
  }) as typeof fetch;
  const store = new LiveStore(`${m.url}/api/i/alpha`, stale);
  const seen: string[] = [];
  store.subscribe(() => { const s = store.getState().status; if (seen.at(-1) !== s) seen.push(s); });
  try {
    await store.start();
    assert.equal(store.getState().lines.length, host.lastSeq - 25, "the record as the page read it is short of the host's");
    await until(() => store.getState().status === "following");
    assert.ok(seen.indexOf("catching up") >= 0 && seen.indexOf("catching up") < seen.indexOf("following"), `statuses: ${seen.join(" > ")}`);
    assert.equal(store.getState().lines.length, host.lastSeq, "the replay was applied");
    assert.deepEqual(store.getState().lines.map((l) => l.seq), host.lines.map((l) => l.seq), "each line once, in order");
  } finally { store.stop(); await m.stop(); }
});

test("a turn with two model calls and a tool: the text is joined as blocks, the tool shows while it runs and is gone when done", async () => {
  const { m, host, store } = await started();
  try {
    const n = host.runTurn({ pace: 40, tools: ["ws_list"] });
    let sawTool = false;
    store.subscribeDelta(() => { if (store.getDelta()?.tool === "ws_list") sawTool = true; });
    await until(() => (store.getDelta()?.text.length ?? 0) > 5);
    await until(() => sawTool, 8000);
    await until(() => store.getDelta()?.tool === null && sawTool && (store.getDelta()?.text.includes("\n\n") ?? false), 8000);
    const text = store.getDelta()!.text;
    assert.ok(text.includes("\n\n"), "the second call's text is set apart from the first's");
    await n;
    await until(() => store.getDelta() === null, 8000);
  } finally { store.stop(); await m.stop(); }
});

test("a behind frame: this reader missed partial frames, so the rest of that turn's text is not shown", async () => {
  const { m, host, store } = await started();
  try {
    const n = host.runTurn({ pace: 40 });
    await until(() => (store.getDelta()?.text.length ?? 0) > 5);
    await until(() => foldTurns(store.getState().lines, { live: true }).at(-1)!.status === "running");
    host.delta({ turn: 0, call: 0, kind: "behind", skipped: 7 });
    // At once, while the turn is still running: not because the turn happened to end.
    await until(() => store.getDelta() === null, 1000);
    assert.equal(foldTurns(store.getState().lines, { live: true }).at(-1)!.status, "running", "the turn is still running");
    await sleep(400);
    assert.equal(store.getDelta(), null, "nothing more of that turn is shown");
    assert.equal(foldTurns(store.getState().lines, { live: true }).at(-1)!.status, "running", "and it still is");
    await n;
    await until(() => store.getState().lines.length === host.lastSeq);
    // The next turn is whole again.
    const next = host.runTurn({ pace: 20 });
    await until(() => (store.getDelta()?.text.length ?? 0) > 10);
    await next;
  } finally { store.stop(); await m.stop(); }
});

test("an end frame: the status says the instance is not running, and the state word comes from the host's summary", async () => {
  const { m, host, store } = await started();
  try {
    await host.halt();
    await until(() => store.getState().status === "ended");
    await until(() => store.getState().host?.state === "Stopped");
    assert.equal(store.getDelta(), null);
    assert.equal(store.getState().lines.at(-1)!.kind, "halted", "the lines up to and including halted were applied");
  } finally { store.stop(); await m.stop(); }
});

test("a stream with no partial frames (a host the gateway cannot reach): the record is followed and nothing streams", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  const store = new LiveStore(`${m.url}/api/i/alpha`, ((url: string, init?: RequestInit) => fetch(String(url).replace("/v1/events?", "/v1/events?partials=off&"), init)) as typeof fetch);
  try {
    await store.start();
    await until(() => store.getState().status === "following");
    const turn = host.runTurn({ pace: 20 });
    await sleep(300);
    assert.equal(store.getDelta(), null, "no partial frame was sent, so none is shown");
    await turn;
    await until(() => store.getState().lines.length === host.lastSeq);
  } finally { store.stop(); await m.stop(); }
});

test("answer text blocks are set apart by a blank line, exactly; one block alone is not", async () => {
  const { m, host, store } = await started();
  try {
    const f = (call: number, rest: Record<string, unknown>) => ({ turn: 77, call, ...rest });
    host.delta(f(0, { kind: "block_start", id: null }));
    host.delta(f(0, { kind: "text", text: "first " }));
    host.delta(f(0, { kind: "text", text: "block" }));
    host.delta(f(0, { kind: "block_end", id: null }));
    await until(() => store.getDelta()?.text === "first block");
    host.delta(f(1, { kind: "block_start", id: null }));
    host.delta(f(1, { kind: "text", text: "second" }));
    await until(() => store.getDelta()?.text === "first block\n\nsecond");
  } finally { store.stop(); await m.stop(); }
});

test("until the replay is applied no partial frame is shown, and the turn running when the page joined is never shown streaming", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  const turn = host.runTurn({ pace: 60 }); // running when the page joins
  await until(() => host.lines.at(-1)?.kind === "turn.started");
  // The summary (where the record stood when the stream opened) is slow to answer: for that long the page cannot tell what was running.
  const slow = (async (url: string, init?: RequestInit) => {
    if (String(url).includes("/v1/summary")) await sleep(500);
    return fetch(url, init);
  }) as typeof fetch;
  const store = new LiveStore(`${m.url}/api/i/alpha`, slow);
  try {
    await store.start();
    await until(() => store.getState().status === "catching up");
    host.delta({ turn: 5000, call: 0, kind: "text", text: "arrives early" });
    await sleep(150);
    assert.equal(store.getDelta(), null, "not shown before the replay is applied");
    await turn;
    await until(() => store.getState().status === "following");
    host.delta({ turn: 5001, call: 0, kind: "text", text: "arrives after" });
    await until(() => store.getDelta()?.text === "arrives after");
  } finally { store.stop(); await m.stop(); }
});
