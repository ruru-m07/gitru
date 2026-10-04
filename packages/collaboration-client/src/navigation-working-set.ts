import type {
  ContextualCapabilitySnapshot,
  DetailSnapshot,
  ItemSnapshot,
  RemoteAccount,
} from "@gitru/commands";
import type { QueryClient, QueryKey } from "@tanstack/react-query";
import type { DemandHandle } from "./demand-coordinator";

export const NAVIGATION_PREFETCH_LIMITS = {
  entries: 24,
  reads: 2,
  bytes: 16 * 1024 * 1024,
  readReservation: 4 * 1024 * 1024,
  bundleBytes: 2 * 1024 * 1024,
  descriptorBytes: 1024,
  dwellMs: 150,
  cacheMs: 120_000,
  interests: 4,
  interestMs: 5000,
  scopes: 8,
} as const;

export type NavigationResource = {
  account: RemoteAccount;
  instanceId: string;
  subjectId: string;
  kind: "pull_request" | "issue";
};
export type NavigationInput = Omit<NavigationResource, "subjectId">;
type Projection = "context" | "item" | "body";
type Receipt = ContextualCapabilitySnapshot | ItemSnapshot | DetailSnapshot;
type SignalSource = "pointer" | "focus";
export interface NavigationScope {
  enter(subjectId: string, source: SignalSource): void;
  leave(subjectId: string, source: SignalSource): void;
  visit(subjectId: string): void;
  dispose(): void;
}
export type NavigationActivity = {
  generation: string | null;
  active: boolean | null;
};
export interface NavigationSource {
  keys(resource: NavigationResource): Record<Projection, QueryKey>;
  read(
    resource: NavigationResource,
    projection: Projection,
    signal: AbortSignal,
  ): Promise<Receipt>;
  retain(resource: NavigationResource): DemandHandle;
  observeActivity(listener: (activity: NavigationActivity) => void): () => void;
}
type Entry = {
  identity: string;
  resource: NavigationResource;
  keys: Record<Projection, QueryKey>;
  signals: Map<symbol, Set<SignalSource>>;
  owners: Set<symbol>;
  sequence: number;
  due: number;
  expires: number;
  interestExpires: number;
  descriptorBytes: number;
  bytes: number;
  stage: number;
  eligible: boolean;
  current: boolean;
  reading: boolean;
  abort: AbortController | null;
  demand: DemandHandle | null;
  owned: Map<Projection, unknown>;
};

const projections: Projection[] = ["context", "item", "body"];
const emptyScope: NavigationScope = {
  enter() {},
  leave() {},
  visit() {},
  dispose() {},
};

/** Conservative resident estimate, not an exact JS heap measurement. */
export function estimateNavigationBytes(
  value: unknown,
  maximum: number,
): number {
  const seen = new Set<object>();
  const pending: unknown[] = [value];
  let size = 0;
  while (pending.length && size <= maximum) {
    const next = pending.pop();
    if (typeof next === "string") size += 24 + next.length * 2;
    else if (next && typeof next === "object") {
      if (seen.has(next)) continue;
      seen.add(next);
      size += Array.isArray(next) ? 32 : 64;
      for (const [key, child] of Object.entries(next)) {
        size += 16 + key.length * 2;
        pending.push(child);
        if (size > maximum) break;
      }
    } else size += 8;
  }
  return size;
}

function identity(resource: NavigationResource) {
  const { account, instanceId, subjectId, kind } = resource;
  return JSON.stringify([
    account.id,
    account.actor_id,
    account.authorization_epoch,
    account.provider,
    account.host,
    instanceId,
    subjectId,
    kind,
    "body",
  ]);
}
function readable(
  context: ContextualCapabilitySnapshot,
  resource: NavigationResource,
) {
  return (
    context.account_id === resource.account.id &&
    context.authorization_epoch === resource.account.authorization_epoch &&
    context.instance.id === resource.instanceId &&
    context.instance.provider === resource.account.provider &&
    context.target.instance_id === resource.instanceId &&
    context.target.kind === "resource" &&
    context.target.resource_id === resource.subjectId &&
    context.target.resource_kind === resource.kind &&
    [
      resource.kind === "issue" ? "issues" : "pull_requests",
      resource.kind === "issue" ? "issue_details" : "pull_details",
    ].every((facet) =>
      context.facets.some(
        (policy) =>
          policy.facet === facet && policy.saved_read.state === "supported",
      ),
    )
  );
}
function synchronize(
  context: ContextualCapabilitySnapshot,
  resource: NavigationResource,
) {
  const policy = context.facets.find(
    (facet) =>
      facet.facet ===
      (resource.kind === "issue" ? "issue_details" : "pull_details"),
  );
  return (
    policy?.saved_read.state === "supported" &&
    (policy.synchronize.state === "supported" ||
      (policy.synchronize.state === "unavailable" &&
        policy.synchronize.reason === "temporarily_unavailable"))
  );
}

