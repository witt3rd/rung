import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
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
  const mine = mkdtempSync(join(tmpdir(), "rung-ui-real-target-"));
  try {
    process.env.RUNG_GATEWAY_BIN = join(import.meta.dirname, "../package.json"); // not executable: the gateway cannot be launched
    await assert.rejects(startReal({ scratch: mine }));
    delete process.env.RUNG_GATEWAY_BIN;
    assert.deepEqual(readdirSync(mine), [], "no scratch tree left by the failed start");
  } finally {
    delete process.env.RUNG_GATEWAY_BIN;
    rmSync(mine, { recursive: true, force: true });
  }
});
