// The mock host is the stand-in for what a gateway cannot show here: its stream must behave as H5's does.
import { test } from "node:test";
import assert from "node:assert/strict";
import { SseParser, type SseEvent } from "../src/live/sse.ts";
import { startMock } from "../mock/gateway.ts";

test("a partials=only stream ends with an end frame when the host stops, as the others do", async () => {
  const m = await startMock();
  const host = m.hosts.get("alpha")!;
  try {
    const got: SseEvent[] = [];
    const ac = new AbortController();
    const r = await fetch(`${m.url}/api/i/alpha/v1/events?after=${host.lastSeq}&partials=only`, { headers: { accept: "text/event-stream" }, signal: ac.signal });
    const p = new SseParser();
    const dec = new TextDecoder();
    const done = (async () => { for await (const c of r.body as unknown as AsyncIterable<Uint8Array>) for (const e of p.push(dec.decode(c, { stream: true }))) got.push(e); })();
    await new Promise((res) => setTimeout(res, 150));
    await host.halt();
    await Promise.race([done, new Promise((_, rej) => setTimeout(() => rej(new Error("the stream did not end")), 3000))]);
    assert.equal(got[0].event, "open");
    assert.equal(got.at(-1)!.event, "end", "the last frame is end");
    assert.equal(got.filter((e) => e.event === "record").length, 0, "partials=only carries no record frame");
    ac.abort();
  } finally { await m.stop(); }
});