/** One bounded speculative working set over the existing webview QueryClient. */
export class NavigationWorkingSet {
  private readonly entries = new Map<string, Entry>();
  private readonly scopes = new Map<
    symbol,
    { accountId: string; valid: boolean; bytes: number }
  >();
  private sequence = 0;
  private reads = 0;
  private activity: NavigationActivity = { generation: null, active: null };
  private unobserve: (() => void) | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private stopped = false;
  private cacheUnsubscribe: () => void;
  private updating = false;

  constructor(
    private readonly cache: QueryClient,
    private readonly source: NavigationSource,
  ) {
    this.cacheUnsubscribe = cache.getQueryCache().subscribe((event) => {
      if (this.updating) return;
      if (event.type === "updated" && event.action.type === "invalidate") {
        for (const entry of this.entries.values())
          if (
            projections.some(
              (projection) =>
                cache
                  .getQueryCache()
                  .find({ queryKey: entry.keys[projection], exact: true }) ===
                event.query,
            )
          )
            this.retire(entry);
      }
      this.prune();
    });
  }

  scope(input: NavigationInput): NavigationScope {
    if (
      this.stopped ||
      input.account.state !== "active" ||
      this.scopes.size >= NAVIGATION_PREFETCH_LIMITS.scopes
    )
      return emptyScope;
    const descriptorBytes = Math.max(
      NAVIGATION_PREFETCH_LIMITS.descriptorBytes,
      estimateNavigationBytes(input, NAVIGATION_PREFETCH_LIMITS.bundleBytes),
    );
    if (
      descriptorBytes > NAVIGATION_PREFETCH_LIMITS.bundleBytes ||
      !this.admit(descriptorBytes, false)
    )
      return emptyScope;
    const owner = Symbol("navigation scope");
    const lifetime = {
      accountId: input.account.id,
      valid: true,
      bytes: descriptorBytes,
    };
    this.scopes.set(owner, lifetime);
    const captured = { ...input, account: { ...input.account } };
    let disposed = false;
    const enter = (subjectId: string, source: SignalSource | "recent") => {
      if (
        disposed ||
        this.stopped ||
        !lifetime.valid ||
        this.activity.active === false
      )
        return;
      const resource = { ...captured, subjectId };
      if (
        ![
          subjectId,
          resource.instanceId,
          resource.account.id,
          resource.account.actor_id,
          resource.account.authorization_epoch,
          resource.account.host,
        ].every(
          (value) =>
            value.length > 0 &&
            value.length <= 512 &&
            !/[\u0000-\u001f]/.test(value),
        )
      )
        return;
      const key = identity(resource);
      let entry = this.entries.get(key);
      if (entry && !entry.current) return;
      if (!entry) {
        const keys = this.source.keys(resource);
        const descriptorBytes = Math.max(
          NAVIGATION_PREFETCH_LIMITS.descriptorBytes,
          512 +
            estimateNavigationBytes(
              { resource, keys, identity: key },
              NAVIGATION_PREFETCH_LIMITS.bundleBytes,
            ),
        );
        if (!this.admit(descriptorBytes)) return;
        entry = {
          identity: key,
          resource,
          keys,
          signals: new Map(),
          owners: new Set(),
          sequence: ++this.sequence,
          due: Date.now() + NAVIGATION_PREFETCH_LIMITS.dwellMs,
          expires: Date.now() + NAVIGATION_PREFETCH_LIMITS.cacheMs,
          interestExpires: 0,
          descriptorBytes,
          bytes: 0,
          stage: 0,
          eligible: false,
          current: true,
          reading: false,
          abort: null,
          demand: null,
          owned: new Map(),
        };
        this.entries.set(key, entry);
      }
      entry.owners.add(owner);
      entry.sequence = ++this.sequence;
      entry.expires = Date.now() + NAVIGATION_PREFETCH_LIMITS.cacheMs;
      if (source !== "recent") {
        let signals = entry.signals.get(owner);
        if (!signals) {
          signals = new Set();
          entry.signals.set(owner, signals);
        }
        signals.add(source);
      }
      if (source === "recent") entry.due = Date.now();
      if (!this.unobserve)
        this.unobserve = this.source.observeActivity((activity) => {
          const changed =
            this.activity.generation !== null &&
            activity.generation !== this.activity.generation;
          this.activity = activity;
          if (activity.active === false || changed) this.invalidate();
          else this.pump();
        });
      this.retain(entry);
      this.pump();
    };
    return {
      enter,
      visit: (subjectId) => enter(subjectId, "recent"),
      leave: (subjectId, source) => {
        const entry = this.entries.get(identity({ ...captured, subjectId }));
        const signals = entry?.signals.get(owner);
        signals?.delete(source);
        if (signals?.size === 0) entry?.signals.delete(owner);
        if (entry && !entry.signals.size) {
          this.release(entry);
          if (entry.stage < projections.length) this.retire(entry);
        }
        this.prune();
      },
      dispose: () => {
        if (disposed) return;
        disposed = true;
        this.scopes.delete(owner);
        for (const entry of this.entries.values()) {
          entry.signals.delete(owner);
          entry.owners.delete(owner);
          if (!entry.owners.size) {
            this.release(entry);
            if (entry.stage < projections.length) this.retire(entry);
          } else if (!entry.signals.size) this.release(entry);
        }
        this.prune();
      },
    };
  }

