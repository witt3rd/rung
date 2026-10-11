// The live proof against the real gateway binary (the H3 doors): the real app in a real browser over `rung-gateway` serving a registry folder.
//   cargo build -p rung-gateway && node --experimental-strip-types census/real-proof.ts      (needs a built dist/)
// What it shows is what the doors give today: the list (a stopped one with its start command, a dead one, an entry that cannot be read),
// the fold for the state word, the record paged. There is no event stream yet, so nothing is followed: the page says so.
import { chromium, VIEWS, here } from "./lib.mjs";
import { measure, judge } from "./measure.mjs";
import { startReal } from "../mock/real.ts";
import { join } from "node:path";
import { mkdirSync } from "node:fs";

const shots = join(here, "shots");
mkdirSync(shots, { recursive: true });
const real = await startReal({ appDir: join(here, "../dist"), extra: true });
let browser!: import("playwright").Browser;
const out: string[] = [];
const say = (s: string) => { console.log(s); out.push(s); };
let failed = 0;
const check = (ok: boolean, what: string) => { say(`${ok ? "pass" : "FAIL"}  ${what}`); if (!ok) failed++; };
try {
  browser = await chromium.launch();
  await real.control!.poke(6);

  const pages = [
    ["real-instances", "#/"],
    ["real-now-stopped", "#/i/gorse"],
    ["real-turns", "#/i/alpha/turns"],
  ] as const;
  for (const [vn, vp] of Object.entries(VIEWS)) {
    const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: vp.deviceScaleFactor });
    for (const [name, hash] of pages) {
      const page = await ctx.newPage();
      await page.goto(`${real.url}/${hash}`, { waitUntil: "load" });
      await page.waitForSelector('.page[data-ready="true"]', { timeout: 20000 });
      await page.evaluate(() => document.fonts.ready);
      if (name === "real-turns") await page.waitForFunction(() => document.body.innerText.includes("Recorded, not followed"), null, { timeout: 15000 });
      const text = await page.evaluate(() => document.body.innerText);
      if (vn === "w1920") {
        if (name === "real-instances") {
          check(/gorse/.test(text) && /fir/.test(text) && /alpha/.test(text) && /\bbad\b/.test(text), "the list holds every registered instance: a stopped one, a dead one, a live-looking one and an entry that cannot be read");
          check(/Unreadable/.test(text), "the entry that cannot be read is listed as Unreadable, never dropped");
        }
        if (name === "real-now-stopped") check(/rung-host run --config/.test(text) && /does not start it/.test(text), "a stopped instance shows the command that starts it, and says the page does not");
        if (name === "real-turns") {
          const shown = await page.$$eval("article.turn h2", (hs) => hs.length);
          check(shown === 3, `the record read through the real door is the record: ${shown} turns shown, 3 in the file`);
        }
      }
      let height = vp.height;
      if (vp.width > 1000) height = Math.min(vp.height, Math.ceil(await page.evaluate(() => document.querySelector(".page")!.getBoundingClientRect().bottom)) + 32);
      // The stopped instance's page is taken whole at 390, so the start command is not cut off at the bottom of the phone's first screen.
      if (name === "real-now-stopped" && vn === "w390") await page.screenshot({ path: join(shots, `${name}-${vn.slice(1)}.png`), fullPage: true });
      else await page.screenshot({ path: join(shots, `${name}-${vn.slice(1)}.png`), clip: { x: 0, y: 0, width: vp.width, height } });
      const r = await measure(page, vp);
      const bad = judge(r);
      check(bad.length === 0, `census, ${name} at ${vn.slice(1)}: chrome ${r.words} words, ${bad.length ? bad.join("; ") : "within every budget"}`);
      await page.close();
    }
    await ctx.close();
  }

} finally {
  await browser?.close();
  await real.stop();
}
say(failed ? `real-gateway proof: ${failed} failure(s)` : "real-gateway proof: all pass");
process.exit(failed ? 1 : 0);
