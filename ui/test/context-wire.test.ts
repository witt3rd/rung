import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { parseContext } from "../src/context/wire.ts";
import { markBands, marksFor } from "../src/context/cache.ts";
import { WIRE_ORDER } from "../src/context/types.ts";

// These are answers of the real door (rung-gateway over a real host's record, and over a record from before context recording):
// `ctx-demo` is a host run on the mock engine; `2026-10-05-qwen-2h` is a recorded run that predates it.
const FIX = join(import.meta.dirname, "fixtures/context");
const fixture = (name: string): Record<string, any> => JSON.parse(readFileSync(join(FIX, name), "utf8"));
const names = (dir: string) => readdirSync(join(FIX, dir)).filter((f) => /^\d+-\d+\.json$/.test(f)).sort();

test("the fixtures are the real door's answers: one host's turns, and an old run's", () => {
  assert.deepEqual(names("ctx-demo"), ["1-1.json", "4-1.json", "4-2.json", "4-3.json", "4-4.json", "5-1.json"]);
  assert.deepEqual(names("2026-10-05-qwen-2h"), ["5-1.json", "5-2.json", "5-4.json"]);
});

test("a recorded turn's call parses into seven bands in wire order, every text whole", () => {
  const raw = fixture("ctx-demo/4-3.json");
  const v = parseContext(raw);
  assert.equal(v.source, "door");
  assert.deepEqual(v.bands.map((b) => b.id), WIRE_ORDER);
  assert.equal(v.turn, 4); assert.equal(v.call, 3); assert.equal(v.calls, 4); assert.equal(v.epoch, 1);
  assert.equal(v.model, raw.model.served);
  assert.deepEqual(v.usage, { prompt: raw.provider.prompt_tokens, cached: raw.provider.cached_tokens, written: raw.provider.cache_write_tokens });
  const by = (id: string) => v.bands.find((b) => b.id === id)!;
  const rb = (id: string) => raw.bands.find((b: any) => b.band === id);
  assert.equal(by("system").text, rb("system").text);
  assert.equal(by("epoch").text, rb("epoch").text);
  assert.equal(by("message").text, rb("message").text);
  assert.equal(by("recall").text, rb("recall").text);
  assert.deepEqual(by("tools").items.map((i) => [i.label, i.text]), rb("tools").items.map((t: any) => [t.name, t.text]));
  assert.equal(by("tools").items.length, rb("tools").count);
  assert.equal(by("history").items.length, rb("history").turns);
  assert.equal(by("this_turn").items.length, rb("this_turn").messages.length);
  assert.ok(by("this_turn").items[0].text.startsWith("call "), "a model message reads as the call it made");
  for (const b of v.bands) assert.equal(b.recorded, true);
  assert.deepEqual(v.bands.map((b) => b.estimate), WIRE_ORDER.map((id) => rb(id)?.est_tokens ?? 0));
});

test("the host's cache boundaries and its cache event come through; a call with no event says none", () => {
  const first = parseContext(fixture("ctx-demo/1-1.json"));
  assert.deepEqual(first.cacheEvent, { kind: "cache.break", cause: "rollover" });
  assert.deepEqual(first.boundaries.map((b) => b.after), ["system", "epoch"]);
  assert.equal(parseContext(fixture("ctx-demo/4-2.json")).cacheEvent, null);
});

test("a turn that was shown no recall has an empty recall band, not a gap", () => {
  const v = parseContext(fixture("ctx-demo/1-1.json"));
  const r = v.bands.find((b) => b.id === "recall")!;
  assert.equal(r.recorded, true);
  assert.equal(r.estimate, 0);
  assert.equal(r.items.length + (r.text ? 1 : 0), 0);
});

test("a turn that predates context recording: the door's note and the bands it names as missing, each with the reason", () => {
  const v = parseContext(fixture("2026-10-05-qwen-2h/5-2.json"));
  assert.equal(v.note, "this turn predates context recording");
  const gaps = v.bands.filter((b) => !b.recorded).map((b) => b.id);
  assert.deepEqual(gaps, ["tools", "system", "epoch", "recall"]);
  for (const b of v.bands.filter((x) => !x.recorded)) assert.match(b.why!, /^This turn predates context recording\./);
  assert.match(v.bands.find((b) => b.id === "recall")!.why!, /may have been shown recalled memory/i);
  for (const id of ["history", "message", "this_turn"]) assert.equal(v.bands.find((b) => b.id === id)!.recorded, true, id);
  assert.ok(v.bands.find((b) => b.id === "history")!.items.length > 0);
});