  stats() {
    return {
      entries: this.entries.size,
      reads: this.reads,
      bytes: this.bytes(),
      interests: [...this.entries.values()].filter((entry) => entry.demand)
        .length,
      queued: [...this.entries.values()].filter(
        (entry) =>
          entry.current && !entry.reading && entry.stage < projections.length,
      ).length,
    };
  }

  clear(accountId?: string) {
    for (const scope of this.scopes.values())
      if (accountId === undefined || scope.accountId === accountId)
        scope.valid = false;
    this.invalidate(accountId);
  }

  /** Revision invalidation keeps the current mounted scope usable for a new gesture. */
  invalidate(accountId?: string) {
    for (const entry of this.entries.values())
      if (accountId === undefined || entry.resource.account.id === accountId)
        this.retire(entry);
    this.prune();
  }

  stop() {
    if (this.stopped) return;
    this.stopped = true;
    this.clear();
    this.unobserve?.();
    this.unobserve = null;
    this.cacheUnsubscribe();
    this.scopes.clear();
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
  }

  private bytes() {
    return (
      this.reads * NAVIGATION_PREFETCH_LIMITS.readReservation +
      [...this.scopes.values()].reduce((sum, scope) => sum + scope.bytes, 0) +
      [...this.entries.values()].reduce(
        (sum, entry) => sum + entry.descriptorBytes + entry.bytes,
        0,
      )
    );
  }

  private admit(extra: number, entryCount = true) {
    this.prune();
    while (
      (entryCount && this.entries.size >= NAVIGATION_PREFETCH_LIMITS.entries) ||
      this.bytes() + extra > NAVIGATION_PREFETCH_LIMITS.bytes
    ) {
      const victim = [...this.entries.values()]
        .filter((entry) => !entry.reading && !this.protected(entry))
        .sort((a, b) => a.sequence - b.sequence)[0];
      if (!victim) return false;
      this.retire(victim);
      this.prune();
    }
    return true;
  }

  private protected(entry: Entry) {
    return [...entry.owned.keys()].some(
      (projection) =>
        (this.cache
          .getQueryCache()
          .find({ queryKey: entry.keys[projection], exact: true })
          ?.getObserversCount() ?? 0) > 0,
    );
  }

