import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { href } from "../route.ts";
import { useInstance, useMode } from "../data/hooks.ts";
import { num } from "../record/words.ts";
import { InstanceHead, Page, Tabs, Top } from "./Shell.tsx";
import { contextQueryKey, loadContext } from "../context/load.ts";
import { cachedLine, marksFor, type Mark } from "../context/cache.ts";
import { BOUNDARY_AFTER, type Band, type BandItem, type ContextView } from "../context/types.ts";
import { BOUNDARY, MARK_WORD, TITLE, WHAT, fact, hasLongLine, isEmpty, isLong, sayEvent, tilde } from "../context/words.ts";

const ROWS = 50;

/** A text, whole. A long one is kept short on the page by a fold; every character is in the page, and "Show more" opens the rest. */
function Text({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  const long = isLong(text);
  const wide = useMemo(() => hasLongLine(text), [text]);
  return (
    <div>
      {wide && <p className="meta" data-content>This text has a very long line, so it scrolls sideways instead of wrapping. It is whole.</p>}
      <pre className={`content mono text${long && !open ? " folded" : ""}${wide ? " nowrap" : ""}`} data-content>{text}</pre>
      {long && <button className="quiet" aria-expanded={open} onClick={() => setOpen(!open)}>{open ? "Show less" : "Show more"}</button>}
    </div>
  );
}

function Row({ it }: { it: BandItem }) {
  return (
    <details className="row-in">
      <summary className="mono" data-content>{it.label}</summary>
      {it.note && <p className="meta" data-content>{it.note}</p>}
      <Text text={it.text} />
    </details>
  );
}

function Drill({ b }: { b: Band }) {
  const [shown, setShown] = useState(ROWS);
  return (
    <div className="drill" id={`band-${b.id}`}>
      <p className="meta">{WHAT[b.id]}</p>
      {!b.recorded && <p className="meta" data-content>{b.why}</p>}
      {b.recorded && b.partial && <p className="meta" data-content>{b.partial}</p>}
      {b.recorded && isEmpty(b) && <p className="meta" data-content>Nothing was sent in this band.</p>}
      {b.text !== null && b.text !== "" && <Text text={b.text} />}
      {b.items.slice(0, shown).map((it, i) => <Row key={i} it={it} />)}
      {b.items.length > shown && (
        <p className="fold">Showing {num(shown)} of {num(b.items.length)}. <button className="quiet" onClick={() => setShown(shown + ROWS)}>Show {num(Math.min(ROWS, b.items.length - shown))} more</button> <button className="quiet" onClick={() => setShown(b.items.length)}>Show all {num(b.items.length)}</button></p>
      )}
    </div>
  );
}

function Mk({ m }: { m: Mark }) {
  return <span className={`mk ${m}`}><span className="cm" aria-hidden="true" />{MARK_WORD[m]}</span>;
}

function BandRow({ b, mark, open, toggle }: { b: Band; mark: Mark; open: boolean; toggle: () => void }) {
  const f = fact(b);
  const canOpen = true; // an unrecorded band opens to say why; an empty one opens to say it is empty
  return (
    <section className={`band${b.recorded ? "" : " gap"}`} data-band={b.id}>
      <button className="bh" aria-expanded={open} aria-controls={`band-${b.id}`} onClick={toggle} disabled={!canOpen}>
        <span className="bt"><b>{TITLE[b.id]}</b><span className="meta" data-content>{f ? ` · ${f}` : ""}</span></span>
        <span className="bs mono" data-content>{!b.recorded ? "" : b.estimate === null ? "size not recorded" : b.estimate === 0 ? "0 tok" : `${tilde(b.estimate)} tok`}</span>
        <span className="bm"><Mk m={mark} /></span>
      </button>
      {open && <Drill b={b} />}
    </section>
  );
}

function Strata({ v }: { v: ContextView }) {
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const marks = useMemo(() => marksFor(v.bands, v.usage), [v]);
  const line = cachedLine(v.usage);
  const sized = v.bands.filter((b) => b.estimate !== null);
  const sum = sized.reduce((a, b) => a + b.estimate!, 0);
  const allSized = sized.length === v.bands.length;
  return (
    <div className="bands">
      {v.bands.map((b, i) => {
        const after = BOUNDARY_AFTER[b.id];
        return (
          <div key={b.id}>
            <BandRow b={b} mark={marks[i]} open={open.has(b.id)} toggle={() => setOpen((s) => { const n = new Set(s); if (!n.delete(b.id)) n.add(b.id); return n; })} />
            {after && <div className="boundary meta">{BOUNDARY[after]}{(() => { const at = v.boundaries.find((x) => x.after === b.id)?.tokens; return at == null ? null : <span data-content>{` · at ~${num(at)} tokens`}</span>; })()}</div>}
          </div>
        );
      })}
      <div className="total">
        <p className="meta" style={{ margin: 0 }} data-content>
          {sum === 0 ? "The host's estimates are not recorded for this call." : `${allSized ? "The host's estimates add up to" : "The host's estimates for the bands it sized add up to"} ${tilde(sum)} tokens.`}
          {v.usage ? ` The provider counted ${num(v.usage.prompt)}, and that is the count to trust.` : " The provider has no count for this call."}
        </p>
        <p className="line" style={{ margin: "6px 0 0", color: "var(--ink)" }} data-content>
          {line ? `Cached ${num(line.cached)} of ${num(line.prompt)} provider tokens${line.pct === null ? "" : ` (${line.pct}%)`}.` : "Cached: not known yet."}
        </p>
      </div>
    </div>
  );
}

export function ContextPage({ id, turn, call }: { id: string; turn: number; call: number | null }) {
  const { entry, folds, indexLoaded, error, live: li } = useInstance(id);
  const mode = useMode().data;
  const missing = indexLoaded && !entry;
  const t = folds?.turns.find((x) => x.n === turn) ?? null;
  const calls = t?.llmCalls ?? 0;
  const k = calls === 0 ? 0 : call !== null && call >= 1 && call <= calls ? call : calls;
  const lastTurn = folds && folds.turns.length ? folds.turns[folds.turns.length - 1] : null;
  const live = !!entry?.lockHeld;
  const q = useQuery({
    queryKey: contextQueryKey(id, turn, k, mode!, folds?.lines ?? []),
    queryFn: () => loadContext({ mode: mode!, id, turn, call: k, lines: folds!.lines }),
    enabled: !!folds && !!t && k > 0 && mode !== undefined,
  });
  const v = q.data ?? null;
  const ready = !!error || missing || (!!folds && (!t || k === 0 || q.isSuccess || q.isError));
  return (
    <>
      <Top name={id} />
      <Page ready={ready}>
        <InstanceHead id={id} s={folds?.summary} live={live} lastTurn={lastTurn} missing={missing} error={error} />
        <Tabs id={id} active="turns" />
        <div className="field" style={{ marginBottom: 8 }}>
          <h2 style={{ margin: 0, flex: "1 1 auto" }}>Context of turn {num(turn)}</h2>
          <select aria-label="Model call" value={k} disabled={calls < 2}
            onChange={(e) => { window.location.hash = href.context(id, turn, Number(e.target.value)); }}>
            {calls === 0 && <option value={0}>No model call</option>}
            {Array.from({ length: calls }, (_, i) => <option key={i} value={i + 1}>Call {i + 1} of {calls}</option>)}
          </select>
        </div>
        <p className="fold" style={{ margin: "0 0 16px" }}><a href={href.turns(id, turn)}>Back to turn {num(turn)}</a></p>
        {folds && !t && <p className="none meta" data-content>There is no turn {num(turn)} in this record.</p>}
        {folds && t && k === 0 && <p className="none meta" data-content>Turn {num(turn)} made no model call, so nothing was sent.</p>}
        {v && (
          <>
            <p className="line" style={{ marginTop: 0 }} data-content>
              Call {num(v.call)} of turn {num(v.turn)}{v.epoch !== null ? ` · epoch ${num(v.epoch)}` : ""}{v.model ? ` · model ${v.model}` : ""}
              {v.usage ? ` · sent ${num(v.usage.prompt)} tokens (provider count)` : ""}
            </p>
            <p className="meta" style={{ margin: "0 0 4px" }} data-content>{v.source === "door" ? "Read from the host's context door." : v.doorNote}</p>
            <p className="meta" style={{ margin: "0 0 4px" }} data-content>{sayEvent(v.cacheEvent)}</p>
            {v.note && <p className="meta" style={{ margin: "0 0 4px" }} data-content>{`${v.note.charAt(0).toUpperCase()}${v.note.slice(1).replace(/\.$/, "")}.`}</p>}
            {v.rewritten && <p className="meta" style={{ margin: "0 0 4px" }} data-content>The engine rewrote what it was given inside this turn, so the bands are what the host gave it, not what the last calls saw.</p>}
            {v.damaged && <p className="meta" style={{ margin: "0 0 4px" }} data-content>The record is damaged: {v.damaged}</p>}
            {v.problems.map((p, i) => <p className="meta" style={{ margin: "0 0 4px" }} key={i} data-content>{p}</p>)}
            <div style={{ height: 16 }} />
            <Strata key={`${v.turn}-${v.call}`} v={v} />
          </>
        )}
        {!v && folds && t && k > 0 && <p className="none meta" data-content>{q.isError ? "The strata could not be read." : "Loading."}</p>}
        {li && <p className="meta" style={{ marginTop: 20 }} data-content><span className="mark live" />{li.status === "following" ? "Following" : "Recorded, not followed"}</p>}
      </Page>
    </>
  );
}
