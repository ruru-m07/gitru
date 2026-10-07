import {
  collaborationDisposeDemandOwner,
  collaborationInspectDemandOwner,
  collaborationSetDemandOwnerActivity,
  disposeRepoContextOwner,
} from "@gitru/commands";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { listen } from "@tauri-apps/api/event";
import { Webview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  sanitizeTabWebviewLabel,
  TAB_WEBVIEW_LABEL_PREFIX,
} from "@/bootstrap/runtime-utils";
import { createRepoContextOwnerId } from "@/state/core/repo-context-registry";
import { useAppStore } from "@/store/use-app-store";
import type { WorkspaceTab } from "@/types/store";

type HostBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type ManagedWebview = {
  tabId: string;
  ownerId: string;
  hostOwner: HostOwner;
  webview: Webview;
  ready: Promise<void>;
  bounds: HostBounds;
};

type HostOwner = object;

const CREATE_TIMEOUT_MS = 1200;

const managedWebviews = new Map<string, ManagedWebview>();
const ensureInFlightByTabId = new Map<
  string,
  { owner: HostOwner; work: Promise<ManagedWebview | null> }
>();
const pendingNativeCreates = new Set<string>();
let desiredActiveTabId: string | null = null;
let visibleTabId: string | null = null;
let liveTabIds = new Set<string>();
let pendingCleanupTimer: number | null = null;
let tabWebviewsSuspended = false;
let latestHostBounds: HostBounds | null = null;
let activeHostOwner: HostOwner | null = null;
let closingWebviews: Promise<void> = Promise.resolve();
let visibilityWork: Promise<unknown> = Promise.resolve();
let nativeReadinessVersion = 0;
let pendingDemandActivation: {
  tab: WorkspaceTab;
  bounds: HostBounds;
  owner: HostOwner;
  readinessVersion: number;
} | null = null;

const retryPendingDemand = () => {
  const pending = pendingDemandActivation;
  if (!pending || pending.readinessVersion >= nativeReadinessVersion) return;
  pendingDemandActivation = null;
  if (
    activeHostOwner !== pending.owner ||
    tabWebviewsSuspended ||
    desiredActiveTabId !== pending.tab.id ||
    visibleTabId !== pending.tab.id
  )
    return;
  // One readiness-triggered retry; errors do not turn this into polling.
  void activateTabWebview(
    pending.tab,
    pending.bounds,
    pending.owner,
    true,
  ).catch(() => {});
};

// Native child views sit above the host DOM. Serialize visibility changes so a
// delayed show cannot cover a host-owned dialog after suspension has completed.
const changeVisibility = <T,>(operation: () => Promise<T>): Promise<T> => {
  const work = visibilityWork.then(operation, operation);
  visibilityWork = work.catch(() => {});
  return work;
};

const demandFailureCode = (error: unknown): string | undefined =>
  typeof error === "object" && error !== null && "code" in error
    ? String(error.code)
    : undefined;

const setDemandActivity = async (ownerLabel: string, active: boolean) => {
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      const current = await collaborationInspectDemandOwner({ ownerLabel });
      await collaborationSetDemandOwnerActivity({
        ownerLabel,
        expectedGeneration: current.generation,
        active,
      });
      return;
    } catch (error) {
      const code = demandFailureCode(error);
      // No runtime/view exists yet, so there is no active interest to revoke.
      if (!active && (code === "not_ready" || code === "not_found")) return;
      // A native visibility fence can advance generation during the local IPC.
      if (code === "stale_view" && attempt === 0) continue;
      throw error;
    }
  }
};

const disposeDemandOwner = async (ownerLabel: string) => {
  await setDemandActivity(ownerLabel, false);
  try {
    const current = await collaborationInspectDemandOwner({ ownerLabel });
    await collaborationDisposeDemandOwner({
      ownerLabel,
      expectedGeneration: current.generation,
    });
  } catch (error) {
    const code = demandFailureCode(error);
    if (code !== "not_ready" && code !== "not_found") throw error;
  }
};

const getRoutePathname = (routePath: string) => {
  try {
    return new URL(routePath, window.location.origin).pathname;
  } catch {
    return routePath.split("?")[0].split("#")[0];
  }
};

const normalizeWorkspaceRoutePath = (routePath: string) => {
  const pathname = getRoutePathname(routePath);
  return pathname === "/app" || pathname === "/app/" ? "/app/git" : routePath;
};

const toChildWebviewPath = (routePath: string) => {
  const url = new URL(routePath, window.location.origin);
  url.searchParams.set("embedded", "1");
  return `${url.pathname}${url.search}${url.hash}`;
};

