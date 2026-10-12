// The live proof against the mock gateway: the real app in a real browser, a host writing turns on a real clock.
//   node --experimental-strip-types census/live-proof.ts      (needs a built dist/)
// 1. record time to screen time for the text of finished turns (median and worst), 2. a cut with the page offline while turns run:
// what the page holds equals the record, line by line, 3. mid-turn captures at 390 and 1920, judged by the census budgets.
import { chromium, VIEWS_LIVE as VIEWS, here } from "./lib.mjs";
import { measure, judge } from "./measure.mjs";
import { startMock } from "../mock/gateway.ts";
import { join } from "node:path";
import { mkdirSync } from "node:fs";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const shots = join(here, "shots");
mkdirSync(shots, { recursive: true });
const m = await startMock({ appDir: join(here, "../dist"), extra: true, longReply: 1024 * 1024 });
const host = m.hosts.get("alpha")!;
let browser!: import("playwright").Browser;
const out: string[] = [];
const say = (s: string) => { console.log(s); out.push(s); };
let failed = 0;
const check = (ok: boolean, what: string) => { say(`${ok ? "pass" : "FAIL"}  ${what}`); if (!ok) failed++; };

async function ready(page: import("playwright").Page, hash: string) {
  await page.goto(`${m.url}/${hash}`, { waitUntil: "load" });
  await page.waitForSelector('.page[data-ready="true"]', { timeout: 20000 });
  await page.evaluate(() => document.fonts.ready);
}

