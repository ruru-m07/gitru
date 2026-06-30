import { createRootRoute, Outlet } from "@tanstack/react-router";
import { PostHogProvider, usePostHog } from "posthog-js/react";
import { useEffect } from "react";

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
        apiKey={import.meta.env.VITE_PUBLIC_POSTHOG_PROJECT_TOKEN}
        options={{
          api_host: import.meta.env.VITE_PUBLIC_POSTHOG_HOST,
          debug: import.meta.env.DEV,
          capture_exceptions: false,
          capture_pageview: false,
          capture_pageleave: false,
          disable_session_recording: true,
          autocapture: false,
        }}
      >
        <AnalyticsBootstrap />
        {content}
      </PostHogProvider>
    );
  },
});
