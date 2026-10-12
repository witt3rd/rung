// The queue doors, as the host serves them (docs/rung-host-api.md, "S1 — the queue"): GET and POST /v1/queue through the gateway.
// The mock in ../mock answers with the doors' own bodies; the same file runs against the real gateway and host
// (RUNG_CONTRACT_TARGET=real-host, or RUNG_CONTRACT_URL=http://… RUNG_CONTRACT_INSTANCE=id). Where the door is not there (404) the checks
// skip, by name.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { startTarget, type Target } from "./contract-target.ts";
import type { SimHost } from "../mock/host.ts";
import type { Line } from "../src/record/types.ts";

let t: Target & { hosts?: Map<string, SimHost> };
let door = true;
const NO_DOOR = "the queue doors (S1) are not on this gateway";
const seen: string[] = [];
before(async () => {
  t = await startTarget();
  door = (await fetch(`${t.url}/api/i/${t.instance}/v1/queue`)).status !== 404;
  // The mock must have the door: a skip is only for a real gateway that does not have it yet.
  if (!door && t.hosts) throw new Error("the mock host serves no /v1/queue");
});
after(async () => { await t?.stop(); });

const api = (path: string, inst = t.instance) => `${t.url}/api/i/${inst}${path}`;
async function call(path: string, init?: RequestInit, inst = t.instance) {
  const r = await fetch(api(path, inst), init);
  const text = await r.text();
  seen.push([...r.headers].map(([k, v]) => `${k}: ${v}`).join("\n"), text);
  return { status: r.status, json: () => JSON.parse(text), text };
}
const post = (body: unknown, headers: Record<string, string> = {}, inst = t.instance) =>
  call("/v1/queue", { method: "POST", headers: { "content-type": "application/json", ...headers }, body: JSON.stringify(body) }, inst);
interface Item { id: string; text: string; role: string; name: string | null; channel: string; kind: string; intent: string; order: number; position: number | null; accepted_at: number | null; state: string; turn: number | null; disposition: string | null; held: string | null }
const list = async (q = ""): Promise<{ items: Item[]; [k: string]: unknown }> => (await call(`/v1/queue${q}`)).json();
const record = async (): Promise<Line[]> => (await call("/v1/record?offset=0&limit=100000000")).json().lines;
const accepted = async (id: string) => (await record()).filter((l) => l.kind === "stimulus.accepted" && l.item.id === id);

test("the list has the documented fields: items with a state, the page, the counts", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const q = await list();
  assert.ok(Array.isArray(q.items));
  for (const k of ["total", "offset", "limit", "record_lines"]) assert.equal(typeof q[k], "number", k);
  assert.ok(q.next === null || typeof q.next === "number");
  const counts = q.counts as Record<string, number>;
  for (const s of ["waiting", "answering", "answered", "withdrawn"]) assert.equal(typeof counts[s], "number", s);
  assert.deepEqual(Object.keys((q.redaction as { env_unset: unknown[] })), ["env_unset"]);
  for (const i of q.items) {
    for (const k of ["id", "text", "role", "channel", "kind", "intent", "state"]) assert.equal(typeof i[k as keyof Item], "string", k);
    assert.ok(i.name === null || typeof i.name === "string");
    assert.equal(typeof i.order, "number");
    assert.ok(i.position === null || typeof i.position === "number");
    assert.ok(i.accepted_at === null || typeof i.accepted_at === "number");
    assert.ok(["waiting", "answering"].includes(i.state), "the default list is the open states");
    assert.ok(i.turn === null || typeof i.turn === "number");
    assert.ok(i.disposition === null || typeof i.disposition === "string");
    assert.ok(i.held === null || typeof i.held === "string");
  }
});

