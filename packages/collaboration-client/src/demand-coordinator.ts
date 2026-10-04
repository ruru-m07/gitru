import type {
  AcquireDemandRequest,
  DemandLeaseReceipt,
  DemandOwnerActivity,
  DemandRenewalReceipt,
  DemandTarget,
  ReleaseDemandRequest,
  RemoteAccount,
  RenewDemandRequest,
} from "@gitru/commands";

export type DemandAccount = Pick<
  RemoteAccount,
  "id" | "actor_id" | "authorization_epoch" | "provider" | "host" | "state"
>;
export interface DemandTransport {
  demandActivity(): Promise<DemandOwnerActivity>;
  acquireDemand(request: AcquireDemandRequest): Promise<DemandLeaseReceipt>;
  renewDemand(request: RenewDemandRequest): Promise<DemandRenewalReceipt>;
  releaseDemand(request: ReleaseDemandRequest): Promise<void>;
  listenDemandActivity(
    onActivity: (activity: DemandOwnerActivity) => void,
  ): Promise<() => void>;
}
export interface DemandHandle {
  subscribe(listener: (error: unknown | null) => void): () => void;
  release(): void;
}
export type DemandAvailability = {
  generation: string | null;
  /** null until native ownership is known; false also covers a hidden document. */
  active: boolean | null;
};

type Entry = {
  key: string;
  account: DemandAccount;
  target: DemandTarget;
  references: number;
  current: boolean;
  token: number;
  acquiring: boolean;
  receipt: DemandLeaseReceipt | null;
  expiresAt: number;
  error: unknown | null;
  readinessAtAttempt: number;
  readinessRetried: boolean;
  listeners: Set<(error: unknown | null) => void>;
};
const HEARTBEAT_MS = 15_000;
const TTL_MS = 45_000;
const MAX_HANDLES = 16;
const MAX_ENTRIES = 128;
const error = (code: string, message: string) => ({ code, message });
const generation = (value: string) =>
  /^[1-9]\d*$/.test(value) ? BigInt(value) : null;

/** A single webview's liveness, independent of content queries and provider due times. */
export class DemandCoordinator {
  private readonly entries = new Map<string, Entry>();
  private readonly pending = new Set<Entry>();
  private attached = false;
  private session = 0;
  private starting = false;
  private started = false;
  private activity: DemandOwnerActivity | null = null;
  private visible = true;
  private unlisten: (() => void) | null = null;
  private removeDomListeners: (() => void) | null = null;
  private heartbeat: ReturnType<typeof setTimeout> | null = null;
  private retirement: ReturnType<typeof setTimeout> | null = null;
  private renewing = false;
  private acquiring = 0;
  private repairing = false;
  private activityRepairVersion = 0;
  private expiryRepairing = false;
  private readonly expired = new Map<Entry, number>();
  private readiness = 0;
  private startupRepairing = false;
  private readonly availabilityListeners = new Set<
    (activity: DemandAvailability) => void
  >();

  constructor(private readonly transport: DemandTransport) {}

  /** Installing a query bridge alone does not cause activity IPC. */
  attach() {
    this.attached = true;
    if (this.entries.size || this.availabilityListeners.size) void this.start();
  }

  /** Share the one authoritative webview lifecycle with local navigation work. */
  observeActivity(listener: (activity: DemandAvailability) => void) {
    this.availabilityListeners.add(listener);
    listener(this.availability());
    if (this.attached) void this.start();
    return () => {
      this.availabilityListeners.delete(listener);
    };
  }

  private availability(): DemandAvailability {
    return {
      generation: this.activity?.generation ?? null,
      active:
        !this.attached || !this.visible
          ? false
          : this.activity
            ? this.activity.active
            : null,
    };
  }

  private publishAvailability() {
    const activity = this.availability();
    for (const listener of this.availabilityListeners) listener(activity);
  }

