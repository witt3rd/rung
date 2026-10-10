/** The mock gateway: the routes of docs/rung-host-api.md's gateway section (health, instances, the /v1 pass-through that adds
 *  the instance's key and streams, the read-only role, the app) in front of simulated hosts. For the contract tests and for
 *  a live view you can open before the real gateway and the real doors exist. */
import { createServer, request, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, join, normalize } from "node:path";
import { SimHost } from "./host.ts";
import type { Target } from "../test/contract-target.ts";

const json = (res: ServerResponse, status: number, body: unknown) => {
  res.writeHead(status, { "content-type": "application/json" }).end(JSON.stringify(body));
};
const err = (res: ServerResponse, status: number, error: string, message: string) => json(res, status, { error, message });

const types: Record<string, string> = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".ndjson": "text/plain", ".svg": "image/svg+xml" };

/** The host's own listener: /v1 doors behind its key. */
function hostServer(host: SimHost, key: string): Server {
  return createServer(async (req, res) => {
    if (req.headers.authorization !== `Bearer ${key}`) return err(res, 401, "unauthorized", "a key is required");
    const url = new URL(req.url!, "http://host");
    const q = url.searchParams;
    const int = (name: string, dflt: number, min: number): number | null => {
      const v = q.get(name);
      if (v === null) return dflt;
      return /^\d+$/.test(v) && Number(v) >= min ? Number(v) : null;
    };
    if (req.method !== "GET") return err(res, 404, "not_found", "no such door on this mock");
    if (url.pathname === "/v1/summary") return json(res, 200, host.summary());
    if (url.pathname === "/v1/record") {
      const offset = int("offset", 0, 0), limit = int("limit", 500, 1);
      const order = q.get("order") ?? "asc";
      if (offset === null || limit === null || (order !== "asc" && order !== "desc")) return err(res, 400, "bad_request", "offset, limit or order is not valid");
      const total = host.lines.length;
      const view = order === "asc" ? host.lines : [...host.lines].reverse();
      const lines = view.slice(offset, offset + limit);
      return json(res, 200, { lines, offset, limit, total, next: offset + limit < total ? offset + limit : null });
    }
    if (url.pathname === "/v1/events") {
      const header = req.headers["last-event-id"];
      const raw = typeof header === "string" ? header : q.get("after") ?? "0";
      if (!/^\d+$/.test(raw) || Number(raw) > host.lastSeq) return err(res, 400, "bad_request", "after is not a line number this host has");
      return void (await host.follow(res, Number(raw)));
    }
    return err(res, 404, "not_found", "no such door on this mock");
  });
}

interface Inst { id: string; name: string; url: string; key: string }

export interface MockOptions {
  /** Serve this directory as the app (the built ui/dist). */
  appDir?: string;
  port?: number;
  host?: string;
  /** Keep a host running turns on a real clock. */
  live?: boolean;
  /** Also list a free-time instance and a stuck one beside the live one, for the overview. */
  extra?: boolean;
}

