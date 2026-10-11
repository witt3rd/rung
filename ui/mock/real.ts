/** The real gateway as a contract target: the built `rung-gateway` binary, over a registry folder and a state directory this file
 *  writes. The gateway never writes and never asks a host, so this file plays the host's part for the record: it appends lines to the
 *  segment file, whole and numbered, as a host would. What a running host adds (the event stream, partial frames) is not here: a check
 *  that needs it is skipped, by name, until the gateway has the door.
 *    cargo build -p rung-gateway && RUNG_CONTRACT_TARGET=real npm run test:contract */
import { spawn } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { synthetic } from "../src/synthetic.ts";
import type { Target } from "../test/contract-target.ts";

const here = resolve(import.meta.dirname);

export interface RealOptions {
  /** Serve this directory as the app (the built ui/dist). */
  appDir?: string;
  /** Also register a stopped instance with a config file (so it has a start command), a dead one, and an entry that cannot be read. */
  extra?: boolean;
  port?: number;
  host?: string;
}

export async function startReal(opts: RealOptions = {}): Promise<Target & { root: string }> {
  const bin = process.env.RUNG_GATEWAY_BIN ?? join(here, "../../target/debug/rung-gateway");
  if (!existsSync(bin)) throw new Error(`no gateway binary at ${bin}: run \`cargo build -p rung-gateway\` or set RUNG_GATEWAY_BIN`);
  const scratch = join(here, "../.test-tmp");
  mkdirSync(scratch, { recursive: true });
  const root = mkdtempSync(join(scratch, "real-"));
  const state = join(root, "state");
  const record = join(state, "record");
  mkdirSync(record, { recursive: true });
  mkdirSync(join(root, "reg"));
  const seg = join(record, "seg-00000001.ndjson");
  const seed = synthetic(Date.now()).find((i) => i.id === "atlas")!.lines;
  writeFileSync(seg, seed.map((l) => JSON.stringify(l)).join("\n") + "\n");
  let seq = seed.length;
  writeFileSync(join(root, "reg", "alpha.json"), JSON.stringify({
    schema: 1, id: "alpha", name: "alpha", state_dir: state, config: null, workspace: join(root, "ws"), address: null, pid: 1, started: Date.now(), version: "contract",
  }));
  if (opts.extra) {
    for (const [id, config] of [["gorse", true], ["fir", false]] as const) {
      const dir = join(root, `state-${id}`);
      mkdirSync(join(dir, "record"), { recursive: true });
      const lines = synthetic(Date.now()).find((i) => i.id === id)!.lines;
      writeFileSync(join(dir, "record", "seg-00000001.ndjson"), lines.map((l) => JSON.stringify(l)).join("\n") + "\n");
      const cfg = join(root, `${id}.yaml`);
      if (config) writeFileSync(cfg, "engine:\n  kind: mock\n");
      writeFileSync(join(root, "reg", `${id}.json`), JSON.stringify({ schema: 1, id, name: id, state_dir: dir, config: config ? cfg : null, workspace: join(root, "ws"), address: null, pid: 1, started: Date.now(), version: "contract" }));
    }
    writeFileSync(join(root, "reg", "bad.json"), "{ this is not json");
  }
  const token = `view-token-${Math.random().toString(36).slice(2)}`;
  writeFileSync(join(root, "gateway.yaml"), `listen: ${opts.host ?? "127.0.0.1"}:${opts.port ?? 0}\n${opts.appDir ? `app_dir: ${opts.appDir}\n` : ""}read_only_token_envs: [RUNG_GATEWAY_VIEW_TOKEN]\nregistry_dir: ${join(root, "reg")}\ninstances: []\n`);
  const child = spawn(bin, ["--config", join(root, "gateway.yaml")], { env: { ...process.env, RUNG_GATEWAY_VIEW_TOKEN: token }, stdio: ["ignore", "pipe", "pipe"] });
  const url = await new Promise<string>((ok, bad) => {
    let out = "", err = "";
    const t = setTimeout(() => bad(new Error(`the gateway did not say where it listens: ${out}${err}`)), 15000);
    child.stdout.on("data", (b) => { out += b; const m = /listening on (http:\/\/\S+)/.exec(out); if (m) { clearTimeout(t); ok(m[1]); } });
    child.stderr.on("data", (b) => { err += b; });
    child.on("exit", (c) => { clearTimeout(t); bad(new Error(`the gateway exited ${c}: ${out}${err}`)); });
  });
  const write = (kind: string, fields: Record<string, unknown>) => {
    appendFileSync(seg, JSON.stringify({ at: Date.now(), kind, seq: ++seq, ...fields }) + "\n");
  };
  return {
    url, root, instance: "alpha", hostKey: null, readOnlyToken: token, deadInstance: null, deltas: false,
    control: {
      poke: async (n) => { for (let i = 0; i < n; i++) write("note.written", { turn: 99, text: `a note, number ${i + 1}` }); },
      flood: async (n, bytes) => { const text = "x".repeat(bytes); for (let i = 0; i < n; i++) write("note.written", { turn: 99, text }); },
      plant: async (key) => { write("note.written", { turn: 99, text: `printed ${key}` }); },
    },
    stop: async () => { child.kill("SIGTERM"); },
  };
}
