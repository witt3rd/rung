import type { ReactNode } from "react";
import { href } from "../route.ts";
import { num } from "../record/words.ts";
import type { Summary, Turn } from "../record/folds.ts";

export function Top({ name }: { name?: string }) {
  return (
    <div className="top"><span className="crumb"><a href={href.instances()}>Instances</a>{name ? ` / ${name}` : ""}</span></div>
  );
}

export function Tabs({ id, active }: { id: string; active: "now" | "turns" }) {
  return (
    <nav className="tabs" aria-label={id}>
      <a href={href.now(id)} aria-current={active === "now" ? "page" : undefined}>Now</a>
      <a href={href.turns(id)} aria-current={active === "turns" ? "page" : undefined}>Turns</a>
    </nav>
  );
}

export function Page({ children, ready }: { children: ReactNode; ready: boolean }) {
  return <div className="page" data-ready={ready ? "true" : "false"}>{children}</div>;
}

/** The instance's name and one line: its state word, and what became of its last turn. */
export function InstanceHead({ id, s, live, lastTurn, missing, error }: { id: string; s: Summary | undefined; live: boolean; lastTurn: Turn | null; missing: boolean; error: unknown }) {
  const turn = !lastTurn ? "No turn yet."
    : lastTurn.status === "running" ? `Turn ${num(lastTurn.n)} is running.`
    : lastTurn.status === "interrupted" ? `Turn ${num(lastTurn.n)} was cut off.`
    : `Turn ${num(lastTurn.n)} ${live ? "finished" : "was the last"}.`;
  return (
    <div className="head">
      <div>
        <h1>{id}</h1>
        <p className="line">
          {s ? (
            <>
              <span className={`mark ${s.needsYou ? "needs" : live ? "live" : ""}`} data-needs={s.needsYou ? "" : undefined} />
              <span className={s.needsYou ? "needs-text" : undefined} data-needs={s.needsYou ? "" : undefined}>{s.state}.</span> {turn}
            </>
          ) : missing ? "There is no instance by this name." : error ? "The record could not be read." : "Loading."}
        </p>
      </div>
    </div>
  );
}
