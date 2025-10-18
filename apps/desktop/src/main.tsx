import { createRouter, RouterProvider } from "@tanstack/react-router";
import { ThemeProvider as NextThemesProvider } from "next-themes";
import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { Toaster } from "sonner";
import reportWebVitals from "./reportWebVitals.ts";
import { routeTree } from "./routeTree.gen";
import "./app.css";
import { colorKeyList } from "./lib/colors.ts";
import { useLastPageStore } from "./store/useLastPageStore.ts";

// Create a new router instance
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

// Register the router instance for type safety
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

async function redirectToLastPage() {
  const { lastPage } = useLastPageStore.getState();
  // If not already on the lastPage, navigate before initial render
  if (!lastPage) return;
  if (lastPage === "/") return;
  if (window.location.pathname + window.location.search !== lastPage) {
    await router.navigate({ to: lastPage });
  }
}

await redirectToLastPage();

// Render the app
const rootElement = document.getElementById("root");
if (rootElement && !rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);
  root.render(
    <StrictMode>
      <NextThemesProvider
        disableTransitionOnChange
        defaultTheme="dark-classic"
        enableColorScheme
        themes={colorKeyList}
      >
        <RouterProvider router={router} />
        <Toaster />
      </NextThemesProvider>
    </StrictMode>,
  );
}

// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
reportWebVitals();
