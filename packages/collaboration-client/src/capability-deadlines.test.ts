import type {
  ContextualCapabilitySnapshot,
  RemoteAccount,
} from "@gitru/commands";
import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import { installCapabilityDeadlines } from "./capability-deadlines";
import { collaborationKeys } from "./client";

const account: RemoteAccount = {
  id: "a",
  provider: "github",
  host: "github.com",
  actor_id: "a",
  login: "a",
  display_name: null,
  authorization_epoch: "1",
  state: "active",
  notifications_supported: false,
};
const target = {
  kind: "account" as const,
  instance_id: null,
  repository_id: null,
  resource_id: null,
  resource_kind: null,
};
const start = Date.parse("2026-10-03T12:00:00Z");
function snapshot(
  deadline: number | null,
  available = false,
): ContextualCapabilitySnapshot {
  return {
    account_id: account.id,
    authorization_epoch: "1",
    instance: {
      id: "github:https://github.com/",
      provider: "github",
      base_url: "https://github.com/",
    },
    target,
    inbox_semantics: "none",
    revision: "1",
    authorization_view: "1",
    facets: [
      {
        facet: "issues",
        saved_read: { state: "supported", reason: null },
        synchronize: available
          ? { state: "supported", reason: null }
          : { state: "unavailable", reason: "temporarily_unavailable" },
        remote_write: { state: "unsupported", reason: "not_implemented" },
        observation: "empty",
        can_recheck_access: false,
        sync: {
          state: "rate_limited",
          next_retry_at:
            deadline === null ? null : new Date(deadline).toISOString(),
          last_success_at: null,
          error: null,
        },
      },
    ],
  };
}
afterEach(() => vi.useRealTimers());

