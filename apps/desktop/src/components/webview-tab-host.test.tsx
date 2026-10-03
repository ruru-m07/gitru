import { act, cleanup, render, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const native = vi.hoisted(() => ({
  creations: [] as Array<{
    label: string;
    x: number;
    y: number;
    width: number;
    height: number;
  }>,
  lookups: [] as string[],
  lookupGate: null as { label: string; ready: Promise<void> } | null,
  views: [] as Array<{
    label: string;
    visible: boolean;
    registered: boolean;
    created?: () => void;
    show: ReturnType<typeof vi.fn>;
    hide: ReturnType<typeof vi.fn>;
    setFocus: ReturnType<typeof vi.fn>;
    close: ReturnType<typeof vi.fn>;
  }>,
  holdCreation: false,
  showGate: null as Promise<void> | null,
  closeGate: null as Promise<void> | null,
  demandAtHide: [] as Array<{ label: string; active: boolean }>,
  demandAtClose: [] as Array<{ label: string; active: boolean }>,
  readinessListeners: new Set<() => void>(),
  demandSequence: 0,
  demandOwners: new Map<string, { generation: string; active: boolean }>(),
  demandTransitions: [] as Array<{
    label: string;
    active: boolean;
    visible: boolean;
  }>,
  state: {
    activeTabId: "a",
    tabs: [
      { id: "a", routePath: "/app/inbox" },
      { id: "b", routePath: "/app/pulls" },
    ],
  },
  listeners: new Set<() => void>(),
}));

vi.mock("@/store/use-app-store", async () => {
  const { useSyncExternalStore } = await import("react");
  const useAppStore = Object.assign(
    (selector: (state: typeof native.state) => unknown) =>
      useSyncExternalStore(
        (listener) => {
          native.listeners.add(listener);
          return () => native.listeners.delete(listener);
        },
        () => selector(native.state),
      ),
    { getState: () => native.state },
  );
  return { useAppStore };
});

vi.mock("@gitru/commands", () => ({
  disposeRepoContextOwner: vi.fn().mockResolvedValue(undefined),
  collaborationInspectDemandOwner: vi.fn(
    async ({ ownerLabel }: { ownerLabel: string }) => {
      let activity = native.demandOwners.get(ownerLabel);
      if (!activity) {
        activity = {
          generation: String(++native.demandSequence),
          active: false,
        };
        native.demandOwners.set(ownerLabel, activity);
      }
      return { ...activity };
    },
  ),
  collaborationSetDemandOwnerActivity: vi.fn(
    async ({
      ownerLabel,
      expectedGeneration,
      active,
    }: {
      ownerLabel: string;
      expectedGeneration: string;
      active: boolean;
    }) => {
      const current = native.demandOwners.get(ownerLabel);
      if (!current || current.generation !== expectedGeneration)
        throw { code: "stale_view" };
      const activity = {
        generation:
          active === current.active
            ? current.generation
            : String(++native.demandSequence),
        active,
      };
      native.demandOwners.set(ownerLabel, activity);
      native.demandTransitions.push({
        label: ownerLabel,
        active,
        visible:
          native.views.find((view) => view.label === ownerLabel)?.visible ??
          true,
      });
      return { ...activity };
    },
  ),
  collaborationDisposeDemandOwner: vi.fn(
    async ({
      ownerLabel,
      expectedGeneration,
    }: {
      ownerLabel: string;
      expectedGeneration: string;
    }) => {
      if (
        native.demandOwners.get(ownerLabel)?.generation !== expectedGeneration
      )
        throw { code: "stale_view" };
      native.demandOwners.delete(ownerLabel);
    },
  ),
}));
vi.mock("@/state/core/repo-context-registry", () => ({
  createRepoContextOwnerId: (label: string) => "owner:" + label,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_event: string, callback: () => void) => {
    native.readinessListeners.add(callback);
    return () => native.readinessListeners.delete(callback);
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ label: "main" }),
  Webview: class {
    static getByLabel = async (label: string) => {
      native.lookups.push(label);
      if (native.lookupGate?.label === label) await native.lookupGate.ready;
      return (
        native.views.find((view) => view.label === label && view.registered) ??
        null
      );
    };
    static getAll = async () => native.views.filter((view) => view.registered);
    visible = true;
    registered = false;
    created?: () => void;
    show = vi.fn(async () => {
      if (native.showGate) await native.showGate;
      this.visible = true;
    });
    hide = vi.fn(async () => {
      native.demandAtHide.push({
        label: this.label,
        active: native.demandOwners.get(this.label)?.active ?? false,
      });
      this.visible = false;
    });
    setFocus = vi.fn().mockResolvedValue(undefined);
    setPosition = vi.fn().mockResolvedValue(undefined);
    setSize = vi.fn().mockResolvedValue(undefined);
    close = vi.fn(async () => {
      native.demandAtClose.push({
        label: this.label,
        active: native.demandOwners.get(this.label)?.active ?? false,
      });
      if (native.closeGate) await native.closeGate;
      native.views = native.views.filter((view) => view !== this);
    });
    constructor(
      _window: unknown,
      public label: string,
      bounds: { x: number; y: number; width: number; height: number },
    ) {
      native.creations.push({ label, ...bounds });
      native.views.push(this);
    }
    once = vi.fn(async (event: string, callback: () => void) => {
      if (event === "tauri://created") {
        this.created = () => {
          this.registered = true;
          callback();
        };
        if (!native.holdCreation) queueMicrotask(this.created);
      }
      return () => {};
    });
  },
}));

