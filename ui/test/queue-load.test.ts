// Reading the whole open queue: every page the host gives, never one page taken for all of it.
import { test } from "node:test";
import assert from "node:assert/strict";
import { loadQueue, QueueUnavailable } from "../src/queue/hooks.ts";

const it = (id: string, state: string, extra: Record<string, unknown> = {}) => ({ id, text: id, role: "owner", name: null, channel: "owner", kind: "peer", intent: "next", order: 1, position: null, accepted_at: 1, state, turn: null, disposition: null, held: null, ...extra });
const page = (items: unknown[], next: number | null) => new Response(JSON.stringify({ items, total: 5, offset: 0, limit: 2, next, counts: {}, record_lines: 9, damaged: null, redaction: { env_unset: [] } }), { status: 200 });

test("every page of the open queue is read, in order, until the host says there is no next", async () => {
  const urls: string[] = [];
  const pages = [page([it("a", "answering", { turn: 3 }), it("b", "waiting")], 2), page([it("c", "waiting"), it("d", "waiting")], 4), page([it("e", "waiting")], null)];
  const q = await loadQueue("x", (async (u: string) => { urls.push(u); return pages.shift()!; }) as unknown as typeof fetch);
  assert.deepEqual(q.now.map((i) => i.id), ["a"]);
  assert.deepEqual(q.waiting.map((i) => i.id), ["b", "c", "d", "e"]);
  assert.deepEqual(urls, ["api/i/x/v1/queue?state=open&offset=0", "api/i/x/v1/queue?state=open&offset=2", "api/i/x/v1/queue?state=open&offset=4"]);
});

test("a gateway with no queue door is a named unavailability, not an empty queue", async () => {
  await assert.rejects(loadQueue("x", (async () => new Response("{}", { status: 404 })) as unknown as typeof fetch), (e: unknown) => e instanceof QueueUnavailable && e.status === 404);
});
