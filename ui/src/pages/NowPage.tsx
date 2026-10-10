import { href } from "../route.ts";
import { useInstance } from "../data/hooks.ts";
import { num, usd } from "../record/words.ts";
import { turnLine } from "../record/folds.ts";
import { requestsLine } from "./InstancesPage.tsx";
import { InstanceHead, Page, Tabs, Top } from "./Shell.tsx";
import { when } from "./format.ts";

export function NowPage({ id }: { id: string }) {
  const { entry, folds, indexLoaded, error } = useInstance(id);
  const s = folds?.summary;
  const live = !!entry?.lockHeld;
  const recent = folds ? folds.turns.slice(-3).reverse() : [];
  const lastTurn = folds && folds.turns.length ? folds.turns[folds.turns.length - 1] : null;
  const missing = indexLoaded && !entry;
  return (
    <>
      <Top name={id} />
      <Page ready={!!folds || missing || !!error}>
        <InstanceHead id={id} s={s} live={live} lastTurn={lastTurn} missing={missing} error={error} />
        <Tabs id={id} active="now" />
        <div className="cols">
          <div>
            <section className="sec">
              <h2>{live ? "Doing now" : "Last doing"}</h2>
              <div className="card look">
                {s ? (
                  <>
                    <p style={{ fontSize: "var(--s-look)", margin: 0 }} data-content>{s.project ? s.project.title : s.doing}</p>
                    {s.project && (
                      <p className="meta" style={{ margin: "6px 0 0" }} data-content>
                        Committed {num(Math.max(0, (s.lastTurn ?? 0) - s.project.sinceTurn))} turns ago.{s.project.doneWhen ? ` Done when ${s.project.doneWhen}.` : ""}
                      </p>
                    )}
                    {!s.project && lastTurn && !live && (
                      <p className="meta" style={{ margin: "6px 0 0" }} data-content>It said: {turnLine(lastTurn)}</p>
                    )}
                  </>
                ) : <p className="none">&nbsp;</p>}
              </div>
            </section>
            <section className="sec">
              <h2>Recent turns</h2>
              {recent.map((t) => (
                <a key={t.n} className="row tr" href={href.turns(id, t.n)} style={{ textDecoration: "none", color: "inherit" }}>
                  <span data-content>{num(t.n)} · {turnLine(t)}</span>
                  <span className="meta" data-content>{when(t.endedAt ?? t.startedAt, folds!.now)}</span>
                </a>
              ))}
              {folds && <p className="fold" style={{ marginTop: 12 }}>Showing {recent.length} of {num(folds.turns.length)}.{folds.turns.length > recent.length && <> <a href={href.turns(id)}>Show more</a></>}</p>}
            </section>
          </div>
          <aside>
            <section className="sec">
              <h2>Next</h2>
              <p style={{ margin: 0 }} data-content>{s ? (s.next ? s.next.text : live ? "Nothing scheduled." : "Not running.") : "\u00a0"}</p>
              {s && <p className="meta" style={{ margin: "2px 0 0" }} data-content>{s.next ? `${when(s.next.at, folds!.now)}. ` : ""}{s.waiting === 0 ? "None waiting" : `${num(s.waiting)} waiting`}</p>}
            </section>
            <section className="sec">
              <h2>Decided</h2>
              <p style={{ margin: 0 }} data-content>{s ? (s.lastDecision ? s.lastDecision.answer : "Nothing decided yet.") : "\u00a0"}</p>
              {s?.lastDecision && s.lastDecision.turn && <p className="meta" style={{ margin: "2px 0 0" }}><a href={href.turns(id, s.lastDecision.turn)}>Why</a></p>}
            </section>
            <section className="sec">
              <h2>Health</h2>
              <p style={{ margin: 0 }} data-content>{s ? `${s.context === null ? "No context yet" : `Context ${s.context}% full`}${s.cache === null ? "" : `. Cache ${s.cache}%`}.` : "\u00a0"}</p>
              {s && <p className="meta" style={{ margin: "2px 0 0" }} data-content>{usd(s.spendDayUsd)} spent. {requestsLine(s)}.</p>}
            </section>
          </aside>
        </div>
      </Page>
    </>
  );
}
