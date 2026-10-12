import { useEffect, useMemo, useRef, useState } from "react";
import { useInstance } from "../data/hooks.ts";
import { duration, num, usd } from "../record/words.ts";
import type { Asked, ToolCall, Turn } from "../record/folds.ts";
import { InstanceHead, Page, Tabs, Top } from "./Shell.tsx";
import { clock, useNow, when } from "./format.ts";
import { href } from "../route.ts";
import { overlayRunning } from "../live/overlay.ts";
import { LongText } from "./LongText.tsx";
import { argFold } from "./argFold.ts";
import { describeStatus } from "../live/status.ts";

const PAGE = 50;
type Filter = "all" | "tools" | "failed" | "model";

function callStatus(c: ToolCall, running: boolean): string {
  if (running && c.ok === null && !c.refused) return "running";
  if (c.refused) return `refused: ${c.refused}`;
  return c.ok === false ? "failed" : "";
}

function matches(t: Turn, q: string, f: Filter): boolean {
  if (f === "tools" && t.calls.length === 0) return false;
  if (f === "failed" && t.status !== "failed" && t.status !== "bounded") return false;
  if (f === "model" && !t.decisions.some((d) => d.who === "model")) return false;
  if (!q) return true;
  const hay = [t.text, t.failure ?? "", ...t.calls.map((c) => `${c.name} ${JSON.stringify(c.input ?? "")} ${c.result ?? ""}`), ...t.asked.map((a) => a.text)].join("\n").toLowerCase();
  return hay.includes(q.toLowerCase());
}

/** What the owner sent, marked: a filled Owner tag, whether it was a steer or a task, when it was sent, and, when it came in while another
 *  turn was running, the moment the agent saw it. Anything else keeps its plain label. */
function AskedRow({ id, a, now }: { id: string; a: Asked; now: number }) {
  const owner = a.role === "owner";
  const kind = a.intent === "steer" ? "steer" : a.intent === "free" ? "task, when free" : "task";
  return (
    <div className="call">
      {owner ? <span className="tag owner" data-content>Owner</span> : <span className="k" data-content>{a.from[0].toUpperCase() + a.from.slice(1)}</span>}
      <div>
        <LongText text={a.text} />
        {owner && (
          <p className="meta" style={{ margin: "4px 0 0" }} data-content>
            {kind}, sent {when(a.sentAt, now)}.
            {a.during !== null && <> It came in while <a href={href.turns(id, a.during)}>turn {num(a.during)}</a> was running; the agent saw it at {clock(a.seenAt)}.</>}
          </p>
        )}
      </div>
    </div>
  );
}

function TurnCard({ id, t, now, open }: { id: string; t: Turn; now: number; open: boolean }) {
  const ref = useRef<HTMLElement>(null);
  useEffect(() => { if (open) ref.current?.scrollIntoView({ block: "start" }); }, [open]);
  const byModel = t.decisions.filter((d) => d.who === "model").length;
  return (
    <article className="turn" id={`turn-${t.n}`} ref={ref}>
      <div className="turnhead">
        <h2>Turn {num(t.n)} <span className="meta" data-content>{when(t.endedAt ?? t.startedAt, now)}</span></h2>
        <a className="quiet" href={href.context(id, t.n)}>Context</a>
      </div>
      {t.asked.map((a) => <AskedRow key={a.id} id={id} a={a} now={now} />)}
      {t.text ? <LongText text={t.text} style={{ marginTop: t.asked.length ? 10 : 0 }} />
        : <p className="meta" style={{ margin: 0 }} data-content>{t.status === "running" ? "In progress." : t.status === "interrupted" ? "Cut off before it finished." : `No reply${t.failure ? ` (${t.failure})` : ""}.`}</p>}
      {t.calls.map((c, i) => {
        const arg = argFold(c.input);
        return (
          <div className="call" key={i}>
            <span className="k" data-content>Tool</span>
            <span className="callbody">
              <span className="mono" data-content>{c.name}{arg.inline}{callStatus(c, t.status === "running") ? ` — ${callStatus(c, t.status === "running")}` : ""}</span>
              {arg.whole !== null && <LongText text={arg.whole} mono />}
            </span>
          </div>
        );
      })}
      {t.decisions.length > 0 && (
        <details className="more" open={open}>
          <summary data-content>
            {t.decisions.length === 1 ? "1 decision" : `${t.decisions.length} decisions`}{byModel ? `, ${byModel} by model` : ", all by rule"}. Details
          </summary>
          <p className="sub" data-content>What was decided</p>
          {t.decisions.map((d) => (
            <dl className="dl" key={d.seq}>
              <dt data-content>Question</dt><dd data-content>{d.question}</dd>
              <dt data-content>Answer</dt><dd data-content>{d.answer}</dd>
              <dt data-content>Decided</dt><dd data-content>{d.label}. {d.reason}</dd>
              {d.modelWould && <><dt data-content>Model would</dt><dd data-content>{d.modelWould}</dd></>}
            </dl>
          ))}
          <p className="sub" data-content>The turn</p>
          <dl className="dl">
            <dt data-content>Model</dt><dd data-content>{t.model ?? "unknown"}</dd>
            <dt data-content>Calls</dt><dd data-content>{num(t.llmCalls)} model {t.llmCalls === 1 ? "call" : "calls"}, {num(t.calls.length)} tool {t.calls.length === 1 ? "call" : "calls"}</dd>
            <dt data-content>Cost</dt><dd data-content>{usd(t.costUsd)}, cache {t.promptTokens ? `${Math.round((t.cachedTokens / t.promptTokens) * 100)}%` : "none"}{t.elapsedMs ? `, took ${duration(t.elapsedMs)}` : ""}</dd>
            <dt data-content>Record</dt><dd data-content>lines {num(t.firstSeq)} to {num(t.lastSeq)}</dd>
          </dl>
          {t.calls.some((c) => c.input !== undefined || c.result) && <p className="sub" data-content>Tool calls in full</p>}
          {t.calls.filter((c) => c.input !== undefined || c.result).map((c, i) => (
            <dl className="dl" key={i}>
              <dt data-content>{c.name}</dt>
              <dd><LongText text={`${typeof c.input === "string" ? c.input : JSON.stringify(c.input, null, 2)}${c.result ? `\n→ ${c.result}` : ""}`} mono /></dd>
            </dl>
          ))}
        </details>
      )}
    </article>
  );
}

