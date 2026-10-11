import { test } from "node:test";
import assert from "node:assert/strict";
import { StoreRegistry } from "../src/live/registry.ts";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
function stub() {
  const made: { id: string; started: number; stopped: number }[] = [];
  const reg = new StoreRegistry((id) => { const s = { id, started: 0, stopped: 0, start() { this.started++; }, stop() { this.stopped++; } }; made.push(s); return s; }, 40);
  return { reg, made };
}

test("two pages on one instance share one store, started once", () => {
  const { reg, made } = stub();
  const a = reg.acquire("alpha"), b = reg.acquire("alpha");
  assert.equal(a, b);
  assert.equal(made.length, 1);
  assert.equal(made[0].started, 1);
});

test("moving between an instance's tabs keeps the store: released and taken again within the grace", async () => {
  const { reg, made } = stub();
  const first = reg.acquire("alpha");
  reg.release("alpha");
  await sleep(10);
  assert.equal(reg.acquire("alpha"), first, "the same store, not a new load");
  await sleep(80);
  assert.equal(made[0].stopped, 0);
  assert.equal(made.length, 1);
});

test("an instance nobody shows is stopped and let go after the grace, and another is unaffected", async () => {
  const { reg, made } = stub();
  reg.acquire("alpha"); reg.acquire("beta");
  reg.release("alpha");
  await sleep(100);
  assert.equal(made.find((m) => m.id === "alpha")!.stopped, 1);
  assert.equal(made.find((m) => m.id === "beta")!.stopped, 0);
  assert.equal(reg.size, 1);
  const again = reg.acquire("alpha");
  assert.equal(made.filter((m) => m.id === "alpha").length, 2, "a later visit makes a fresh store");
  assert.ok(again);
});

test("peek shows a held store without holding it", async () => {
  const { reg, made } = stub();
  assert.equal(reg.peek("alpha"), undefined);
  const s = reg.acquire("alpha");
  assert.equal(reg.peek("alpha"), s);
  reg.release("alpha");
  assert.equal(reg.peek("alpha"), s, "still held during the grace");
  await sleep(100);
  assert.equal(reg.peek("alpha"), undefined);
  assert.equal(made[0].stopped, 1);
});

test("a store another page still holds is not stopped", async () => {
  const { reg, made } = stub();
  reg.acquire("alpha"); reg.acquire("alpha");
  reg.release("alpha");
  await sleep(100);
  assert.equal(made[0].stopped, 0);
});

test("a release that follows a re-acquire gets its own full grace, not what is left of the first", async () => {
  const { reg, made } = stub();
  reg.acquire("alpha");
  reg.release("alpha"); // the first grace starts: 40 ms
  await sleep(25);
  reg.acquire("alpha");
  reg.release("alpha"); // the second starts here and ends at 65 ms
  await sleep(25); // 50 ms in: the first grace would have ended; the second has not
  assert.equal(made[0].stopped, 0, "not stopped on the first release's clock");
  await sleep(60);
  assert.equal(made[0].stopped, 1);
});