test("an added item is on the record before the answer, and in the list after it", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const key = randomUUID();
  const r = await post({ key, text: "also include the appendix", intent: "next" });
  assert.equal(r.status, 201);
  assert.equal(r.json().id, key, "the key is the item's id");
  assert.equal(r.json().repeat, false);
  assert.equal(r.json().item.state, "waiting");
  const lines = await accepted(key);
  assert.equal(lines.length, 1, "the host wrote it down before it answered");
  assert.equal(lines[0].item.role, "owner");
  assert.equal(lines[0].item.text, "also include the appendix");
  const mine = (await list()).items.find((i) => i.id === key)!;
  assert.equal(mine.role, "owner");
  assert.equal(mine.intent, "next");
  assert.equal(mine.state, "waiting");
  assert.equal(typeof mine.position, "number");
  assert.equal(mine.order, lines[0].seq, "order is the number of the acceptance line");
});

test("an item with no intent is read as next, and the key is optional", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const r = await post({ text: "no key, no intent" });
  assert.equal(r.status, 201);
  assert.equal(r.json().item.intent, "next");
  assert.equal(typeof r.json().id, "string");
});

test("the same key twice adds once; the second answer says it was a repeat", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const key = randomUUID();
  const a = await post({ key, text: "once" });
  const b = await post({ key, text: "once" });
  assert.equal(a.status, 201);
  assert.equal(b.status, 200);
  assert.equal(b.json().repeat, true);
  assert.equal((await accepted(key)).length, 1);
  assert.equal((await list()).items.filter((i) => i.id === key).length, 1);
});

test("a key that already names another message is 409 key_reused, and the first text stands", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const key = randomUUID();
  await post({ key, text: "first" });
  const r = await post({ key, text: "second" });
  assert.equal(r.status, 409);
  assert.equal(r.json().error, "key_reused");
  assert.equal((await accepted(key))[0].item.text, "first");
});

test("the text is kept whole, however long, with its line breaks", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const key = randomUUID();
  const text = Array.from({ length: 3000 }, (_, i) => `line ${i} of a long brief\n\tindented`).join("\n");
  assert.equal((await post({ key, text })).status, 201);
  assert.equal((await list()).items.find((i) => i.id === key)!.text, text);
});

test("a bad request is refused in words and writes nothing: blank text, a bad key, a field it does not know, an intent not served yet", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const before = (await record()).length;
  const bad: [unknown, number, string][] = [
    [{ text: "" }, 400, "bad_request"], [{ text: "   " }, 400, "bad_request"], [{ key: randomUUID() }, 400, "bad_request"],
    [{ key: "-starts-with-a-dash", text: "x" }, 400, "bad_request"], [{ key: "has a space", text: "x" }, 400, "bad_request"],
    [{ text: "x", colour: "red" }, 400, "bad_request"],
    [{ text: "x", intent: "steer" }, 400, "bad_request"], [{ text: "x", intent: "free" }, 400, "bad_request"], [{ text: "x", intent: "later" }, 400, "bad_request"],
  ];
  for (const [body, status, error] of bad) {
    const r = await post(body);
    assert.equal(r.status, status, JSON.stringify(body));
    assert.equal(r.json().error, error);
    assert.equal(typeof r.json().message, "string");
  }
  assert.equal((await call("/v1/queue", { method: "POST", headers: { "content-type": "application/json" }, body: "{nope" })).status, 400);
  assert.equal((await call("/v1/queue", { method: "POST", headers: { "content-type": "text/plain" }, body: "hello" })).status, 415);
  assert.equal((await record()).length, before, "nothing was written");
});

test("the list is ordered for a page: the owner's items above a peer's, each in order of arrival", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const host = t.hosts?.get(t.instance);
  const peer = host?.addPeer("a peer wrote first", "atlas");
  const key = randomUUID();
  if (host) await post({ key, text: "the owner wrote second" });
  const items = (await list()).items.filter((i) => i.state === "waiting");
  if (host) {
    const ids = items.map((i) => i.id);
    assert.ok(ids.indexOf(key) >= 0 && ids.indexOf(peer!) >= 0 && ids.indexOf(key) < ids.indexOf(peer!), "the owner's item is above the peer's");
  }
  const roles = items.map((i) => i.role);
  const firstOther = roles.findIndex((r) => r !== "owner");
  if (firstOther >= 0) assert.ok(roles.slice(firstOther).every((r) => r !== "owner"), "no owner item sits below another sender's");
  items.forEach((i, n) => assert.equal(i.position, n + 1, "position is the 1-based place among those that wait"));
});