  retain(account: DemandAccount, target: DemandTarget): DemandHandle {
    const captured = { ...account };
    const scoped: DemandTarget = {
      kind: target.kind,
      repository_id: target.repository_id,
      subject_id: target.subject_id,
      facet: target.facet,
    };
    const key = JSON.stringify([
      captured.id,
      captured.actor_id,
      captured.authorization_epoch,
      captured.provider,
      captured.host,
      scoped.kind,
      scoped.repository_id,
      scoped.subject_id,
      scoped.facet,
    ]);
    let entry = this.entries.get(key);
    if (!entry) {
      const retiredPending = [...this.pending].filter(
        (value) => this.entries.get(value.key) !== value,
      ).length;
      if (this.entries.size + retiredPending >= MAX_ENTRIES)
        return this.refused(
          error("busy", "Too many visible collaboration targets"),
        );
      entry = {
        key,
        account: captured,
        target: scoped,
        references: 0,
        current: true,
        token: 0,
        acquiring: false,
        receipt: null,
        expiresAt: 0,
        error:
          captured.state === "active"
            ? null
            : error(
                "auth_required",
                "Connect the current account to follow this view",
              ),
        listeners: new Set(),
        readinessAtAttempt: this.readiness,
        readinessRetried: false,
      };
      this.entries.set(key, entry);
    }
    entry.references += 1;
    const retained = entry;
    let released = false;
    const listeners = new Set<(error: unknown | null) => void>();
    if (this.attached) void this.start();
    this.pump();
    return {
      subscribe: (listener) => {
        if (released) return () => {};
        retained.listeners.add(listener);
        listeners.add(listener);
        listener(retained.error);
        return () => {
          retained.listeners.delete(listener);
          listeners.delete(listener);
        };
      },
      release: () => {
        if (released) return;
        released = true;
        for (const listener of listeners) retained.listeners.delete(listener);
        listeners.clear();
        retained.references -= 1;
        if (retained.references === 0 && this.retirement === null)
          this.retirement = setTimeout(() => {
            this.retirement = null;
            for (const candidate of this.entries.values()) {
              if (candidate.references !== 0) continue;
              this.deactivate(candidate);
              if (!candidate.acquiring) this.entries.delete(candidate.key);
            }
            this.scheduleHeartbeat();
          }, 0);
      },
    };
  }

  /** Invalidate before provider cache removal; a late raw acquire is still released. */
  clear(accountId?: string) {
    for (const [key, entry] of this.entries) {
      if (accountId !== undefined && entry.account.id !== accountId) continue;
      entry.current = false;
      this.expired.delete(entry);
      this.deactivate(entry);
      this.notify(entry, null);
      // Pending receipts need bounded cleanup ownership until their IPC returns.
      this.entries.delete(key);
    }
    this.scheduleHeartbeat();
  }

  /** A successful durable catch-up proves service readiness, not content freshness. */
  ready() {
    this.readiness += 1;
    void this.repairStartup();
  }

  stop() {
    this.attached = false;
    this.session += 1;
    this.started = false;
    this.starting = false;
    this.activity = null;
    this.publishAvailability();
    this.repairing = false;
    this.activityRepairVersion += 1;
    this.expiryRepairing = false;
    this.startupRepairing = false;
    this.expired.clear();
    this.unlisten?.();
    this.unlisten = null;
    this.removeDomListeners?.();
    this.removeDomListeners = null;
    this.clear();
    if (this.retirement !== null) clearTimeout(this.retirement);
    this.retirement = null;
  }

  private refused(failure: unknown): DemandHandle {
    return {
      subscribe: (listener) => {
        listener(failure);
        return () => {};
      },
      release: () => {},
    };
  }

  private async start() {
    if (!this.attached || this.started || this.starting) return;
    this.starting = true;
    const session = this.session;
    this.visible =
      typeof document === "undefined" || document.visibilityState === "visible";
    this.publishAvailability();
    try {
      const unlisten = await this.transport.listenDemandActivity((activity) => {
        if (this.attached && this.session === session)
          this.acceptActivity(activity);
      });
      if (!this.attached || this.session !== session) {
        unlisten();
        return;
      }
      this.unlisten = unlisten;
      this.started = true;
      this.installDomListeners(session);
      // The document may have been shown/hidden while native event subscription
      // was pending. Sample only after DOM listeners are installed so that an
      // earlier transition cannot leave the local liveness gate stuck forever.
      this.visible =
        typeof document === "undefined" ||
        document.visibilityState === "visible";
      // Events are already subscribed. A late initial snapshot cannot undo them.
      const activity = await this.transport.demandActivity();
      if (this.attached && this.session === session)
        this.acceptActivity(activity);
    } catch (failure) {
      if (this.attached && this.session === session)
        for (const entry of this.entries.values()) this.notify(entry, failure);
    } finally {
      if (this.session === session) this.starting = false;
    }
  }