export async function startMock(opts: MockOptions = {}): Promise<Target & { hosts: Map<string, SimHost>; url: string }> {
  const KEY = `mock-host-key-${Math.random().toString(36).slice(2)}${Math.random().toString(36).slice(2)}`;
  const READ_TOKEN = `mock-view-token-${Math.random().toString(36).slice(2)}`;
  const hosts = new Map<string, SimHost>();
  const servers: Server[] = [];
  const instances: Inst[] = [];
  const listen = (s: Server, port = 0, host = "127.0.0.1") => new Promise<number>((ok) => s.listen(port, host, () => ok((s.address() as { port: number }).port)));

  const alpha = new SimHost();
  hosts.set("alpha", alpha);
  const as = hostServer(alpha, KEY);
  servers.push(as);
  instances.push({ id: "alpha", name: "alpha", url: `http://127.0.0.1:${await listen(as)}`, key: KEY });
  if (opts.extra) {
    for (const [id, seed] of [["bramble", "bramble"], ["cedar", "cedar"]] as const) {
      const h = new SimHost(seed);
      hosts.set(id, h);
      const hs = hostServer(h, KEY);
      servers.push(hs);
      instances.push({ id, name: id, url: `http://127.0.0.1:${await listen(hs)}`, key: KEY });
    }
  }
  // An instance that is registered and does not answer.
  instances.push({ id: "gone", name: "gone", url: "http://127.0.0.1:9", key: KEY });

  const gateway = createServer(async (req, res) => {
    const url = new URL(req.url ?? "/", "http://gw");
    const path = url.pathname;
    // Who is asking. No token is the owner; a token must be one of the read-only tokens, wherever it appears.
    const bearer = /^Bearer (.+)$/.exec(req.headers.authorization ?? "")?.[1];
    const presented = bearer ?? url.searchParams.get("token") ?? undefined;
    if (presented !== undefined && presented !== READ_TOKEN) return err(res, 401, "unauthorized", "that token is not known");
    const readOnly = presented === READ_TOKEN;

    if (path === "/api/health") return json(res, 200, { ok: true });
    if (path === "/api/instances") {
      const out = await Promise.all(instances.map(async (i) => {
        try {
          const r = await fetch(`${i.url}/v1/summary`, { headers: { authorization: `Bearer ${i.key}` }, signal: AbortSignal.timeout(1000) });
          if (!r.ok) throw new Error(`status ${r.status}`);
          return { id: i.id, name: i.name, reachable: true, summary: await r.json(), error: null };
        } catch (e) { return { id: i.id, name: i.name, reachable: false, summary: null, error: `no answer: ${(e as Error).message}` }; }
      }));
      return json(res, 200, { instances: out });
    }
    const m = /^\/api\/i\/([^/]+)(\/.*)?$/.exec(path);
    if (m) {
      const inst = instances.find((i) => i.id === decodeURIComponent(m[1]));
      if (!inst) return err(res, 404, "no_such_instance", "no instance by that id");
      const rest = m[2] ?? "/";
      if (!rest.startsWith("/v1/")) return err(res, 404, "not_found", "only /v1 passes");
      if (readOnly && req.method !== "GET" && req.method !== "HEAD") return err(res, 403, "read_only", "this token can only read");
      return passThrough(req, res, inst, rest + url.search.replace(/([?&])token=[^&]*&?/, "$1").replace(/[?&]$/, ""));
    }
    if (path.startsWith("/api/")) return err(res, 404, "not_found", "no such route");
    return serveApp(res, path, opts.appDir);
  });
  servers.push(gateway);
  const port = await listen(gateway, opts.port ?? 0, opts.host ?? "127.0.0.1");
  const base = `http://${opts.host ?? "127.0.0.1"}:${port}`;
  if (opts.live) void alpha.live();

  return {
    url: base, instance: "alpha", hostKey: KEY, readOnlyToken: READ_TOKEN, deadInstance: "gone", hosts,
    control: { poke: (n) => alpha.poke(n), flood: (n, b) => alpha.flood(n, b) },
    stop: async () => { for (const h of hosts.values()) h.stop(); for (const s of servers) { s.closeAllConnections(); await new Promise((r) => s.close(r)); } },
  };
}

function passThrough(req: IncomingMessage, res: ServerResponse, inst: Inst, upstreamPath: string) {
  const u = new URL(inst.url);
  const headers: Record<string, string | string[]> = {};
  for (const [k, v] of Object.entries(req.headers)) {
    if (v === undefined || ["host", "connection", "authorization", "content-length", "keep-alive", "transfer-encoding"].includes(k)) continue;
    headers[k] = v;
  }
  headers.authorization = `Bearer ${inst.key}`;
  const up = request({ host: u.hostname, port: u.port, path: upstreamPath, method: req.method, headers }, (ur) => {
    const out: Record<string, string | string[]> = {};
    for (const [k, v] of Object.entries(ur.headers)) if (v !== undefined && !["connection", "keep-alive", "transfer-encoding", "authorization"].includes(k)) out[k] = v;
    res.writeHead(ur.statusCode ?? 502, out);
    res.flushHeaders();
    ur.pipe(res); // backpressure runs back to the host: a reader that does not read slows only its own connection
    ur.on("error", () => res.destroy());
  });
  up.on("error", () => { if (!res.headersSent) err(res, 502, "instance_unreachable", "the instance did not answer"); else res.destroy(); });
  res.on("close", () => up.destroy());
  req.pipe(up);
}

function serveApp(res: ServerResponse, path: string, appDir?: string) {
  if (!appDir) return err(res, 404, "not_found", "this mock serves no app");
  let rel = normalize(decodeURIComponent(path)).replace(/^(\.\.[/\\])+/, "");
  if (rel.split(/[/\\]/).includes("..")) return err(res, 404, "not_found", "no such file");
  if (!extname(rel)) rel = "/index.html";
  const file = join(appDir, rel);
  if (!existsSync(file) || !statSync(file).isFile()) return err(res, 404, "not_found", "no such file");
  res.writeHead(200, { "content-type": types[extname(file)] ?? "application/octet-stream" }).end(readFileSync(file));
}
