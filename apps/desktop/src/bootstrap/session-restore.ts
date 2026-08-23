import { useLastPageStore } from "../store/use-last-page-store";
import { router } from "./create-router";
import {
  getRoutePathname,
  HOST_SHELL_ROUTE,
  isDesktopHostRuntime,
  isEmbeddedRuntime,
} from "./runtime-utils";

export async function redirectToLastPage() {
  if (isEmbeddedRuntime()) return;

  if (isDesktopHostRuntime()) {
    const currentPathIsApp = window.location.pathname.startsWith("/app");
    const alreadyAtHostShell =
      window.location.pathname === HOST_SHELL_ROUTE &&
      window.location.search.length === 0 &&
      window.location.hash.length === 0;

    if (currentPathIsApp && !alreadyAtHostShell) {
      await router.navigate({ to: HOST_SHELL_ROUTE });
      return;
    }
  }

  const { lastPage } = useLastPageStore.getState();
  if (!lastPage) return;
  if (lastPage === "/") return;

  const hasEmbeddedFlag = (() => {
    try {
      const url = new URL(lastPage, window.location.origin);
      const embedded = url.searchParams.get("embedded");
      return embedded === "1" || embedded === "true";
    } catch {
      return (
        lastPage.includes("embedded=1") || lastPage.includes("embedded=true")
      );
    }
  })();

  if (hasEmbeddedFlag) {
    return;
  }

  if (isDesktopHostRuntime() && getRoutePathname(lastPage).startsWith("/app")) {
    if (
      window.location.pathname !== HOST_SHELL_ROUTE ||
      window.location.search.length > 0 ||
      window.location.hash.length > 0
    ) {
      await router.navigate({ to: HOST_SHELL_ROUTE });
    }
    return;
  }

  if (window.location.pathname + window.location.search !== lastPage) {
    await router.navigate({ to: lastPage });
  }
}
