import {
  SessionNavigationInfo,
  sessionClearHistory,
  sessionGetNavigationState,
  sessionGoBack,
  sessionGoForward,
  sessionPushToHistory,
} from "@gitru/commands";
import { useCallback, useEffect, useState } from "react";

export function useSessionNavigation(sessionId: string | null) {
  const [navigationState, setNavigationState] =
    useState<SessionNavigationInfo | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  // Fetch current navigation state
  const getNavigationState = useCallback(async () => {
    if (!sessionId) return;

    try {
      setIsLoading(true);
      const state = await sessionGetNavigationState({
        req: {
          sessionId,
        },
      });
      console.log("[SessionNav] Got navigation state:", state);
      setNavigationState(state);
    } catch (error) {
      console.error("Failed to get navigation state:", error);
    } finally {
      setIsLoading(false);
    }
  }, [sessionId]);

  // Push a new route to history
  const pushToHistory = useCallback(
    async (routePath: string) => {
      if (!sessionId) {
        console.log("[SessionNav] pushToHistory skipped - no sessionId");
        return;
      }

      try {
        console.log("[SessionNav] Pushing to history:", {
          sessionId,
          routePath,
        });
        const state = await sessionPushToHistory({
          req: {
            routePath,
            sessionId,
          },
        });
        console.log("[SessionNav] Push successful:", state);
        setNavigationState(state);
      } catch (error) {
        console.error("Failed to push to history:", error);
      }
    },
    [sessionId],
  );

  // Go back in history
  const goBack = useCallback(async () => {
    if (!sessionId) {
      console.log("[SessionNav] goBack skipped - no sessionId");
      return;
    }

    try {
      console.log("[SessionNav] Going back:", { sessionId });
      const state = await sessionGoBack({
        req: {
          sessionId,
        },
      });
      console.log("[SessionNav] Go back result:", state);
      if (state) {
        setNavigationState(state);
        return state;
      }
    } catch (error) {
      console.error("Failed to go back:", error);
    }
  }, [sessionId]);

  // Go forward in history
  const goForward = useCallback(async () => {
    if (!sessionId) {
      console.log("[SessionNav] goForward skipped - no sessionId");
      return;
    }

    try {
      console.log("[SessionNav] Going forward:", { sessionId });
      const state = await sessionGoForward({
        req: {
          sessionId,
        },
      });
      console.log("[SessionNav] Go forward result:", state);
      if (state) {
        setNavigationState(state);
        return state;
      }
    } catch (error) {
      console.error("Failed to go forward:", error);
    }
  }, [sessionId]);

  // Clear session history
  const clearHistory = useCallback(async () => {
    if (!sessionId) return;

    try {
      console.log("[SessionNav] Clearing history for session:", sessionId);
      await sessionClearHistory({
        req: {
          sessionId,
        },
      });
      setNavigationState(null);
    } catch (error) {
      console.error("Failed to clear history:", error);
    }
  }, [sessionId]);

  // Fetch initial state when sessionId changes
  useEffect(() => {
    console.log("[SessionNav] sessionId changed:", sessionId);
    void getNavigationState();
  }, [sessionId, getNavigationState]);

  return {
    navigationState,
    isLoading,
    pushToHistory,
    goBack,
    goForward,
    clearHistory,
    getNavigationState,
  };
}