  private installDomListeners(session: number) {
    if (typeof document === "undefined" || typeof window === "undefined")
      return;
    const hide = () => {
      if (this.session !== session) return;
      this.visible = false;
      this.suspend();
      this.publishAvailability();
    };
    const show = () => {
      if (this.session !== session || document.visibilityState !== "visible")
        return;
      this.visible = true;
      this.activityRepairVersion += 1;
      void this.repairActivity(session);
    };
    const changed = () =>
      document.visibilityState === "visible" ? show() : hide();
    document.addEventListener("visibilitychange", changed);
    window.addEventListener("pagehide", hide);
    window.addEventListener("pageshow", show);
    this.removeDomListeners = () => {
      document.removeEventListener("visibilitychange", changed);
      window.removeEventListener("pagehide", hide);
      window.removeEventListener("pageshow", show);
    };
  }

  private async repairActivity(session: number) {
    if (this.repairing || !this.attached) return;
    this.repairing = true;
    const version = this.activityRepairVersion;
    const bindings = [...this.entries.values()].map((entry) => ({
      entry,
      token: entry.token,
    }));
    try {
      const activity = await this.transport.demandActivity();
      if (!this.attached || this.session !== session) return;
      this.acceptActivity(activity);
      // A DOM-only hide can interrupt expiry repair without changing the native
      // generation. A fresh authoritative resume read gets one current-binding
      // attempt; denied/Busy entries remain terminal.
      for (const { entry, token } of bindings)
        if (
          this.current(entry, token, session, activity.generation) &&
          activity.active &&
          this.failureCode(entry.error) === "stale_view"
        )
          this.notify(entry, null);
      this.pump();
    } catch {
      // Native expiry handles a lost lifecycle hint; never turn this into provider retry.
    } finally {
      if (this.session === session) {
        this.repairing = false;
        // Coalesce an actual newer resume while this getter was pending, rather
        // than trusting the obsolete DOM ownership of its captured bindings.
        if (
          version !== this.activityRepairVersion &&
          this.attached &&
          this.visible
        )
          void this.repairActivity(session);
      }
    }
  }

  private acceptActivity(activity: DemandOwnerActivity) {
    const next = generation(activity.generation);
    if (next === null) return;
    const previous = this.activity && generation(this.activity.generation);
    if (previous !== null && previous !== undefined && next < previous) return;
    if (
      this.activity?.generation === activity.generation &&
      this.activity.active !== activity.active
    )
      return; // Native transitions always receive a new positive generation.
    const changed = this.activity?.generation !== activity.generation;
    this.activity = { ...activity };
    this.publishAvailability();
    if (changed) {
      this.suspend();
      for (const entry of this.entries.values())
        if (
          entry.current &&
          entry.account.state === "active" &&
          (!entry.error ||
            ["stale_view", "not_ready"].includes(
              this.failureCode(entry.error) ?? "",
            ))
        )
          this.notify(entry, null);
    }
    if (activity.active && this.visible) this.pump();
  }

  private suspend() {
    for (const entry of this.entries.values()) this.deactivate(entry);
    this.scheduleHeartbeat();
  }

  private deactivate(entry: Entry) {
    entry.token += 1;
    if (entry.receipt) this.release(entry.receipt.lease_id);
    entry.receipt = null;
    entry.expiresAt = 0;
  }

  private release(leaseId: string) {
    void this.transport.releaseDemand({ lease_id: leaseId }).catch(() => {});
  }

  private notify(entry: Entry, failure: unknown | null) {
    entry.error = failure;
    for (const listener of entry.listeners) listener(failure);
    if (this.failureCode(failure) === "not_ready") void this.repairStartup();
  }

  private failureCode(failure: unknown) {
    return typeof failure === "object" && failure !== null && "code" in failure
      ? String(failure.code)
      : null;
  }

