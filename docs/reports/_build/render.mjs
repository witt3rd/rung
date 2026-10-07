// Render build/deck.html to a vector PDF and one PNG per page, plus a layout lint
// (nothing may leave its page or run into the footer). Playwright resolution follows
// the explainer's render.mjs: PLAYWRIGHT_MODULE, then the mise-pinned 1.62.1.
import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
const require = createRequire(import.meta.url);
const candidates = [
  process.env.PLAYWRIGHT_MODULE,
  join(homedir(), ".local/share/mise/installs/npm-playwright/1.62.1/node_modules/playwright"),
].filter(Boolean);
let chromium;
for (const c of candidates) { try { ({ chromium } = require(c)); break; } catch {} }
if (!chromium) throw new Error(`Playwright not found in: ${candidates.join(", ")}`);
const build = resolve(process.argv[2] ?? "build");
mkdirSync(join(build, "png"), { recursive: true });
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
await page.goto("file://" + join(build, "deck.html"), { waitUntil: "load" });
await page.evaluate(() => document.fonts.ready);
const problems = await page.evaluate(() => {
  const out = [];
  document.querySelectorAll(".slide").forEach((s, i) => {
    const S = s.getBoundingClientRect(), floor = S.bottom - 72;
    const body = s.querySelector(".body");
    if (body) {
      let max = 0;
      body.querySelectorAll("*").forEach((e) => { const r = e.getBoundingClientRect(); if (r.height) max = Math.max(max, r.bottom); });
      if (max > floor + 1) out.push(`page ${i + 1}: content runs into the footer by ${Math.round(max - floor)}px`);
      const h = [...s.querySelectorAll("h1,h2")].map((e) => e.getBoundingClientRect().bottom);
      const top = body.getBoundingClientRect().top;
      if (h.some((b) => b > top + 1)) out.push(`page ${i + 1}: heading overlaps body`);
    }
    s.querySelectorAll("*").forEach((e) => {
      const r = e.getBoundingClientRect();
      if (r.width && r.height && (r.right > S.right + 1 || r.bottom > S.bottom + 1)) out.push(`page ${i + 1}: <${e.tagName}> leaves the page`);
    });
  });
  return [...new Set(out)];
});
if (problems.length) { console.error(problems.join("\n")); }
await page.pdf({ path: join(build, "deck.pdf"), preferCSSPageSize: true, printBackground: true });
const n = await page.evaluate(() => document.querySelectorAll(".slide").length);
for (let i = 0; i < n; i++) {
  const el = (await page.$$(".slide"))[i];
  await el.screenshot({ path: join(build, "png", `p${String(i + 1).padStart(2, "0")}.png`) });
}
await browser.close();
if (problems.length) process.exit(1);
