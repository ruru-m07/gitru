import { DEFAULT_TAB_ROUTE } from "./constants";

export const areSameTabOrder = (left: string[], right: string[]) =>
  left.length === right.length &&
  left.every((value, index) => value === right[index]);

export const isEmbeddedRuntime = () => {
  try {
    return window.location.search.includes("embedded=1") ||
      window.location.search.includes("embedded=true")
      ? true
      : ((
          window as Window & {
            __TAURI_INTERNALS__?: {
              metadata?: { currentWebview?: { label?: string } };
            };
          }
        ).__TAURI_INTERNALS__?.metadata?.currentWebview?.label?.startsWith(
          "tab-webview:",
        ) ?? false);
  } catch {
    return false;
  }
};

export const getRoutePathname = (routePath: string) => {
  try {
    const origin =
      typeof window !== "undefined"
        ? window.location.origin
        : "http://localhost";
    return new URL(routePath, origin).pathname;
  } catch {
    return routePath.split("?")[0].split("#")[0];
  }
};

export const getTitleFromRoute = (routePath: string) => {
  const pathname = getRoutePathname(routePath);

  if (pathname.startsWith("/app/")) {
    const segment = pathname.slice("/app/".length).split("/")[0];

    if (segment) {
      return segment.charAt(0).toUpperCase() + segment.slice(1);
    }
  }

  return "Workspace";
};

export const isGitRoute = (routePath: string) =>
  getRoutePathname(routePath).startsWith("/app/git");

export const normalizeTabRoutePath = (routePath: string | null | undefined) => {
  if (!routePath) {
    return DEFAULT_TAB_ROUTE;
  }

  const pathname = getRoutePathname(routePath);
  if (pathname === "/app" || pathname === "/app/") {
    return DEFAULT_TAB_ROUTE;
  }

  return routePath;
};