beforeEach(() => {
  vi.resetModules();
  native.creations = [];
  native.lookups = [];
  native.lookupGate = null;
  native.views = [];
  native.holdCreation = false;
  native.showGate = null;
  native.closeGate = null;
  native.demandOwners.clear();
  native.demandAtHide = [];
  native.demandAtClose = [];
  native.demandTransitions = [];
  native.demandSequence = 0;
  native.readinessListeners.clear();
  native.listeners.clear();
  native.state = {
    activeTabId: "a",
    tabs: [
      { id: "a", routePath: "/app/inbox" },
      { id: "b", routePath: "/app/pulls" },
    ],
  };
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: 0,
    y: 40,
    top: 40,
    left: 0,
    right: 1280,
    bottom: 800,
    width: 1280,
    height: 760,
    toJSON: () => ({}),
  });
});

afterEach(async () => {
  cleanup();
  // Let the host's existing StrictMode cleanup timer dispose its own views.
  await new Promise((resolve) => setTimeout(resolve, 5));
});

async function mountHost() {
  const module = await import("./webview-tab-host");
  render(<module.default />);
  return module;
}

async function changeActive(id: string, addColdTab = false) {
  await act(async () => {
    native.state = {
      activeTabId: id,
      tabs: addColdTab
        ? [...native.state.tabs, { id, routePath: "/app/issues" }]
        : native.state.tabs,
    };
    for (const listener of native.listeners) listener();
  });
}

it("does not recreate native children when modal completion resumes after host unmount", async () => {
  const host = await import("./webview-tab-host");
  const mounted = render(<host.default />);
  await waitFor(() => expect(native.views).toHaveLength(2));
  await waitFor(() => expect(native.views[0].setFocus).toHaveBeenCalled());
  await host.setTabWebviewsSuspended(true);
  const creationsBeforeUnmount = native.creations.length;

  mounted.unmount();
  // A dialog may finish closing before or after the deferred native cleanup.
  await host.setTabWebviewsSuspended(false);
  await waitFor(() => expect(native.views).toHaveLength(0));
  await host.setTabWebviewsSuspended(false);
  expect(native.views).toHaveLength(0);
  expect(native.creations).toHaveLength(creationsBeforeUnmount);
});

