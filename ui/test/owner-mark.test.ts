// The owner mark in the turn view: what the fold must give a turn for each item it was asked (S1, page side).
import { test } from "node:test";
import assert from "node:assert/strict";
import { foldTurns } from "../src/record/folds.ts";
import type { Line } from "../src/record/types.ts";

let seq = 0;
const L = (at: number, kind: string, f: Record<string, unknown> = {}): Line => ({ seq: ++seq, at, kind, ...f });
const item = (id: string, at: number, extra: Record<string, unknown> = {}) => ({ id, kind: "peer", role: "owner", channel: "owner", firm: false, at, text: `text of ${id}`, ...extra });

function run(): Line[] {
  seq = 0;
  return [
    L(1000, "host.start", {}),
    L(1100, "turn.started", { turn: 1 }),
    L(1500, "stimulus.accepted", { item: item("s1", 1500, { intent: "steer" }) }), // arrives while turn 1 runs
    L(2000, "turn.ended", { turn: 1, status: "completed" }),
    L(2100, "stimulus.accepted", { item: item("t1", 2100, { intent: "next" }) }), // arrives between turns
    L(2200, "stimulus.accepted", { item: item("old", 2200) }), // a record from before the host wrote an intent
    L(2300, "stimulus.accepted", { item: { id: "p1", kind: "peer", role: "peer", channel: "peer:atlas", firm: false, at: 2300, text: "from a peer" } }),
    L(2500, "stimulus.admitted", { turn: 2, boundary: 2, ids: ["s1", "t1", "old", "p1"], digests: [] }),
    L(2600, "turn.started", { turn: 2 }),
    L(3000, "turn.ended", { turn: 2, status: "completed" }),
  ];
}

test("an owner item carries its role, its intent, when it was sent and when the agent saw it", () => {
  const t2 = foldTurns(run()).find((t) => t.n === 2)!;
  const by = Object.fromEntries(t2.asked.map((a) => [a.id, a]));
  assert.equal(by.s1.from, "owner");
  assert.equal(by.s1.role, "owner");
  assert.equal(by.s1.intent, "steer");
  assert.equal(by.s1.sentAt, 1500);
  assert.equal(by.s1.seenAt, 2500, "the agent saw it at the line that admitted it");
  assert.equal(by.t1.intent, "next");
});

test("an arrival while a turn was running names that turn; an arrival between turns names none", () => {
  const by = Object.fromEntries(foldTurns(run()).find((t) => t.n === 2)!.asked.map((a) => [a.id, a]));
  assert.equal(by.s1.during, 1);
  assert.equal(by.t1.during, null);
  assert.equal(by.old.during, null);
});

test("an item with no intent (an older record) has none, and a peer item is not marked as the owner's", () => {
  const by = Object.fromEntries(foldTurns(run()).find((t) => t.n === 2)!.asked.map((a) => [a.id, a]));
  assert.equal(by.old.intent, null);
  assert.equal(by.old.role, "owner");
  assert.equal(by.p1.role, "peer");
  assert.notEqual(by.p1.from, "owner");
  assert.equal(by.p1.intent, null);
});

test("an arrival while the running turn has no end yet still names that turn", () => {
  seq = 0;
  const lines = [
    L(1000, "host.start"), L(1100, "turn.started", { turn: 1 }),
    L(1500, "stimulus.accepted", { item: item("s1", 1500, { intent: "steer" }) }),
    L(1600, "stimulus.admitted", { turn: 2, ids: ["s1"], digests: [] }),
    L(1700, "turn.started", { turn: 2 }),
  ];
  assert.equal(foldTurns(lines, { live: true }).find((t) => t.n === 2)!.asked[0].during, 1);
});

test("the text of an item is whole, however long", () => {
  seq = 0;
  const long = "word ".repeat(20_000);
  const lines = [L(1000, "host.start"), L(1100, "stimulus.accepted", { item: item("big", 1100, { text: long }) }), L(1200, "stimulus.admitted", { turn: 1, ids: ["big"], digests: [] }), L(1300, "turn.started", { turn: 1 })];
  assert.equal(foldTurns(lines)[0].asked[0].text, long);
});
