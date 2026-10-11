// The contract test for the host's context door, GET /v1/turns/{n}/context?call=k (ui/contract/README.md, "Proposed").
// The door is the host's S4 (docs/rung-host-api.md): the real gateway serves it over an instance's state directory. A gateway that
// does not serve it (a 404 on a turn the record holds) skips every check here, by name. Run it with:
//   cargo build -p rung-gateway && RUNG_CONTRACT_TARGET=real npm run test:contract:context     (the suite plays the host's part for the record)
//   RUNG_CONTRACT_URL=http://… RUNG_CONTRACT_INSTANCE=id npm run test:contract:context          (any gateway; a real host's record shows the whole bands)
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { startTarget, type Target } from "./contract-target.ts";
import { parseContext } from "../src/context/wire.ts";
import { markBands } from "../src/context/cache.ts";
import { foldContext } from "../src/context/fold.ts";
import { WIRE_ORDER } from "../src/context/types.ts";
import type { Line } from "../src/record/types.ts";

const NO_DOOR = "the context door (S4) is not on this gateway";
let t: Target;
let door = false;
let lines: Line[] = [];
/** A turn the record holds with at least one model call, and the last of its calls. */
let pick: { turn: number; calls: number } | null = null;
const seen: string[] = [];

const api = (path: string) => `${t.url}/api/i/${t.instance}${path}`;
async function get(url: string, init?: RequestInit) {
  const r = await fetch(url, init);
  const text = await r.text();
  seen.push([...r.headers].map(([k, v]) => `${k}: ${v}`).join("\n"), text);
  return { status: r.status, text, json: () => JSON.parse(text) };
}

before(async () => {
  t = await startTarget();
  lines = (await get(api("/v1/record?offset=0&limit=100000000"))).json().lines as Line[];
  const counts = new Map<number, number>();
  for (const l of lines) if (l.kind === "llm.call" && typeof l.turn === "number") counts.set(l.turn, (counts.get(l.turn) ?? 0) + 1);
  const turn = [...counts.keys()].filter((n) => lines.some((l) => l.kind === "turn.started" && l.turn === n)).sort((a, b) => a - b).at(-1);
  if (turn !== undefined) {
    pick = { turn, calls: counts.get(turn)! };
    door = (await get(api(`/v1/turns/${turn}/context?call=${pick.calls}`))).status !== 404;
  }
});
after(async () => { await t?.stop(); });

const need = (ctx: { skip(m: string): void }) => { if (!door || !pick) { ctx.skip(NO_DOOR); return false; } return true; };

test("context door: a turn's last call answers in the contract's shape, seven bands in wire order", async (ctx) => {
  if (!need(ctx)) return;
  const r = await get(api(`/v1/turns/${pick!.turn}/context?call=${pick!.calls}`));
  assert.equal(r.status, 200);
  const v = parseContext(r.json());
  assert.equal(v.turn, pick!.turn);
  assert.equal(v.call, pick!.calls);
  assert.equal(v.calls, pick!.calls);
  assert.deepEqual(v.bands.map((b) => b.id), WIRE_ORDER);
  assert.equal(markBands(v.bands, v.usage).length, 7);
});

test("context door: every call of the turn answers, and the provider's counts are the record's", async (ctx) => {
  if (!need(ctx)) return;
  const recorded = lines.filter((l) => l.kind === "llm.call" && l.turn === pick!.turn);
  for (let k = 1; k <= pick!.calls; k++) {
    const v = parseContext((await get(api(`/v1/turns/${pick!.turn}/context?call=${k}`))).json());
    assert.equal(v.call, k);
    assert.ok(v.usage, `call ${k} was answered, so the provider's count is there`);
    assert.equal(v.usage!.prompt, recorded[k - 1].prompt_tokens, `call ${k}: the provider's prompt count`);
    assert.equal(v.usage!.cached, recorded[k - 1].cached_tokens, `call ${k}: the provider's cached count`);
  }
});

