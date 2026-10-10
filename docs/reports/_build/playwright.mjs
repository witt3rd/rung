// One place that finds Playwright for every report script (render.mjs, mockup capture and census).
// Resolution: PLAYWRIGHT_MODULE, then the mise-pinned 1.62.1 (the explainer's resolution).
import { createRequire } from "node:module";
import { join } from "node:path";
import { homedir } from "node:os";
const require = createRequire(import.meta.url);
const candidates = [
  process.env.PLAYWRIGHT_MODULE,
  join(homedir(), ".local/share/mise/installs/npm-playwright/1.62.1/node_modules/playwright"),
].filter(Boolean);
let found;
for (const c of candidates) { try { ({ chromium: found } = require(c)); break; } catch {} }
if (!found) throw new Error(`Playwright not found in: ${candidates.join(", ")}`);
export const chromium = found;
