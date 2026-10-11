import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { useQuery } from "@tanstack/react-query";
import { detectMode, loadIndex, loadLiveIndex, loadRecord, type InstanceEntry } from "./source.ts";
import { foldDecisions, foldPack, foldSpend, foldTurns, summarize } from "../record/folds.ts";
import { LiveStore, type LiveDelta, type LiveState } from "../live/store.ts";
import type { Status } from "../live/client.ts";

/** Is the app beside a gateway (live) or beside recorded files (static)? Asked once. */
export const useMode = () => useQuery({ queryKey: ["mode"], queryFn: detectMode });

export function useIndex() {
  const mode = useMode().data;
  return useQuery({
    queryKey: ["index", mode],
    queryFn: mode === "live" ? loadLiveIndex : loadIndex,
    enabled: mode !== undefined,
    refetchInterval: mode === "live" ? 5000 : false, // the overview of live instances is asked again; one instance page is followed
  });
}

// One store per followed instance, shared by everything that shows it, so a page and its tabs hold one connection.
const stores = new Map<string, { store: LiveStore; refs: number }>();
function acquire(id: string): LiveStore {
  let e = stores.get(id);
  if (!e) { e = { store: new LiveStore(`api/i/${encodeURIComponent(id)}`), refs: 0 }; stores.set(id, e); void e.store.start(); }
  e.refs++;
  return e.store;
}
function release(id: string): void {
  const e = stores.get(id);
  if (e && --e.refs <= 0) { e.store.stop(); stores.delete(id); }
}
const idle: LiveState = { loaded: false, error: null, status: "connecting", lines: [], host: null };
const never = () => () => {};

export interface LiveInfo { status: Status; delta: LiveDelta | null }

/** One instance's record and every fold the pages use, computed once per change of the record. In live mode the record is
 *  followed and the host's own summary decides the state word; in static mode it is a recorded file. */
export function useInstance(id: string) {
  const mode = useMode().data;
  const index = useIndex();
  const entry: InstanceEntry | undefined = index.data?.instances.find((i) => i.id === id);
  const live = mode === "live";

  const rec = useQuery({ queryKey: ["record", id], queryFn: () => loadRecord(entry!), enabled: !live && !!entry });

  const followed = live && !!entry;
  const [store, setStore] = useState<LiveStore | null>(null);
  useEffect(() => {
    if (!followed) return;
    setStore(acquire(id));
    return () => { release(id); setStore(null); };
  }, [followed, id]);
  const ls = useSyncExternalStore(store?.subscribe ?? never, store?.getState ?? (() => idle));
  const delta = useSyncExternalStore(store?.subscribeDelta ?? never, store?.getDelta ?? (() => null));

  const lines = live ? (ls.loaded ? ls.lines : undefined) : rec.data;
  const folds = useMemo(() => {
    if (!lines || !entry) return null;
    const now = live ? Date.now() : index.data!.generatedAt;
    const lockHeld = live ? (ls.host ? ls.host.state !== "Stopped" && ls.host.state !== "Down" : entry.lockHeld) : entry.lockHeld;
    const folded = summarize(lines, { lockHeld, now });
    // The host knows whether it holds its lock; its word for the state is the one shown.
    const summary = live && ls.host ? { ...folded, state: ls.host.state as typeof folded.state, doing: ls.host.doing, needsYou: ls.host.needs_you } : folded;
    return { lines, turns: foldTurns(lines, { live: lockHeld }), decisions: foldDecisions(lines), pack: foldPack(lines), spend: foldSpend(lines), summary, now, lockHeld };
  }, [lines, entry, index.data, live, ls.host]);

  const liveInfo: LiveInfo | null = live ? { status: ls.status, delta } : null;
  return { entry, folds, indexLoaded: index.isSuccess, error: index.error ?? rec.error ?? (live && ls.error ? new Error(ls.error) : null), live: liveInfo };
}
