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
    },
  },
});

let focusBridgeInitialized = false;

export async function refreshActiveQueriesAfterNativeFocus(
  client: QueryClient,
  invalidateBackendCaches: () => Promise<unknown>,
) {
  await client.cancelQueries({ type: "active" });

  try {
    await invalidateBackendCaches();
  } catch {
    // A frontend refetch is still useful if the native cache bridge is
    // unavailable; the watcher remains the primary freshness path.
  }

  await client.invalidateQueries({ refetchType: "active" });
}

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
    let nativeFocusGeneration = 0;

    // Use dynamic import so this remains safe in non-Tauri runtime contexts.
    void import("@tauri-apps/api/window")
      .then(async ({ getCurrentWindow }) => {
        if (disposed) return;
        tauriUnlisten = await getCurrentWindow().onFocusChanged(
          ({ payload: focused }) => {
            nativeFocusGeneration += 1;
            const generation = nativeFocusGeneration;

            if (!focused) {
              handleFocus(false);
              return;
            }

            // Pause focus-triggered refetches until the Rust cache is cleared.
            // This keeps the fallback correct even when filesystem watching
            // could not be established for a repository.
            handleFocus(false);
            void refreshActiveQueriesAfterNativeFocus(queryClient, async () => {
              const { invalidateRepoContextCaches } = await import(
                "@gitru/commands"
              );
              await invalidateRepoContextCaches();
            }).finally(() => {
              if (!disposed && generation === nativeFocusGeneration) {
                handleFocus(true);
              }
            });
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
