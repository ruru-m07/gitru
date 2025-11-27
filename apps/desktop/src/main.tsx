import { createRouter, RouterProvider } from "@tanstack/react-router";
import { ThemeProvider as NextThemesProvider } from "next-themes";
import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { Toaster } from "sonner";

import { colorKeyList } from "./lib/colors.ts";
import { routeTree } from "./routeTree.gen";
import { useLastPageStore } from "./store/useLastPageStore.ts";
import "./app.css";

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

const rootElement = document.getElementById("root");
if (rootElement && !rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);
  root.render(
    <StrictMode>
      <NextThemesProvider
        disableTransitionOnChange
        defaultTheme="light"
        enableColorScheme
        themes={colorKeyList}
      >
        <RouterProvider router={router} />
        <Toaster />
      </NextThemesProvider>
    </StrictMode>,
  );
}
