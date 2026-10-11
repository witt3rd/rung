import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { parseRecord } from "../src/record/parse.ts";
import { parseContext } from "../src/context/wire.ts";
import { foldContext } from "../src/context/fold.ts";
import { WIRE_ORDER } from "../src/context/types.ts";
import type { Line } from "../src/record/types.ts";

const RUNS = join(import.meta.dirname, "../../rung-host/live/runs");
const load = (id: string): Line[] =>
  readdirSync(join(RUNS, id, "state/record")).sort().flatMap((f) => parseRecord(readFileSync(join(RUNS, id, "state/record", f), "utf8")));
const FIX = join(import.meta.dirname, "fixtures/context");
const demo = (): Line[] => parseRecord(readFileSync(join(FIX, "ctx-demo/record.ndjson"), "utf8"));
const door = (inst: string, n: number, k: number) => parseContext(JSON.parse(readFileSync(join(FIX, `${inst}/${n}-${k}.json`), "utf8")));

test("an old run: seven bands in wire order, and each band the record cannot give says so and why", () => {
  const v = foldContext(load("2026-10-05-hostkey-2h"), 21, 3)!;
  assert.equal(v.source, "record");
  assert.deepEqual(v.bands.map((b) => b.id), WIRE_ORDER);
  const by = Object.fromEntries(v.bands.map((b) => [b.id, b]));
  assert.equal(by.tools.recorded, false);
  assert.match(by.tools.why!, /fingerprint|hash/i);
  assert.equal(by.system.recorded, false);
  assert.equal(by.recall.recorded, false);
  assert.match(by.recall.why!, /report|not the text/i);
  // the epoch's title line is all the record has of the epoch layer: shown, and marked as partial
  assert.equal(by.epoch.recorded, true);
  assert.match(by.epoch.partial!, /title/i);
  assert.match(by.epoch.items[0].text, /^\[epoch \d+/);
});

test("the provider's counts for the call come from its llm.call line", () => {
  const lines = load("2026-10-05-hostkey-2h");
  const call = lines.filter((l) => l.kind === "llm.call" && l.turn === 21)[2];
  const v = foldContext(lines, 21, 3)!;
  assert.equal(v.calls, 5);
  assert.equal(v.call, 3);
  assert.equal(v.usage!.prompt, call.prompt_tokens);
  assert.equal(v.usage!.cached, call.cached_tokens);
  assert.equal(v.model, call.model_served);
});

test("'this turn so far' holds the messages before the call, and grows with the call; call 1 holds nothing", () => {
  const lines = load("2026-10-05-hostkey-2h");
  const size = (k: number) => foldContext(lines, 21, k)!.bands.find((b) => b.id === "this_turn")!;
  assert.equal(size(1).items.length, 0);
  assert.equal(size(1).recorded, true);
  assert.ok(size(2).items.length > 0);
  assert.ok(size(4).items.length > size(2).items.length);
  // whole: the item texts are the messages as the record kept them, joined without loss
  const log = lines.find((l) => l.kind === "turn.log" && l.turn === 21)!;
  const resultText = JSON.stringify(log.messages).length;
  assert.ok(resultText > 0);
  const all = size(5).items.map((i) => i.text).join("");
  for (const m of log.messages as Line[]) for (const b of Array.isArray(m.content) ? m.content : []) {
    if (b.type === "tool_result" && typeof b.content === "string" && size(5).items.length) assert.ok(all.includes(b.content), "a tool result is shown whole");
  }
});

test("history is one row per earlier turn of the same epoch, each with the turn's header and messages whole", () => {
  const lines = load("2026-10-05-hostkey-2h");
  const turnStart = lines.find((l) => l.kind === "turn.started" && l.turn === 21)!;
  const v = foldContext(lines, 21, 1)!;
  const h = v.bands.find((b) => b.id === "history")!;
  assert.equal(h.recorded, true);
  assert.match(h.partial!, /rebuilt/i);
  const sameEpoch = lines.filter((l) => l.kind === "turn.started" && l.epoch === turnStart.epoch && l.turn < 21).map((l) => l.turn);
  assert.equal(h.items.length, sameEpoch.length);
  const first = lines.find((l) => l.kind === "turn.log" && l.turn === sameEpoch[0])!;
  assert.ok(h.items[0].text.startsWith(first.header), "the header is kept whole");
  assert.match(h.items[0].label, new RegExp(`Turn ${sameEpoch[0]}\\b`));
});

test("the message band is the turn's header as logged, and says the recalled memory is not in it", () => {
  const lines = load("2026-10-05-hostkey-2h");
  const log = lines.find((l) => l.kind === "turn.log" && l.turn === 21)!;
  const m = foldContext(lines, 21, 1)!.bands.find((b) => b.id === "message")!;
  assert.equal(m.text, log.header);
  assert.match(m.partial!, /recall/i);
});

test("a cache break recorded at this call is named; other calls have none", () => {
  const lines = load("2026-10-05-hostkey-2h");
  const br = lines.find((l) => l.kind === "cache.break")!;
  assert.equal(foldContext(lines, br.turn, br.call)!.cacheEvent?.cause, br.cause);
  const other = lines.find((l) => l.kind === "llm.call" && !lines.some((x) => x.kind === "cache.break" && x.turn === l.turn && x.call === l.call))!;
  assert.equal(foldContext(lines, other.turn, other.call)!.cacheEvent, null);
});

test("a turn whose log is not written yet (running): 'this turn so far' says why it is missing", () => {
  const lines = load("2026-10-05-hostkey-2h").filter((l) => !(l.kind === "turn.log" && l.turn === 21));
  const b = foldContext(lines, 21, 2)!.bands.find((x) => x.id === "this_turn")!;
  assert.equal(b.recorded, false);
  assert.match(b.why!, /ends|not written/i);
});

test("a call beyond what the log can place is not drawn from guesses", () => {
  const lines = load("2026-10-05-hostkey-2h");
  // turn 7 made one call and logged no assistant message: only call 1 can be placed
  const v = foldContext(lines, 7, 1)!;
  assert.equal(v.bands.find((b) => b.id === "this_turn")!.items.length, 0);
});

test("a turn with no model call, or no such turn", () => {
  const lines = demo().filter((l) => l.kind !== "llm.call");
  assert.equal(foldContext(lines, 1, 1)!.calls, 0);
  assert.equal(foldContext(demo(), 999, 1), null);
});

// The record's own strata against what the real door answered over the same record: where the fold gives a band, it must say what the door says.
const same = (inst: string, lines: Line[], n: number, k: number) => {
  const d = door(inst, n, k), f = foldContext(lines, n, k)!;
  assert.deepEqual(f.usage, d.usage, "the provider's counts");
  assert.equal(f.model, d.model);
  assert.equal(f.epoch, d.epoch);
  assert.equal(f.calls, d.calls);
  assert.deepEqual(f.cacheEvent, d.cacheEvent);
  const by = (v: typeof f, id: string) => v.bands.find((b) => b.id === id)!;
  assert.deepEqual(by(f, "this_turn").items.map((i) => [i.label, i.text]), by(d, "this_turn").items.map((i) => [i.label, i.text]), "this turn so far");
  assert.deepEqual(by(f, "history").items.map((i) => i.label), by(d, "history").items.map((i) => i.label), "the same earlier turns");
  return { d, f, by };
};

test("a host's record, the record's strata against the real door, call by call", () => {
  const lines = demo();
  for (const [n, k] of [[1, 1], [4, 1], [4, 2], [4, 3], [4, 4], [5, 1]]) {
    const { d, f, by } = same("ctx-demo", lines, n, k);
    assert.equal(by(f, "epoch").items[0].text, by(d, "epoch").text!.split("\n")[0], "the epoch's title line is the first line of the epoch layer");
    assert.ok(by(f, "tools").why, "the fold says the tools are not in the record");
  }
});

test("the same for an old run: the door and the fold agree on what an old record holds", () => {
  const lines = load("2026-10-05-qwen-2h");
  for (const k of [1, 2, 4]) {
    const { d, f, by } = same("2026-10-05-qwen-2h", lines, 5, k);
    assert.equal(by(f, "message").text, by(d, "message").text, "the turn's header as logged");
    assert.deepEqual(by(f, "history").items.map((i) => i.text), by(d, "history").items.map((i) => i.text), "history rows, whole");
    // the fold also keeps the epoch's title line, which the door does not give an old record: it shows a little more, never a different thing
    assert.deepEqual(d.bands.filter((b) => !b.recorded).map((b) => b.id).filter((id) => id !== "epoch"), f.bands.filter((b) => !b.recorded).map((b) => b.id), "the same bands are missing");
  }
});
