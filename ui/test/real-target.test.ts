import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { startReal } from "../mock/real.ts";

const BIN = process.env.RUNG_GATEWAY_BIN ?? join(import.meta.dirname, "../../target/debug/rung-gateway");

// What the real-gateway harness starts it retires: the gateway process and the scratch tree it wrote.
test("stopping the real-gateway target removes its scratch tree and ends the gateway", { skip: existsSync(BIN) ? false : "the gateway binary is not built" }, async () => {
  const t = await startReal();
  const root = t.root;
  assert.ok(existsSync(root), "there is a tree to retire");
  assert.equal((await fetch(`${t.url}/api/health`)).status, 200);
  await t.stop();
  assert.equal(existsSync(root), false, "the scratch tree is gone");
  await assert.rejects(fetch(`${t.url}/api/health`), "the gateway no longer answers");
});

test("a harness that cannot start the gateway leaves nothing behind", { skip: existsSync(BIN) ? false : "the gateway binary is not built" }, async () => {
  const before = (await import("node:fs")).readdirSync(join(import.meta.dirname, "../.test-tmp")).filter((f) => f.startsWith("real-")).length;
  // A registry the gateway refuses (a read-only token variable it cannot read is a start error): the gateway exits at once.
  process.env.RUNG_GATEWAY_BIN = join(import.meta.dirname, "../package.json"); // not executable: spawn fails
  await assert.rejects(startReal());
  delete process.env.RUNG_GATEWAY_BIN;
  const after = (await import("node:fs")).readdirSync(join(import.meta.dirname, "../.test-tmp")).filter((f) => f.startsWith("real-")).length;
  assert.equal(after, before, "no scratch tree left by the failed start");
});
