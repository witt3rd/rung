/** The cache marker per band, derived from the call's cached-token count against the bands in wire order.
 *  The provider caches a prefix: the first `cached` tokens of the request. The host's band sizes are estimates, so they are
 *  scaled to the provider's count before the cut is placed; a band the cut passes through is "partly". */
import type { BandId, Usage } from "./types.ts";

export type Mark = "cached" | "new" | "partly" | "empty" | "unknown";

export function markBands(bands: readonly { id: BandId; estimate: number | null }[], usage: Usage | null): Mark[] {
  const n = bands.length;
  if (!usage || usage.prompt <= 0) return Array(n).fill("unknown");
  const sizeKnown = bands.every((b) => b.estimate !== null);
  const whole = (m: Mark): Mark[] => bands.map((b) => (b.estimate === 0 ? "empty" : m));
  const unknown = (): Mark[] => whole("unknown"); // a band known to be empty is empty, whatever the rest
  // The two cases that need no sizes: nothing was cached, or everything was.
  if (usage.cached <= 0) return whole("new");
  if (usage.cached >= usage.prompt) return whole("cached");
  if (!sizeKnown) return unknown();
  const sum = bands.reduce((a, b) => a + b.estimate!, 0);
  if (sum <= 0) return unknown();
  const scale = usage.prompt / sum;
  let at = 0;
  return bands.map((b) => {
    const start = at * scale;
    at += b.estimate!;
    const end = at * scale;
    if (b.estimate === 0) return "empty";
    if (end <= usage.cached + 1e-9) return "cached";
    if (start >= usage.cached - 1e-9) return "new";
    return "partly";
  });
}

/** "cached N of M provider tokens (P%)": null when the provider has not counted. */
export function cachedLine(u: Usage | null): { cached: number; prompt: number; pct: number | null } | null {
  if (!u) return null;
  return { cached: u.cached, prompt: u.prompt, pct: u.prompt > 0 ? Math.round((u.cached / u.prompt) * 100) : null };
}

/** The marker per band as the page draws it: the host's own mark where its door gave one for a request it has whole (it laid the provider's
 *  cached count against the bands in wire order itself), else derived here. A band with nothing in it is "empty" either way. */
export function marksFor(bands: readonly { id: BandId; recorded?: boolean; estimate: number | null; cache: "cached" | "partly" | "new" | "unknown" | null }[], usage: Usage | null): Mark[] {
  const derived = markBands(bands, usage);
  // A host that lacks some of the bands lays the cached count against the bands it has, so its marks are not the request's: derive them here,
  // where a band with no size leaves the answer "unknown" rather than a guess.
  const trust = bands.every((b) => b.recorded !== false);
  return bands.map((b, i) => (b.estimate === 0 ? "empty" : (trust ? b.cache : null) ?? derived[i]));
}
