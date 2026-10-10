// Shared: Playwright resolution (same as ../../_build/render.mjs) and the page list.
import { createRequire } from "node:module";
import { join } from "node:path";
import { homedir } from "node:os";
const require = createRequire(import.meta.url);
const candidates = [process.env.PLAYWRIGHT_MODULE,
  join(homedir(), ".local/share/mise/installs/npm-playwright/1.62.1/node_modules/playwright")].filter(Boolean);
export let chromium;
for (const c of candidates) { try { ({ chromium } = require(c)); break; } catch {} }
if (!chromium) throw new Error(`Playwright not found in: ${candidates.join(", ")}`);
export const PAGES = ["index", "instance", "turns", "queue", "configure", "console"];
export const VIEWS = { w1920: { width: 1920, height: 1080, deviceScaleFactor: 1 }, w390: { width: 390, height: 844, deviceScaleFactor: 2 } };
