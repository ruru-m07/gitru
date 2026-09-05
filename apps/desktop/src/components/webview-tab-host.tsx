import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { Webview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useMemo, useRef, useState } from "react";
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
  webview: Webview;
  ready: Promise<void>;
  bounds: HostBounds;
};

const WEBVIEW_LABEL_PREFIX = "tab-webview:";
const CREATE_TIMEOUT_MS = 1200;

const managedWebviews = new Map<string, ManagedWebview>();
const ensureInFlightByTabId = new Map<string, Promise<ManagedWebview | null>>();
let desiredActiveTabId: string | null = null;
let visibleTabId: string | null = null;
let liveTabIds = new Set<string>();
let pendingCleanupTimer: number | null = null;

const sanitizeWebviewLabel = (tabId: string) =>
  `${WEBVIEW_LABEL_PREFIX}${tabId.replace(/[^a-zA-Z0-9\-/:_]/g, "_")}`;

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
  await Promise.allSettled([entry.webview.close()]);
};

const hideUnlessActive = async (entry: ManagedWebview) => {
  if (entry.tabId !== desiredActiveTabId) {
    await Promise.allSettled([entry.webview.hide()]);
  }
};

const ensureTabWebview = async (
  tab: WorkspaceTab,
  bounds: HostBounds,
): Promise<ManagedWebview | null> => {
  const existing = managedWebviews.get(tab.id);
  if (existing) {
    await existing.ready;
    return managedWebviews.get(tab.id) ?? null;
  }

  const existingEnsure = ensureInFlightByTabId.get(tab.id);
  if (existingEnsure) return await existingEnsure;

  const task = (async (): Promise<ManagedWebview | null> => {
    const normalized = normalizeBounds(bounds);
    const label = sanitizeWebviewLabel(tab.id);
    const existingByLabel = await Webview.getByLabel(label);

    if (existingByLabel) {
      const reused: ManagedWebview = {
        tabId: tab.id,
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
    const ready = new Promise<void>((resolve) => {
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        resolve();
      };

      void webview.once("tauri://created", () => {
        void hideUnlessActive({
          tabId: tab.id,
          webview,
          ready: Promise.resolve(),
          bounds: normalized,
        }).finally(finish);
      });

      void webview.once("tauri://error", (event) => {
        createError = event.payload;
        finish();
      });

      window.setTimeout(finish, CREATE_TIMEOUT_MS);
    });

    const created: ManagedWebview = {
      tabId: tab.id,
      webview,
      ready,
      bounds: normalized,
    };
    managedWebviews.set(tab.id, created);
    await ready;

    if (createError !== null) {
      const recovered = await Webview.getByLabel(label);
      if (recovered) {
        const entry: ManagedWebview = {
          tabId: tab.id,
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

  ensureInFlightByTabId.set(tab.id, task);
  try {
    return await task;
  } finally {
    if (ensureInFlightByTabId.get(tab.id) === task) {
      ensureInFlightByTabId.delete(tab.id);
    }
  }
};

const activateTabWebview = async (tab: WorkspaceTab, bounds: HostBounds) => {
  desiredActiveTabId = tab.id;
  liveTabIds.add(tab.id);
  const entry = await ensureTabWebview(tab, bounds);

  if (!entry || desiredActiveTabId !== tab.id) return;

  const previousEntry = visibleTabId ? managedWebviews.get(visibleTabId) : null;

  // Reveal first so warm switches never expose the empty host between tabs.
  await entry.webview.show();
  visibleTabId = tab.id;
  void entry.webview.setFocus();

  if (previousEntry && previousEntry.tabId !== tab.id) {
    void previousEntry.webview.hide();
  }
};

const reconcileTabWebviews = async (
  tabs: WorkspaceTab[],
  bounds: HostBounds,
) => {
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

  const activeTab = tabs.find((tab) => tab.id === desiredActiveTabId);
  const backgroundTabs = tabs.filter((tab) => tab.id !== desiredActiveTabId);

  if (activeTab) void activateTabWebview(activeTab, bounds);

  // Prewarm background tabs concurrently without blocking the selected tab.
  await Promise.all(
    backgroundTabs.map(async (tab) => {
      const entry = await ensureTabWebview(tab, bounds);
      if (entry) await hideUnlessActive(entry);
    }),
  );
};

const resizeManagedWebviews = async (bounds: HostBounds) => {
  await Promise.all(
    Array.from(managedWebviews.values()).map((entry) =>
      updateManagedBounds(entry, bounds),
    ),
  );
};

const cleanupAllWebviews = async () => {
  desiredActiveTabId = null;
  visibleTabId = null;
  liveTabIds.clear();
  ensureInFlightByTabId.clear();
  const entries = Array.from(managedWebviews.values());
  managedWebviews.clear();
  await Promise.all(entries.map(closeManagedWebview));
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
    desiredActiveTabId = activeTabId;
    const currentBounds = boundsRef.current;
    const activeTab = tabsRef.current.find((tab) => tab.id === activeTabId);
    if (currentBounds && activeTab) {
      void activateTabWebview(activeTab, currentBounds);
    }
  }, [activeTabId, hasBounds]);

  useEffect(() => {
    const currentBounds = boundsRef.current;
    if (currentBounds) {
      void reconcileTabWebviews(tabsRef.current, currentBounds);
    }
  }, [tabIdSignature, hasBounds]);

  useEffect(() => {
    if (bounds) void resizeManagedWebviews(bounds);
  }, [bounds]);

  useEffect(() => {
    if (pendingCleanupTimer !== null) {
      window.clearTimeout(pendingCleanupTimer);
      pendingCleanupTimer = null;
    }

    return () => {
      // StrictMode immediately remounts effects in development. Delaying this
      // prevents its probe from destroying the persistent child surfaces.
      pendingCleanupTimer = window.setTimeout(() => {
        pendingCleanupTimer = null;
        void cleanupAllWebviews();
      }, 0);
    };
  }, []);

  return (
    <div
      ref={hostRef}
      className="h-full w-full rounded-lg bg-background/40"
      data-tab-webview-host
    />
  );
}
