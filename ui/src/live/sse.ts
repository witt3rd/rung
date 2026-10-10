/** A server-sent event parser, fed text in any chunking. An event is complete at a blank line; a half frame waits for
 *  the rest, and a frame cut off by a dropped connection is never delivered. `id` is that event's own: it is not carried over
 *  to the next event, because a delta has none (ui/contract/README.md). Lines starting with a colon are comments (keep-alives). */
export interface SseEvent {
  event: string;
  data: string;
  id?: string;
}

export class SseParser {
  private buf = "";
  private event = "";
  private id: string | undefined;
  private data: string[] = [];
  private seenField = false;

  push(chunk: string): SseEvent[] {
    this.buf += chunk;
    const out: SseEvent[] = [];
    for (;;) {
      const m = /\r\n|\n|\r/.exec(this.buf);
      if (!m) break;
      // A lone "\r" at the very end may be the first half of "\r\n": wait for the next chunk.
      if (m[0] === "\r" && m.index === this.buf.length - 1) break;
      const line = this.buf.slice(0, m.index);
      this.buf = this.buf.slice(m.index + m[0].length);
      if (line === "") {
        if (this.seenField) out.push({ event: this.event || "message", data: this.data.join("\n"), id: this.id });
        this.event = ""; this.id = undefined; this.data = []; this.seenField = false;
        continue;
      }
      if (line.startsWith(":")) continue;
      const i = line.indexOf(":");
      const field = i < 0 ? line : line.slice(0, i);
      let value = i < 0 ? "" : line.slice(i + 1);
      if (value.startsWith(" ")) value = value.slice(1);
      if (field === "event") { this.event = value; this.seenField = true; }
      else if (field === "data") { this.data.push(value); this.seenField = true; }
      else if (field === "id") { this.id = value; this.seenField = true; }
    }
    return out;
  }
}
