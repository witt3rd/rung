/** Prepare the recorded data the observer opens: copies every recorded run's record (and a few synthetic
 *  instances) into a data directory, with a small index of one summary per instance.
 *
 *    node --experimental-strip-types scripts/prepare-data.ts [--out DIR] [--runs DIR] [--no-synthetic] [--now MS]
 *
 *  This stands where a host's read doors will stand (slice 0 has no running host to ask). */
import { mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
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

const out = resolve(arg("--out") ?? join(here, "../public/data"));
const runs = resolve(arg("--runs") ?? join(here, "../../rung-host/live/runs"));
const withSynthetic = !process.argv.includes("--no-synthetic");
const now = Number(arg("--now") ?? Date.now());

// Secrets are named by environment variable in each run's config (`api_key_env`), never held in a file.
// RUNG_UI_REDACT_ENV adds more names, comma separated. A value is read from this process's environment.
const secretNames = new Set<string>((process.env.RUNG_UI_REDACT_ENV ?? "").split(",").map((s) => s.trim()).filter(Boolean));
if (existsSync(runs)) {
  for (const id of readdirSync(runs)) {
    const cfg = join(runs, id, "rung-host.yaml");
    if (!existsSync(cfg)) continue;
    for (const m of readFileSync(cfg, "utf8").matchAll(/^\s*api_key_env:\s*([A-Za-z_][A-Za-z0-9_]*)/gm)) secretNames.add(m[1]);
  }
}
const redact = makeRedactor([...secretNames].map((n) => process.env[n] ?? "").filter(Boolean));

interface Entry { id: string; name: string; kind: "recorded" | "synthetic"; lockHeld: boolean; record: string; summary: unknown }
const entries: Entry[] = [];

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

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