it("fences a cold-tab resume whose native lookup finishes after unmount", async () => {
  const host = await import("./webview-tab-host");
  const mounted = render(<host.default />);
  await waitFor(() => expect(native.views[0]?.setFocus).toHaveBeenCalled());
  await host.setTabWebviewsSuspended(true);
  await changeActive("c", true);
  let finishLookup!: () => void;
  native.lookupGate = {
    label: "tab-webview:c",
    ready: new Promise((resolve) => {
      finishLookup = resolve;
    }),
  };
  const resume = host.setTabWebviewsSuspended(false);
  await waitFor(() => expect(native.lookups).toContain("tab-webview:c"));
  mounted.unmount();
  await waitFor(() => expect(native.views).toHaveLength(0));
  finishLookup();
  await resume;
  expect(native.views).toHaveLength(0);
  expect(native.creations.map(({ label }) => label)).not.toContain(
    "tab-webview:c",
  );
});

it("uses a remounted host's geometry and restores its current tab", async () => {
  const host = await import("./webview-tab-host");
  const first = render(<host.default />);
  await waitFor(() => expect(native.views[0]?.setFocus).toHaveBeenCalled());
  await host.setTabWebviewsSuspended(true);
  first.unmount();
  await waitFor(() => expect(native.views).toHaveLength(0));
  await host.setTabWebviewsSuspended(false);

  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: 20,
    y: 80,
    top: 80,
    left: 20,
    right: 920,
    bottom: 680,
    width: 900,
    height: 600,
    toJSON: () => ({}),
  });
  native.state = { ...native.state, activeTabId: "b" };
  render(<host.default />);
  await waitFor(() =>
    expect(
      native.views.find((view) => view.label === "tab-webview:b")?.setFocus,
    ).toHaveBeenCalled(),
  );
  expect(native.creations.at(-1)).toMatchObject({
    x: 20,
    y: 80,
    width: 900,
    height: 600,
  });
  await host.setTabWebviewsSuspended(true);
  expect(native.views.every((view) => !view.visible)).toBe(true);
  await host.setTabWebviewsSuspended(false);
  expect(
    native.views.find((view) => view.label === "tab-webview:b")?.visible,
  ).toBe(true);
});

it("keeps the current host usable across StrictMode effect remounts", async () => {
  const host = await import("./webview-tab-host");
  render(
    <StrictMode>
      <host.default />
    </StrictMode>,
  );
  await waitFor(() => expect(native.views).toHaveLength(2));
  await waitFor(() => expect(native.views[0].setFocus).toHaveBeenCalled());
  expect(native.creations).toHaveLength(2);
  await host.setTabWebviewsSuspended(true);
  await host.setTabWebviewsSuspended(false);
  expect(
    native.views.find((view) => view.label === "tab-webview:a")?.visible,
  ).toBe(true);
});

it("waits for an old host's native close before creating the remounted host's children", async () => {
  const host = await import("./webview-tab-host");
  const first = render(<host.default />);
  await waitFor(() => expect(native.views[0]?.setFocus).toHaveBeenCalled());
  await waitFor(() => expect(native.views).toHaveLength(2));
  await host.setTabWebviewsSuspended(true);
  const oldViews = [...native.views];
  let finishClose!: () => void;
  native.closeGate = new Promise((resolve) => {
    finishClose = resolve;
  });
  first.unmount();
  // The cleanup timer has fired and close() is in flight, rather than merely
  // scheduled and cancellable by a StrictMode remount.
  await waitFor(() =>
    expect(oldViews.every((view) => view.close.mock.calls.length > 0)).toBe(
      true,
    ),
  );
  const creationsBeforeRemount = native.creations.length;
  render(<host.default />);
  // Let the new host measure its bounds while the old native close remains
  // blocked. Resume must wait rather than adopting a view about to disappear.
  await act(async () => {
    await new Promise<void>((resolve) => {
      window.requestAnimationFrame(() => resolve());
    });
  });
  let resumed = false;
  const resume = host.setTabWebviewsSuspended(false).then(() => {
    resumed = true;
  });
  await Promise.resolve();
  expect(resumed).toBe(false);
  expect(native.creations).toHaveLength(creationsBeforeRemount);
  finishClose();
  await resume;
  await waitFor(() => expect(native.views).toHaveLength(2));
  await waitFor(() => expect(native.views[0].setFocus).toHaveBeenCalled());
  expect(oldViews.every((view) => !native.views.includes(view))).toBe(true);
  await host.setTabWebviewsSuspended(true);
  await host.setTabWebviewsSuspended(false);
  expect(
    native.views.find((view) => view.label === "tab-webview:a")?.visible,
  ).toBe(true);
});