test("the host's own cache marks are what the page shows when it has the whole request", () => {
  for (const f of names("ctx-demo")) {
    const raw = fixture(`ctx-demo/${f}`);
    const v = parseContext(raw);
    const marks = marksFor(v.bands, v.usage);
    v.bands.forEach((b, i) => {
      const door = raw.bands.find((x: any) => x.band === b.id)?.cache;
      assert.equal(marks[i], b.estimate === 0 ? "empty" : door, `${f} ${b.id}`);
    });
  }
});

test("the page's own derivation (provider cached count against the bands in wire order) agrees with the host's marks", () => {
  for (const f of names("ctx-demo")) {
    const raw = fixture(`ctx-demo/${f}`);
    const v = parseContext(raw);
    const mine = markBands(v.bands, v.usage);
    v.bands.forEach((b, i) => {
      if (b.estimate === 0) return;
      assert.equal(mine[i], raw.bands.find((x: any) => x.band === b.id).cache, `${f} ${b.id}`);
    });
  }
});

test("when bands are missing the host's marks are not trusted: they were laid against fewer bands than the request had", () => {
  const raw = fixture("2026-10-05-qwen-2h/5-1.json");
  const v = parseContext(raw);
  // the door calls the history band cached or partly; the request began with bands it does not have, so the page says unknown
  assert.ok(raw.bands.some((b: any) => b.cache === "partly" || b.cache === "cached"));
  const marks = marksFor(v.bands, v.usage);
  assert.ok(!marks.includes("cached") && !marks.includes("partly"), marks.join(","));
});

test("a recall that does not match its recorded hash is shown as recorded, and said so", () => {
  const raw = structuredClone(fixture("ctx-demo/4-1.json"));
  raw.bands.find((b: any) => b.band === "recall").intact = false;
  const r = parseContext(raw).bands.find((b) => b.id === "recall")!;
  assert.match(r.partial!, /does not match/);
  assert.equal(r.text, raw.bands.find((b: any) => b.band === "recall").text);
});

test("a history turn whose header lacks its recall says so, on the row and on the band", () => {
  const raw = structuredClone(fixture("ctx-demo/4-1.json"));
  raw.bands.find((b: any) => b.band === "history").rows[0].header_complete = false;
  const h = parseContext(raw).bands.find((b) => b.id === "history")!;
  assert.match(h.items[0].note!, /recalled memory/);
  assert.equal(h.items[1].note, null);
  assert.match(h.partial!, /1 of the earlier turns was shown recalled memory/);
});

test("a key shape the host failed to redact is replaced here, in every band's text and every row", () => {
  const key = "sk-or-v1-" + "a1b2c3d4e5f6".repeat(6);
  const dirty = structuredClone(fixture("ctx-demo/4-3.json"));
  dirty.bands.find((b: any) => b.band === "system").text = `identity. the key is ${key} ok`;
  dirty.bands.find((b: any) => b.band === "tools").items[0].text += ` ${key}`;
  dirty.bands.find((b: any) => b.band === "history").rows[0].messages[0] = { role: "assistant", content: `see ${key}` };
  dirty.bands.find((b: any) => b.band === "this_turn").messages[0] = { role: "assistant", content: [{ type: "text", text: key }] };
  const all = JSON.stringify(parseContext(dirty));
  assert.ok(!all.includes(key), "the key shape is gone");
  assert.ok(all.includes("[redacted]"));
});

test("a body that is not a context answer is refused in plain words, never half-drawn", () => {
  assert.throws(() => parseContext(null), /context/i);
  assert.throws(() => parseContext({ turn: 3 }), /bands/i);
  assert.throws(() => parseContext({ turn: 3, call: 1, calls: 1, bands: [{ band: "tools", items: "no" }] }), /tools/i);
  assert.throws(() => parseContext({ turn: 3, call: 1, calls: 1, bands: [{ band: "system" }] }), /system/i);
  assert.throws(() => parseContext({ turn: 3, call: 1, calls: 1, bands: [{ band: "history", rows: [{ header: "x" }] }] }), /history/i);
  assert.throws(() => parseContext({ turn: 3, call: 1, calls: 1, bands: [{ band: "system", text: "a" }, { band: "system", text: "b" }] }), /twice/i);
  assert.throws(() => parseContext({ call: 1, calls: 1, bands: [] }), /turn/i);
});

test("a very long band comes through whole (no cap anywhere)", () => {
  const big = structuredClone(fixture("ctx-demo/4-3.json"));
  const text = "line of the system text, again and again.\n".repeat(20000);
  big.bands.find((b: any) => b.band === "system").text = text;
  assert.equal(parseContext(big).bands.find((b) => b.id === "system")!.text, text);
});
