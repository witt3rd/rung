/** Prepare the recorded data the observer opens: copies every recorded run's record (and a few synthetic
 *  instances) into a data directory, with a small index of one summary per instance.
 *
 *    node --experimental-strip-types scripts/prepare-data.ts [--out DIR] [--runs DIR] [--no-synthetic] [--now MS]
 *                                                              [--allow-unset-keys]
 *
 *  --out is emptied before it is written, so it must be a directory this script owns: strictly inside ui/public,
 *  ui/dist or ui/.test-tmp, and either empty/absent or marked by an earlier run. Anything else is refused.
 *  A key variable named in a run's config but unset here means exact-value redaction cannot run for that key:
 *  that is refused (exit 2, naming the variable) unless --allow-unset-keys says the shapes-only pass is intended.
 *
 *  This stands where a host's read doors will stand (slice 0 has no running host to ask). */
import { mkdirSync, readdirSync, readFileSync, realpathSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { parseRecord } from "../src/record/parse.ts";
import { summarize } from "../src/record/folds.ts";
import { makeRedactor } from "../src/record/redact.ts";
import { synthetic } from "../src/synthetic.ts";
import type { Line } from "../src/record/types.ts";

const here = resolve(import.meta.dirname);
const arg = (name: string): string | undefined => {
  const i = process.argv.indexOf(name);
  return i >= 0 ? process.argv[i + 1] : undefined;
};

const refuse = (why: string): never => {
  console.error(`prepare-data: ${why}`);
  process.exit(2);
};

const uiRoot = realpathSync(resolve(here, ".."));
// The real path of the nearest existing ancestor, then the rest: a symlink cannot carry --out out of the allowed roots.
function realish(p: string): string {
  const rest: string[] = [];
  let cur = resolve(p);
  while (!existsSync(cur)) { rest.unshift(cur.slice(dirname(cur).length + 1)); cur = dirname(cur); }
  return join(realpathSync(cur), ...rest);
}
const out = realish(arg("--out") ?? join(here, "../public/data"));
const MARK = ".rung-ui-data";
const owned = ["public", "dist", ".test-tmp"].map((d) => join(uiRoot, d) + sep);
if (!owned.some((root) => out.startsWith(root))) {
  refuse(`--out ${out} is not a directory this script may empty; it must be inside ui/public, ui/dist or ui/.test-tmp`);
}
if (existsSync(out) && readdirSync(out).length > 0 && !existsSync(join(out, MARK))) {
  refuse(`--out ${out} has files and no ${MARK} mark from an earlier run; refusing to empty it`);
}
const runs = resolve(arg("--runs") ?? join(here, "../../rung-host/live/runs"));
const withSynthetic = !process.argv.includes("--no-synthetic");
const now = Number(arg("--now") ?? Date.now());

// Secrets are named by environment variable in each run's config (`api_key_env`), never held in a file.
// RUNG_REDACT_ENVS (rung's one surface for this, shared with the agent crates) adds more names, comma separated.
// A value is read from this process's environment.
const secretNames = new Set<string>((process.env.RUNG_REDACT_ENVS ?? "").split(",").map((s) => s.trim()).filter(Boolean));
if (existsSync(runs)) {
  for (const id of readdirSync(runs)) {
    const cfg = join(runs, id, "rung-host.yaml");
    if (!existsSync(cfg)) continue;
    for (const m of readFileSync(cfg, "utf8").matchAll(/^\s*api_key_env:\s*([A-Za-z_][A-Za-z0-9_]*)/gm)) secretNames.add(m[1]);
  }
}
// The well-known provider key variables the agent crates always redact: redacted here when set, never required.
const WELL_KNOWN = ["OPENROUTER_API_KEY", "ANTHROPIC_API_KEY", "OPENAI_API_KEY", "RUNG_API_KEY", "XAI_API_KEY"];
const unset = [...secretNames].filter((n) => !process.env[n]);
if (unset.length) {
  const msg = `the key variable${unset.length > 1 ? "s" : ""} ${unset.join(", ")} ${unset.length > 1 ? "are" : "is"} named by a run's config but not set here, so exact-value redaction cannot run for ${unset.length > 1 ? "them" : "it"}; only key shapes are removed`;
  if (!process.argv.includes("--allow-unset-keys")) refuse(`${msg}. Set ${unset.length > 1 ? "them" : "it"}, or pass --allow-unset-keys to accept the shapes-only pass.`);
  console.warn(`prepare-data: WARNING: ${msg}.`);
}
const redact = makeRedactor([...new Set([...secretNames, ...WELL_KNOWN])].map((n) => process.env[n] ?? "").filter(Boolean));

interface Entry { id: string; name: string; kind: "recorded" | "synthetic"; lockHeld: boolean; record: string; summary: unknown }
const entries: Entry[] = [];

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
writeFileSync(join(out, MARK), "written by ui/scripts/prepare-data.ts; safe to empty\n");

function put(id: string, kind: Entry["kind"], lockHeld: boolean, lines: Line[]) {
  mkdirSync(join(out, id), { recursive: true });
  writeFileSync(join(out, id, "record.ndjson"), lines.map((l) => JSON.stringify(redact(l))).join("\n") + "\n");
  entries.push({ id, name: id, kind, lockHeld, record: `${id}/record.ndjson`, summary: summarize(lines.map((l) => redact(l) as Line), { lockHeld, now }) });
}

if (existsSync(runs)) {
  for (const id of readdirSync(runs).sort()) {
    const dir = join(runs, id, "state", "record");
    if (!existsSync(dir)) continue;
    const segs = readdirSync(dir).filter((f) => f.endsWith(".ndjson")).sort();
    const lines = segs.flatMap((f) => parseRecord(readFileSync(join(dir, f), "utf8")));
    if (lines.length) put(id, "recorded", false, lines);
  }
}
if (withSynthetic) for (const s of synthetic(now)) put(s.id, "synthetic", s.lockHeld, s.lines);

writeFileSync(join(out, "index.json"), JSON.stringify({ generatedAt: now, instances: entries }));
console.log(`prepared ${entries.length} instances in ${out}`);
