// The queue answer as the page reads it, the words it uses, the order it shows, and the fold of a long text (S1, page side).
import { test } from "node:test";
import assert from "node:assert/strict";
import { parseQueue, groupWaiting, intentWord, roleWord, foldText, isBlank, afterSend } from "../src/queue/wire.ts";

// The doors' own shape (docs/rung-host-api.md, S1): items with a state, not two lists.
const it = (id: string, role: string, extra: Record<string, unknown> = {}) => ({
  id, text: `text ${id}`, role, name: role === "owner" ? null : "atlas", channel: role === "owner" ? "owner" : "peer:atlas", kind: "peer", intent: "next",
  order: 1, position: null, accepted_at: 1000, state: "waiting", turn: null, disposition: null, held: null, ...extra,
});
const answer = (items: unknown[], o: Record<string, unknown> = {}) => ({
  items, total: items.length, offset: 0, limit: 500, next: null, counts: { waiting: 0, answering: 0, answered: 0, withdrawn: 0 },
  record_lines: 12, damaged: null, redaction: { env_unset: [] }, ...o,
});

test("a queue answer is read whole: the item being answered, what waits in the host's order, and the desk's reason", () => {
  const q = parseQueue(answer([
    it("n1", "owner", { state: "answering", turn: 7 }),
    it("a", "owner", { position: 1 }),
    it("b", "peer", { position: 2, held: "held by the decision desk until the agent's next break" }),
    it("old", "owner", { state: "answered", turn: 6, disposition: "answered" }),
  ]));
  assert.equal(q.lastSeq, 12);
  assert.deepEqual(q.now.map((n) => [n.id, n.turn]), [["n1", 7]]);
  assert.deepEqual(q.waiting.map((w) => w.id), ["a", "b"]);
  assert.equal(q.waiting[1].held, "held by the decision desk until the agent's next break");
  assert.equal(q.waiting[0].held, null);
  assert.equal(q.waiting[0].at, 1000, "the time sent is the host's accepted_at");
  assert.equal(q.next, null);
});

test("a page of the list says where the next one starts", () => {
  assert.equal(parseQueue(answer([it("a", "owner")], { next: 500 })).next, 500);
});

test("an item with no time sent has none", () => {
  assert.equal(parseQueue(answer([it("a", "owner", { accepted_at: null })])).waiting[0].at, null);
});

test("an answer that is not a queue is refused, not shown as empty", () => {
  assert.throws(() => parseQueue({ contract: 1 }), /queue/);
  assert.throws(() => parseQueue(null), /queue/);
  assert.throws(() => parseQueue(answer([{ id: "x" }])), /queue/);
  assert.throws(() => parseQueue(answer([it("a", "owner", { state: "lost" })])), /queue/);
});

test("an intent the page does not know is shown as none, not guessed", () => {
  const q = parseQueue(answer([it("a", "owner", { intent: "teleport" })]));
  assert.equal(q.waiting[0].intent, null);
});

test("text is read whole and a key in it is redacted", () => {
  const long = "line of a long note\n".repeat(5000);
  const q = parseQueue(answer([it("a", "owner", { text: long }), it("b", "owner", { text: "use sk-or-v1-0123456789abcdef0123456789abcdef now" })]));
  assert.equal(q.waiting[0].text, long);
  assert.ok(!q.waiting[1].text.includes("0123456789abcdef"));
  assert.match(q.waiting[1].text, /\[redacted\]/);
});

test("owner items are their own group on top, numbered on in one run; peer items follow", () => {
  const g = groupWaiting(parseQueue(answer([it("p", "peer"), it("o1", "owner"), it("o2", "owner"), it("h", "host")])).waiting);
  assert.deepEqual(g.owner.map((r) => [r.pos, r.item.id]), [[1, "o1"], [2, "o2"]]);
  assert.deepEqual(g.others.map((r) => [r.pos, r.item.id]), [[3, "p"], [4, "h"]]);
});

test("the words for an intent and for a role are plain", () => {
  assert.equal(intentWord("next"), "Do next");
  assert.equal(intentWord("free"), "When free");
  assert.equal(intentWord("steer"), "Steer");
  assert.equal(intentWord(null), "");
  assert.equal(roleWord({ role: "owner", name: null, channel: "owner" }), "Owner");
  assert.equal(roleWord({ role: "peer", name: "atlas", channel: "peer:atlas" }), "Peer atlas");
  assert.equal(roleWord({ role: "peer", name: null, channel: "peer" }), "Peer");
  assert.equal(roleWord({ role: "host", name: "calendar", channel: "calendar" }), "Host");
});

test("a long text folds in two parts that join to exactly the text; a short one does not fold", () => {
  assert.deepEqual(foldText("short", 280), { head: "short", rest: "" });
  const long = Array.from({ length: 400 }, (_, i) => `w${i}`).join(" ");
  const f = foldText(long, 280);
  assert.ok(f.rest.length > 0);
  assert.equal(f.head + f.rest, long, "nothing is cut");
  assert.ok(f.head.length <= 280);
  assert.ok(!/[…]|\.\.\.$/.test(f.head), "no ellipsis is added");
  assert.ok(/\S$/.test(f.head) && /^\s/.test(f.rest), "the fold falls at a space, not inside a word");
  const solid = "x".repeat(1000);
  const g = foldText(solid, 280);
  assert.equal(g.head + g.rest, solid, "a text with no space folds where the limit is");
  const nl = ("line\n").repeat(100);
  assert.equal(foldText(nl, 50).head + foldText(nl, 50).rest, nl);
});

test("a text of lines folds at the end of a line", () => {
  const lines = Array.from({ length: 60 }, (_, i) => `Item ${i + 1} of the long brief, kept whole.`).join("\n");
  const f = foldText(lines, 280);
  assert.equal(f.head + f.rest, lines);
  assert.ok(f.head.endsWith("kept whole."), "the head ends where a line ends");
  assert.ok(f.rest.startsWith("\n"));
});

test("a message of nothing but white space or invisible characters is empty", () => {
  for (const t of ["", "   ", "\n\t ", "\u200b", "\u200b\u200b \u200d", "\ufeff", "\u00ad", "\u2060", "\u3164", "\u2800", "\u200e\u200f", "\u0000\u0007"]) assert.equal(isBlank(t), true, JSON.stringify(t));
  for (const t of ["a", " a ", "\u200ba\u200b", "0", ".", "é", "好"]) assert.equal(isBlank(t), false, JSON.stringify(t));
});

test("the draft is kept until the door answers: a success clears only what was sent, a failure clears nothing", () => {
  assert.equal(afterSend("hello", "hello", true), "", "sent and unchanged: cleared");
  assert.equal(afterSend("hello and more", "hello", true), "hello and more", "typed on while it was in flight: kept");
  assert.equal(afterSend("hello", "hello", false), "hello", "refused: kept");
  assert.equal(afterSend("", "hello", false), "hello", "refused after the box was emptied some other way: the sent text comes back");
});
