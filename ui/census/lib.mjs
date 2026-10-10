// Shared by the census and the capture: serves the built app, lists the pages and the two widths.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize, resolve } from "node:path";
export { chromium } from "playwright";

export const here = resolve(new URL(".", import.meta.url).pathname);
export const dist = process.env.RUNG_UI_DIST ?? join(here, "../dist");
export const VIEWS = { w1920: { width: 1920, height: 1080, deviceScaleFactor: 1 }, w390: { width: 390, height: 844, deviceScaleFactor: 2 } };

// Each page is a hash route. One recorded run and two synthetic instances stand for the states.
export const PAGES = [
  { name: "instances", hash: "#/" },
  { name: "now-recorded", hash: "#/i/2026-10-05-qwen-2h" },
  { name: "now-working", hash: "#/i/atlas" },
  { name: "now-stuck", hash: "#/i/cedar" },
  { name: "turns-recorded", hash: "#/i/2026-10-05-qwen-2h/turns" },
  { name: "turns-working", hash: "#/i/atlas/turns" },
  { name: "turns-why", hash: "#/i/2026-10-05-qwen-2h/turns?turn=51", focus: "#turn-51" },
];

const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".ndjson": "text/plain", ".svg": "image/svg+xml" };

export async function serve() {
  const server = createServer(async (req, res) => {
    let path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^(\.\.[/\\])+/, "");
    if (path === "/") path = "/index.html";
    try {
      const body = await readFile(join(dist, path));
      res.writeHead(200, { "content-type": types[extname(path)] ?? "application/octet-stream" }).end(body);
    } catch { res.writeHead(404).end("not found"); }
  });
  await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
  return { url: `http://127.0.0.1:${server.address().port}/`, close: () => server.close() };
}

export async function open(ctx, url, hash, focus) {
  const page = await ctx.newPage();
  await page.goto(url + hash, { waitUntil: "load" });
  await page.waitForSelector('.page[data-ready="true"]', { timeout: 20000 });
  await page.evaluate(() => document.fonts.ready);
  // A link to one turn lands on it, with its details open. A page with a focus is measured and captured there:
  // the first screen the owner sees after tapping "Why". Every other page is measured at the top.
  await page.evaluate((sel) => (sel ? document.querySelector(sel).scrollIntoView({ block: "start" }) : window.scrollTo(0, 0)), focus ?? null);
  return page;
}
