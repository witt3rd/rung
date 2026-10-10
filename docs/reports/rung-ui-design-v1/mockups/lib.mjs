// Shared: the page list and viewports. Playwright is resolved once, in ../../_build/playwright.mjs.
export { chromium } from "../../_build/playwright.mjs";
export const PAGES = ["index", "instance", "turns", "queue", "configure", "console"];
export const VIEWS = { w1920: { width: 1920, height: 1080, deviceScaleFactor: 1 }, w390: { width: 390, height: 844, deviceScaleFactor: 2 } };
