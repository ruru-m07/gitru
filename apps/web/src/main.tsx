import {
  createRootRoute,
  createRoute,
  createRouter,
  HeadContent,
  Outlet,
  RouterProvider,
  redirect,
} from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/react-router-devtools";
import { StrictMode } from "react";
import ReactDOM from "react-dom/client";

import "./styles.css";
import "@fontsource/jetbrains-mono/500.css";

import App from "./App.tsx";
import reportWebVitals from "./reportWebVitals.ts";

const rootRoute = createRootRoute({
  component: () => (
    <>
      <HeadContent />
      <Outlet />
      <TanStackRouterDevtools />
    </>
  ),
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  beforeLoad: () => {
    throw redirect({ to: "/waitlist" });
  },
});

const waitlistRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/waitlist",
  component: App,
  head: () => {
    return {
      meta: [
        { property: "og:title", content: "Join the Waitlist" },
        { property: "og:description", content: "Sign up to get early access." },
        {
          property: "og:image",
          content: `${window.location.origin}/waitlist-og.png`,
        },
        { property: "og:url", content: `${window.location.origin}/waitlist` },
        { name: "twitter:card", content: "summary_large_image" },
      ],
      title: "Join the Waitlist - Gitru",
    };
  },
});

const routeTree = rootRoute.addChildren([indexRoute, waitlistRoute]);

const router = createRouter({
  routeTree,
  context: {},
  defaultPreload: "intent",
  scrollRestoration: true,
  defaultStructuralSharing: true,
  defaultPreloadStaleTime: 0,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

const rootElement = document.getElementById("app");
if (rootElement && !rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);
  root.render(
    <StrictMode>
      <RouterProvider router={router} />
    </StrictMode>,
  );
}

reportWebVitals();