const normalizeBounds = (bounds: HostBounds): HostBounds => ({
  x: Math.round(bounds.x),
  y: Math.round(bounds.y),
  width: Math.max(1, Math.round(bounds.width)),
  height: Math.max(1, Math.round(bounds.height)),
});

const areSameBounds = (left: HostBounds, right: HostBounds) =>
  left.x === right.x &&
  left.y === right.y &&
  left.width === right.width &&
  left.height === right.height;

const updateManagedBounds = async (
  entry: ManagedWebview,
  bounds: HostBounds,
) => {
  const normalized = normalizeBounds(bounds);
  if (areSameBounds(entry.bounds, normalized)) return;

  entry.bounds = normalized;
  await Promise.allSettled([
    entry.webview.setPosition(new LogicalPosition(normalized.x, normalized.y)),
    entry.webview.setSize(new LogicalSize(normalized.width, normalized.height)),
  ]);
};

const closeManagedWebview = async (entry: ManagedWebview) => {
  try {
    try {
      await disposeDemandOwner(entry.webview.label);
    } finally {
      // Closing the native surface is also a demand fence if IPC revocation
      // failed; never leave an abandoned view alive during host disposal.
      await entry.webview.close();
    }
  } finally {
    await Promise.allSettled([
      disposeRepoContextOwner({ ownerId: entry.ownerId }),
    ]);
  }
};

const hideUnlessActive = async (entry: ManagedWebview) => {
  await changeVisibility(async () => {
    if (tabWebviewsSuspended || entry.tabId !== desiredActiveTabId) {
      await setDemandActivity(entry.webview.label, false);
      await entry.webview.hide();
    }
  });
};

const ensureTabWebview = async (
  tab: WorkspaceTab,
  bounds: HostBounds,
  owner: HostOwner,
): Promise<ManagedWebview | null> => {
  // A replacement host must not adopt native views that the previous host is
  // still closing. Their map entries are removed before close() completes.
  await closingWebviews;
  if (activeHostOwner !== owner) return null;
  const existing = managedWebviews.get(tab.id);
  if (existing) {
    existing.hostOwner = owner;
    await existing.ready;
    return activeHostOwner === owner
      ? (managedWebviews.get(tab.id) ?? null)
      : null;
  }

  const existingEnsure = ensureInFlightByTabId.get(tab.id);
  if (existingEnsure?.owner === owner) return await existingEnsure.work;

  const task = (async (): Promise<ManagedWebview | null> => {
    const normalized = normalizeBounds(bounds);
    const label = sanitizeTabWebviewLabel(tab.id);
    const childScopeId = label.slice(TAB_WEBVIEW_LABEL_PREFIX.length);
    const ownerId = createRepoContextOwnerId(label, childScopeId);
    const existingByLabel = await Webview.getByLabel(label);
    // Native lookups can finish after unmount or a new host has taken over.
    // Only that current host may adopt/create a view with its geometry.
    if (activeHostOwner !== owner) return null;

    if (existingByLabel) {
      const reused: ManagedWebview = {
        tabId: tab.id,
        ownerId,
        hostOwner: owner,
        webview: existingByLabel,
        ready: Promise.resolve(),
        // Force one geometry sync because the native view can outlive a host
        // component during HMR or development StrictMode probes.
        bounds: { ...normalized, width: -1 },
      };
      managedWebviews.set(tab.id, reused);
      await Promise.all([
        updateManagedBounds(reused, normalized),
        hideUnlessActive(reused),
      ]);
      return reused;
    }

    const routePath = normalizeWorkspaceRoutePath(tab.routePath);
    // Defer cold creation while a host modal is open. A newly created native
    // Webview is initially visible, regardless of CSS stacking or focus=false.
    if (tabWebviewsSuspended) return null;
    let targetUrl: string;

    try {
      targetUrl = toChildWebviewPath(routePath);
    } catch (error) {
      console.error("Failed to resolve child webview URL", {
        tabId: tab.id,
        routePath,
        error,
      });
      return null;
    }

    // A previous close may have succeeded while demand disposal failed. Reset
    // its native incarnation before this label becomes physically visible again.
    await disposeDemandOwner(label);
    if (activeHostOwner !== owner || tabWebviewsSuspended) return null;

    let webview: Webview;
    try {
      webview = new Webview(getCurrentWindow(), label, {
        url: targetUrl,
        x: normalized.x,
        y: normalized.y,
        width: normalized.width,
        height: normalized.height,
        focus: false,
      });
    } catch (error) {
      console.error("Failed to create child webview", {
        tabId: tab.id,
        targetUrl,
        error,
      });
      return null;
    }

    let createError: unknown = null;
    pendingNativeCreates.add(label);
    const ready = new Promise<void>((resolve) => {
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        resolve();
      };

      void webview.once("tauri://created", () => {
        pendingNativeCreates.delete(label);
        void hideUnlessActive({
          tabId: tab.id,
          ownerId,
          hostOwner: owner,
          webview,
          ready: Promise.resolve(),
          bounds: normalized,
        })
          .catch(() => {})
          .finally(finish);
      });

      void webview.once("tauri://error", (event) => {
        pendingNativeCreates.delete(label);
        createError = event.payload;
        finish();
      });

      window.setTimeout(finish, CREATE_TIMEOUT_MS);
    });

    const created: ManagedWebview = {
      tabId: tab.id,
      ownerId,
      hostOwner: owner,
      webview,
      ready,
      bounds: normalized,
    };
    managedWebviews.set(tab.id, created);
    await ready;
    if (activeHostOwner !== owner) return null;

    if (createError !== null) {
      const recovered = await Webview.getByLabel(label);
      if (activeHostOwner !== owner) return null;
      if (recovered) {
        const entry: ManagedWebview = {
          tabId: tab.id,
          ownerId,
          hostOwner: owner,
          webview: recovered,
          ready: Promise.resolve(),
          bounds: normalized,
        };
        managedWebviews.set(tab.id, entry);
        await hideUnlessActive(entry);
        return entry;
      }

      console.error("Child webview failed to initialize", {
        tabId: tab.id,
        targetUrl,
        createError,
      });
      await closeManagedWebview(created);
      managedWebviews.delete(tab.id);
      return null;
    }

    if (!liveTabIds.has(tab.id)) {
      await closeManagedWebview(created);
      managedWebviews.delete(tab.id);
      return null;
    }

    return created;
  })();

  ensureInFlightByTabId.set(tab.id, { owner, work: task });
  try {
    return await task;
  } finally {
    if (ensureInFlightByTabId.get(tab.id)?.work === task) {
      ensureInFlightByTabId.delete(tab.id);
    }
  }
};

