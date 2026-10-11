import { test } from "node:test";
import assert from "node:assert/strict";
import { loadLiveIndex } from "../src/data/source.ts";

// The rows the real gateway lists (docs/rung-host-api.md, H3): a native row has `word`, `source`, `start_command`, and a summary that is the fold.
const row = (over: Record<string, unknown>) => ({ id: "alpha", name: "Alpha", source: "registry", reachable: true, word: "stopped", last_at: 5, start_command: null, error: null, summary: null, ...over });
const fold = (over: Record<string, unknown> = {}) => ({ contract: 1, id: "alpha", name: "Alpha", word: "stopped", state: "Stopped", doing: "Reached its run limit.", needs_you: false, turn: 4, last_seq: 9, last_at: 1791208194000, requests: 1, quota: 400, spend_usd_day: 0, day: "2026-10-05", ...over });
const fetchOf = (instances: unknown[]) => (async () => new Response(JSON.stringify({ instances, registry_error: null }), { status: 200 })) as typeof fetch;

test("a stopped native instance shows how to start it, and is not marked as needing the owner", async () => {
  const idx = await loadLiveIndex("api", fetchOf([row({ start_command: "rung-host run --config '/srv/alpha/rung-host.yaml'", summary: fold() })]));
  const e = idx.instances[0];
  assert.equal(e.startCommand, "rung-host run --config '/srv/alpha/rung-host.yaml'");
  assert.equal(e.summary.state, "Stopped");
  assert.equal(e.summary.needsYou, false);
  assert.equal(e.lockHeld, false);
});

test("a running native instance holds its lock; a down one needs the owner", async () => {
  const idx = await loadLiveIndex("api", fetchOf([
    row({ id: "a", word: "running", summary: fold({ state: "Working", word: "running", doing: "Committed to notes" }) }),
    row({ id: "b", word: "down", summary: fold({ state: "Down", word: "down", needs_you: true, doing: "It died" }) }),
  ]));
  assert.equal(idx.instances[0].lockHeld, true);
  assert.equal(idx.instances[1].lockHeld, false);
  assert.equal(idx.instances[1].summary.needsYou, true);
});

test("an entry the gateway could not read is listed as Unreadable with the reason, and needs the owner", async () => {
  const idx = await loadLiveIndex("api", fetchOf([row({ id: "bad", name: "bad.json", reachable: false, word: "unreadable", error: "the entry is not valid JSON" })]));
  const e = idx.instances[0];
  assert.equal(e.summary.state, "Unreadable");
  assert.equal(e.summary.doing, "Its registry entry cannot be read.", "a sentence, not a path");
  assert.equal(e.problem, "the entry is not valid JSON", "the reason is kept for the row to offer");
  assert.equal(e.summary.needsYou, true);
  assert.equal(e.reachable, false);
});
