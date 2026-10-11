/** One followed store per instance, shared by everything that shows it. A page that goes away releases its hold; the store is
 *  stopped only if nothing takes it again within `graceMs`, so moving between an instance's tabs keeps one connection and one record,
 *  and an instance nobody is looking at is not followed. */
export interface Followed { start(): unknown; stop(): void }

export class StoreRegistry<S extends Followed> {
  private held = new Map<string, { store: S; refs: number; timer: ReturnType<typeof setTimeout> | null }>();
  private make: (id: string) => S;
  private graceMs: number;

  constructor(make: (id: string) => S, graceMs = 5000) { this.make = make; this.graceMs = graceMs; }

  get size(): number { return this.held.size; }

  /** The store if one is held, without taking a hold: a page drawing for the first time shows what is already loaded. */
  peek(id: string): S | undefined { return this.held.get(id)?.store; }

  acquire(id: string): S {
    let e = this.held.get(id);
    if (!e) { e = { store: this.make(id), refs: 0, timer: null }; this.held.set(id, e); void e.store.start(); }
    if (e.timer) { clearTimeout(e.timer); e.timer = null; }
    e.refs++;
    return e.store;
  }

  release(id: string): void {
    const e = this.held.get(id);
    if (!e || --e.refs > 0) return;
    e.refs = 0;
    e.timer = setTimeout(() => { if (e.refs === 0) { e.store.stop(); this.held.delete(id); } }, this.graceMs);
  }
}