describe("shared contextual capability deadlines", () => {
  it("does not rescan or replace the deadline timer for unrelated cache events", () => {
    vi.useFakeTimers();
    vi.setSystemTime(start);
    const cache = new QueryClient();
    const key = collaborationKeys.contextualCapabilities(account, target);
    cache.setQueryData(key, snapshot(start + 1000));
    const stop = installCapabilityDeadlines(cache);
    const timeout = vi.spyOn(globalThis, "setTimeout");
    cache.setQueryData(["git", "history", "repo"], ["local history"]);
    cache.setQueryData(["git", "history", "repo"], ["new local history"]);
    expect(
      timeout.mock.calls.filter(([, delay]) => delay === 1000),
    ).toHaveLength(0);
    timeout.mockRestore();
    stop();
    cache.clear();
  });
  it("wakes one real observer at expiry and does not loop while the replacement is delayed", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(start);
    const cache = new QueryClient();
    const key = collaborationKeys.contextualCapabilities(account, target);
    cache.setQueryData(key, snapshot(start + 1000));
    let finish!: (value: ContextualCapabilitySnapshot) => void;
    const read = vi.fn(
      () =>
        new Promise<ContextualCapabilitySnapshot>((resolve) => {
          finish = resolve;
        }),
    );
    const observer = new QueryObserver(cache, {
      queryKey: key,
      queryFn: read,
      staleTime: Infinity,
      retry: false,
    });
    const unsubscribe = observer.subscribe(() => {});
    const stop = installCapabilityDeadlines(cache);
    await vi.advanceTimersByTimeAsync(999);
    expect(read).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(read).toHaveBeenCalledTimes(1);
    expect(cache.getQueryState(key)?.fetchStatus).toBe("fetching");
    await vi.advanceTimersByTimeAsync(5000);
    expect(read).toHaveBeenCalledTimes(1);
    finish(snapshot(start + 1000, true));
    await vi.advanceTimersByTimeAsync(0);
    expect(observer.getCurrentResult().data?.facets[0].synchronize.state).toBe(
      "supported",
    );
    stop();
    unsubscribe();
    cache.clear();
  });

  it("cancels an in-flight old snapshot before expiry invalidation and keeps authored queries intact", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(start);
    const cache = new QueryClient();
    const key = collaborationKeys.contextualCapabilities(account, target);
    cache.setQueryData(key, snapshot(start + 1000));
    const draft = collaborationKeys.draft(account, "subject");
    cache.setQueryData(draft, "private authored text");
    let oldSignal!: AbortSignal;
    let finishOld!: (value: ContextualCapabilitySnapshot) => void;
    const current = snapshot(start + 1000, true);
    const read = vi
      .fn(({ signal }: { signal: AbortSignal }) => {
        oldSignal = signal;
        return new Promise<ContextualCapabilitySnapshot>((resolve) => {
          finishOld = resolve;
        });
      })
      .mockResolvedValueOnce(current);
    // The first explicit fetch is held; expiry must cancel it and start a fresh read.
    read
      .mockReset()
      .mockImplementationOnce(({ signal }) => {
        oldSignal = signal;
        return new Promise((resolve) => {
          finishOld = resolve;
        });
      })
      .mockResolvedValue(current);
    const observer = new QueryObserver(cache, {
      queryKey: key,
      queryFn: read,
      staleTime: Infinity,
      retry: false,
    });
    const unsubscribe = observer.subscribe(() => {});
    const stop = installCapabilityDeadlines(cache);
    void observer.refetch();
    expect(read).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(oldSignal.aborted).toBe(true);
    expect(read).toHaveBeenCalledTimes(2);
    finishOld(snapshot(start + 1000));
    await vi.advanceTimersByTimeAsync(0);
    expect(cache.getQueryData(key)).toEqual(current);
    expect(cache.getQueryState(draft)?.isInvalidated).toBe(false);
    stop();
    unsubscribe();
    cache.clear();
  });

  it("marks inactive cache stale, ignores unknown eligibility and unrelated keys, and cleans up on removal or teardown", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(start);
    const cache = new QueryClient();
    const key = collaborationKeys.contextualCapabilities(account, target);
    cache.setQueryData(key, snapshot(start + 1000));
    const unknownKey = collaborationKeys.contextualCapabilities(
      { ...account, id: "unknown" },
      target,
    );
    const unknown = snapshot(start + 1000);
    unknown.facets[0].synchronize = {
      state: "unavailable",
      reason: "not_observed",
    };
    cache.setQueryData(unknownKey, unknown);
    const unrelated = ["other", "account", "a", "1", "capabilities", "context"];
    cache.setQueryData(unrelated, { not: "a capability snapshot" });
    const removed = collaborationKeys.contextualCapabilities(
      { ...account, authorization_epoch: "old" },
      target,
    );
    cache.setQueryData(removed, snapshot(start + 1000));
    const stop = installCapabilityDeadlines(cache);
    cache.removeQueries({ queryKey: removed, exact: true });
    await vi.advanceTimersByTimeAsync(1000);
    expect(cache.getQueryState(key)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(unknownKey)?.isInvalidated).toBe(false);
    expect(cache.getQueryState(unrelated)?.isInvalidated).toBe(false);
    expect(cache.getQueryState(removed)).toBeUndefined();
    const future = collaborationKeys.contextualCapabilities(
      { ...account, id: "future" },
      target,
    );
    cache.setQueryData(future, snapshot(start + 2000));
    stop();
    await vi.advanceTimersByTimeAsync(1000);
    expect(cache.getQueryState(future)?.isInvalidated).toBe(false);
    expect(vi.getTimerCount()).toBe(0);
    cache.clear();
  });

  it("clamps a distant deadline without shortening native eligibility", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(start);
    const cache = new QueryClient();
    const key = collaborationKeys.contextualCapabilities(account, target);
    cache.setQueryData(key, snapshot(start + 2_147_483_647 + 1000));
    const stop = installCapabilityDeadlines(cache);
    await vi.advanceTimersByTimeAsync(2_147_483_647);
    expect(cache.getQueryState(key)?.isInvalidated).toBe(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(cache.getQueryState(key)?.isInvalidated).toBe(true);
    stop();
    cache.clear();
  });
});
