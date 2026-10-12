// The record is read by following `next`, in pages: never by one request with a limit big enough to hope for.
import { test } from "node:test";
import assert from "node:assert/strict";
import { LiveStore } from "../src/live/store.ts";
import { startMock } from "../mock/gateway.ts";

test("the record is read by following next, in pages: no request carries a limit big enough to hope for, and the record is the host's", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  await host.poke(2500);
  const asked: string[] = [];
  const spy = (async (url: string, init?: RequestInit) => { if (String(url).includes("/v1/record")) asked.push(String(url)); return fetch(url, init); }) as typeof fetch;
  const store = new LiveStore(`${m.url}/api/i/alpha`, spy);
  try {
    await store.start();
    assert.ok(asked.length >= 2, `more than one page was read (${asked.length})`);
    for (const u of asked) assert.ok(!/limit=\d{6,}/.test(u), `a page, not a hope: ${u}`);
    assert.deepEqual(store.getState().lines.map((l) => l.seq), host.lines.map((l) => l.seq), "every line once, in order");
  } finally { store.stop(); await m.stop(); }
});

test("lines written while the pages are being read are not lost and not repeated", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  await host.poke(2200);
  let pages = 0;
  const growing = (async (url: string, init?: RequestInit) => {
    const r = await fetch(url, init);
    if (String(url).includes("/v1/record") && ++pages === 1) await host.poke(50); // the host writes between the first page and the next
    return r;
  }) as typeof fetch;
  const store = new LiveStore(`${m.url}/api/i/alpha`, growing);
  try {
    await store.start();
    const until = async (c: () => boolean) => { const end = Date.now() + 8000; while (!c()) { if (Date.now() > end) assert.fail("timed out"); await new Promise((r) => setTimeout(r, 20)); } };
    await until(() => store.getState().lines.length === host.lastSeq);
    assert.deepEqual(store.getState().lines.map((l) => l.seq), host.lines.map((l) => l.seq));
  } finally { store.stop(); await m.stop(); }
});