it("hides an old native show that finishes after cleanup and a new host mounts", async () => {
  let finishShow!: () => void;
  native.showGate = new Promise((resolve) => {
    finishShow = resolve;
  });
  const host = await import("./webview-tab-host");
  const mounted = render(<host.default />);
  await waitFor(() => expect(native.views[0]?.show).toHaveBeenCalled());
  const oldActive = native.views[0];
  mounted.unmount();
  await waitFor(() => expect(native.views).toHaveLength(0));
  // The new host selects the same tab ID, so that selection alone cannot
  // distinguish the stale surface from the replacement host's current view.
  render(<host.default />);
  await waitFor(() => expect(native.views).toHaveLength(2));
  expect(native.views).not.toContain(oldActive);
  finishShow();
  await waitFor(() => expect(oldActive.visible).toBe(false));
  expect(oldActive.setFocus).not.toHaveBeenCalled();
  await waitFor(() => expect(native.views[0].setFocus).toHaveBeenCalled());
  expect(native.views[0].visible).toBe(true);
});

it("hides native tabs for the host modal and restores the newly selected warm tab", async () => {
  const host = await mountHost();
  await waitFor(() => expect(native.views).toHaveLength(2));
  await waitFor(() => expect(native.views[0].setFocus).toHaveBeenCalled());
  await host.setTabWebviewsSuspended(true);
  expect(native.views.every((view) => !view.visible)).toBe(true);
  await changeActive("b");
  expect(native.views.every((view) => !view.visible)).toBe(true);
  await host.setTabWebviewsSuspended(false);
  expect(
    native.views.find((view) => view.label === "tab-webview:a")?.visible,
  ).toBe(false);
  const active = native.views.find((view) => view.label === "tab-webview:b");
  expect(active?.visible).toBe(true);
  expect(active?.setFocus).toHaveBeenCalled();
});

it("waits for a delayed show to be hidden before suspension resolves", async () => {
  let finishShow!: () => void;
  native.showGate = new Promise((resolve) => {
    finishShow = resolve;
  });
  const host = await mountHost();
  await waitFor(() => expect(native.views[0]?.show).toHaveBeenCalled());
  let suspended = false;
  const work = host.setTabWebviewsSuspended(true).then(() => {
    suspended = true;
  });
  await Promise.resolve();
  expect(suspended).toBe(false);
  finishShow();
  await work;
  expect(native.views.every((view) => !view.visible)).toBe(true);
  expect(native.views[0].setFocus).not.toHaveBeenCalled();
});

it("waits for in-flight native creation and keeps it hidden while suspended", async () => {
  native.holdCreation = true;
  const host = await mountHost();
  await waitFor(() => expect(native.views).toHaveLength(2));
  let suspended = false;
  const work = host.setTabWebviewsSuspended(true).then(() => {
    suspended = true;
  });
  await Promise.resolve();
  expect(suspended).toBe(false);
  for (const view of native.views) view.created?.();
  await work;
  expect(native.views.every((view) => !view.visible)).toBe(true);
  expect(native.views.every((view) => view.show.mock.calls.length === 0)).toBe(
    true,
  );
});

it("defers cold tab creation during the modal and creates the current tab on close", async () => {
  const host = await mountHost();
  await waitFor(() => expect(native.views[0]?.setFocus).toHaveBeenCalled());
  await host.setTabWebviewsSuspended(true);
  await changeActive("c", true);
  expect(
    native.views.find((view) => view.label === "tab-webview:c"),
  ).toBeUndefined();
  expect(native.views.every((view) => !view.visible)).toBe(true);
  await host.setTabWebviewsSuspended(false);
  const active = native.views.find((view) => view.label === "tab-webview:c");
  expect(active?.visible).toBe(true);
  expect(active?.setFocus).toHaveBeenCalled();
});