export function TurnsPage({ id, turn }: { id: string; turn: number | null }) {
  const { entry, folds, indexLoaded, error, live: li } = useInstance(id);
  const [q, setQ] = useState("");
  const [f, setF] = useState<Filter>("all");
  const [shown, setShown] = useState(PAGE);
  const missing = indexLoaded && !entry;

  const turnsNow = useMemo(() => (folds ? overlayRunning(folds.turns, li?.delta ?? null) : []), [folds, li?.delta]);
  const list = useMemo(() => turnsNow.filter((t) => matches(t, q, f)).reverse(), [turnsNow, q, f]);
  const now = useNow(!!li, folds?.now ?? 0);
  // A link to one turn opens the list far enough to hold it.
  const need = turn ? list.findIndex((t) => t.n === turn) + 1 : 0;
  const count = Math.max(shown, need);
  const s = folds?.summary;
  const lastTurn = folds && folds.turns.length ? folds.turns[folds.turns.length - 1] : null;
  const live = folds ? folds.lockHeld : !!entry?.lockHeld;
  const filtered = q !== "" || f !== "all";

  return (
    <>
      <Top name={id} />
      <Page ready={!!folds || missing || !!error}>
        <InstanceHead id={id} s={s} live={live} lastTurn={lastTurn} missing={missing} error={error} />
        <Tabs id={id} active="turns" />
        <div className="field" style={{ marginBottom: 20 }}>
          <input type="search" placeholder="Search turns" aria-label="Search turns" value={q} onChange={(e) => { setQ(e.target.value); setShown(PAGE); }} />
          <select aria-label="Show" value={f} onChange={(e) => { setF(e.target.value as Filter); setShown(PAGE); }}>
            <option value="all">All turns</option><option value="tools">Tool calls</option><option value="failed">Failed turns</option><option value="model">Decided by model</option>
          </select>
        </div>
        {li && (
          <p className="meta" style={{ margin: "0 0 20px" }} data-content>
            <span className="mark live" />{describeStatus(li.status).turnsLine}
          </p>
        )}
        <section>
          {list.slice(0, count).map((t) => <TurnCard key={t.n} id={id} t={t} now={now} open={t.n === turn} />)}
          {folds && list.length === 0 && <p className="none meta" data-content>{filtered ? "No turn matches." : "No turn yet."}</p>}
        </section>
        {folds && list.length > 0 && (
          <p className="fold">
            Showing {num(Math.min(count, list.length))} of {num(list.length)}{filtered ? ` matching (of ${num(folds.turns.length)})` : ""}.
            {count < list.length && (<> <button className="quiet" onClick={() => setShown(count + PAGE)}>Show {num(Math.min(PAGE, list.length - count))} more</button> <button className="quiet" onClick={() => setShown(list.length)}>Show all {num(list.length)}</button></>)}
            {count > PAGE && (<> <button className="quiet" onClick={() => setShown(PAGE)}>Show less</button></>)}
          </p>
        )}
      </Page>
    </>
  );
}
