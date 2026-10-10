/** Serve the built app and the mock gateway on one address, with one host running turns on a real clock.
 *    node --experimental-strip-types mock/serve.ts [--port 8742] [--bind 127.0.0.1] [--app dist]
 *  A stand-in for the real gateway until the host's doors land; the contract tests are what the real one must pass. */
import { resolve } from "node:path";
import { startMock } from "./gateway.ts";

const arg = (n: string, d: string) => { const i = process.argv.indexOf(n); return i >= 0 ? process.argv[i + 1] : d; };
const m = await startMock({ appDir: resolve(arg("--app", "dist")), port: Number(arg("--port", "8742")), host: arg("--bind", "127.0.0.1"), live: true, extra: true });
console.log(`mock gateway and app on ${m.url}`);
process.on("SIGTERM", () => { void m.stop().then(() => process.exit(0)); });
