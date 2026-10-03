/** Revisions are decimal strings so SQLite sequence values never lose precision. */
export function compareRevisions(left: string, right: string): number {
  if (!/^\d+$/.test(left) || !/^\d+$/.test(right)) {
    throw new Error("Invalid collaboration revision");
  }
  const a = left.replace(/^0+(?=\d)/, "");
  const b = right.replace(/^0+(?=\d)/, "");
  return a.length === b.length
    ? a < b
      ? -1
      : a > b
        ? 1
        : 0
    : a.length < b.length
      ? -1
      : 1;
}

export type RevisionBatch<TChange> = {
  changes: TChange[];
  fromExclusive: string | null;
  toInclusive: string;
  reset: boolean;
  hasMore: boolean;
  authorizationView?: string;
};

export interface RevisionTransport<TChange> {
  listen(onWake: () => void): Promise<() => void>;
  catchUp(afterRevision: string | null): Promise<RevisionBatch<TChange>>;
}

/**
 * Native events are wake-up hints. Durable local catch-up is the source of truth.
 * One bridge per webview coalesces any number of consumers and never calls HTTP.
 */
export class RevisionBridge<TChange> {
  private cursor: string | null = null;
  private dirty = false;
  private running: Promise<void> | null = null;
  private stopped = true;
  private generation = 0;
  private unlisten: (() => void) | null = null;

  constructor(
    private readonly transport: RevisionTransport<TChange>,
    private readonly apply: (
      batch: RevisionBatch<TChange>,
    ) => void | Promise<void>,
    private readonly onError: () => void = () => {},
  ) {}

  async start(): Promise<void> {
    if (!this.stopped) return this.wake();
    this.stopped = false;
    const generation = ++this.generation;
    try {
      const unlisten = await this.transport.listen(() => void this.wake());
      if (this.stopped || generation !== this.generation) {
        unlisten();
        return;
      }
      this.unlisten = unlisten;
    } catch {
      if (!this.stopped && generation === this.generation) this.onError();
    }
    if (!this.stopped && generation === this.generation) await this.wake();
  }

  wake(): Promise<void> {
    if (this.stopped) return Promise.resolve();
    this.dirty = true;
    if (this.running) return this.running;
    const generation = this.generation;
    const run = this.drain(generation);
    this.running = run;
    void run.finally(() => {
      if (this.running === run) this.running = null;
      if (this.dirty && !this.stopped) void this.wake();
    });
    return run;
  }

  stop(): void {
    this.stopped = true;
    this.generation += 1;
    this.dirty = false;
    this.running = null;
    this.unlisten?.();
    this.unlisten = null;
  }

  private async drain(generation: number): Promise<void> {
    while (this.dirty && !this.stopped && generation === this.generation) {
      this.dirty = false;
      try {
        const batch = await this.transport.catchUp(this.cursor);
        if (this.stopped || generation !== this.generation) return;
        compareRevisions(batch.toInclusive, batch.toInclusive);
        if (!batch.reset && batch.fromExclusive !== this.cursor) {
          // A discontinuity cannot be repaired by trusting an event payload.
          if (this.cursor === null)
            throw new Error("Invalid collaboration catch-up baseline");
          this.cursor = null;
          this.dirty = true;
          continue;
        }
        if (
          this.cursor !== null &&
          !batch.reset &&
          compareRevisions(batch.toInclusive, this.cursor) < 0
        ) {
          throw new Error("Collaboration revision moved backwards");
        }
        if (batch.hasMore && batch.toInclusive === this.cursor) {
          throw new Error("Collaboration catch-up did not advance");
        }
        await this.apply(batch);
        if (this.stopped || generation !== this.generation) return;
        this.cursor = batch.toInclusive;
        this.dirty ||= batch.hasMore;
      } catch {
        // A later native wake/focus/online event retries. Never busy-loop errors.
        this.dirty = false;
        this.onError();
      }
    }
  }
}
