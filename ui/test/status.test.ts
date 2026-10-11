// One mapping from what the stream (and the summary) can say to the words and the meaning every page uses, so two pages cannot disagree.
import { test } from "node:test";
import assert from "node:assert/strict";
import { STATUSES } from "../src/live/client.ts";
import { describeStatus, stoppedByStream, TURNS_LINE, withStreamEnd } from "../src/live/status.ts";
import { summarize } from "../src/record/folds.ts";
import { synthetic } from "../src/synthetic.ts";

test("every status the stream can produce has words and a meaning: a new one is a compile error, and here a test failure", () => {
  assert.deepEqual([...STATUSES].sort(), ["catching up", "connecting", "ended", "following", "reconnecting", "unavailable"]);
  for (const s of STATUSES) {
    const d = describeStatus(s);
    assert.ok(d.turnsLine.length > 3, `${s}: words for the Turns page`);
    assert.ok(d.nowWords.length > 3, `${s}: words for the Now page`);
    assert.ok(["yes", "no", "unknown"].includes(d.running), `${s}: whether it is running`);
  }
  assert.equal(Object.keys(TURNS_LINE).length, STATUSES.length);
});

test("an ended stream says Not running on both pages, and means not running, whatever a stale record says", () => {
  const d = describeStatus("ended");
  assert.equal(d.turnsLine, "Not running");
  assert.equal(d.nowWords, "Not running.");
  assert.equal(d.running, "no");
  // A stale record: its last turn is still 'running'. The stream's word for the instance wins.
  assert.equal(describeStatus("ended", { turnRunning: true }).nowWords, "Not running.");
});

test("while following, the Now page says what the running turn is doing, and between turns says so; the Turns page says Following", () => {
  assert.equal(describeStatus("following").turnsLine, "Following");
  assert.equal(describeStatus("following", { turnRunning: true, tool: "ws_write", elapsed: "12 s" }).nowWords, "Running ws_write, 12 s in.");
  assert.equal(describeStatus("following", { turnRunning: true, elapsed: "3 s" }).nowWords, "Writing, 3 s in.");
  assert.equal(describeStatus("following", { turnRunning: false }).nowWords, "Between turns.");
  assert.equal(describeStatus("following").running, "yes");
});

test("unavailable, reconnecting, catching up: each its own plain words", () => {
  assert.equal(describeStatus("unavailable").turnsLine, "Recorded, not followed");
  assert.equal(describeStatus("unavailable").nowWords, "Not followed.");
  assert.equal(describeStatus("reconnecting").turnsLine, "Reconnecting");
  assert.equal(describeStatus("connecting").nowWords, "Reconnecting.");
  assert.equal(describeStatus("catching up").turnsLine, "Catching up");
  assert.equal(describeStatus("catching up").running, "yes");
});

const stale = () => summarize(synthetic(Date.now()).find((i) => i.id === "atlas")!.lines, { lockHeld: true, now: Date.now() }); // says Working

test("a stale summary that still says Working is replaced by the stream's word once the host stopped, on every page that reads it", () => {
  assert.equal(stale().state, "Working");
  const stopped = withStreamEnd(stale(), { reason: "stopped", last_seq: 9, problem: null });
  assert.equal(stopped.state, "Stopped");
  assert.equal(stopped.doing, "Not running.");
  assert.equal(stopped.needsYou, false);
  const down = withStreamEnd(stale(), { reason: "down", last_seq: 9, problem: null });
  assert.equal(down.state, "Down");
  assert.equal(down.needsYou, true, "a host that died needs the owner");
  assert.equal(stoppedByStream({ reason: "stopped", last_seq: 1, problem: null }), true);
  assert.equal(stoppedByStream({ reason: "damaged", last_seq: 1, problem: "x" }), false, "a damaged record is not a stopped host");
  assert.equal(stoppedByStream(null), false);
});

test("a summary that already says the host is not running is left as it is, and no end leaves a running one alone", () => {
  const s = { ...stale(), state: "Stopped" as const, doing: "Reached its run limit." };
  assert.equal(withStreamEnd(s, { reason: "down", last_seq: 1, problem: null }), s);
  const w = stale();
  assert.equal(withStreamEnd(w, null), w);
  assert.equal(withStreamEnd(w, { reason: "error", last_seq: 1, problem: "read failed" }), w);
});
