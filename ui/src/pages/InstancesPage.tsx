import { href } from "../route.ts";
import { useIndex } from "../data/hooks.ts";
import { num } from "../record/words.ts";
import type { InstanceEntry } from "../data/source.ts";
import { Page, Top } from "./Shell.tsx";
import { when } from "./format.ts";

export const requestsLine = (s: InstanceEntry["summary"]): string => `${num(s.requests)}${s.quota ? ` of ${num(s.quota)}` : ""} requests`;

/** The row that needs the owner first, then the most recently active. */
export function order(list: InstanceEntry[]): InstanceEntry[] {
  return [...list].sort((a, b) => Number(b.summary.needsYou) - Number(a.summary.needsYou) || b.summary.lastAt - a.summary.lastAt);
}

export function InstancesPage() {
  const { data, error } = useIndex();
  const list = data ? order(data.instances) : [];
  const needs = list.filter((i) => i.summary.needsYou).length;
  return (
    <>
      <Top />
      <Page ready={!!data}>
        <h1>Instances</h1>
        <p className="line">{data ? `${list.length === 1 ? "One instance" : `${list.length} instances`}. ${needs === 0 ? "None needs you." : needs === 1 ? "One needs you." : `${needs} need you.`}` : error ? "The list could not be read." : "Loading the list."}</p>
        <div style={{ height: 24 }} />
        {list.map((i) => {
          const s = i.summary;
          return (
            <a key={i.id} className={`row ov${s.needsYou ? " next" : ""}`} href={href.now(i.id)} data-needs={s.needsYou ? "" : undefined}>
              <span className="t" data-content><b>{i.name}</b></span>
              <span data-content className={s.needsYou ? "needs-text" : undefined}>{s.needsYou && <span className="mark needs" />}{s.state}</span>
              <span data-content>{s.doing}</span>
              <span className="meta" data-content>{i.reachable === false ? "No answer" : requestsLine(s)}</span>
              <span className="meta" data-content>{i.reachable === false ? "" : when(s.lastAt, data!.generatedAt)}</span>
            </a>
          );
        })}
        {data && <p className="fold" style={{ marginTop: 16 }}>Showing all {list.length}.</p>}
      </Page>
    </>
  );
}