test("context door: the first call of a turn has nothing of 'this turn so far'; a later call has the earlier replies and results", async (ctx) => {
  if (!need(ctx)) return;
  const first = parseContext((await get(api(`/v1/turns/${pick!.turn}/context?call=1`))).json());
  const b1 = first.bands.find((b) => b.id === "this_turn")!;
  assert.equal(b1.items.length + (b1.text ? 1 : 0), 0);
  if (pick!.calls > 1) {
    const second = parseContext((await get(api(`/v1/turns/${pick!.turn}/context?call=2`))).json());
    const b2 = second.bands.find((b) => b.id === "this_turn")!;
    assert.ok(b2.recorded && (b2.items.length > 0 || !!b2.text), "call 2 holds what call 1 produced");
  }
});

test("context door: its bands agree with what the record says of the same turn, where both give the band", async (ctx) => {
  if (!need(ctx)) return;
  const v = parseContext((await get(api(`/v1/turns/${pick!.turn}/context?call=${pick!.calls}`))).json());
  const rec = foldContext(lines, pick!.turn, pick!.calls)!;
  assert.equal(v.epoch, rec.epoch);
  assert.deepEqual(v.cacheEvent, rec.cacheEvent);
  // a past turn in the history is the turn's own logged messages: the door's row carries the record's header whole
  const h = v.bands.find((b) => b.id === "history")!;
  const rh = rec.bands.find((b) => b.id === "history")!;
  if (h.recorded && rh.items.length) for (const row of rh.items.slice(-3)) {
    const same = h.items.find((i) => i.label === row.label);
    if (same) assert.ok(same.text.includes(row.text.split("\n")[0]), `${row.label}: the header line is whole`);
  }
});

test("context door: a turn or a call that does not exist is refused in the error shape, never a body of bands", async (ctx) => {
  if (!need(ctx)) return;
  for (const path of [`/v1/turns/99999999/context?call=1`, `/v1/turns/${pick!.turn}/context?call=${pick!.calls + 1}`, `/v1/turns/${pick!.turn}/context?call=0`, `/v1/turns/x/context?call=1`]) {
    const r = await get(api(path));
    assert.ok(r.status >= 400 && r.status < 500, `${path} is a client error, not ${r.status}`);
    const body = r.json();
    assert.equal(typeof body.error, "string", `${path}: an error code`);
    assert.equal(typeof body.message, "string", `${path}: a message`);
    assert.equal(body.bands, undefined);
  }
});

test("context door: it only reads, and a read-only token reads it", async (ctx) => {
  if (!need(ctx)) return;
  const w = await get(api(`/v1/turns/${pick!.turn}/context?call=1`), { method: "POST", body: "{}" });
  assert.ok(w.status >= 400 && w.status < 500, "a write to the door is refused");
  if (!t.readOnlyToken) return;
  const r = await get(api(`/v1/turns/${pick!.turn}/context?call=1`), { headers: { authorization: `Bearer ${t.readOnlyToken}` } });
  assert.equal(r.status, 200);
});

test("context door: a key a tool printed is on no band", async (ctx) => {
  if (!need(ctx)) return;
  if (!t.control) return ctx.skip("this target cannot be made to print a key");
  const KEY = "sk-or-v1-" + "d4c3b2a1f0e9".repeat(6);
  await t.control.plant(KEY);
  const total = (await get(api("/v1/summary"))).json().turn;
  const turns = [pick!.turn, ...(typeof total === "number" ? [total] : [])];
  for (const n of turns) {
    const r = await get(api(`/v1/turns/${n}/context?call=1`));
    assert.equal(r.text.includes(KEY), false, "a key in a context answer");
  }
});

test("context door: no key or authorization header in any answer", (ctx) => {
  if (!need(ctx)) return;
  if (!t.hostKey) return ctx.skip("the key is not known to this target");
  for (const s of seen) assert.equal(s.includes(t.hostKey!), false, "the instance key appeared in an answer");
  assert.equal(seen.some((s) => /^authorization:/im.test(s)), false, "no authorization header in any answer");
});
