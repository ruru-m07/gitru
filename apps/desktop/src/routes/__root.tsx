import { createRootRoute, Outlet } from "@tanstack/react-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { StateFlags, saveWindowState } from "@tauri-apps/plugin-window-state";
import { PostHogProvider, usePostHog } from "posthog-js/react";
import { useEffect } from "react";

const WINDOW_STATE_FLAGS =
  StateFlags.SIZE | StateFlags.POSITION | StateFlags.MAXIMIZED;
const WINDOW_STATE_SAVE_DELAY_MS = 400;
const WINDOW_STATE_STARTUP_DELAY_MS = 1_000;

const isEmbeddedRuntime = () => {
  if (typeof window === "undefined") {
    return false;
  }

  const searchParams = new URLSearchParams(window.location.search);
  return (
    searchParams.get("embedded") === "1" ||
    searchParams.get("embedded") === "true"
  );
};

function AnalyticsBootstrap() {
  const posthog = usePostHog();

  useEffect(() => {
    posthog.opt_in_capturing();
    posthog.capture("desktop_app_open");

    const sendPresencePing = () => {
      if (document.visibilityState === "visible") {
        posthog.capture("desktop_app_presence_ping");
      }
    };

    sendPresencePing();

    const intervalId = window.setInterval(sendPresencePing, 120_000);

    document.addEventListener("visibilitychange", sendPresencePing);
    window.addEventListener("focus", sendPresencePing);

    return () => {
      window.clearInterval(intervalId);
      document.removeEventListener("visibilitychange", sendPresencePing);
      window.removeEventListener("focus", sendPresencePing);
    };
  }, [posthog]);

  return null;
}

function WindowStatePersistence() {
  useEffect(() => {
    let disposed = false;
    let saveTimer: number | undefined;
    let removeMoveListener: (() => void) | undefined;
    let removeResizeListener: (() => void) | undefined;

    const save = () => {
      saveTimer = undefined;
      void saveWindowState(WINDOW_STATE_FLAGS).catch((error) => {
        console.error("failed to persist window state", error);
      });
    };

    const scheduleSave = () => {
      window.clearTimeout(saveTimer);
      saveTimer = window.setTimeout(save, WINDOW_STATE_SAVE_DELAY_MS);
    };

    const registerListeners = async () => {
      const appWindow = getCurrentWindow();
      const listeners = await Promise.all([
        appWindow.onMoved(scheduleSave),
        appWindow.onResized(scheduleSave),
      ]);

      if (disposed) {
        for (const removeListener of listeners) removeListener();
        return;
      }

      [removeMoveListener, removeResizeListener] = listeners;
    };

    const startupTimer = window.setTimeout(() => {
      void registerListeners().catch((error) => {
        console.error("failed to monitor window state", error);
      });
    }, WINDOW_STATE_STARTUP_DELAY_MS);

    return () => {
      disposed = true;
      window.clearTimeout(startupTimer);
      window.clearTimeout(saveTimer);
      removeMoveListener?.();
      removeResizeListener?.();
    };
  }, []);

  return null;
}

export const Route = createRootRoute({
  component: () => {
    const content = (
      <div className="h-screen w-full">
        <Outlet />
      </div>
    );

    if (isEmbeddedRuntime()) {
      return content;
    }

    return (
      <PostHogProvider
        apiKey={import.meta.env.VITE_PUBLIC_POSTHOG_PROJECT_TOKEN || "disabled"}
        options={{
          api_host:
            import.meta.env.VITE_PUBLIC_POSTHOG_HOST ||
            "https://us.i.posthog.com",
          debug: import.meta.env.DEV,
          capture_exceptions: false,
          capture_pageview: false,
          capture_pageleave: false,
          disable_session_recording: true,
          disable_surveys: true,
          advanced_disable_flags: true,
          autocapture: false,
          opt_out_capturing_by_default: false,
          persistence: "memory",
          person_profiles: "never",
          sanitize_properties: (properties) => {
            const sanitized = { ...properties };
            for (const key of [
              "$current_url",
              "$host",
              "$initial_current_url",
              "$initial_referrer",
              "$initial_referring_domain",
              "$pageview_id",
              "$pathname",
              "$referrer",
              "$referring_domain",
              "$title",
            ]) {
              delete sanitized[key];
            }
            return sanitized;
          },
        }}
      >
        <WindowStatePersistence />
        <AnalyticsBootstrap />
        {content}
      </PostHogProvider>
    );
  },
});
