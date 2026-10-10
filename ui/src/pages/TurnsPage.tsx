import { useEffect, useMemo, useRef, useState } from "react";
import { useInstance } from "../data/hooks.ts";
import { duration, num, usd } from "../record/words.ts";
import type { ToolCall, Turn } from "../record/folds.ts";
import { InstanceHead, Page, Tabs, Top } from "./Shell.tsx";
import { when } from "./format.ts";

const PAGE = 50;
type Filter = "all" | "tools" | "failed" | "model";

const oneLine = (v: unknown): string => {
  if (v === undefined || v === null) return "";
  if (typeof v === "string") return v;
  const o = v as Record<string, unknown>;
  for (const k of ["path", "query", "text", "cmd", "url", "note", "next_step"]) if (typeof o[k] === "string") return o[k] as string;
  const first = Object.values(o).find((x) => typeof x === "string");
  return typeof first === "string" ? first : JSON.stringify(v);
};
/** A short argument (a path or a query) rides on the call's line. A long one, such as a note, waits in the fold with the whole call. */
const shortArg = (v: unknown): string => {
  const s = oneLine(v);
  return s && !s.includes("\n") && s.length <= 80 ? ` ${s}` : "";
};

function callStatus(c: ToolCall): string {
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

function TurnCard({ t, now, open }: { t: Turn; now: number; open: boolean }) {
  const ref = useRef<HTMLElement>(null);
  useEffect(() => { if (open) ref.current?.scrollIntoView({ block: "start" }); }, [open]);
  const byModel = t.decisions.filter((d) => d.who === "model").length;
  return (
    <article className="turn" id={`turn-${t.n}`} ref={ref}>
      <h2>Turn {num(t.n)} <span className="meta" data-content>{when(t.endedAt ?? t.startedAt, now)}</span></h2>
      {t.asked.map((a) => (
        <div className="call" key={a.id}><span className="k" data-content>{a.from[0].toUpperCase() + a.from.slice(1)}</span><span className="content" data-content>{a.text}</span></div>
      ))}
      {t.text ? <p className="content" data-content style={{ marginTop: t.asked.length ? 10 : 0 }}>{t.text}</p>
        : <p className="meta" style={{ margin: 0 }} data-content>{t.status === "running" ? "In progress." : t.status === "interrupted" ? "Cut off before it finished." : `No reply${t.failure ? ` (${t.failure})` : ""}.`}</p>}
      {t.calls.map((c, i) => (
        <div className="call" key={i}>
          <span className="k" data-content>Tool</span>
          <span className="mono" data-content>{c.name}{shortArg(c.input)}{callStatus(c) ? ` — ${callStatus(c)}` : ""}</span>
        </div>
      ))}
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
            <dt data-content>Calls</dt><dd data-content>{num(t.llmCalls)} model calls, {num(t.calls.length)} tool calls</dd>
            <dt data-content>Cost</dt><dd data-content>{usd(t.costUsd)}, cache {t.promptTokens ? `${Math.round((t.cachedTokens / t.promptTokens) * 100)}%` : "none"}{t.elapsedMs ? `, took ${duration(t.elapsedMs)}` : ""}</dd>
            <dt data-content>Record</dt><dd data-content>lines {num(t.firstSeq)} to {num(t.lastSeq)}</dd>
          </dl>
          {t.calls.some((c) => c.input !== undefined || c.result) && <p className="sub" data-content>Tool calls in full</p>}
          {t.calls.filter((c) => c.input !== undefined || c.result).map((c, i) => (
            <dl className="dl" key={i}>
              <dt data-content>{c.name}</dt>
              <dd className="mono" data-content>{typeof c.input === "string" ? c.input : JSON.stringify(c.input, null, 2)}{c.result ? `\n→ ${c.result}` : ""}</dd>
            </dl>
          ))}
        </details>
      )}
    </article>
  );
}

export function TurnsPage({ id, turn }: { id: string; turn: number | null }) {
  const { entry, folds, indexLoaded, error } = useInstance(id);
  const [q, setQ] = useState("");
  const [f, setF] = useState<Filter>("all");
  const [shown, setShown] = useState(PAGE);
  const missing = indexLoaded && !entry;

  const list = useMemo(() => (folds ? folds.turns.filter((t) => matches(t, q, f)).reverse() : []), [folds, q, f]);
  // A link to one turn opens the list far enough to hold it.
  const need = turn ? list.findIndex((t) => t.n === turn) + 1 : 0;
  const count = Math.max(shown, need);
  const s = folds?.summary;
  const lastTurn = folds && folds.turns.length ? folds.turns[folds.turns.length - 1] : null;
  const live = !!entry?.lockHeld;
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
        <section>
          {list.slice(0, count).map((t) => <TurnCard key={t.n} t={t} now={folds!.now} open={t.n === turn} />)}
          {folds && list.length === 0 && <p className="none meta" data-content>{filtered ? "No turn matches." : "No turn yet."}</p>}
        </section>
        {folds && list.length > 0 && (
          <p className="fold">
            Showing {num(Math.min(count, list.length))} of {num(list.length)}{filtered ? ` matching (of ${num(folds.turns.length)})` : ""}.
            {count < list.length && (<> <button className="quiet" onClick={() => setShown(count + PAGE)}>Show {num(Math.min(PAGE, list.length - count))} more</button> <button className="quiet" onClick={() => setShown(list.length)}>Show all {num(list.length)}</button></>)}
          </p>
        )}
      </Page>
    </>
  );
}