test("a peer's item names its sender, and the desk's hold is said in words", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const host = t.hosts?.get(t.instance);
  if (!host) return ctx.skip("no control hook on this target");
  const id = host.addPeer("a note from atlas", "atlas", "at_break");
  const item = (await list()).items.find((i) => i.id === id)!;
  assert.equal(item.role, "peer");
  assert.equal(item.name, "atlas");
  assert.equal(item.channel, "peer:atlas");
  assert.match(item.held ?? "", /decision desk/);
});

test("an item taken into a running turn is answering, with its turn; settled, it leaves the default list and keeps its disposition", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const host = t.hosts?.get(t.instance);
  if (!host) return ctx.skip("no control hook on this target");
  const key = randomUUID();
  await post({ key, text: "answer this" });
  const run = host.runTurn({ pace: 40, kind: "responding", take: [key] });
  await new Promise((r) => setTimeout(r, 120));
  const mid = (await list()).items.find((i) => i.id === key)!;
  assert.equal(mid.state, "answering");
  assert.equal(typeof mid.turn, "number");
  assert.equal(mid.position, null);
  await run;
  assert.ok(!(await list()).items.some((i) => i.id === key), "answered: no longer in the open list");
  const over = (await list("?state=answered")).items.find((i) => i.id === key)!;
  assert.equal(over.state, "answered");
  assert.equal(over.disposition, "answered");
  assert.equal(typeof ((await list("?state=all")).counts as Record<string, number>).answered, "number");
});

test("the list pages: limit and offset, an exact total, next; a query it does not know is 400", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  for (let i = 0; i < 3; i++) await post({ text: `paging ${i}` });
  const all = await list("?state=all&limit=100000");
  assert.equal(all.next, null);
  assert.equal(all.total, all.items.length);
  const one = await list("?state=all&limit=1&offset=1");
  assert.equal(one.items.length, 1);
  assert.equal(one.items[0].id, all.items[1].id);
  assert.equal(one.next, 2);
  assert.equal(one.total, all.total);
  for (const bad of ["?state=nope", "?limit=0", "?limit=x", "?colour=red", "?limit=1&limit=2"]) assert.equal((await call(`/v1/queue${bad}`)).status, 400, bad);
});

test("a key typed into the text is redacted at the door", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  await post({ key: randomUUID(), text: "use sk-or-v1-0123456789abcdef0123456789abcdef for it" });
  assert.ok(!(await call("/v1/queue?state=all&limit=100000")).text.includes("0123456789abcdef"));
});

test("the read-only token may read the queue and may not add to it", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  if (!t.readOnlyToken) return ctx.skip("this target has no read-only token");
  const h = { authorization: `Bearer ${t.readOnlyToken}` };
  assert.equal((await call("/v1/queue", { headers: h })).status, 200);
  const r = await post({ key: randomUUID(), text: "x" }, h);
  assert.equal(r.status, 403);
  assert.equal(r.json().error, "read_only");
});

test("an instance that is not running lists, and adding to it is 409 not_running: start it first", async (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  const host = t.hosts?.get(t.instance);
  if (!host && !t.control) return ctx.skip("no control hook on this target");
  if (host) host.stop(); else await t.control!.halt();
  const lines = (await record()).length;
  const r = await post({ key: randomUUID(), text: "too late" });
  assert.equal(r.status, 409);
  assert.equal(r.json().error, "not_running");
  assert.match(r.json().message, /start it first/i);
  assert.equal((await record()).length, lines, "nothing was written");
  assert.equal((await call("/v1/queue")).status, 200);
});

test("no key in any answer or header", (ctx) => {
  if (!door) return ctx.skip(NO_DOOR);
  if (!t.hostKey) return ctx.skip("a real gateway's key is not known to the test");
  for (const s of seen) assert.ok(!s.includes(t.hostKey), "the host's key never reaches the page");
});