  private pump() {
    if (
      !this.attached ||
      !this.started ||
      !this.visible ||
      !this.activity?.active
    )
      return;
    for (const entry of this.entries.values()) {
      if (
        !entry.current ||
        entry.account.state !== "active" ||
        entry.references === 0 ||
        entry.acquiring ||
        entry.receipt ||
        entry.error
      )
        continue;
      const active = [...this.entries.values()].filter(
        (value) => value.receipt,
      ).length;
      if (active + this.acquiring >= MAX_HANDLES) {
        this.notify(
          entry,
          error("busy", "Too many visible collaboration targets"),
        );
        continue;
      }
      void this.acquire(entry, this.activity.generation);
    }
  }

  private async acquire(entry: Entry, ownerGeneration: string) {
    const token = entry.token;
    const session = this.session;
    const startedAt = Date.now();
    entry.acquiring = true;
    entry.readinessAtAttempt = this.readiness;
    this.pending.add(entry);
    this.acquiring += 1;
    try {
      const receipt = await this.transport.acquireDemand({
        account_id: entry.account.id,
        authorization_epoch: entry.account.authorization_epoch,
        owner_generation: ownerGeneration,
        target: { ...entry.target },
      });
      if (
        !this.current(entry, token, session, ownerGeneration) ||
        !this.validReceipt(receipt, ownerGeneration) ||
        Date.now() >= startedAt + TTL_MS
      ) {
        this.release(receipt.lease_id);
        if (this.current(entry, token, session, ownerGeneration))
          this.notify(
            entry,
            error(
              "stale_view",
              "The foreground lease arrived after its valid session",
            ),
          );
        return;
      }
      entry.receipt = { ...receipt };
      entry.expiresAt = startedAt + TTL_MS;
      this.notify(entry, null);
    } catch (failure) {
      if (this.current(entry, token, session, ownerGeneration))
        this.notify(entry, failure);
    } finally {
      entry.acquiring = false;
      this.pending.delete(entry);
      this.acquiring -= 1;
      if (
        (!entry.current || entry.references === 0) &&
        this.entries.get(entry.key) === entry
      )
        this.entries.delete(entry.key);
      this.pump();
      this.scheduleHeartbeat();
    }
  }

  private current(
    entry: Entry,
    token: number,
    session: number,
    ownerGeneration: string,
  ) {
    return (
      this.attached &&
      this.started &&
      this.visible &&
      this.session === session &&
      entry.current &&
      entry.references > 0 &&
      entry.token === token &&
      this.activity?.active &&
      this.activity.generation === ownerGeneration
    );
  }

  private validReceipt(receipt: DemandLeaseReceipt, ownerGeneration: string) {
    return (
      receipt.owner_generation === ownerGeneration &&
      receipt.lease_id.length > 0 &&
      receipt.expires_in_seconds === TTL_MS / 1000 &&
      receipt.renew_after_seconds === HEARTBEAT_MS / 1000
    );
  }

  private scheduleHeartbeat() {
    if (
      !this.attached ||
      !this.visible ||
      !this.activity?.active ||
      ![...this.entries.values()].some(
        (entry) => entry.current && entry.references > 0 && entry.receipt,
      )
    ) {
      if (this.heartbeat !== null) clearTimeout(this.heartbeat);
      this.heartbeat = null;
      return;
    }
    if (this.heartbeat !== null) return;
    this.heartbeat = setTimeout(() => {
      this.heartbeat = null;
      void this.tick();
    }, HEARTBEAT_MS);
  }

