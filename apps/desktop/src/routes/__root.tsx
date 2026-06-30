import { createRootRoute, Outlet } from "@tanstack/react-router";
import { PostHogProvider } from "posthog-js/react";

export const Route = createRootRoute({
  component: () => (
    <PostHogProvider
      apiKey={import.meta.env.VITE_PUBLIC_POSTHOG_PROJECT_TOKEN}
      options={{
        api_host: import.meta.env.VITE_PUBLIC_POSTHOG_HOST,
        debug: import.meta.env.DEV,
        capture_exceptions: true,
      }}
    >
      <div className="h-screen w-full">
        <Outlet />
      </div>
    </PostHogProvider>
  ),
});
