import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { join } from "node:path";
import { parseRecord } from "../src/record/parse.ts";
import { foldDecisions, foldPack, foldSpend, foldTurns, summarize, THRESHOLDS } from "../src/record/folds.ts";
import { synthetic } from "../src/synthetic.ts";
import { makeRedactor } from "../src/record/redact.ts";
import { ago, sayWho } from "../src/record/words.ts";
import type { Line } from "../src/record/types.ts";

const RUNS = join(import.meta.dirname, "../../rung-host/live/runs");
const recorded = readdirSync(RUNS).filter((id) => existsSync(join(RUNS, id, "state/record")));
const load = (id: string): Line[] =>
  readdirSync(join(RUNS, id, "state/record")).sort().flatMap((f) => parseRecord(readFileSync(join(RUNS, id, "state/record", f), "utf8")));

test("the recorded runs are there to test against", () => {
  assert.ok(recorded.length >= 5, `found ${recorded.length} recorded runs`);
  assert.ok(recorded.includes("2026-10-05-qwen-2h"));
});

test("every recorded run parses with its numbers gap-free", () => {
  for (const id of recorded) {
    const lines = load(id);
    assert.ok(lines.length > 10, id);
    lines.forEach((l, i) => assert.equal(l.seq, i + 1, `${id}: line ${i + 1} has seq ${l.seq}`));
  }
});

test("a torn last line is skipped; a bad line elsewhere is an error", () => {
  assert.equal(parseRecord('{"seq":1,"at":1,"kind":"a"}\n{"seq":2,"at":2,"ki').length, 1);
  assert.throws(() => parseRecord('{"seq":1,"at":1,"kind":"a"}\n{bad\n{"seq":3,"at":3,"kind":"c"}\n'), /not JSON/);
});

test("turns: text as the model wrote it, calls, one summary per turn", () => {
  const lines = load("2026-10-05-qwen-2h");
  const turns = foldTurns(lines);
  assert.equal(turns.length, new Set(lines.filter((l) => l.kind === "turn.started").map((l) => l.turn)).size);
  const t2 = turns.find((t) => t.n === 2)!;
  assert.ok(t2.text.startsWith("**What was done"), "turn 2 text is the model's final answer");
  assert.ok(t2.calls.length >= 3, "turn 2 has its tool calls from the messages as sent");
  assert.ok(t2.calls.every((c) => c.input !== undefined), "each call carries its input");
  assert.equal(turns[0].n, 1);
  assert.equal(turns.at(-1)!.n, 154, "a boundary that decided for a turn the host never started is not a turn");
  // A turn with no end line: cut off in a recorded run, which has no process to be running it; running when a lock is held.
  const cut = lines.filter((l) => !(l.kind === "turn.ended" && l.turn === 154));
  assert.equal(foldTurns(cut).at(-1)!.status, "interrupted");
  assert.equal(foldTurns(cut, { live: true }).at(-1)!.status, "running");
});

test("turn totals agree with the record's own per-turn cost", () => {
  const lines = load("2026-10-05-qwen-2h");
  const turns = foldTurns(lines);
  for (const e of lines.filter((l) => l.kind === "turn.ended" && l.status === "completed")) {
    const t = turns.find((x) => x.n === e.turn)!;
    assert.equal(t.promptTokens, e.cost.prompt, `turn ${e.turn} prompt tokens`);
    assert.equal(t.cachedTokens, e.cost.cached, `turn ${e.turn} cached tokens`);
  }
});

test("decisions: question, answer, who and why, for each of the five families", () => {
  const families = new Set<string>();
  for (const id of recorded) {
    const ds = foldDecisions(load(id));
    for (const d of ds) {
      families.add(d.family);
      assert.ok(d.question.length > 5, `${id} seq ${d.seq}: a question`);
      assert.ok(d.answer.length > 3, `${id} seq ${d.seq}: an answer`);
      assert.notEqual(d.who, "unknown", `${id} seq ${d.seq}: who decided`);
      assert.ok(d.reason, `${id} seq ${d.seq}: a decision with no recorded reason fails the page test`);
    }
  }
  assert.deepEqual([...families].sort(), ["admit", "consolidate", "inject", "pack", "tools"]);
});

test("a decision whose line records no decider has no reason, and the page test refuses it", () => {
  const d = foldDecisions([{ seq: 1, at: 1, kind: "decision.tools", turn: 1, choice: { enabled: ["core"] } }]);
  assert.equal(d[0].reason, null);
  assert.equal(d[0].who, "unknown");
  assert.equal(sayWho(undefined).reason, null);
});

test("a model-made decision says so, and a timeout says why the rule stepped in", () => {
  assert.match(sayWho({ jev: { model: "m-1" } }).label, /By model m-1/);
  assert.match(sayWho({ rule: "timeout" }).reason!, /did not answer in time/);
  assert.match(sayWho({ rule: "something_new" }).reason!, /something new/);
});

