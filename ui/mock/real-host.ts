/** A real host behind a real gateway as a contract target: the built `rung-host sim` (the scripted model and fake world, no network call) with
 *  its owner-token HTTP listener, registered in a registry folder, and the built `rung-gateway` naming it with the key it holds for it. This is
 *  the only target that runs the queue's write path end to end.
 *    cargo build -p rung-host -p rung-gateway && RUNG_CONTRACT_TARGET=real-host npm run test:contract:queue
 *  The paths of the two binaries come from RUNG_HOST_BIN and RUNG_GATEWAY_BIN (default: ../target/debug). The host's calls are slowed so
 *  that a message sent to it waits for a boundary long enough to be seen waiting. */
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { randomBytes } from "node:crypto";
import type { Target } from "../test/contract-target.ts";

const here = resolve(import.meta.dirname);

export async function startRealHost(opts: { appDir?: string; port?: number } = {}): Promise<Target & { root: string }> {
  const hostBin = process.env.RUNG_HOST_BIN ?? join(here, "../../target/debug/rung-host");
  const gwBin = process.env.RUNG_GATEWAY_BIN ?? join(here, "../../target/debug/rung-gateway");
  for (const b of [hostBin, gwBin]) if (!existsSync(b)) throw new Error(`no binary at ${b}: run \`cargo build -p rung-host -p rung-gateway\` or set RUNG_HOST_BIN and RUNG_GATEWAY_BIN`);
  const scratch = join(here, "../.test-tmp");
  mkdirSync(scratch, { recursive: true });
  const root = mkdtempSync(join(scratch, "realhost-"));
  const home = join(root, "home");
  mkdirSync(home);
  const ownerKey = `owner-${randomBytes(12).toString("hex")}`;
  const viewKey = `view-${randomBytes(12).toString("hex")}`;
  const children: ChildProcess[] = [];
  const retire = () => { for (const c of children) { try { c.kill("SIGKILL"); } catch { /* gone */ } } rmSync(root, { recursive: true, force: true }); };
  const env = { ...process.env, RUNG_HOME: home, RUNG_CONTRACT_OWNER_KEY: ownerKey, RUNG_GATEWAY_VIEW_TOKEN: viewKey };

  const waitFor = (child: ChildProcess, re: RegExp, what: string) => new Promise<string>((ok, bad) => {
    let out = "";
    const t = setTimeout(() => bad(new Error(`${what} did not say where it listens: ${out}`)), 20000);
    const on = (b: Buffer) => { out += b; const m = re.exec(out); if (m) { clearTimeout(t); ok(m[1]); } };
    child.stdout?.on("data", on); child.stderr?.on("data", on);
    child.on("exit", (c) => { clearTimeout(t); bad(new Error(`${what} exited ${c}: ${out}`)); });
  });

  try {
    const host = spawn(hostBin, ["sim", "--state", join(root, "state"), "--seed", "1", "--turns", "100000", "--clock", "real", "--call-ms", "20000,30000", "--name", "alpha",
      "--acp-http", "127.0.0.1:0", "--acp-token-env", "owner=RUNG_CONTRACT_OWNER_KEY"], { env, stdio: ["ignore", "pipe", "pipe"] });
    children.push(host);
    const hostUrl = await waitFor(host, /ACP HTTP at (http:\/\/\S+?)\/acp/, "the host");
    writeFileSync(join(root, "gateway.yaml"),
      `listen: 127.0.0.1:${opts.port ?? 0}\n${opts.appDir ? `app_dir: ${opts.appDir}\n` : ""}read_only_token_envs: [RUNG_GATEWAY_VIEW_TOKEN]\nregistry_dir: ${join(home, "instances")}\ninstances:\n  - id: alpha\n    name: alpha\n    url: ${hostUrl}\n    key_env: RUNG_CONTRACT_OWNER_KEY\n`);
    const gw = spawn(gwBin, ["--config", join(root, "gateway.yaml")], { env, stdio: ["ignore", "pipe", "pipe"] });
    children.push(gw);
    const url = await waitFor(gw, /listening on (http:\/\/\S+)/, "the gateway");
    // The host must have written its first lines before a test reads the record.
    for (let i = 0; i < 100; i++) {
      const r = await fetch(`${url}/api/i/alpha/v1/summary`).catch(() => null);
      if (r?.ok && (await r.json()).last_seq >= 10) break;
      await new Promise((r) => setTimeout(r, 200));
    }
    return {
      url, root, instance: "alpha", hostKey: ownerKey, readOnlyToken: viewKey, deadInstance: null, deltas: false,
      control: {
        poke: async () => {}, flood: async () => {}, plant: async () => {}, turn: async () => {},
        // The host ends: its lock is released and the gateway reads the instance as not running.
        halt: async () => { host.kill("SIGKILL"); await new Promise((r) => host.once("exit", r)); await new Promise((r) => setTimeout(r, 300)); },
      },
      stop: async () => { retire(); },
    };
  } catch (e) {
    retire();
    throw e;
  }
}