const activateTabWebview = async (
  tab: WorkspaceTab,
  bounds: HostBounds,
  owner: HostOwner,
  readinessRetry = false,
) => {
  if (activeHostOwner !== owner) return;
  pendingDemandActivation = null;
  desiredActiveTabId = tab.id;
  latestHostBounds = bounds;
  liveTabIds.add(tab.id);
  const entry = await ensureTabWebview(tab, bounds, owner);

  if (!entry || activeHostOwner !== owner || desiredActiveTabId !== tab.id)
    return;

  await changeVisibility(async () => {
    if (activeHostOwner !== owner) return;
    if (tabWebviewsSuspended || desiredActiveTabId !== tab.id) {
      await setDemandActivity(entry.webview.label, false);
      await entry.webview.hide();
      return;
    }
    const previousEntry = visibleTabId
      ? managedWebviews.get(visibleTabId)
      : null;
    if (previousEntry && previousEntry.tabId !== tab.id) {
      await setDemandActivity(previousEntry.webview.label, false);
    }
    await setDemandActivity("main", false);
    // Reveal first so warm switches never expose the empty host between tabs.
    await entry.webview.show();
    if (activeHostOwner !== owner) {
      // An already-started native show can outlive cleanup. Hide its old
      // surface unless a current host has deliberately adopted that entry.
      if (entry.hostOwner === owner) {
        await setDemandActivity(entry.webview.label, false);
        await entry.webview.hide().catch(() => {});
      }
      return;
    }
    // Suspension/tab selection may change while the native show is pending.
    if (tabWebviewsSuspended || desiredActiveTabId !== tab.id) {
      await setDemandActivity(entry.webview.label, false);
      await entry.webview.hide();
      return;
    }
    visibleTabId = tab.id;
    const readinessVersion = nativeReadinessVersion;
    try {
      await setDemandActivity(entry.webview.label, true);
    } catch (error) {
      if (demandFailureCode(error) !== "not_ready") throw error;
      if (!readinessRetry) {
        pendingDemandActivation = { tab, bounds, owner, readinessVersion };
        // Also cover readiness delivered while the bounded command was pending.
        retryPendingDemand();
      }
      return;
    }
    if (tabWebviewsSuspended || desiredActiveTabId !== tab.id) {
      await setDemandActivity(entry.webview.label, false);
      await entry.webview.hide();
      visibleTabId = null;
      return;
    }
    if (previousEntry && previousEntry.tabId !== tab.id) {
      await previousEntry.webview.hide();
    }
    if (!tabWebviewsSuspended && desiredActiveTabId === tab.id) {
      await entry.webview.setFocus();
    }
  });
};

