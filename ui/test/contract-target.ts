/** What the contract tests talk to: a gateway URL and one instance behind it. By default the mock in ../mock is started;
 *  with RUNG_CONTRACT_URL (and RUNG_CONTRACT_INSTANCE) the same tests run against a real gateway, with no control hooks. */
export interface Control {
  /** Make the host write `n` more record lines (with deltas) and resolve once they are on the record. */
  poke(n: number): Promise<void>;
  /** Write `n` record lines of about `bytes` each, as fast as possible. */
  flood(n: number, bytes: number): Promise<void>;
}
export interface Target {
  url: string;
  instance: string;
  /** The key the gateway holds for the instance: used only to prove it never leaks. Null for a real gateway. */
  hostKey: string | null;
  readOnlyToken: string | null;
  /** An instance id the gateway lists that does not answer, when the target has one. */
  deadInstance: string | null;
  control: Control | null;
  stop(): Promise<void>;
}

export async function startTarget(): Promise<Target> {
  if (process.env.RUNG_CONTRACT_URL) {
    return {
      url: process.env.RUNG_CONTRACT_URL.replace(/\/$/, ""),
      instance: process.env.RUNG_CONTRACT_INSTANCE ?? "alpha",
      hostKey: null,
      readOnlyToken: process.env.RUNG_CONTRACT_READ_TOKEN ?? null,
      deadInstance: process.env.RUNG_CONTRACT_DEAD_INSTANCE ?? null,
      control: null,
      stop: async () => {},
    };
  }
  const { startMock } = await import("../mock/gateway.ts");
  return startMock();
}
