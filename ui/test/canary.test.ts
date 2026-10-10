// The canary check, written before the redactor: a key value placed in the environment, and a tool that
// prints it into a record. After the data is prepared, the value must occur nowhere in what the page can fetch.
// Also the refusals of scripts/prepare-data.ts: an unset key variable, and an --out it does not own.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const UI = join(import.meta.dirname, "..");
const SCRATCH = join(UI, ".test-tmp"); // inside ui/, where prepare-data may write
mkdirSync(SCRATCH, { recursive: true });

const CANARY = "canary-7f3a91c2e8b4d6051a9c";
const SHAPED = "sk-or-v1-0123456789abcdef0123456789abcdef";

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

function prepare(args: string[], env: Record<string, string | undefined> = {}) {
  const e = { ...process.env, ...env };
  for (const k of Object.keys(e)) if (e[k] === undefined) delete e[k];
  const r = spawnSync("node", ["--experimental-strip-types", "scripts/prepare-data.ts", ...args], { cwd: UI, env: e as NodeJS.ProcessEnv, encoding: "utf8" });
  return { status: r.status, out: r.stdout, err: r.stderr };
}

/** A run directory whose config names CANARY_KEY, with a tool that printed it. */
function canaryRuns() {
  const root = mkdtempSync(join(SCRATCH, "canary-"));
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
  return { root, runs: join(root, "runs"), out: join(root, "out") };
}

test("no key value reaches any prepared file", () => {
  const c = canaryRuns();
  const r = prepare(["--out", c.out, "--runs", c.runs], { CANARY_KEY: CANARY });
  assert.equal(r.status, 0, r.err);
  const files = walk(c.out);
  assert.ok(files.length >= 2, "the data was prepared");
  for (const f of files) {
    const text = readFileSync(f, "utf8");
    assert.equal(text.includes(CANARY), false, `canary value in ${f}`);
    assert.equal(text.includes(SHAPED), false, `key-shaped value in ${f}`);
  }
});

test("a key variable named by a config but not set is refused, by name", () => {
  const c = canaryRuns();
  const r = prepare(["--out", c.out, "--runs", c.runs], { CANARY_KEY: undefined });
  assert.equal(r.status, 2, "non-zero exit");
  assert.match(r.err, /CANARY_KEY/, "the message names the variable");
  assert.match(r.err, /exact-value redaction cannot run/);
  assert.equal(existsSync(c.out), false, "nothing was written");
});

test("--allow-unset-keys accepts the shapes-only pass, loudly, and the shapes are still removed", () => {
  const c = canaryRuns();
  const r = prepare(["--out", c.out, "--runs", c.runs, "--allow-unset-keys"], { CANARY_KEY: undefined });
  assert.equal(r.status, 0, r.err);
  assert.match(r.err, /WARNING.*CANARY_KEY/);
  for (const f of walk(c.out)) assert.equal(readFileSync(f, "utf8").includes(SHAPED), false, `key-shaped value in ${f}`);
});

test("the recorded runs: key shapes are gone, and exact-value redaction really runs on them", () => {
  const shapes = [/\bsk-[A-Za-z0-9_-]{16,}/, /\bgh[pousr]_[A-Za-z0-9]{20,}/, /\bAKIA[0-9A-Z]{16}\b/, /\bBearer\s+[A-Za-z0-9._~+/=-]{20,}/, /-----BEGIN [A-Z ]*PRIVATE KEY-----/];
  const out = mkdtempSync(join(SCRATCH, "real-"));
  // Plant as a "secret" a phrase the real records do contain. If exact-value redaction is skipped, it stays and this fails.
  const PLANT = "Holding — e10 due";
  const before = readdirSync(join(UI, "../rung-host/live/runs")).map((id) => join(UI, "../rung-host/live/runs", id, "state/record/seg-00000001.ndjson")).filter(existsSync)
    .some((f) => readFileSync(f, "utf8").includes(PLANT));
  assert.ok(before, "the planted phrase is in the real records, so this test can fail");
  // Every variable the real configs name is set (to unrelated values) so the run needs no flag; RUNG_UI_REDACT_ENV adds the plant.
  const r = prepare(["--out", join(out, "d")], { OPENROUTER_API_KEY: "unrelated-value-1", RUNG_HOST_OPENROUTER_API_KEY: "unrelated-value-2", RUNG_UI_PLANT: PLANT, RUNG_UI_REDACT_ENV: "RUNG_UI_PLANT" });
  assert.equal(r.status, 0, r.err);
  const files = walk(join(out, "d"));
  assert.ok(files.length > 5);
  for (const f of files) {
    const text = readFileSync(f, "utf8");
    assert.equal(text.includes(PLANT), false, `planted value survived in ${f}: exact-value redaction was skipped`);
    for (const re of shapes) assert.equal(re.test(text), false, `${re} in ${f}`);
  }
});

test("the recorded runs are refused when their configs name key variables that are unset", () => {
  const out = mkdtempSync(join(SCRATCH, "real-unset-"));
  const r = prepare(["--out", join(out, "d")], { OPENROUTER_API_KEY: undefined, RUNG_HOST_OPENROUTER_API_KEY: undefined, RUNG_UI_REDACT_ENV: undefined });
  assert.equal(r.status, 2, r.err);
  assert.match(r.err, /OPENROUTER_API_KEY/);
});

test("--out outside what the script owns is refused and nothing in it is touched", () => {
  const sentinelDir = (base: string) => {
    const d = mkdtempSync(join(base, "keep-"));
    writeFileSync(join(d, "precious.txt"), "do not delete");
    return d;
  };
  const args = (out: string) => ["--out", out, "--no-synthetic", "--runs", join(SCRATCH, "none")];
  // Outside the repo.
  const outside = sentinelDir(tmpdir());
  let r = prepare(args(outside));
  assert.equal(r.status, 2);
  assert.match(r.err, /not a directory this script may empty/);
  assert.equal(existsSync(join(outside, "precious.txt")), true);
  // Inside the repo but not an owned root: the source tree, and the repository itself.
  for (const bad of [join(UI, "src"), join(UI, "test"), UI, join(UI, "..")]) {
    r = prepare(args(bad));
    assert.equal(r.status, 2, `${bad}: ${r.err}`);
  }
  assert.equal(existsSync(join(UI, "src/main.tsx")), true);
  // An owned root, but a directory with files and no mark from an earlier run.
  const unmarked = sentinelDir(SCRATCH);
  r = prepare(args(unmarked));
  assert.equal(r.status, 2);
  assert.match(r.err, /no \.rung-ui-data mark/);
  assert.equal(existsSync(join(unmarked, "precious.txt")), true);
  // A link that leads out of the owned roots.
  const link = join(SCRATCH, `link-${process.pid}`);
  symlinkSync(outside, link);
  r = prepare(args(link));
  assert.equal(r.status, 2);
  assert.equal(existsSync(join(outside, "precious.txt")), true);
  // And an owned, marked directory is emptied and rewritten.
  const mine = join(SCRATCH, `mine-${process.pid}`);
  assert.equal(prepare(args(mine)).status, 0);
  writeFileSync(join(mine, "stale.txt"), "x");
  assert.equal(prepare(args(mine)).status, 0);
  assert.equal(existsSync(join(mine, "stale.txt")), false);
});