/** Suspend native tabs before mounting a host dialog; restore current selection. */
export async function setTabWebviewsSuspended(
  suspended: boolean,
): Promise<void> {
  tabWebviewsSuspended = suspended;
  if (suspended) {
    // Already-started creates must reach their created/hide callbacks before a
    // host modal is exposed. New creates are deferred by ensureTabWebview.
    await Promise.allSettled(
      [...ensureInFlightByTabId.values()].map(({ work }) => work),
    );
    await changeVisibility(async () => {
      if (!tabWebviewsSuspended) return;
      await setDemandActivity("main", false);
      // Include native surfaces that survived a host HMR/remount and have not
      // yet been adopted into managedWebviews. Credentials stay in the host.
      const nativeViews = await Webview.getAll();
      // The normal tab warm-up timeout is not proof that native creation has
      // finished. A later create could otherwise cover an already-open form.
      for (const view of nativeViews) pendingNativeCreates.delete(view.label);
      if (pendingNativeCreates.size > 0) {
        throw new Error("Native tabs are still starting");
      }
      const hidden = await Promise.allSettled(
        nativeViews
          .filter((view) => view.label.startsWith(TAB_WEBVIEW_LABEL_PREFIX))
          .map(async (view) => {
            await setDemandActivity(view.label, false);
            await view.hide();
          }),
      );
      // Drain every hide before permitting resume: an older pending hide must
      // not finish after restoration and leave the selected tab invisible.
      if (hidden.some((result) => result.status === "rejected")) {
        throw new Error("Could not hide tab views");
      }
      visibleTabId = null;
    });
    return;
  }
  const state = useAppStore.getState();
  const owner = activeHostOwner;
  const bounds = latestHostBounds;
  const activeTab = state.tabs.find((tab) => tab.id === state.activeTabId);
  if (owner && activeTab && bounds) {
    await activateTabWebview(activeTab, bounds, owner);
  } else {
    await changeVisibility(() => setDemandActivity("main", true));
  }
}

const reconcileTabWebviews = async (
  tabs: WorkspaceTab[],
  bounds: HostBounds,
  owner: HostOwner,
) => {
  if (activeHostOwner !== owner) return;
  liveTabIds = new Set(tabs.map((tab) => tab.id));

  const staleEntries = Array.from(managedWebviews.entries()).filter(
    ([tabId]) => !liveTabIds.has(tabId),
  );
  await Promise.all(
    staleEntries.map(async ([tabId, entry]) => {
      managedWebviews.delete(tabId);
      if (visibleTabId === tabId) visibleTabId = null;
      await closeManagedWebview(entry);
    }),
  );
  if (activeHostOwner !== owner) return;

  const activeTab = tabs.find((tab) => tab.id === desiredActiveTabId);
  const backgroundTabs = tabs.filter((tab) => tab.id !== desiredActiveTabId);

  if (activeTab) void activateTabWebview(activeTab, bounds, owner);

  // Prewarm background tabs concurrently without blocking the selected tab.
  await Promise.all(
    backgroundTabs.map(async (tab) => {
      const entry = await ensureTabWebview(tab, bounds, owner);
      if (entry) await hideUnlessActive(entry);
    }),
  );
};

const resizeManagedWebviews = async (bounds: HostBounds, owner: HostOwner) => {
  if (activeHostOwner !== owner) return;
  latestHostBounds = bounds;
  await Promise.all(
    Array.from(managedWebviews.values()).map((entry) =>
      updateManagedBounds(entry, bounds),
    ),
  );
};

const cleanupAllWebviews = () => {
  pendingDemandActivation = null;
  latestHostBounds = null;
  desiredActiveTabId = null;
  visibleTabId = null;
  liveTabIds.clear();
  ensureInFlightByTabId.clear();
  const entries = Array.from(managedWebviews.values());
  managedWebviews.clear();
  closingWebviews = Promise.allSettled([
    closingWebviews,
    ...entries.map((entry) => closeManagedWebview(entry)),
  ]).then(() => {});
  return closingWebviews;
};

const readHostBounds = (element: HTMLDivElement | null): HostBounds | null => {
  if (!element) return null;
  const rect = element.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return null;

  return {
    x: rect.left,
    y: rect.top,
    width: rect.width,
    height: rect.height,
  };
};

