// The contract tests for the live view's doors (docs/rung-host-api.md: H3 reads, H5 events; ui/contract/README.md). Written before the mock they first ran against.
// The same file runs against a real gateway: RUNG_CONTRACT_URL=http://… RUNG_CONTRACT_INSTANCE=id npm run test:contract
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { connect } from "node:net";
import { startTarget, type Target } from "./contract-target.ts";
import { SseParser, type SseEvent } from "../src/live/sse.ts";

let t: Target;
const seen: string[] = []; // every body and header block the target answered, for the leak check
before(async () => { t = await startTarget(); });
after(async () => { await t?.stop(); });

const api = (path: string) => `${t.url}/api/i/${t.instance}${path}`;
async function get(url: string, init?: RequestInit) {
  const r = await fetch(url, init);
  const text = await r.text();
  seen.push([...r.headers].map(([k, v]) => `${k}: ${v}`).join("\n"), text);
  return { status: r.status, headers: r.headers, text, json: () => JSON.parse(text) };
}
const summary = async () => (await get(api("/v1/summary"))).json();
const record = async (q = "") => (await get(api(`/v1/record${q}`))).json();
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Open the event stream; `onEvent` sees each parsed event with its receipt time. Returns a handle to abort. */
function stream(path: string, onEvent: (e: SseEvent, at: number) => void, headers: Record<string, string> = {}) {
  const ac = new AbortController();
  const done = (async () => {
    const r = await fetch(api(path), { headers: { accept: "text/event-stream", ...headers }, signal: ac.signal });
    assert.equal(r.status, 200);
    const p = new SseParser();
    const dec = new TextDecoder();
    try {
      for await (const chunk of r.body as unknown as AsyncIterable<Uint8Array>) {
        for (const e of p.push(dec.decode(chunk, { stream: true }))) onEvent(e, Date.now());
      }
    } catch (e) { if (!ac.signal.aborted) throw e; }
  })();
  return { abort: () => ac.abort(), done };
}
async function until(cond: () => boolean, ms = 5000, what = "condition") {
  const end = Date.now() + ms;
  while (!cond()) { if (Date.now() > end) assert.fail(`timed out waiting for ${what}`); await sleep(10); }
}

test("health, and the instance is listed with its summary", async () => {
  assert.deepEqual((await get(`${t.url}/api/health`)).json(), { ok: true });
  const list = (await get(`${t.url}/api/instances`)).json().instances;
  const me = list.find((i: { id: string }) => i.id === t.instance);
  assert.ok(me, "the instance is listed");
  assert.equal(me.reachable, true);
  assert.equal(me.summary.contract, 1);
  if (t.deadInstance) {
    const dead = list.find((i: { id: string }) => i.id === t.deadInstance);
    assert.ok(dead, "an instance that does not answer is listed, never left out");
    assert.equal(dead.reachable, false);
    assert.equal(dead.summary, null);
    assert.equal(typeof dead.error, "string");
  }
});

test("summary has the documented fields and types", async () => {
  const s = await summary();
  assert.equal(s.contract, 1);
  assert.ok(["Working", "Answering", "Free time", "Waiting", "Stuck", "Down", "Stopped", "Unreadable"].includes(s.state), s.state);
  for (const k of ["doing"]) assert.equal(typeof s[k], "string");
  assert.equal(typeof s.needs_you, "boolean");
  for (const k of ["last_seq", "last_at", "requests", "spend_usd_day"]) assert.equal(typeof s[k], "number", k);
  assert.ok(s.quota === null || typeof s.quota === "number");
  assert.ok(s.turn === null || typeof s.turn === "number");
});

