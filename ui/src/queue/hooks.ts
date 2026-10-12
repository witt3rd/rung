import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { parseQueue, type QueueAnswer } from "./wire.ts";
import { Sender } from "./send.ts";

export const queueUrl = (id: string) => `api/i/${encodeURIComponent(id)}/v1/queue`;

/** Why the list could not be read: the door is not on this gateway (404), or anything else. */
export class QueueUnavailable extends Error {
  status: number;
  constructor(status: number) { super(status === 404 ? "this instance has no queue door" : `the queue could not be read (${status})`); this.status = status; }
}

/** The open queue (what is being answered and what waits), every page of it: the list is paged by the host, never cut by the page. */
export async function loadQueue(id: string, fetchImpl: typeof fetch = (...a) => fetch(...a)): Promise<QueueAnswer> {
  let all: QueueAnswer | null = null;
  let offset = 0;
  for (;;) {
    const r = await fetchImpl(`${queueUrl(id)}?state=open&offset=${offset}`);
    if (!r.ok) throw new QueueUnavailable(r.status);
    const page = parseQueue(await r.json());
    all = all ? { ...page, now: [...all.now, ...page.now], waiting: [...all.waiting, ...page.waiting] } : page;
    if (page.next === null) return all;
    offset = page.next;
  }
}

/** The host's list, read through the queue door. It is asked again whenever the record has moved (a line was written) and now and then,
 *  and never edited by the page: an item is in it when the host says so. */
export function useQueue(id: string, enabled: boolean, lastSeq: number | null) {
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ["queue", id], queryFn: () => loadQueue(id), enabled, refetchInterval: 5000, retry: false });
  useEffect(() => { if (enabled && lastSeq !== null) void qc.invalidateQueries({ queryKey: ["queue", id] }); }, [enabled, lastSeq, id, qc]);
  return q;
}

// One sender and one draft per instance, kept across the page's tabs: the press whose answer may have been lost keeps its key, and a
// message typed and not yet sent is still there when the owner comes back from another tab.
const senders = new Map<string, Sender>();
const drafts = new Map<string, string>();
export function senderFor(id: string): Sender {
  let s = senders.get(id);
  if (!s) senders.set(id, (s = new Sender({ url: queueUrl(id) })));
  return s;
}
export const draftFor = (id: string): string => drafts.get(id) ?? "";
export const keepDraft = (id: string, text: string): void => { if (text === "") drafts.delete(id); else drafts.set(id, text); };
