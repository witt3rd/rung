// One capture of a live page at each of the census's four viewports (lib.mjs: VIEWS_LIVE), each in a context of its own so the device scale
// is the viewport's own: the page is loaded afresh, scrolled if asked, captured whole and judged by the census budgets.
import { VIEWS_LIVE, here } from "./lib.mjs";
import { measure, judge } from "./measure.mjs";
import { join } from "node:path";
import { mkdirSync } from "node:fs";

export async function shootViews(browser, url, name, { ready, scroll, report, sizes } = {}) {
  const out = join(here, "shots");
  mkdirSync(out, { recursive: true });
  for (const vp of Object.values(VIEWS_LIVE)) {
    if (sizes && !sizes.includes(vp.width)) continue;
    const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: vp.deviceScaleFactor });
    const page = await ctx.newPage();
    await page.goto(url, { waitUntil: "load" });
    await page.waitForSelector('.page[data-ready="true"]', { timeout: 20000 });
    await page.evaluate(() => document.fonts.ready);
    if (ready) await ready(page);
    if (scroll) await page.evaluate(scroll.fn, scroll.arg); else await page.evaluate(() => window.scrollTo(0, 0));
    await page.screenshot({ path: join(out, `${name}-${vp.width}.png`), fullPage: true });
    const r = await measure(page, vp);
    const bad = judge(r);
    report(bad.length === 0, `census, ${name} at ${vp.width}: chrome ${r.words} words, ${bad.length ? bad.join("; ") : "within every budget"}`);
    await ctx.close();
  }
}
