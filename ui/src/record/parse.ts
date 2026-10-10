import type { Line } from "./types.ts";

/** Parse NDJSON. A torn last line (a host killed mid-write) is skipped, as the host itself does on recovery;
 *  a bad line anywhere else is an error, never silently dropped. */
export function parseRecord(text: string): Line[] {
  const rows = text.split("\n");
  const out: Line[] = [];
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i];
    if (row.trim() === "") continue;
    try {
      out.push(JSON.parse(row) as Line);
    } catch (e) {
      if (i === rows.length - 1 || rows.slice(i + 1).every((r) => r.trim() === "")) break;
      throw new Error(`record line ${i + 1} is not JSON: ${(e as Error).message}`);
    }
  }
  return out;
}
