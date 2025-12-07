import {
  createRootRoute,
  createRoute,
  createRouter,
  HeadContent,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { StrictMode } from "react";
import ReactDOM from "react-dom/client";

import "./styles.css";
import "@fontsource/jetbrains-mono/500.css";

import App from "./App.tsx";
import Progress from "./components/progress.tsx";
import Roadmap from "./components/roadmap.tsx";
import Waitlist from "./components/waitlist.tsx";
import reportWebVitals from "./reportWebVitals.ts";

const rootRoute = createRootRoute({
  component: () => (
    <>
      <HeadContent />
      <Outlet />
      {/* <TanStackRouterDevtools /> */}
    </>
  ),
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: App,
});

const waitlistRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/waitlist",
  component: Waitlist,
});

const roadmapRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/roadmap",
  component: Roadmap,
});

const progressRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/progress",
  component: Progress,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  waitlistRoute,
  roadmapRoute,
  progressRoute,
]);

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