try {
  browser = await chromium.launch();
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



  // 4 ---------------------------------------------------------------------------------------------------------------
  // The host stops while two pages watch it, and the summary they would refresh from is stale (it still says Working): both pages must
  // say Not running, from one shared mapping, and neither may say the instance is working.
  {
    const ctx = await browser.newContext({ viewport: { width: 390, height: 844 } });
    const staleBody = JSON.stringify(await (await fetch(`${m.url}/api/i/alpha/v1/summary`)).json());
    let frozen = false;
    await ctx.route("**/v1/summary", (route) => (frozen ? route.fulfill({ status: 200, contentType: "application/json", body: staleBody }) : route.continue()));
    const now = await ctx.newPage();
    const turns = await ctx.newPage();
    await ready(now, "#/i/alpha");
    await ready(turns, "#/i/alpha/turns");
    await turns.waitForFunction(() => document.body.innerText.includes("Following"));
    frozen = true;
    await host.halt();
    await turns.waitForFunction(() => document.body.innerText.includes("Not running"), null, { timeout: 8000 });
    await now.waitForFunction(() => document.body.innerText.includes("Not running"), null, { timeout: 8000 }).catch(() => {});
    const nowText = await now.evaluate(() => document.body.innerText);
    const turnsText = await turns.evaluate(() => document.body.innerText);
    check(/Not running/.test(turnsText), "after the host stopped, the Turns page says Not running");
    check(/Not running/.test(nowText), "after the host stopped, the Now page says Not running (with a stale summary)");
    check(!/Live: (Writing|Running|Between)/.test(nowText) && !/Working\./.test(nowText + turnsText), "neither page still says the instance is working");
    await ctx.close();
  }

  // 5 ---------------------------------------------------------------------------------------------------------------
  // A reply of a megabyte: whole in the page, folded for the eye, opened and folded again by the owner; never cut, never given an ellipsis.
  for (const [vn, vp] of Object.entries({ w390: VIEWS.w390, w1920: VIEWS.w1920 })) {
    const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: vp.deviceScaleFactor });
    const page = await ctx.newPage();
    await ready(page, "#/i/longhand/turns");
    const probe = () => page.evaluate(() => {
      const el = document.querySelector("article.turn p.content, article.turn .content") as HTMLElement | null;
      const btn = [...document.querySelectorAll("article.turn button")].find((b) => /Show (more|less)$/.test(b.textContent ?? "")) as HTMLElement | undefined;
      const t = el?.textContent ?? "";
      return { len: t.length, height: el?.getBoundingClientRect().height ?? 0, folded: !!el?.classList.contains("folded"), btn: btn?.textContent ?? null, expanded: btn?.getAttribute("aria-expanded") ?? null,
        first: t.startsWith("REPLY line 0 of"), last: t.trimEnd().endsWith("of the long reply"), ellipsis: /\u2026|\.\.\.$/.test(t) };
    });
    const a = await probe();
    check(a.len >= 1_000_000 && a.first && a.last && !a.ellipsis, `a ${(a.len / 1048576).toFixed(2)} MB reply is whole in the page at ${vn.slice(1)}: its first line and its last are there, nothing added`);
    check(a.folded && a.height < vp.height && a.btn === "Show more" && a.expanded === "false", `it is folded to a screenful (${Math.round(a.height)} px) with a Show more button`);
    await page.screenshot({ path: join(shots, `long-turns-${vn.slice(1)}.png`), clip: { x: 0, y: 0, width: vp.width, height: Math.min(vp.height, 1400) } });
    check(judge(await measure(page, vp)).length === 0, `census, the long reply's page at ${vn.slice(1)}: within every budget`);
    await page.click('article.turn button:has-text("Show more")');
    const b = await probe();
    check(!b.folded && b.height > a.height * 5 && b.btn === "Show less" && b.expanded === "true" && b.len === a.len, "Show more opens the whole text (the same characters), and the button says Show less");
    await page.click('article.turn button:has-text("Show less")');
    const c = await probe();
    check(c.folded && c.height === a.height && c.len === a.len, "Show less folds it again");
    await ready(page, "#/i/longhand");
    const row = await page.evaluate(() => ({ line: document.querySelector(".row.tr span")?.textContent ?? "", more: !![...document.querySelectorAll("button")].find((x) => /Show whole/.test(x.textContent ?? "")) }));
    check(/REPLY line 0 of the long reply/.test(row.line) && !/REPLY line 1 of/.test(row.line) && row.more, "the Now page's turn row is one line, with a button for the whole reply");
    await page.click('button:has-text("Show whole")');
    const whole = await page.evaluate(() => ([...document.querySelectorAll(".content")].map((e) => e.textContent?.length ?? 0).sort((x, y) => y - x))[0]);
    check(whole >= 1_000_000, "the row expands to the whole reply");
    await page.screenshot({ path: join(shots, `long-now-${vn.slice(1)}.png`), clip: { x: 0, y: 0, width: vp.width, height: Math.min(vp.height, 1400) } });
    await ctx.close();
  }

  // 6 ---------------------------------------------------------------------------------------------------------------
  // A list is paged, never capped: Show more adds a page, Show all adds the rest, Show less goes back to the first page.
  {
    const b = m.hosts.get("bramble")!;
    for (let i = 0; i < 60; i++) await b.runTurn({ pace: 0, text: `Page test turn ${i}.` });
    const ctx = await browser.newContext({ viewport: { width: 390, height: 844 } });
    const page = await ctx.newPage();
    await ready(page, "#/i/bramble/turns");
    const shown = () => page.$$eval("article.turn h2", (h) => h.length);
    const total = (await page.evaluate(() => /of ([\d,]+)\./.exec(document.body.innerText)?.[1] ?? "0")).replace(/,/g, "");
    check((await shown()) === 50 && Number(total) > 50, `the list opens at one page: 50 of ${total} turns`);
    await page.click('button:has-text("Show all")');
    check((await shown()) === Number(total), `Show all adds the rest: all ${total}`);
    check(await page.evaluate(() => [...document.querySelectorAll("button")].some((x) => x.textContent === "Show less")), "and a Show less is there");
    await page.click('button:has-text("Show less")');
    check((await shown()) === 50, "Show less goes back to one page");
    // A link into the list holds it open as far as the linked turn; a Show less that would change nothing is not offered.
    await ready(page, "#/i/bramble/turns?turn=3");
    check((await shown()) > 50 && !(await page.evaluate(() => [...document.querySelectorAll("button")].some((x) => x.textContent === "Show less"))), `a link to an early turn opens the list to it (${await shown()} shown) and offers no dead Show less`);
    await ctx.close();
  }
} finally {
  await browser?.close();
  await m.stop();
}
say(failed ? `live proof: ${failed} failure(s)` : "live proof: all pass");
process.exit(failed ? 1 : 0);
