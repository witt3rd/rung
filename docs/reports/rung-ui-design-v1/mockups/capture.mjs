// Screenshot every mockup at 390 (phone) and 1920 (desktop): first screen only.
import { chromium, PAGES, VIEWS } from "./lib.mjs";
import { resolve, join } from "node:path";
import { mkdirSync } from "node:fs";
const here = resolve(new URL(".", import.meta.url).pathname);
mkdirSync(join(here, "shots"), { recursive: true });
const browser = await chromium.launch();
for (const [vn, vp] of Object.entries(VIEWS)) {
  const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: vp.deviceScaleFactor });
  for (const p of PAGES) {
    const page = await ctx.newPage();
    await page.goto("file://" + join(here, p + ".html"), { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    // First screen only. At 1920 the shot stops 32 px under the content (or at 1080), so the page is not mostly empty.
    let height = vp.height;
    if (vp.width > 1000) height = Math.min(vp.height, Math.ceil(await page.evaluate(() => document.querySelector(".page").getBoundingClientRect().bottom)) + 32);
    await page.screenshot({ path: join(here, "shots", `${p}-${vn.slice(1)}.png`), clip: { x: 0, y: 0, width: vp.width, height } });
    await page.close();
  }
  await ctx.close();
}
await browser.close();
console.log("captured", PAGES.length * 2);
