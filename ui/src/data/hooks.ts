import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { loadIndex, loadRecord, type InstanceEntry } from "./source.ts";
import { foldDecisions, foldPack, foldSpend, foldTurns, summarize } from "../record/folds.ts";

export const useIndex = () => useQuery({ queryKey: ["index"], queryFn: loadIndex });

/** One instance's record and every fold the pages use, computed once per record. */
export function useInstance(id: string) {
  const index = useIndex();
  const entry: InstanceEntry | undefined = index.data?.instances.find((i) => i.id === id);
  const rec = useQuery({ queryKey: ["record", id], queryFn: () => loadRecord(entry!), enabled: !!entry });
  const folds = useMemo(() => {
    if (!rec.data || !entry) return null;
    const now = index.data!.generatedAt;
    return {
      lines: rec.data,
      turns: foldTurns(rec.data, { live: entry.lockHeld }),
      decisions: foldDecisions(rec.data),
      pack: foldPack(rec.data),
      spend: foldSpend(rec.data),
      summary: summarize(rec.data, { lockHeld: entry.lockHeld, now }),
      now,
    };
  }, [rec.data, entry, index.data]);
  return { entry, folds, indexLoaded: index.isSuccess, error: index.error ?? rec.error };
}