it("also hides native children not yet adopted after a host remount", async () => {
  const host = await import("./webview-tab-host");
  const { Webview } = await import("@tauri-apps/api/webview");
  const geometry = { x: 0, y: 0, width: 1280, height: 760 };
  const orphan = new Webview({} as never, "tab-webview:surviving", geometry);
  const main = new Webview({} as never, "main", geometry);
  for (const view of native.views) view.registered = true;
  await host.setTabWebviewsSuspended(true);
  expect(orphan.hide).toHaveBeenCalled();
  expect(main.hide).not.toHaveBeenCalled();
});

it("rejects a failed native hide instead of claiming the host can collect credentials", async () => {
  const host = await mountHost();
  await waitFor(() => expect(native.views[0]?.setFocus).toHaveBeenCalled());
  native.views[0].hide.mockRejectedValueOnce(new Error("fixture hide failure"));
  await expect(host.setTabWebviewsSuspended(true)).rejects.toThrow(
    "Could not hide tab views",
  );
  await host.setTabWebviewsSuspended(false);
  expect(native.views[0].visible).toBe(true);
});

it("refuses a host modal after the warm-up timeout while native creation is still unknown", async () => {
  vi.useFakeTimers();
  vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
    queueMicrotask(() => callback(0));
    return 1;
  });
  native.holdCreation = true;
  try {
    const host = await mountHost();
    await act(async () => {});
    expect(native.views).toHaveLength(2);
    const refusal = expect(host.setTabWebviewsSuspended(true)).rejects.toThrow(
      "Native tabs are still starting",
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1300);
    });
    await refusal;
    // Retry succeeds only after the real native creation events arrive.
    for (const view of native.views) view.created?.();
    await host.setTabWebviewsSuspended(true);
    expect(native.views.every((view) => !view.visible)).toBe(true);
  } finally {
    vi.useRealTimers();
  }
});

it("drains pending hides on failure before restoring the active tab", async () => {
  const host = await mountHost();
  await waitFor(() => expect(native.views).toHaveLength(2));
  await waitFor(() => expect(native.views[0].setFocus).toHaveBeenCalled());
  let finishHide!: () => void;
  const delayedHide = new Promise<void>((resolve) => {
    finishHide = resolve;
  });
  native.views[0].hide.mockImplementationOnce(async () => {
    await delayedHide;
    native.views[0].visible = false;
  });
  native.views[1].hide.mockRejectedValueOnce(
    new Error("fixture background hide failure"),
  );
  const backgroundHidesBeforeSuspension =
    native.views[1].hide.mock.calls.length;
  let refused = false;
  const work = host.setTabWebviewsSuspended(true).catch(() => {
    refused = true;
  });
  await waitFor(() =>
    expect(native.views[1].hide).toHaveBeenCalledTimes(
      backgroundHidesBeforeSuspension + 1,
    ),
  );
  await Promise.resolve();
  expect(refused).toBe(false);
  const resumed = host.setTabWebviewsSuspended(false);
  finishHide();
  await work;
  await resumed;
  expect(refused).toBe(true);
  expect(native.views[0].visible).toBe(true);
  expect(native.views[0].setFocus).toHaveBeenCalled();
});

it("grants urgency only after show and revokes every hidden prewarm and modal owner", async () => {
  const host = await mountHost();
  await waitFor(() => expect(native.views[0]?.setFocus).toHaveBeenCalled());
  await waitFor(() => expect(native.views).toHaveLength(2));
  expect(native.demandOwners.get("tab-webview:a")?.active).toBe(true);
  expect(native.demandOwners.get("tab-webview:b")?.active).toBe(false);
  expect(native.demandOwners.get("main")?.active).toBe(false);
  expect(
    native.demandTransitions
      .filter(({ active }) => active)
      .every(({ visible }) => visible),
  ).toBe(true);
  const originalGeneration =
    native.demandOwners.get("tab-webview:a")?.generation;

  await host.setTabWebviewsSuspended(true);
  expect([...native.demandOwners.values()].every(({ active }) => !active)).toBe(
    true,
  );
  await changeActive("b");
  await host.setTabWebviewsSuspended(false);
  expect(native.demandOwners.get("tab-webview:b")?.active).toBe(true);
  expect(native.demandOwners.get("tab-webview:a")?.active).toBe(false);
  await changeActive("a");
  await waitFor(() =>
    expect(native.demandOwners.get("tab-webview:a")?.active).toBe(true),
  );
  expect(native.demandOwners.get("tab-webview:a")?.generation).not.toBe(
    originalGeneration,
  );
  expect(native.demandAtHide.every(({ active }) => !active)).toBe(true);
});

