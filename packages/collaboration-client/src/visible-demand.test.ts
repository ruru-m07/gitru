// @vitest-environment jsdom
import type {
  DemandLeaseReceipt,
  DemandOwnerActivity,
  DemandRenewalReceipt,
  DemandTarget,
  RemoteAccount,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen } from "@testing-library/react";
import { createElement, StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { collaboration, collaborationErrorMessage } from "./index";
import { useCollaborationDetail, useVisibleDemand } from "./react";

const account: RemoteAccount = {
  id: "account-a",
  actor_id: "actor-a",
  authorization_epoch: "1",
  provider: "github",
  host: "github.com",
  state: "active",
  login: "fixture",
  display_name: null,
  notifications_supported: true,
};
const target: DemandTarget = {
  kind: "detail",
  repository_id: null,
  subject_id: "issue:7",
  facet: "body",
};
const receipt = (
  leaseId: string,
  ownerGeneration = "1",
): DemandLeaseReceipt => ({
  lease_id: leaseId,
  owner_generation: ownerGeneration,
  expires_in_seconds: 45,
  renew_after_seconds: 15,
});
const flush = async () => {
  for (let i = 0; i < 16; i += 1) await Promise.resolve();
};
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((finish) => {
    resolve = finish;
  });
  return { promise, resolve };
};
let activity: DemandOwnerActivity;
let emit!: (next: DemandOwnerActivity) => void;
let visible: DocumentVisibilityState;
let revision: string;
let authorizationView: string;
let stop: () => void;
let cache: QueryClient;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(0);
  activity = { generation: "1", active: true };
  visible = "visible";
  revision = "1";
  authorizationView = "1";
  let sequence = 0;
  vi.spyOn(document, "visibilityState", "get").mockImplementation(
    () => visible,
  );
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "changesSince").mockImplementation(
    async () => ({
      revision,
      authorization_view: authorizationView,
      changes: [],
      has_more: false,
      reset_required: false,
    }),
  );
  vi.spyOn(collaboration.transport, "demandActivity").mockImplementation(
    async () => activity,
  );
  vi.spyOn(collaboration.transport, "listenDemandActivity").mockImplementation(
    async (listener) => {
      emit = (next) => {
        activity = next;
        listener(next);
      };
      return () => {};
    },
  );
  vi.spyOn(collaboration.transport, "acquireDemand").mockImplementation(
    async (request) => receipt(`lease-${++sequence}`, request.owner_generation),
  );
  vi.spyOn(collaboration.transport, "renewDemand").mockImplementation(
    async (request) => ({
      leases: request.leases.map((lease) =>
        receipt(lease.lease_id, request.owner_generation),
      ),
    }),
  );
  vi.spyOn(collaboration.transport, "releaseDemand").mockResolvedValue();
  vi.spyOn(collaboration.transport, "hydrateDetail").mockRejectedValue(
    new Error("Automatic durable hydration is forbidden"),
  );
  vi.spyOn(collaboration.transport, "refresh").mockRejectedValue(
    new Error("Heartbeat is not refresh"),
  );
  cache = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  stop = collaboration.installBridge(cache);
});
afterEach(async () => {
  cleanup();
  stop();
  cache.clear();
  await flush();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

function View({
  actor = account,
  enabled = true,
}: {
  actor?: RemoteAccount;
  enabled?: boolean;
}) {
  const failure = useVisibleDemand({
    account: actor,
    target: { ...target },
    enabled,
  });
  return createElement(
    "p",
    null,
    failure === null ? "No lease error" : collaborationErrorMessage(failure),
  );
}
function mount(actor = account, enabled = true) {
  return render(
    createElement(StrictMode, null, createElement(View, { actor, enabled })),
  );
}

describe("visible demand hook lifecycle", () => {
  it("coalesces real StrictMode effects and stable object rerenders while heartbeat performs no hydration", async () => {
    const view = mount();
    await act(flush);
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(1);
    view.rerender(
      createElement(
        StrictMode,
        null,
        createElement(View, { actor: { ...account } }),
      ),
    );
    await act(flush);
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(1);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(collaboration.transport.renewDemand).toHaveBeenCalledTimes(1);
    expect(collaboration.transport.hydrateDetail).not.toHaveBeenCalled();
    expect(collaboration.transport.refresh).not.toHaveBeenCalled();
    view.unmount();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(collaboration.transport.releaseDemand).toHaveBeenCalledTimes(1);
  });

  it("reads a saved authoritative null immediately while acquisition is pending and keeps queries timer-free", async () => {
    const acquisition = deferred<DemandLeaseReceipt>();
    vi.mocked(collaboration.transport.acquireDemand).mockImplementationOnce(
      () => acquisition.promise,
    );
    vi.spyOn(collaboration.transport, "detail").mockResolvedValue({
      pending_intent: null,

      subject_id: "issue:7",
      body: { state: "known", text: null },
      metadata: null,
      entries: [],
      next_cursor: null,
      revision: "1",
      authorization_view: "1",
      evidence: {
        facet: "body",
        availability: "ready",
        coverage: {
          state: "complete",
          validated_at: null,
          remote_has_more: false,
        },
        freshness: "fresh",
        stale_at: null,
        facet_revision: "1",
        authorization_epoch: "1",
        access_reason: null,
        source: null,
        value_source: null,
        saved_empty: true,
        observed_state: "known",
        sync: {
          state: "idle",
          last_success_at: null,
          next_retry_at: null,
          error: null,
        },
      },
    });
    function SavedView() {
      useVisibleDemand({ account, target, enabled: true });
      const query = useCollaborationDetail(account, {
        subject_id: "issue:7",
        facet: "body",
        cursor: null,
        limit: 50,
      });
      return createElement(
        "p",
        null,
        query.data?.body.state === "known"
          ? "Known saved absence"
          : "Loading saved data",
      );
    }
    render(
      createElement(
        QueryClientProvider,
        { client: cache },
        createElement(SavedView),
      ),
    );
    await act(flush);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(screen.getByText("Known saved absence")).toBeDefined();
    const localReads = vi.mocked(collaboration.transport.detail).mock.calls
      .length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(collaboration.transport.detail).toHaveBeenCalledTimes(localReads);
    expect(collaboration.transport.renewDemand).not.toHaveBeenCalled();
    acquisition.resolve(receipt("cached-view"));
    await act(flush);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(collaboration.transport.renewDemand).toHaveBeenCalledTimes(1);
    expect(collaboration.transport.detail).toHaveBeenCalledTimes(localReads);
    expect(collaboration.transport.hydrateDetail).not.toHaveBeenCalled();
  });

  it("stops on DOM hide and waits for native activity before reacquiring after resume", async () => {
    mount();
    await act(flush);
    await act(async () => {
      visible = "hidden";
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(collaboration.transport.releaseDemand).toHaveBeenCalledTimes(1);
    await act(async () => {
      emit({ generation: "2", active: false });
      await vi.advanceTimersByTimeAsync(60_000);
    });
    await act(async () => {
      visible = "visible";
      document.dispatchEvent(new Event("visibilitychange"));
      await flush();
    });
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(1);
    expect(collaboration.transport.renewDemand).not.toHaveBeenCalled();
    await act(async () => {
      emit({ generation: "3", active: true });
      await flush();
    });
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(2);
  });

  it("repairs DOM-only resume after a pending expiry observation completes while hidden", async () => {
    const renewal = deferred<DemandRenewalReceipt>();
    vi.mocked(collaboration.transport.renewDemand).mockImplementationOnce(
      () => renewal.promise,
    );
    mount();
    await act(flush);
    const observation = deferred<DemandOwnerActivity>();
    vi.mocked(collaboration.transport.demandActivity).mockImplementationOnce(
      () => observation.promise,
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(45_000);
    });
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(1);
    await act(async () => {
      visible = "hidden";
      document.dispatchEvent(new Event("visibilitychange"));
    });
    observation.resolve({ generation: "1", active: true });
    await act(flush);
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(1);
    await act(async () => {
      visible = "visible";
      document.dispatchEvent(new Event("visibilitychange"));
      await flush();
    });
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(2);
    expect(collaboration.transport.demandActivity).toHaveBeenCalledTimes(3);
    expect(screen.getByText("No lease error")).toBeTruthy();
    renewal.resolve({ leases: [receipt("lease-1")] });
    await act(flush);
    expect(collaboration.transport.hydrateDetail).not.toHaveBeenCalled();
    expect(collaboration.transport.refresh).not.toHaveBeenCalled();
  });

  it.each([
    "busy",
    "permission_denied",
  ])("does not rearm %s on DOM-only resume", async (code) => {
    vi.mocked(collaboration.transport.acquireDemand).mockRejectedValueOnce({
      code,
    });
    mount();
    await act(flush);
    await act(async () => {
      visible = "hidden";
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await act(async () => {
      visible = "visible";
      document.dispatchEvent(new Event("visibilitychange"));
      await flush();
    });
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(1);
  });

  it("coalesces a newer DOM resume during a delayed getter and never adopts an obsolete binding", async () => {
    mount();
    await act(flush);
    const observation = deferred<DemandOwnerActivity>();
    vi.mocked(collaboration.transport.demandActivity).mockImplementationOnce(
      () => observation.promise,
    );
    await act(async () => {
      visible = "hidden";
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await act(async () => {
      visible = "visible";
      document.dispatchEvent(new Event("visibilitychange"));
      await flush();
    });
    await act(async () => {
      visible = "hidden";
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await act(async () => {
      visible = "visible";
      document.dispatchEvent(new Event("visibilitychange"));
      await flush();
    });
    observation.resolve({ generation: "1", active: true });
    await act(flush);
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(2);
    expect(collaboration.transport.demandActivity).toHaveBeenCalledTimes(3);
    expect(collaboration.transport.hydrateDetail).not.toHaveBeenCalled();
  });

  it("re-retains after an actual authorization-view reset even when the account epoch is unchanged", async () => {
    mount();
    await act(flush);
    await act(async () => {
      revision = "2";
      authorizationView = "2";
      await collaboration.wake();
      await flush();
    });
    expect(collaboration.transport.releaseDemand).toHaveBeenCalledTimes(1);
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(2);
    expect(
      vi.mocked(collaboration.transport.acquireDemand).mock.calls[1][0]
        .authorization_epoch,
    ).toBe("1");
  });

  it("fences a late actor-A receipt after the same subject changes to another actor and epoch", async () => {
    const old = deferred<DemandLeaseReceipt>();
    vi.mocked(collaboration.transport.acquireDemand).mockImplementationOnce(
      () => old.promise,
    );
    const view = mount();
    await act(flush);
    view.rerender(
      createElement(
        StrictMode,
        null,
        createElement(View, {
          actor: {
            ...account,
            id: "account-b",
            actor_id: "actor-b",
            authorization_epoch: "9",
          },
        }),
      ),
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    old.resolve(receipt("late-actor-a"));
    await act(flush);
    expect(collaboration.transport.releaseDemand).toHaveBeenCalledWith({
      lease_id: "late-actor-a",
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(
      vi.mocked(collaboration.transport.renewDemand).mock.calls[0][0].leases,
    ).toEqual([
      {
        lease_id: "lease-1",
        account_id: "account-b",
        authorization_epoch: "9",
      },
    ]);
  });

  it("repairs a startup NotReady admission on actual bridge catch-up without remounting or provider work", async () => {
    vi.mocked(collaboration.transport.acquireDemand).mockRejectedValueOnce({
      code: "not_ready",
    });
    mount();
    await act(flush);
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(1);
    await act(async () => {
      await collaboration.wake();
      await flush();
    });
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(2);
    expect(screen.getByText("No lease error")).toBeTruthy();
    await act(async () => {
      await collaboration.wake();
      await flush();
    });
    expect(collaboration.transport.acquireDemand).toHaveBeenCalledTimes(2);
    expect(collaboration.transport.refresh).not.toHaveBeenCalled();
    expect(collaboration.transport.hydrateDetail).not.toHaveBeenCalled();
  });

  it("makes disabled or denied consumers issue zero activity/admission calls", async () => {
    mount(account, false);
    await act(flush);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(90_000);
    });
    expect(collaboration.transport.demandActivity).not.toHaveBeenCalled();
    expect(collaboration.transport.acquireDemand).not.toHaveBeenCalled();
    expect(collaboration.transport.hydrateDetail).not.toHaveBeenCalled();
  });
});
