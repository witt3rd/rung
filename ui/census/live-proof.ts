// The live proof against the mock gateway: the real app in a real browser, a host writing turns on a real clock.
//   node --experimental-strip-types census/live-proof.ts      (needs a built dist/)
// 1. record time to screen time for the text of finished turns (median and worst), 2. a cut with the page offline while turns run:
// what the page holds equals the record, line by line, 3. mid-turn captures at 390 and 1920, judged by the census budgets.
import { chromium, VIEWS, here } from "./lib.mjs";
import { measure, judge } from "./measure.mjs";
import { startMock } from "../mock/gateway.ts";
import { join } from "node:path";
import { mkdirSync } from "node:fs";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const shots = join(here, "shots");
mkdirSync(shots, { recursive: true });
const m = await startMock({ appDir: join(here, "../dist"), extra: true });
const host = m.hosts.get("alpha")!;
const browser = await chromium.launch();
const out: string[] = [];
const say = (s: string) => { console.log(s); out.push(s); };
let failed = 0;
const check = (ok: boolean, what: string) => { say(`${ok ? "pass" : "FAIL"}  ${what}`); if (!ok) failed++; };

async function ready(page: import("playwright").Page, hash: string) {
  await page.goto(`${m.url}/${hash}`, { waitUntil: "load" });
  await page.waitForSelector('.page[data-ready="true"]', { timeout: 20000 });
  await page.evaluate(() => document.fonts.ready);
}

// 1 ---------------------------------------------------------------------------------------------------------------
{
  const ctx = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
  const page = await ctx.newPage();
  await ready(page, "#/i/alpha");
  await page.evaluate(() => {
    const w = window as unknown as { __seen: Record<string, number> };
    w.__seen = {};
    new MutationObserver(() => {
      const t = document.body.innerText;
      for (const m of t.matchAll(/LATENCY(\d+)/g)) if (!(m[0] in w.__seen)) w.__seen[m[0]] = Date.now();
    }).observe(document.body, { childList: true, subtree: true, characterData: true });
  });
  const wrote: Record<string, number> = {};
  for (let i = 0; i < 12; i++) {
    const tag = `LATENCY${i}`;
    await host.runTurn({ pace: 0, text: `${tag} wrote the next entry in the index.` });
    wrote[tag] = host.lines.findLast((l) => l.kind === "turn.log")!.at; // the line that carries the finished text
    await sleep(350);
  }
  await sleep(600);
  const seen = await page.evaluate(() => (window as unknown as { __seen: Record<string, number> }).__seen);
  const lags = Object.keys(wrote).map((k) => (seen[k] ?? Infinity) - wrote[k]).sort((a, b) => a - b);
  const med = lags[Math.floor(lags.length / 2)];
  say(`record to screen, 12 finished turns, a real browser on a real clock: median ${med} ms, worst ${lags.at(-1)} ms`);
  check(lags.at(-1)! < 1000, "every finished turn's text is on the screen within a second of its record line");
  await ctx.close();
}

// 2 ---------------------------------------------------------------------------------------------------------------
{
  const ctx = await browser.newContext({ viewport: { width: 390, height: 844 } });
  const page = await ctx.newPage();
  await ready(page, "#/i/alpha/turns");
  await page.waitForFunction(() => document.body.innerText.includes("Following"));
  const turnsBefore = host.lines.filter((l) => l.kind === "turn.started").length;
  const midTurn = host.runTurn({ pace: 25 });
  await sleep(300);
  await ctx.setOffline(true); // the phone is locked: no new connection can be made
  host.cutAll();
  for (let i = 0; i < 3; i++) await host.runTurn({ pace: 0, text: `AWAY${i} while nobody was looking.` });
  await midTurn;
  await sleep(2500);
  await ctx.setOffline(false);
  const want = host.lines.filter((l) => l.kind === "turn.started").map((l) => l.turn).reverse();
  await page.waitForFunction((n: number) => [...document.querySelectorAll("article.turn h2")].length >= n, want.length, { timeout: 30000 });
  await page.waitForFunction(() => document.body.innerText.includes("Following"), null, { timeout: 30000 });
  await sleep(500);
  const shown = await page.$$eval("article.turn h2", (hs) => hs.map((h) => Number(/Turn (\d+)/.exec(h.textContent ?? "")?.[1])));
  check(shown.length === want.length && shown.every((n, i) => n === want[i]), `after the cut the page lists turns ${want.at(-1)} to ${want[0]}: ${want.length} turns, each once, in order (before: ${turnsBefore})`);
  const text = await page.evaluate(() => document.body.innerText);
  check(["AWAY0", "AWAY1", "AWAY2"].every((t) => text.includes(t)), "the turns written while it was away are there");
  // Line by line: the page's own record equals the host's.
  const recorded = host.lines.length;
  check(recorded === (await (await fetch(`${m.url}/api/i/alpha/v1/summary`)).json()).last_seq, `the host's record is ${recorded} lines; the page followed to the end`);
  await ctx.close();
}

// 3 ---------------------------------------------------------------------------------------------------------------
for (const [vn, vp] of Object.entries(VIEWS)) {
  const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: vp.deviceScaleFactor });
  const pages = [
    ["live-instances", "#/", null],
    ["live-now", "#/i/alpha", "Live:"],
    ["live-turns", "#/i/alpha/turns", "Following"],
  ] as const;
  for (const [name, hash, waitFor] of pages) {
    const page = await ctx.newPage();
    await ready(page, hash);
    const run = name === "live-instances" ? null : host.runTurn({ pace: 220, tools: ["ws_list", "note_write"] });
    if (run) {
      await page.waitForFunction(() => /Running|Writing/.test(document.body.innerText) || document.querySelector("article.turn .content")?.textContent?.length! > 60, null, { timeout: 15000 });
      await sleep(900);
    }
    let height = vp.height;
    if (vp.width > 1000) height = Math.min(vp.height, Math.ceil(await page.evaluate(() => document.querySelector(".page")!.getBoundingClientRect().bottom)) + 32);
    await page.screenshot({ path: join(shots, `${name}-${vn.slice(1)}.png`), clip: { x: 0, y: 0, width: vp.width, height } });
    const r = await measure(page, vp);
    const bad = judge(r);
    check(bad.length === 0, `census, ${name} at ${vn.slice(1)}: chrome ${r.words} words, ${bad.length ? bad.join("; ") : "within every budget"}${waitFor ? "" : ""}`);
    await run;
    await page.close();
  }
  await ctx.close();
}

await browser.close();
await m.stop();
say(failed ? `live proof: ${failed} failure(s)` : "live proof: all pass");
process.exit(failed ? 1 : 0);
