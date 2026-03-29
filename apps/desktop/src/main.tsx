import { createRouter, RouterProvider } from "@tanstack/react-router";
import { ThemeProvider as NextThemesProvider } from "next-themes";
import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { scan } from "react-scan";
import { Toaster } from "sonner";

import { colorKeyList } from "./lib/colors.ts";
import { routeTree } from "./routeTree.gen";
import { useLastPageStore } from "./store/useLastPageStore.ts";
import "./app.css";

import { QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { TabContextProvider } from "./context/TabContextProvider";
import { appState } from "./state";
import { initializeQueryFocusBridge } from "./state/core/StateManager";
import { useAppStore } from "./store/useAppStore";

const router = createRouter({
  routeTree,
  context: {},
  defaultPreload: "intent",
  scrollRestoration: true,
  defaultStructuralSharing: true,
  defaultPreloadStaleTime: 0,
});

router.subscribe("onResolved", (state) => {
  useLastPageStore.getState().setLastPage(state.toLocation.href);
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

async function redirectToLastPage() {
  const { lastPage } = useLastPageStore.getState();
  if (!lastPage) return;
  if (lastPage === "/") return;
  if (window.location.pathname + window.location.search !== lastPage) {
    await router.navigate({ to: lastPage });
  }
}

await redirectToLastPage();
initializeQueryFocusBridge();

const rootElement = document.getElementById("root");
if (rootElement && !rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);

  const AppRouter = () => {
    const activeTabId = useAppStore((state) => state.activeTabId);

    return (
      <TabContextProvider scopeId={activeTabId ?? "tab-main"}>
        <RouterProvider router={router} />
      </TabContextProvider>
    );
  };

  root.render(
    <StrictMode>
      <QueryClientProvider client={appState.queryClient}>
        <NextThemesProvider
          disableTransitionOnChange
          defaultTheme="light"
          enableColorScheme
          themes={colorKeyList}
        >
          <AppRouter />
          <Toaster />
          {import.meta.env.DEV && (
            <ReactQueryDevtools
              buttonPosition="top-right"
              initialIsOpen={false}
            />
          )}
        </NextThemesProvider>
      </QueryClientProvider>
    </StrictMode>,
  );
}

if (import.meta.env.DEV) {
  scan({
    enabled: true,
  });
}
