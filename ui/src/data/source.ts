/** Where the observer gets its data. Slice 0 reads recorded files served beside the app; a running host's
 *  doors (docs/rung-host-api.md) replace this one module, and nothing above it changes. */
import { parseRecord } from "../record/parse.ts";
import { makeRedactor } from "../record/redact.ts";
import type { Line } from "../record/types.ts";
import type { Summary } from "../record/folds.ts";

export interface InstanceEntry {
  id: string;
  name: string;
  kind: "recorded" | "synthetic";
  /** Stands for the lock on the state directory. A recorded run has no process, so false. */
  lockHeld: boolean;
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