  private retain(entry: Entry) {
    if (
      !entry.current ||
      entry.stage < 2 ||
      !entry.eligible ||
      entry.demand ||
      (this.cache
        .getQueryCache()
        .find({ queryKey: entry.keys.body, exact: true })
        ?.getObserversCount() ?? 0) > 0 ||
      this.activity.active !== true ||
      this.stats().interests >= NAVIGATION_PREFETCH_LIMITS.interests
    )
      return;
    entry.demand = this.source.retain(entry.resource);
    entry.interestExpires = Date.now() + NAVIGATION_PREFETCH_LIMITS.interestMs;
  }

  private release(entry: Entry) {
    entry.demand?.release();
    entry.demand = null;
    entry.interestExpires = 0;
  }

  private retire(entry: Entry) {
    entry.current = false;
    entry.eligible = false;
    entry.abort?.abort();
    entry.signals.clear();
    entry.owners.clear();
    this.release(entry);
  }

  private prune() {
    if (this.updating) return;
    this.updating = true;
    try {
      for (const entry of this.entries.values()) {
        if (entry.current && entry.expires <= Date.now()) this.retire(entry);
        if (entry.interestExpires && entry.interestExpires <= Date.now())
          this.release(entry);
        entry.bytes = 0;
        for (const [projection, value] of entry.owned) {
          const query = this.cache
            .getQueryCache()
            .find({ queryKey: entry.keys[projection], exact: true });
          if (!query || query.state.data !== value) {
            entry.owned.delete(projection);
            continue;
          }
          if (!entry.current && query.getObserversCount() === 0) {
            this.cache.removeQueries({
              queryKey: entry.keys[projection],
              exact: true,
            });
            entry.owned.delete(projection);
          } else if (query.getObserversCount() === 0)
            entry.bytes += estimateNavigationBytes(
              value,
              NAVIGATION_PREFETCH_LIMITS.bundleBytes,
            );
        }
        if (!entry.current && !entry.reading && !entry.owned.size)
          this.entries.delete(entry.identity);
      }
      // A selected observer can relinquish a formerly excluded projection.
      // Re-admission must honor the same byte budget without evicting any view
      // that still observes its data or releasing an unsettled IPC reservation.
      while (this.bytes() > NAVIGATION_PREFETCH_LIMITS.bytes) {
        const victim = [...this.entries.values()]
          .filter((entry) => !entry.reading && entry.bytes > 0)
          .sort((a, b) => a.sequence - b.sequence)[0];
        if (!victim) break;
        this.retire(victim);
        for (const projection of victim.owned.keys()) {
          const query = this.cache
            .getQueryCache()
            .find({ queryKey: victim.keys[projection], exact: true });
          if (!query || query.getObserversCount() === 0) {
            this.cache.removeQueries({
              queryKey: victim.keys[projection],
              exact: true,
            });
            victim.owned.delete(projection);
          }
        }
        victim.bytes = 0;
        if (!victim.owned.size) this.entries.delete(victim.identity);
      }
    } finally {
      this.updating = false;
    }
    this.schedule();
  }

