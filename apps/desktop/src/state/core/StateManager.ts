import { focusManager, QueryClient } from "@tanstack/react-query";

/**
 * Centralized Query Client for the application state management.
 * We are using tanstack's react query for state invalidation and caching.
 * This client is shared across all state **domains**.
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5,
      gcTime: 1000 * 60 * 30,
      retry: 2,
      refetchOnWindowFocus: true,
      refetchInterval: 30000,
    },
  },
});

let focusBridgeInitialized = false;

/**
 * React Query relies on browser focus events by default. In Tauri, those can be
 * inconsistent, so we bridge Tauri's native focus change signal into Query's
 * focus manager and invalidate active queries when the app regains focus.
 */
export function initializeQueryFocusBridge() {
  if (focusBridgeInitialized || typeof window === "undefined") return;
  focusBridgeInitialized = true;

  focusManager.setEventListener((handleFocus) => {
    const onWindowFocus = () => handleFocus(true);
    const onWindowBlur = () => handleFocus(false);
    const onVisibilityChange = () => {
      handleFocus(document.visibilityState !== "hidden");
    };

    window.addEventListener("focus", onWindowFocus, false);
    window.addEventListener("blur", onWindowBlur, false);
    window.addEventListener("visibilitychange", onVisibilityChange, false);

    let disposed = false;
    let tauriUnlisten: (() => void) | undefined;

    // Use dynamic import so this remains safe in non-Tauri runtime contexts.
    void import("@tauri-apps/api/window")
      .then(async ({ getCurrentWindow }) => {
        if (disposed) return;
        tauriUnlisten = await getCurrentWindow().onFocusChanged(
          ({ payload: focused }) => {
            handleFocus(focused);
            if (focused) {
              void queryClient.invalidateQueries({ refetchType: "active" });
            }
          },
        );
      })
      .catch(() => {
        // Ignore when Tauri APIs are unavailable (e.g. plain web runtime).
      });

    return () => {
      disposed = true;
      window.removeEventListener("focus", onWindowFocus);
      window.removeEventListener("blur", onWindowBlur);
      window.removeEventListener("visibilitychange", onVisibilityChange);
      tauriUnlisten?.();
    };
  });
}

/**
 * Base class for all state domains.
 * Provides common utilities for query key management and invalidation.
 * More like a Base of all state domains.
 */
export abstract class StateDomain {
  constructor(protected queryClient: QueryClient) {}

  protected getQueryKey(...keys: string[]): string[] {
    return [this.constructor.name, ...keys];
  }
}