  private async tick() {
    const now = Date.now();
    for (const entry of this.entries.values())
      if (entry.receipt && entry.expiresAt <= now) {
        this.deactivate(entry);
        this.notify(
          entry,
          error(
            "stale_view",
            "Foreground synchronization paused after its lease expired",
          ),
        );
        if (entry.current && entry.references > 0)
          this.expired.set(entry, entry.token);
      }
    if (this.expired.size) void this.repairExpired();
    if (
      this.renewing ||
      !this.attached ||
      !this.visible ||
      !this.activity?.active
    ) {
      this.scheduleHeartbeat();
      return;
    }
    const ownerGeneration = this.activity.generation;
    const session = this.session;
    const entries = [...this.entries.values()].filter(
      (entry) =>
        entry.current && entry.references > 0 && entry.receipt !== null,
    );
    if (!entries.length) return;
    const bindings = entries.map((entry) => ({
      entry,
      token: entry.token,
      leaseId: entry.receipt!.lease_id,
    }));
    this.renewing = true;
    this.scheduleHeartbeat();
    try {
      const result = await this.transport.renewDemand({
        owner_generation: ownerGeneration,
        leases: bindings.map(({ entry, leaseId }) => ({
          lease_id: leaseId,
          account_id: entry.account.id,
          authorization_epoch: entry.account.authorization_epoch,
        })),
      });
      for (const { entry, token, leaseId } of bindings) {
        if (
          !this.current(entry, token, session, ownerGeneration) ||
          entry.receipt?.lease_id !== leaseId
        )
          continue;
        const receipts = result.leases.filter(
          (receipt) => receipt.lease_id === leaseId,
        );
        if (
          receipts.length !== 1 ||
          !this.validReceipt(receipts[0], ownerGeneration) ||
          Date.now() >= now + TTL_MS
        ) {
          this.deactivate(entry);
          this.notify(
            entry,
            error("stale_view", "The foreground lease could not be renewed"),
          );
        } else {
          entry.receipt = { ...receipts[0] };
          entry.expiresAt = now + TTL_MS;
        }
      }
    } catch (failure) {
      // An atomic batch invalidated by a local reset has not identified any live
      // remaining lease as denied. Keep those bounded receipts until next tick/TTL.
      const unchanged = bindings.every(({ entry, token }) =>
        this.current(entry, token, session, ownerGeneration),
      );
      if (unchanged)
        for (const { entry } of bindings) {
          this.deactivate(entry);
          this.notify(entry, failure);
        }
    } finally {
      this.renewing = false;
      this.scheduleHeartbeat();
    }
  }

  /** One authoritative repair per expiry incident, never a polling/retry engine. */
  private async repairExpired() {
    if (this.expiryRepairing || !this.attached || !this.visible) return;
    this.expiryRepairing = true;
    const session = this.session;
    try {
      const activity = await this.transport.demandActivity();
      if (!this.attached || this.session !== session) return;
      this.acceptActivity(activity);
      // Include other current leases that expired while this single getter was
      // pending. Entry/token checks fence actor/target reset, hide and disposal.
      for (const [entry, token] of this.expired)
        if (
          this.entries.get(entry.key) === entry &&
          this.current(entry, token, session, activity.generation) &&
          activity.active &&
          this.failureCode(entry.error) === "stale_view"
        )
          this.notify(entry, null);
      this.pump();
    } catch {
      // A failed/inactive observation stays suspended until new native activity
      // or an intentional new selection. Busy/permission errors are untouched.
    } finally {
      if (this.session === session) {
        this.expiryRepairing = false;
        this.expired.clear();
      }
    }
  }

  private async repairStartup() {
    if (
      this.startupRepairing ||
      !this.attached ||
      !this.started ||
      !this.visible
    )
      return;
    const entries = [...this.entries.values()].filter(
      (entry) =>
        entry.current &&
        entry.references > 0 &&
        !entry.readinessRetried &&
        this.failureCode(entry.error) === "not_ready" &&
        this.readiness > entry.readinessAtAttempt,
    );
    if (!entries.length) return;
    this.startupRepairing = true;
    const session = this.session;
    const bindings = entries.map((entry) => {
      entry.readinessRetried = true;
      return { entry, token: entry.token };
    });
    try {
      const activity = await this.transport.demandActivity();
      if (!this.attached || this.session !== session) return;
      this.acceptActivity(activity);
      for (const { entry, token } of bindings)
        if (
          this.current(entry, token, session, activity.generation) &&
          activity.active &&
          this.failureCode(entry.error) === "not_ready"
        )
          this.notify(entry, null);
      this.pump();
    } catch {
      // One readiness transition gets one repair opportunity; no interval loop.
    } finally {
      if (this.session === session) this.startupRepairing = false;
    }
  }
}