  private schedule() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    if (this.stopped) return;
    const times = [...this.entries.values()]
      .flatMap((entry) => [
        entry.expires,
        entry.interestExpires,
        entry.current &&
        entry.stage < projections.length &&
        entry.due > Date.now()
          ? entry.due
          : 0,
      ])
      .filter((time) => time > Date.now());
    if (times.length)
      this.timer = setTimeout(
        () => {
          this.prune();
          this.pump();
        },
        Math.min(...times) - Date.now(),
      );
  }

  private pump() {
    if (this.stopped || !this.activity.active) return;
    this.prune();
    for (const entry of [...this.entries.values()].sort(
      (a, b) => b.sequence - a.sequence,
    )) {
      if (this.reads >= NAVIGATION_PREFETCH_LIMITS.reads) break;
      if (
        !entry.current ||
        entry.reading ||
        entry.due > Date.now() ||
        entry.stage >= projections.length
      )
        continue;
      while (
        this.bytes() + NAVIGATION_PREFETCH_LIMITS.readReservation >
        NAVIGATION_PREFETCH_LIMITS.bytes
      ) {
        const victim = [...this.entries.values()]
          .filter(
            (candidate) =>
              candidate !== entry &&
              !candidate.reading &&
              !this.protected(candidate),
          )
          .sort((a, b) => a.sequence - b.sequence)[0];
        if (!victim) break;
        this.retire(victim);
        this.prune();
      }
      if (
        !entry.current ||
        this.bytes() + NAVIGATION_PREFETCH_LIMITS.readReservation >
          NAVIGATION_PREFETCH_LIMITS.bytes
      )
        continue;
      void this.read(entry);
    }
    this.schedule();
  }

  private async read(entry: Entry) {
    const projection = projections[entry.stage];
    entry.reading = true;
    entry.abort = new AbortController();
    this.reads += 1;
    try {
      const query = this.cache
        .getQueryCache()
        .find({ queryKey: entry.keys[projection], exact: true });
      // Selection owns its reads, including currently disabled observers waiting
      // for contextual policy. Never dispatch a competing speculative IPC.
      if (
        query &&
        (query.getObserversCount() > 0 || query.state.fetchStatus !== "idle")
      ) {
        this.retire(entry);
        return;
      }
      const initialData = query?.state.data;
      const receipt =
        query?.state.data !== undefined && !query.state.isInvalidated
          ? (query.state.data as Receipt)
          : await this.source.read(
              entry.resource,
              projection,
              entry.abort.signal,
            );
      if (
        !entry.current ||
        entry.abort.signal.aborted ||
        this.stopped ||
        !this.activity.active
      )
        return;
      const bytes = estimateNavigationBytes(
        receipt,
        NAVIGATION_PREFETCH_LIMITS.bundleBytes,
      );
      if (
        bytes + entry.bytes > NAVIGATION_PREFETCH_LIMITS.bundleBytes ||
        this.bytes() - NAVIGATION_PREFETCH_LIMITS.readReservation + bytes >
          NAVIGATION_PREFETCH_LIMITS.bytes
      ) {
        this.retire(entry);
        return;
      }
      if (projection === "context") {
        const context = receipt as ContextualCapabilitySnapshot;
        if (!readable(context, entry.resource)) {
          this.retire(entry);
          return;
        }
        entry.eligible = synchronize(context, entry.resource);
      }
      if (projection === "item") {
        const item = (receipt as ItemSnapshot).item;
        if (
          !item ||
          item.id !== entry.resource.subjectId ||
          item.account_id !== entry.resource.account.id ||
          item.kind !== entry.resource.kind
        ) {
          this.retire(entry);
          return;
        }
      }
      if (
        projection === "body" &&
        ((receipt as DetailSnapshot).subject_id !== entry.resource.subjectId ||
          (receipt as DetailSnapshot).evidence.authorization_epoch !==
            entry.resource.account.authorization_epoch)
      ) {
        this.retire(entry);
        return;
      }
      this.updating = true;
      try {
        const currentQuery = this.cache
          .getQueryCache()
          .find({ queryKey: entry.keys[projection], exact: true });
        // Ordinary active views and their in-flight queries own their data. Only
        // speculative writes to an unobserved key are counted/removed here.
        if (
          !currentQuery ||
          (currentQuery === query &&
            currentQuery.getObserversCount() === 0 &&
            currentQuery.state.data === initialData &&
            (initialData === undefined || currentQuery.state.isInvalidated) &&
            currentQuery.state.data !== receipt &&
            currentQuery.state.fetchStatus === "idle")
        ) {
          const target =
            currentQuery ??
            this.cache.getQueryCache().build(this.cache, {
              queryKey: entry.keys[projection],
              gcTime: NAVIGATION_PREFETCH_LIMITS.cacheMs,
              retry: false,
              networkMode: "always",
              meta: { collaboration: true },
            });
          target.setData(receipt);
          entry.owned.set(projection, target.state.data);
        }
      } finally {
        this.updating = false;
      }
      entry.bytes += bytes;
      entry.stage += 1;
      if (projection === "item") this.retain(entry);
    } catch {
      this.retire(entry);
    } finally {
      entry.reading = false;
      entry.abort = null;
      this.reads -= 1;
      this.prune();
      this.pump();
    }
  }
}
