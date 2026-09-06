import { createRootRoute, Outlet } from "@tanstack/react-router";
import { PostHogProvider, usePostHog } from "posthog-js/react";
import { useEffect } from "react";
import { useTelemetryConsent } from "@/lib/telemetry-preference";

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

function AnalyticsBootstrap({ enabled }: { enabled: boolean }) {
  const posthog = usePostHog();

  useEffect(() => {
    if (!enabled) {
      posthog.opt_out_capturing();
      return;
    }

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
  }, [enabled, posthog]);

  return null;
}

export const Route = createRootRoute({
  component: () => {
    const telemetryEnabled = useTelemetryConsent();
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
          opt_out_capturing_by_default: true,
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
        <AnalyticsBootstrap enabled={telemetryEnabled} />
        {content}
      </PostHogProvider>
    );
  },
});
