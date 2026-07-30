import { createRouter as createTanStackRouter } from "@tanstack/react-router";

import { routeTree } from "../routeTree.gen";
import { useLastPageStore } from "../store/use-last-page-store";
import {
  getRoutePathname,
  HOST_SHELL_ROUTE,
  isDesktopHostRuntime,
  isEmbeddedRuntime,
} from "./runtime-utils";

export const router = createTanStackRouter({
  routeTree,
  context: {},
  defaultPreload: "intent",
  scrollRestoration: true,
  defaultStructuralSharing: true,
  defaultPreloadStaleTime: 0,
});

if (!isEmbeddedRuntime()) {
  router.subscribe("onResolved", (state) => {
    if (
      isDesktopHostRuntime() &&
      getRoutePathname(state.toLocation.href).startsWith("/app")
    ) {
      useLastPageStore.getState().setLastPage(HOST_SHELL_ROUTE);
      return;
    }

    useLastPageStore.getState().setLastPage(state.toLocation.href);
  });
}

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