test("record: paging with exact totals, no gaps, either order, no cap on limit", async () => {
  const first = await record("?offset=0&limit=10");
  assert.equal(first.offset, 0);
  assert.equal(first.limit, 10);
  assert.equal(first.lines.length, 10);
  assert.deepEqual(first.lines.map((l: { seq: number }) => l.seq), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
  assert.equal(first.next, 10);
  const total = first.total;
  assert.ok(total >= 10);
  const mid = await record("?offset=5&limit=10");
  assert.deepEqual(mid.lines.map((l: { seq: number }) => l.seq), [6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
  // limit as large as total: the whole record, in one answer, and nothing after it.
  const all = await record(`?offset=0&limit=${total}`);
  assert.equal(all.lines.length, all.total);
  assert.equal(all.next, null);
  all.lines.forEach((l: { seq: number }, i: number) => assert.equal(l.seq, i + 1, "numbers have no gaps"));
  const past = await record(`?offset=${all.total - 3}&limit=1000000`);
  assert.equal(past.lines.length, 3, "a limit past the end is not an error");
  assert.equal(past.next, null);
  const desc = await record("?offset=0&limit=5&order=desc");
  assert.deepEqual(desc.lines.map((l: { seq: number }) => l.seq), [all.total, all.total - 1, all.total - 2, all.total - 3, all.total - 4]);
  for (const bad of ["?limit=abc", "?offset=-1", "?order=sideways", "?limit=0"]) {
    const r = await get(api(`/v1/record${bad}`));
    assert.equal(r.status, 400, bad);
    assert.equal(r.json().error, "bad_request");
  }
});

const lastSeq = async () => (await summary()).last_seq as number;
const records = (evs: SseEvent[]) => evs.filter((e) => e.event === "record");

test("events: an open frame first, then every line after N once and in order, equal to the record", async () => {
  const total = await lastSeq();
  const after_ = Math.max(0, total - 5);
  const got: SseEvent[] = [];
  const s = stream(`/v1/events?after=${after_}`, (e) => got.push(e));
  await until(() => records(got).length >= total - after_, 5000, "the replay");
  s.abort(); await s.done;
  assert.equal(got[0].event, "open", "the first frame is open");
  assert.equal(got[0].id, undefined);
  const open = JSON.parse(got[0].data);
  assert.equal(open.contract, 1);
  assert.equal(open.after, after_);
  assert.equal(typeof open.live, "boolean");
  assert.equal(typeof open.partials, "boolean");
  assert.ok(Array.isArray(open.redaction.env_unset));
  const want = (await record(`?offset=${after_}&limit=100000000`)).lines;
  const recs = records(got);
  recs.slice(0, total - after_).forEach((e, i) => {
    assert.equal(e.id, String(after_ + 1 + i), "the id is the line's number, with no gap");
    assert.deepEqual(JSON.parse(e.data), want[i]);
  });
});

test("Last-Event-ID is the cursor and beats ?after; partials=off carries no partial frame", async () => {
  const total = await lastSeq();
  const ids = async (path: string, headers: Record<string, string> = {}) => {
    const got: SseEvent[] = [];
    const s = stream(path, (e) => got.push(e), headers);
    await until(() => records(got).length >= 3, 5000, "three lines");
    s.abort(); await s.done;
    return records(got).slice(0, 3).map((e) => e.id);
  };
  const a = await ids(`/v1/events?after=${total - 3}`);
  const b = await ids("/v1/events", { "last-event-id": String(total - 3) });
  const c = await ids(`/v1/events?after=${total - 1}`, { "last-event-id": String(total - 3) });
  const d = await ids(`/v1/events?after=${total - 3}&partials=off`);
  assert.deepEqual(b, a);
  assert.deepEqual(c, a, "the header wins when both are given");
  assert.deepEqual(d, a);
  const off: SseEvent[] = [];
  const s = stream(`/v1/events?after=${total - 3}&partials=off`, (e) => off.push(e));
  await until(() => off.some((e) => e.event === "open"), 5000, "open");
  s.abort(); await s.done;
  assert.equal(JSON.parse(off[0].data).partials, false, "open says no partial frames will come");
});

test("events: a cursor past the record is 409 cursor_ahead; a bad query is 400 bad_request", async () => {
  const total = await lastSeq();
  const ahead = await get(api(`/v1/events?after=${total + 1000000}`), { headers: { accept: "text/event-stream" } });
  assert.equal(ahead.status, 409);
  assert.equal(ahead.json().error, "cursor_ahead");
  for (const bad of ["?after=abc", "?after=-1", "?partials=maybe", "?after=1&after=2", "?nothing=1"]) {
    const r = await get(api(`/v1/events${bad}`), { headers: { accept: "text/event-stream" } });
    assert.equal(r.status, 400, bad);
    assert.equal(r.json().error, "bad_request", bad);
  }
});

test("a line the host records reaches the stream within a second, in order", async (ctx) => {
  if (!t.control) return ctx.skip("no control hook on this target");
  const total = await lastSeq();
  const got: { seq: number; lag: number }[] = [];
  const s = stream(`/v1/events?after=${total}`, (e, at) => { if (e.event === "record") got.push({ seq: Number(e.id), lag: at - JSON.parse(e.data).at }); });
  await sleep(150);
  await t.control.poke(8);
  await until(() => got.length >= 8, 5000, "eight lines");
  s.abort(); await s.done;
  got.forEach((g, i) => assert.equal(g.seq, total + 1 + i));
  const lags = got.map((g) => g.lag).sort((a, b) => a - b);
  assert.ok(lags.at(-1)! < 1000, `worst lag ${lags.at(-1)} ms`);
});

test("cut and resume: exactly once, in order, nothing missing", async (ctx) => {
  if (!t.control) return ctx.skip("no control hook on this target");
  const total = await lastSeq();
  const a: number[] = [];
  const s1 = stream(`/v1/events?after=${total}`, (e) => { if (e.event === "record") a.push(Number(e.id)); });
  await sleep(150);
  await t.control.poke(4);
  await until(() => a.length >= 4, 5000, "first four");
  s1.abort(); await s1.done; // the cut
  const last = a.at(-1)!;
  await t.control.poke(6); // written while nobody is listening
  const b: number[] = [];
  const s2 = stream("/v1/events", (e) => { if (e.event === "record") b.push(Number(e.id)); }, { "last-event-id": String(last) });
  await until(() => b.length >= 6, 5000, "the missed lines");
  s2.abort(); await s2.done;
  const all = [...a, ...b];
  assert.deepEqual(all, Array.from({ length: all.length }, (_, i) => total + 1 + i), "contiguous: nothing twice, nothing missing");
  const rec = (await record(`?offset=${total}&limit=1000`)).lines.map((l: { seq: number }) => l.seq);
  assert.deepEqual(all, rec.slice(0, all.length), "equal to the record, line by line");
});

test("partial frames are short-lived: no id, seen live, never replayed, never in the record", async (ctx) => {
  if (!t.control) return ctx.skip("no control hook on this target");
  if (!t.deltas) return ctx.skip("this target runs no turn, so there is nothing partial to stream (a running host is needed)");
  const total = await lastSeq();
  const live: SseEvent[] = [];
  const s1 = stream(`/v1/events?after=${total}`, (e) => live.push(e));
  await until(() => live.some((e) => e.event === "open"), 5000, "open");
  assert.equal(JSON.parse(live[0].data).partials, true, "open says partial frames will come");
  await t.control.turn();
  await until(() => live.some((e) => e.event === "delta" && JSON.parse(e.data).kind === "turn_end"), 8000, "the turn's end");
  s1.abort(); await s1.done;
  const deltas = live.filter((e) => e.event === "delta");
  assert.ok(deltas.every((d) => d.id === undefined), "a partial frame has no id");
  const kinds = new Set(deltas.map((d) => JSON.parse(d.data).kind));
  for (const k of ["text", "tool_call", "tool_run", "tool_done", "turn_end"]) assert.ok(kinds.has(k), `a ${k} frame was seen`);
  for (const d of deltas) { const j = JSON.parse(d.data); assert.equal(typeof j.turn, "number"); assert.equal(typeof j.call, "number"); }
  const text = deltas.map((d) => JSON.parse(d.data)).filter((j) => j.kind === "text").map((j) => j.text).join("");
  const done = records(live).findLast((e) => JSON.parse(e.data).kind === "turn.ended");
  assert.ok(done, "the turn's end is on the record too");
  assert.ok(String(JSON.parse(done!.data).final_text).includes(text.trim().slice(0, 40)), "the finished text on the record is the text that streamed");
  const replay: SseEvent[] = [];
  const s2 = stream(`/v1/events?after=${total}`, (e) => replay.push(e));
  await until(() => records(replay).length >= records(live).length, 5000, "the replay");
  s2.abort(); await s2.done;
  assert.equal(replay.some((e) => e.event === "delta"), false, "a replay carries no partial frame");
  assert.equal((await record("?offset=0&limit=100000000")).lines.some((l: { kind: string }) => l.kind === "delta"), false, "a partial frame is never in the record");
});

test("a slow reader holds only its own place: others are not slowed, and it loses nothing", { timeout: 60000 }, async (ctx) => {
  if (!t.control) return ctx.skip("no control hook on this target");
  const total = await lastSeq();
  const u = new URL(t.url);
  // A raw socket that asks for the stream and then does not read.
  const sock = connect({ host: u.hostname, port: Number(u.port) });
  sock.pause();
  await new Promise((r) => sock.once("connect", r));
  sock.write(`GET /api/i/${t.instance}/v1/events?after=${total} HTTP/1.1\r\nHost: 127.0.0.1\r\naccept: text/event-stream\r\n\r\n`);
  await sleep(200);
  const fast: number[] = [];
  const s = stream(`/v1/events?after=${total}`, (e) => { if (e.event === "record") fast.push(Number(e.id)); });
  await sleep(150);
  const N = 120;
  await t.control.flood(N, 40_000); // about 5 MB: far more than a socket buffers
  await until(() => fast.length >= N, 30000, "the fast reader to get every line");
  s.abort(); await s.done;
  assert.deepEqual(fast, Array.from({ length: N }, (_, i) => total + 1 + i), "the fast reader was not slowed or short-changed");
  // The slow reader reads at last: every line, once, in order. Nothing was dropped for it and nothing was skipped.
  const p = new SseParser();
  const dec = new TextDecoder();
  const slow: number[] = [];
  sock.on("data", (b: Buffer) => { for (const e of p.push(dec.decode(b, { stream: true }))) if (e.event === "record") slow.push(Number(e.id)); });
  sock.resume();
  await until(() => slow.length >= N, 30000, "the slow reader to catch up");
  sock.destroy();
  assert.deepEqual(slow.slice(0, N), Array.from({ length: N }, (_, i) => total + 1 + i), "every line once and in order");
});

test("the read-only token may read and may not write; a wrong token is 401", async (ctx) => {
  if (!(t.readOnlyToken)) return ctx.skip("no read-only token on this target");
  const tok = t.readOnlyToken!;
  assert.equal((await get(api("/v1/summary"), { headers: { authorization: `Bearer ${tok}` } })).status, 200);
  const w = await get(api("/v1/queue"), { method: "POST", headers: { authorization: `Bearer ${tok}`, "content-type": "application/json" }, body: "{}" });
  assert.equal(w.status, 403);
  assert.equal(w.json().error, "read_only");
  assert.equal((await get(api("/v1/summary"), { headers: { authorization: "Bearer not-a-token" } })).status, 401);
});

test("a key a tool prints is on no door and no frame: the host redacts first", async (ctx) => {
  if (!t.control) return ctx.skip("no control hook on this target");
  const KEY = "sk-or-v1-0123456789abcdef0123456789abcdef";
  const total = await lastSeq();
  const live: SseEvent[] = [];
  const s = stream(`/v1/events?after=${total}`, (e) => live.push(e));
  await sleep(150);
  await t.control.plant(KEY);
  await until(() => records(live).length >= 1, 5000, "the planted line");
  await sleep(300);
  s.abort(); await s.done;
  const doors = [await get(api("/v1/summary")), await get(`${t.url}/api/instances`), await get(api("/v1/record?offset=0&limit=100000000"))];
  assert.ok(doors[2].text.includes("printed "), "the planted line is on the record, so the check can fail");
  for (const d of doors) assert.equal(d.text.includes(KEY), false, "a key in an answer");
  assert.equal(JSON.stringify(live).includes(KEY), false, "a key in a frame");
  const replay: SseEvent[] = [];
  const s2 = stream(`/v1/events?after=${Math.max(0, total - 2)}`, (e) => replay.push(e));
  await until(() => records(replay).length >= 3, 5000, "the replay");
  s2.abort(); await s2.done;
  assert.equal(JSON.stringify(replay).includes(KEY), false, "a key in a replay");
});

test("only /v1 passes, and an unknown instance is 404", async () => {
  const r = await get(`${t.url}/api/i/${t.instance}/acp`);
  assert.equal(r.status, 404);
  const n = await get(`${t.url}/api/i/no-such-instance/v1/summary`);
  assert.equal(n.status, 404);
  assert.equal(n.json().error, "no_such_instance");
});

test("a host that stops: the lines up to and including halted, then an end frame, then the body ends", async (ctx) => {
  if (!t.control) return ctx.skip("no control hook on this target");
  const total = await lastSeq();
  const got: SseEvent[] = [];
  const s = stream(`/v1/events?after=${total}`, (e) => got.push(e));
  await until(() => got.some((e) => e.event === "open"), 5000, "open");
  await t.control.halt();
  await until(() => got.some((e) => e.event === "end"), 10000, "the end frame");
  await s.done; // the body ends by itself
  const recs = records(got);
  assert.equal(JSON.parse(recs.at(-1)!.data).kind, "halted", "the last line sent is halted");
  const end = JSON.parse(got.at(-1)!.data);
  assert.equal(got.at(-1)!.event, "end", "end is the last frame");
  assert.equal(end.reason, "stopped");
  assert.equal(end.last_seq, Number(recs.at(-1)!.id));
  // The stopped instance streams too: a new stream replays what it has and ends.
  const again: SseEvent[] = [];
  const s2 = stream(`/v1/events?after=${Math.max(0, total)}`, (e) => again.push(e));
  await s2.done;
  assert.equal(again.at(-1)!.event, "end");
  assert.equal(JSON.parse(again.at(-1)!.data).reason, "stopped");
  assert.equal((await summary()).state, "Stopped");
});

test("no key in any answer, stream line or header", (ctx) => {
  if (!(t.hostKey)) return ctx.skip("the key is not known to this target");
  assert.ok(seen.length > 10, "there was something to check");
  for (const s of seen) assert.equal(s.includes(t.hostKey!), false, "the instance key appeared in an answer");
  assert.equal(seen.some((s) => /^authorization:/im.test(s)), false, "no authorization header in any answer");
});
