import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { Webview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";
import { useAppStore, type WorkspaceTab } from "@/store/useAppStore";

type HostBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type ManagedWebview = {
  tabId: string;
  routePath: string;
  webview: Webview;
  ready: Promise<void>;
};

const WEBVIEW_LABEL_PREFIX = "tab-webview:";

const managedWebviews = new Map<string, ManagedWebview>();
const ensureInFlightByTabId = new Map<string, Promise<ManagedWebview | null>>();

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
  if (pathname === "/app" || pathname === "/app/") {
    return "/app/git";
  }
  return routePath;
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

const setWebviewBounds = async (webview: Webview, bounds: HostBounds) => {
  const normalized = normalizeBounds(bounds);

  await Promise.allSettled([
    webview.setPosition(new LogicalPosition(normalized.x, normalized.y)),
    webview.setSize(new LogicalSize(normalized.width, normalized.height)),
  ]);
};

const closeManagedWebview = async (entry: ManagedWebview) => {
  await Promise.allSettled([entry.webview.close()]);
};

const ensureTabWebview = async (
  tab: WorkspaceTab,
  bounds: HostBounds,
): Promise<ManagedWebview | null> => {
  const existingEnsure = ensureInFlightByTabId.get(tab.id);
  if (existingEnsure) {
    const pending = await existingEnsure;
    if (pending) {
      await setWebviewBounds(pending.webview, bounds);
    }
    return pending;
  }

  const task = (async (): Promise<ManagedWebview | null> => {
    const normalizedRoutePath = normalizeWorkspaceRoutePath(tab.routePath);
    const existing = managedWebviews.get(tab.id);

    if (existing) {
      if (existing.routePath !== normalizedRoutePath) {
        existing.routePath = normalizedRoutePath;
      }

      await setWebviewBounds(existing.webview, bounds);
      return existing;
    }

    const label = sanitizeWebviewLabel(tab.id);
    const existingByLabel = await Webview.getByLabel(label);

    if (existingByLabel) {
      const reused: ManagedWebview = {
        tabId: tab.id,
        routePath: normalizedRoutePath,
        webview: existingByLabel,
        ready: Promise.resolve(),
      };

      managedWebviews.set(tab.id, reused);
      await setWebviewBounds(existingByLabel, bounds);
      return reused;
    }

    let targetUrl = "";

    try {
      targetUrl = toChildWebviewPath(normalizedRoutePath);
    } catch (error) {
      console.error("Failed to resolve child webview URL", {
        tabId: tab.id,
        routePath: normalizedRoutePath,
        error,
      });
      return null;
    }

    const normalized = normalizeBounds(bounds);
    const appWindow = getCurrentWindow();
    let webview: Webview;
    let alreadyExistsError = false;
    let createErrorPayload = "";

    try {
      webview = new Webview(appWindow, label, {
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

    let didFailToCreate = false;

    const ready = new Promise<void>((resolve) => {
      let settled = false;

      void webview.once("tauri://created", () => {
        if (settled) return;
        settled = true;
        resolve();
      });

      void webview.once("tauri://error", (event) => {
        didFailToCreate = true;
        const payload =
          typeof event.payload === "string"
            ? event.payload
            : String(event.payload);
        createErrorPayload = payload;
        alreadyExistsError = payload.includes("already exists");
        console.error("Child webview failed to initialize", {
          tabId: tab.id,
          targetUrl,
          event,
        });
        if (settled) return;
        settled = true;
        resolve();
      });

      setTimeout(() => {
        if (settled) return;
        settled = true;
        resolve();
      }, 1200);
    });

    const created: ManagedWebview = {
      tabId: tab.id,
      routePath: normalizedRoutePath,
      webview,
      ready,
    };

    managedWebviews.set(tab.id, created);

    await created.ready;

    if (didFailToCreate) {
      if (alreadyExistsError) {
        const recoveryWebview = await Webview.getByLabel(label);
        if (recoveryWebview) {
          const recovered: ManagedWebview = {
            tabId: tab.id,
            routePath: tab.routePath,
            webview: recoveryWebview,
            ready: Promise.resolve(),
          };

          managedWebviews.set(tab.id, recovered);
          await setWebviewBounds(recoveryWebview, normalized);
          return recovered;
        }
      }

      if (createErrorPayload) {
        console.error("Child webview create failure was not recoverable", {
          tabId: tab.id,
          label,
          targetUrl,
          createErrorPayload,
        });
      }

      await closeManagedWebview(created);
      managedWebviews.delete(tab.id);
      return null;
    }

    await setWebviewBounds(webview, normalized);

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

const syncTabWebviews = async (
  tabs: WorkspaceTab[],
  activeTabId: string,
  bounds: HostBounds,
) => {
  const activeIds = new Set(tabs.map((tab) => tab.id));

  for (const [tabId, entry] of Array.from(managedWebviews.entries())) {
    if (activeIds.has(tabId)) {
      continue;
    }

    await closeManagedWebview(entry);
    managedWebviews.delete(tabId);
  }

  for (const tab of tabs) {
    await ensureTabWebview(tab, bounds);
  }

  for (const tab of tabs) {
    const entry = managedWebviews.get(tab.id);

    if (!entry) {
      continue;
    }

    await entry.ready;
    await setWebviewBounds(entry.webview, bounds);

    if (tab.id === activeTabId) {
      await Promise.allSettled([
        entry.webview.show(),
        entry.webview.setFocus(),
      ]);
      continue;
    }

    await Promise.allSettled([entry.webview.hide()]);
  }
};

const cleanupAllWebviews = async () => {
  ensureInFlightByTabId.clear();

  for (const entry of Array.from(managedWebviews.values())) {
    await closeManagedWebview(entry);
  }

  managedWebviews.clear();
};

const readHostBounds = (element: HTMLDivElement | null): HostBounds | null => {
  if (!element) {
    return null;
  }

  const rect = element.getBoundingClientRect();

  if (rect.width <= 0 || rect.height <= 0) {
    return null;
  }

  return {
    x: rect.left,
    y: rect.top,
    width: rect.width,
    height: rect.height,
  };
};

export default function WebviewTabHost() {
  const hostRef = useRef<HTMLDivElement>(null);
  const syncQueueRef = useRef<Promise<void>>(Promise.resolve());
  const tabs = useAppStore((state) => state.tabs);
  const activeTabId = useAppStore((state) => state.activeTabId);
  const [bounds, setBounds] = useState<HostBounds | null>(null);

  useEffect(() => {
    const host = hostRef.current;

    if (!host) {
      return;
    }

    const updateBounds = () => {
      setBounds(readHostBounds(host));
    };

    updateBounds();

    const observer = new ResizeObserver(() => {
      updateBounds();
    });

    observer.observe(host);
    window.addEventListener("resize", updateBounds);

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", updateBounds);
    };
  }, []);

  useEffect(() => {
    if (!bounds || !activeTabId || tabs.length === 0) {
      return;
    }

    syncQueueRef.current = syncQueueRef.current
      .catch(() => {
        // Keep queue alive even if previous cycle failed.
      })
      .then(() => syncTabWebviews(tabs, activeTabId, bounds));
  }, [activeTabId, bounds, tabs]);

  useEffect(() => {
    return () => {
      void cleanupAllWebviews();
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