it("disposes demand before native close and a same-label replacement gets a new generation", async () => {
  const host = await import("./webview-tab-host");
  const first = render(<host.default />);
  await waitFor(() => expect(native.views[0]?.setFocus).toHaveBeenCalled());
  const original = native.demandOwners.get("tab-webview:a")?.generation;
  first.unmount();
  await waitFor(() => expect(native.views).toHaveLength(0));
  expect(native.demandAtClose.length).toBeGreaterThan(0);
  expect(native.demandAtClose.every(({ active }) => !active)).toBe(true);
  expect(native.demandOwners.has("tab-webview:a")).toBe(false);
  render(<host.default />);
  await waitFor(() => expect(native.views[0]?.setFocus).toHaveBeenCalled());
  expect(native.demandOwners.get("tab-webview:a")?.active).toBe(true);
  expect(native.demandOwners.get("tab-webview:a")?.generation).not.toBe(
    original,
  );
});

it("still closes the native surface when demand revocation fails during disposal", async () => {
  const { collaborationSetDemandOwnerActivity } = await import(
    "@gitru/commands"
  );
  const host = await import("./webview-tab-host");
  const first = render(<host.default />);
  await waitFor(() => expect(native.views[0]?.setFocus).toHaveBeenCalled());
  vi.mocked(collaborationSetDemandOwnerActivity).mockRejectedValueOnce({
    code: "busy",
  });
  const previous = native.demandOwners.get("tab-webview:a")?.generation;
  first.unmount();
  await waitFor(() => expect(native.views).toHaveLength(0));
  // Reopen without waiting for the native observer to notice the missing view.
  render(<host.default />);
  await waitFor(() => expect(native.views[0]?.setFocus).toHaveBeenCalled());
  expect(native.demandOwners.get("tab-webview:a")?.generation).not.toBe(
    previous,
  );
  expect(native.demandOwners.get("tab-webview:a")?.active).toBe(true);
});

it("repairs a still-visible startup owner once after native readiness without a tab revisit", async () => {
  const { collaborationSetDemandOwnerActivity } = await import(
    "@gitru/commands"
  );
  const setter = vi.mocked(collaborationSetDemandOwnerActivity);
  const original = setter.getMockImplementation();
  if (!original) throw new Error("Missing explicit demand fixture");
  let ready = false;
  let rejectedActiveAttempts = 0;
  setter.mockImplementation(async (request) => {
    if (request.active && !ready) {
      rejectedActiveAttempts += 1;
      throw { code: "not_ready" };
    }
    return original(request);
  });
  await mountHost();
  await waitFor(() => expect(native.views[0]?.show).toHaveBeenCalled());
  await waitFor(() => expect(rejectedActiveAttempts).toBeGreaterThan(0));
  expect(native.demandOwners.get("tab-webview:a")?.active).toBe(false);
  ready = true;
  await act(async () => {
    for (const listener of native.readinessListeners) listener();
  });
  await waitFor(() =>
    expect(native.demandOwners.get("tab-webview:a")?.active).toBe(true),
  );
  const activeCalls = setter.mock.calls.filter(
    ([request]) => request.active,
  ).length;
  await act(async () => {
    for (const listener of native.readinessListeners) listener();
  });
  expect(setter.mock.calls.filter(([request]) => request.active).length).toBe(
    activeCalls,
  );
  setter.mockImplementation(original);
});