export default function WebviewTabHost() {
  const hostRef = useRef<HTMLDivElement>(null);
  const hostOwnerRef = useRef<HostOwner | null>(null);
  const tabs = useAppStore((state) => state.tabs);
  const activeTabId = useAppStore((state) => state.activeTabId);
  const [bounds, setBounds] = useState<HostBounds | null>(null);
  const tabsRef = useRef(tabs);
  const boundsRef = useRef(bounds);
  tabsRef.current = tabs;
  boundsRef.current = bounds;

  const tabIdSignature = useMemo(
    () => tabs.map((tab) => tab.id).join("\u0000"),
    [tabs],
  );
  const hasBounds = bounds !== null;

  useEffect(() => {
    const owner = {};
    hostOwnerRef.current = owner;
    activeHostOwner = owner;
    latestHostBounds = null;
    let stopped = false;
    let stopReadiness: (() => void) | null = null;
    let stopRuntimeReset: (() => void) | null = null;
    void listen("gitru:collaboration-change", () => {
      if (stopped || activeHostOwner !== owner) return;
      nativeReadinessVersion += 1;
      retryPendingDemand();
    })
      .then((unlisten) => {
        if (stopped) unlisten();
        else stopReadiness = unlisten;
      })
      .catch(() => {});
    void listen("gitru:collaboration-runtime-reset", () => {
      if (stopped || activeHostOwner !== owner || tabWebviewsSuspended) return;
      const state = useAppStore.getState();
      const activeTab = state.tabs.find((tab) => tab.id === state.activeTabId);
      const bounds = latestHostBounds;
      if (!activeTab || !bounds || visibleTabId !== activeTab.id) return;
      // Recovery discards native owner generations. Only the main host may
      // re-authorize its already-visible child in the replacement runtime.
      // Entry can reject while paused; the resume reset retries with fresh proof.
      nativeReadinessVersion += 1;
      void activateTabWebview(activeTab, bounds, owner).catch(() => {});
    })
      .then((unlisten) => {
        if (stopped) unlisten();
        else stopRuntimeReset = unlisten;
      })
      .catch(() => {});
    if (pendingCleanupTimer !== null) {
      window.clearTimeout(pendingCleanupTimer);
      pendingCleanupTimer = null;
    }

    return () => {
      stopped = true;
      stopReadiness?.();
      stopRuntimeReset?.();
      if (pendingDemandActivation?.owner === owner)
        pendingDemandActivation = null;
      if (activeHostOwner !== owner) return;
      // Dialog completion can release suspension after this host disappears.
      // Fence it immediately, even before delayed native cleanup has run.
      activeHostOwner = null;
      hostOwnerRef.current = null;
      latestHostBounds = null;
      // StrictMode immediately remounts effects in development. Delaying this
      // prevents its probe from destroying the persistent child surfaces.
      pendingCleanupTimer = window.setTimeout(() => {
        pendingCleanupTimer = null;
        if (activeHostOwner === null) void cleanupAllWebviews();
      }, 0);
    };
  }, []);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    let animationFrame: number | null = null;
    const updateBounds = () => {
      if (animationFrame !== null) return;
      animationFrame = window.requestAnimationFrame(() => {
        animationFrame = null;
        const nextBounds = readHostBounds(host);
        setBounds((current) =>
          current && nextBounds && areSameBounds(current, nextBounds)
            ? current
            : nextBounds,
        );
      });
    };

    updateBounds();
    const observer = new ResizeObserver(updateBounds);
    observer.observe(host);
    window.addEventListener("resize", updateBounds);

    return () => {
      if (animationFrame !== null) window.cancelAnimationFrame(animationFrame);
      observer.disconnect();
      window.removeEventListener("resize", updateBounds);
    };
  }, []);

  useEffect(() => {
    const owner = hostOwnerRef.current;
    if (!owner) return;
    desiredActiveTabId = activeTabId;
    const currentBounds = boundsRef.current;
    const activeTab = tabsRef.current.find((tab) => tab.id === activeTabId);
    if (currentBounds && activeTab) {
      void activateTabWebview(activeTab, currentBounds, owner);
    }
  }, [activeTabId, hasBounds]);

  useEffect(() => {
    const owner = hostOwnerRef.current;
    if (!owner) return;
    const currentBounds = boundsRef.current;
    if (currentBounds) {
      void reconcileTabWebviews(tabsRef.current, currentBounds, owner);
    }
  }, [tabIdSignature, hasBounds]);

  useEffect(() => {
    const owner = hostOwnerRef.current;
    if (bounds && owner) void resizeManagedWebviews(bounds, owner);
  }, [bounds]);

  return (
    <div
      ref={hostRef}
      className="h-full w-full rounded-lg bg-background/40"
      data-tab-webview-host
    />
  );
}
