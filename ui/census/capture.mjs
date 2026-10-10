// Screenshot every page at 390 (phone, 2x) and 1920 (desktop): first screen. At 1920 the shot stops 32 px under the content
// (or at 1080). With --full, also a full-length capture of each page, for looking at what the first screen does not show.
import { chromium, PAGES, VIEWS, serve, open, here } from "./lib.mjs";
import { join } from "node:path";
import { mkdirSync } from "node:fs";

const full = process.argv.includes("--full");
const outDir = process.env.RUNG_UI_SHOTS ?? join(here, "shots");
mkdirSync(outDir, { recursive: true });
const srv = await serve();
const browser = await chromium.launch();
let n = 0;
for (const [vn, vp] of Object.entries(VIEWS)) {
  const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: vp.deviceScaleFactor });
  for (const p of PAGES) {
    const page = await open(ctx, srv.url, p.hash);
    let height = vp.height;
    if (vp.width > 1000) height = Math.min(vp.height, Math.ceil(await page.evaluate(() => document.querySelector(".page").getBoundingClientRect().bottom)) + 32);
    await page.screenshot({ path: join(outDir, `${p.name}-${vn.slice(1)}.png`), clip: { x: 0, y: 0, width: vp.width, height } });
    n++;
    // A page with a focus is also captured as that element: what a link to one turn lands on.
    if (p.focus && full) { await page.locator(p.focus).screenshot({ path: join(outDir, `${p.name}-${vn.slice(1)}-focus.png`) }); n++; }
    if (full) { await page.screenshot({ path: join(outDir, `${p.name}-${vn.slice(1)}-full.png`), fullPage: true }); n++; }
    await page.close();
  }
  await ctx.close();
}
await browser.close(); srv.close();
console.log("captured", n, "into", outDir);