test("summary of a stopped recorded run", () => {
  const lines = load("2026-10-05-qwen-2h");
  const s = summarize(lines, { lockHeld: false, now: Date.now() });
  assert.equal(s.state, "Stopped");
  assert.equal(s.needsYou, false);
  assert.equal(s.quota, 400);
  assert.equal(s.requests, lines.filter((l) => l.kind === "llm.call").length, "one UTC day: every request counted");
  assert.ok(s.cache !== null && s.cache > 0 && s.cache <= 100);
  assert.ok(s.context !== null && s.context >= 0);
});

test("spend and pack folds add up to the record", () => {
  for (const id of recorded) {
    const lines = load(id);
    const sp = foldSpend(lines);
    const calls = lines.filter((l) => l.kind === "llm.call");
    assert.equal(Object.values(sp.byDay).reduce((n, d) => n + d.requests, 0), calls.length, id);
    assert.equal(Object.values(sp.byModel).reduce((n, d) => n + d.requests, 0), calls.length, id);
    const cost = calls.reduce((n, l) => n + l.cost_usd, 0);
    assert.ok(Math.abs(Object.values(sp.byDay).reduce((n, d) => n + d.costUsd, 0) - cost) < 1e-9, id);
    assert.equal(foldPack(lines).rollovers.length, lines.filter((l) => l.kind === "epoch.rollover").length, id);
  }
});

test("a run that did not end on a halt is Down when no lock is held", () => {
  const lines = load("2026-10-05-qwen-2h").filter((l) => l.kind !== "halted");
  const s = summarize(lines, { lockHeld: false, now: Date.now() });
  assert.equal(s.state, "Down");
  assert.equal(s.needsYou, true);
});

test("the seven state words, each from its own record", () => {
  const base = Date.UTC(2026, 9, 7, 12, 0, 0);
  const by = Object.fromEntries(synthetic(base).map((i) => [i.id, summarize(i.lines, { lockHeld: i.lockHeld, now: base })]));
  assert.equal(by.atlas.state, "Working");
  assert.equal(by.bramble.state, "Free time");
  assert.equal(by.bramble.idle, true);
  assert.equal(by.cedar.state, "Stuck");
  assert.equal(by.dune.state, "Waiting");
  assert.equal(by.elm.state, "Answering");
  assert.equal(by.fir.state, "Down");
  assert.equal(by.gorse.state, "Stopped");
  for (const [id, s] of Object.entries(by)) assert.equal(s.needsYou, id === "cedar" || id === "fir", `${id} needs-you`);
  assert.equal(by.atlas.project?.title, "Tidy the notes index");
  assert.equal(by.atlas.next?.text, "Stand-up: say what you are on");
});

test("a commitment with no progress for too many turns is Stuck", () => {
  const base = Date.UTC(2026, 9, 7, 12, 0, 0);
  const atlas = synthetic(base).find((i) => i.id === "atlas")!;
  const lines = [...atlas.lines];
  let seq = lines.length;
  let turn = 3;
  for (let i = 0; i < THRESHOLDS.stuckTurnsWithoutProgress; i++) {
    turn++;
    lines.push({ seq: ++seq, at: base + i, kind: "turn.started", turn, turn_kind: "committed", epoch: 1, pack_tokens: 1 });
    lines.push({ seq: ++seq, at: base + i, kind: "turn.ended", turn, turn_kind: "committed", status: "completed", final_text: "x", cost: {} });
  }
  assert.equal(summarize(lines, { lockHeld: true, now: base }).state, "Stuck");
});

test("synthetic instances are deterministic for a given time", () => {
  assert.deepEqual(synthetic(1_000_000_000), synthetic(1_000_000_000));
});

test("the redactor replaces and never shortens, drops or caps", () => {
  const r = makeRedactor(["hunter2hunter2"]);
  const long = "x".repeat(12 * 1024 * 1024);
  assert.equal((r(long) as string).length, long.length, "a 12 MB string is whole");
  assert.equal(r("a hunter2hunter2 b"), "a [redacted] b");
  assert.equal(r("token sk-or-v1-0123456789abcdef0123456789abcdef end"), "token [redacted] end");
  assert.deepEqual(r({ k: ["hunter2hunter2", 3, null] }), { k: ["[redacted]", 3, null] });
  assert.equal(r("short"), "short");
  const r2 = makeRedactor(["abc"]);
  assert.equal(r2("abc"), "abc", "a value too short to be a key is left alone");
});

test("time reads in the largest whole unit", () => {
  assert.equal(ago(0, 30_000), "30 s ago");
  assert.equal(ago(0, 5 * 60_000), "5 min ago");
  assert.equal(ago(0, 3 * 86_400_000), "3 d ago");
});
