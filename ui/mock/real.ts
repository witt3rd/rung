/** The real gateway as a contract target: the built `rung-gateway` binary, over a registry folder and a state directory this file
 *  writes. The gateway never writes and never asks a host, so this file plays the host's part for the record: it appends lines to the
 *  segment file, whole and numbered, as a host would. What a running host adds (the event stream, partial frames) is not here: a check
 *  that needs it is skipped, by name, until the gateway has the door.
 *    cargo build -p rung-gateway && RUNG_CONTRACT_TARGET=real npm run test:contract */
import { spawn } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
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
  /** Where the scratch tree is made (default: ui/.test-tmp). A test gives its own, so what a failed start leaves is its alone to count. */
  scratch?: string;
}

export async function startReal(opts: RealOptions = {}): Promise<Target & { root: string }> {
  const bin = process.env.RUNG_GATEWAY_BIN ?? join(here, "../../target/debug/rung-gateway");
  if (!existsSync(bin)) throw new Error(`no gateway binary at ${bin}: run \`cargo build -p rung-gateway\` or set RUNG_GATEWAY_BIN`);
  const scratch = opts.scratch ?? join(here, "../.test-tmp");
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
  let turnsWritten = 0;
  writeFileSync(join(root, "reg", "alpha.json"), JSON.stringify({
    schema: 1, id: "alpha", name: "alpha", state_dir: state, config: null, workspace: join(root, "ws"), address: null, pid: 1, started: Date.now(), version: "contract",
  }));
  if (opts.extra) {
    for (const [id, config] of [["gorse", true], ["fir", false]] as const) {
      const dir = join(root, `state-${id}`);
      mkdirSync(join(dir, "record"), { recursive: true });
      const lines = synthetic(Date.now()).find((i) => i.id === id)!.lines;
      writeFileSync(join(dir, "record", "seg-00000001.ndjson"), lines.map((l) => JSON.stringify(l)).join("\n") + "\n");
      // The entry names a config file by an invented, neutral path: the page shows the start command it gives, and a capture of it must not
      // show where this machine keeps its files. The gateway only needs the path to build the command.
      const cfg = `/srv/${id}/rung-host.yaml`;
      writeFileSync(join(root, "reg", `${id}.json`), JSON.stringify({ schema: 1, id, name: id, state_dir: dir, config: config ? cfg : null, workspace: join(root, "ws"), address: null, pid: 1, started: Date.now(), version: "contract" }));
    }
    writeFileSync(join(root, "reg", "bad.json"), "{ this is not json");
  }
  const token = `view-token-${Math.random().toString(36).slice(2)}`;
  writeFileSync(join(root, "gateway.yaml"), `listen: ${opts.host ?? "127.0.0.1"}:${opts.port ?? 0}\n${opts.appDir ? `app_dir: ${opts.appDir}\n` : ""}read_only_token_envs: [RUNG_GATEWAY_VIEW_TOKEN]\nregistry_dir: ${join(root, "reg")}\ninstances: []\n`);
  // What this starts it retires: the gateway process and the scratch tree, on the error path too.
  const retire = () => rmSync(root, { recursive: true, force: true });
  // A host holds its state directory's lock for its whole life (H1); the gateway reads that lock to tell a running instance from a stopped one.
  // `flock` holds it for this harness, which plays the host's part for the record. Nothing else of a host is here.
  const holders: ReturnType<typeof spawn>[] = [];
  const hold = (dir: string) => { const h = spawn("flock", ["-x", join(dir, "host.lock"), "sleep", "100000"], { stdio: "ignore", detached: true }); holders.push(h); return h; };
  // flock runs `sleep` as a child that holds the lock too: end the whole group, or the lock outlives the harness.
  const kill = (h: ReturnType<typeof spawn>) => { try { process.kill(-h.pid!, "SIGKILL"); } catch { /* already gone */ } };
  const lock = hold(state);
  await new Promise((r) => setTimeout(r, 200));
  const child = spawn(bin, ["--config", join(root, "gateway.yaml")], { env: { ...process.env, RUNG_GATEWAY_VIEW_TOKEN: token }, stdio: ["ignore", "pipe", "pipe"] });
  let url: string;
  try {
    url = await new Promise<string>((ok, bad) => {
    let out = "", err = "";
    const t = setTimeout(() => bad(new Error(`the gateway did not say where it listens: ${out}${err}`)), 15000);
    child.stdout.on("data", (b) => { out += b; const m = /listening on (http:\/\/\S+)/.exec(out); if (m) { clearTimeout(t); ok(m[1]); } });
    child.stderr.on("data", (b) => { err += b; });
    child.on("error", (e) => { clearTimeout(t); bad(new Error(`the gateway could not be started: ${e.message}`)); });
    child.on("exit", (c) => { clearTimeout(t); bad(new Error(`the gateway exited ${c}: ${out}${err}`)); });
    });
  } catch (e) {
    child.kill("SIGKILL");
    for (const h of holders) kill(h);
    retire();
    throw e;
  }
  const write = (kind: string, fields: Record<string, unknown>) => {
    appendFileSync(seg, JSON.stringify({ at: Date.now(), kind, seq: ++seq, ...fields }) + "\n");
  };
  return {
    url, root, instance: "alpha", hostKey: null, readOnlyToken: token, deadInstance: null, deltas: false,
    control: {
      poke: async (n) => { for (let i = 0; i < n; i++) write("note.written", { turn: 99, text: `a note, number ${i + 1}` }); },
      flood: async (n, bytes) => { const text = "x".repeat(bytes); for (let i = 0; i < n; i++) write("note.written", { turn: 99, text }); },
      plant: async (key) => { write("note.written", { turn: 99, text: `printed ${key}` }); },
      writeTurn: async (text) => { const n = (seed.filter((l) => l.kind === "turn.started").length) + (turnsWritten++) + 1; const t0 = Date.now(); write("boundary", { n, mode: "free", pending: 0 }); write("turn.started", { turn: n, turn_kind: "free", mode: "free", model: "model-a", rung: 0, epoch: 1, pack_tokens: 30000, header_tokens: 100, enabled: ["core"], project: null }); write("tool.call", { turn: n, name: "ws_list", group: "read", ok: true }); write("turn.log", { turn: n, header: `[turn ${n}]`, messages: [{ role: "assistant", content: text }], recall: false }); write("turn.ended", { turn: n, turn_kind: "free", status: "completed", calls: 1, elapsed_ms: Date.now() - t0, model: "model-a", rung: 0, final_text: text, cost: { cached: 0, calls: 1, cost_usd: 0, prompt: 0 } }); },
      turn: async () => { throw new Error("this target runs no turn: it has no host to stream partial frames"); },
      halt: async () => { write("halted", { why: { by: "limit", why: "stopped" } }); kill(lock); await new Promise((r) => setTimeout(r, 200)); },
    },
    stop: async () => {
      if (child.exitCode === null) {
        const gone = new Promise<void>((r) => child.once("exit", () => r()));
        child.kill("SIGTERM");
        await Promise.race([gone, new Promise((r) => setTimeout(r, 5000))]);
        if (child.exitCode === null) child.kill("SIGKILL");
      }
      for (const h of holders) kill(h);
      retire();
    },
  };
}
