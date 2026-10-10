// The canary check, written before the redactor: a key value placed in the environment, and a tool that
// prints it into a record. After the data is prepared, the value must occur nowhere in what the page can fetch.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const CANARY = "canary-7f3a91c2e8b4d6051a9c";
const SHAPED = "sk-or-v1-0123456789abcdef0123456789abcdef";

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

test("no key value reaches any prepared file", () => {
  const root = mkdtempSync(join(tmpdir(), "rung-ui-canary-"));
  const run = join(root, "runs", "canary-run");
  mkdirSync(join(run, "state", "record"), { recursive: true });
  writeFileSync(join(run, "rung-host.yaml"), "engine:\n  kind: agent\n  api_key_env: CANARY_KEY\n");
  const lines = [
    { seq: 1, at: 1, kind: "host.start", config: { epoch_budget_tokens: 1000 } },
    { seq: 2, at: 2, kind: "turn.started", turn: 1, turn_kind: "free", epoch: 1, pack_tokens: 10 },
    // A tool made to print the key, in the text and in the messages as sent, inside a JSON-escaped string.
    { seq: 3, at: 3, kind: "tool.call", turn: 1, name: "shell", ok: true, group: "core" },
    { seq: 4, at: 4, kind: "turn.log", turn: 1, header: "h", messages: [
      { role: "assistant", content: [{ type: "tool_use", id: "a", name: "shell", input: { cmd: `echo "$CANARY_KEY"` } }] },
      { role: "user", content: [{ type: "tool_result", tool_use_id: "a", content: `KEY=${CANARY}\nalso ${SHAPED}` }] },
    ] },
    { seq: 5, at: 5, kind: "turn.ended", turn: 1, status: "completed", final_text: `I saw the key ${CANARY} and "${SHAPED}"`, cost: {} },
  ];
  writeFileSync(join(run, "state", "record", "seg-00000001.ndjson"), lines.map((l) => JSON.stringify(l)).join("\n") + "\n");
  const outDir = join(root, "out");
  execFileSync("node", ["--experimental-strip-types", "scripts/prepare-data.ts", "--out", outDir, "--runs", join(root, "runs")],
    { env: { ...process.env, CANARY_KEY: CANARY }, stdio: "pipe" });
  const files = walk(outDir);
  assert.ok(files.length >= 2, "the data was prepared");
  for (const f of files) {
    const text = readFileSync(f, "utf8");
    assert.equal(text.includes(CANARY), false, `canary value in ${f}`);
    assert.equal(text.includes(SHAPED), false, `key-shaped value in ${f}`);
  }
});

test("the recorded runs in the repo carry no key-shaped value into the prepared data", () => {
  const outDir = mkdtempSync(join(tmpdir(), "rung-ui-real-"));
  execFileSync("node", ["--experimental-strip-types", "scripts/prepare-data.ts", "--out", outDir], { stdio: "pipe" });
  const shapes = [/\bsk-[A-Za-z0-9_-]{16,}/, /\bgh[pousr]_[A-Za-z0-9]{20,}/, /\bAKIA[0-9A-Z]{16}\b/, /\bBearer\s+[A-Za-z0-9._~+/=-]{20,}/, /-----BEGIN [A-Z ]*PRIVATE KEY-----/];
  const files = walk(outDir);
  assert.ok(files.length > 5);
  for (const f of files) {
    const text = readFileSync(f, "utf8");
    for (const re of shapes) assert.equal(re.test(text), false, `${re} in ${f}`);
  }
});
