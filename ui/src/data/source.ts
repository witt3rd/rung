/** Where the observer gets its data. Slice 0 reads recorded files served beside the app; a running host's
 *  doors (docs/rung-host-api.md) replace this one module, and nothing above it changes. */
import { parseRecord } from "../record/parse.ts";
import { makeRedactor } from "../record/redact.ts";
import type { Line } from "../record/types.ts";
import type { Summary } from "../record/folds.ts";
import type { HostSummary } from "../live/store.ts";

export interface InstanceEntry {
  id: string;
  name: string;
  kind: "recorded" | "synthetic" | "live";
  /** Stands for the lock on the state directory. A recorded run has no process, so false. */
  lockHeld: boolean;
  /** A live instance that did not answer is listed, with nothing known about it but that. Recorded ones always are. */
  reachable?: boolean;
  record: string;
  summary: Summary;
}
export interface InstanceIndex { generatedAt: number; instances: InstanceEntry[] }

async function get(url: string): Promise<Response> {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`${url}: ${r.status}`);
  return r;
}

export const loadIndex = async (): Promise<InstanceIndex> => (await get("data/index.json")).json();

// The data is redacted where it is prepared; the shapes of known keys are removed again here, so a page
// never shows one whatever served the data.
const redact = makeRedactor([]);

export async function loadRecord(entry: InstanceEntry): Promise<Line[]> {
  const text = await (await get(`data/${entry.record}`)).text();
  return parseRecord(text).map((l) => redact(l) as Line);
}

// -- A running gateway ------------------------------------------------------------------------------------------

/** Is a gateway answering beside the app? Then the data is live; otherwise it is the recorded files. */
export async function detectMode(): Promise<"live" | "static"> {
  try {
    const r = await fetch("api/health");
    return r.ok && (await r.json()).ok === true ? "live" : "static";
  } catch {
    return "static";
  }
}

interface GatewayInstance { id: string; name: string; reachable: boolean; summary: HostSummary | null; error: string | null }

/** The overview's row for a live instance: what the host's own summary says, and nothing the page could only guess. */
export function fromHostSummary(g: GatewayInstance, now: number): Summary {
  const h = g.summary;
  const day = new Date(h?.last_at ?? now).toISOString().slice(0, 10);
  return {
    state: h ? (h.state as Summary["state"]) : "Down",
    doing: h ? h.doing : "It does not answer.",
    needsYou: h ? h.needs_you : true,
    turns: h?.turn ?? 0, lastTurn: h?.turn ?? null, lastAt: h?.last_at ?? now,
    requests: h?.requests ?? 0, quota: h?.quota ?? null, requestsDay: day, spendDayUsd: h?.spend_usd_day ?? 0,
    project: null, idle: false, next: null, waiting: 0, lastDecision: null, context: null, cache: null,
  };
}

export async function loadLiveIndex(): Promise<InstanceIndex> {
  const now = Date.now();
  const r = await get("api/instances");
  const list = ((await r.json()) as { instances: GatewayInstance[] }).instances;
  return {
    generatedAt: now,
    instances: list.map((g) => ({
      id: g.id, name: g.name, kind: "live" as const,
      lockHeld: !!g.summary && g.summary.state !== "Stopped" && g.summary.state !== "Down",
      reachable: g.reachable, record: "", summary: fromHostSummary(g, now),
    })),
  };
}
