import { useEffect, useState } from "react";
import { ago } from "../record/words.ts";

/** Recent times read as "3 min ago"; older ones as a date, so a recorded run is not "9 d ago" on every line. */
export function when(at: number, now: number): string {
  if (at > now && at - now < 2 * 86_400_000) {
    // A future time reads "in 25 min"; under the threshold where `ago` says "just now" it is just that.
    const span = ago(now - (at - now), now);
    return span === "just now" ? span : `in ${span.replace(" ago", "")}`;
  }
  if (now - at < 2 * 86_400_000 && now >= at) return ago(at, now);
  const d = new Date(at);
  const mon = d.toLocaleString("en-US", { month: "short", timeZone: "UTC" });
  return `${d.getUTCDate()} ${mon} ${d.toISOString().slice(11, 16)} UTC`;
}

/** The time, as of now: ticking each second while `on`, so a live page's "3 s ago" and "12 s in" stay true. */
export function useNow(on: boolean, fixed: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!on) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [on]);
  return on ? now : fixed;
}
