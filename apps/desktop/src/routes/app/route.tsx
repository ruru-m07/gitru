import {
  createFileRoute,
  Outlet,
  useRouterState,
} from "@tanstack/react-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import CustomTitleBar from "@/CustomTitleBar";
import { ActionPannel } from "@/components/actionPannel";
import Sidebar from "@/components/sidebar";
import WebviewTabHost from "@/components/WebviewTabHost";

const isTauriRuntime = () =>
  typeof window !== "undefined" &&
  typeof (window as Window & { __TAURI_INTERNALS__?: unknown })
    .__TAURI_INTERNALS__ !== "undefined";

export const Route = createFileRoute("/app")({
  component: RouteComponent,
});

function RouteComponent() {
  const routerState = useRouterState();
  const searchParams = new URLSearchParams(routerState.location.search);
  const hasEmbeddedSearchFlag =
    searchParams.get("embedded") === "1" ||
    searchParams.get("embedded") === "true";

  let isEmbeddedWebviewLabel = false;

  if (isTauriRuntime()) {
    try {
      const label = getCurrentWebview().label;
      isEmbeddedWebviewLabel = label.startsWith("tab-webview:");
    } catch {
      isEmbeddedWebviewLabel = false;
    }
  }

  const isEmbedded = hasEmbeddedSearchFlag || isEmbeddedWebviewLabel;

  // Embedded tab webviews show actual content with ActionPanel
  if (isEmbedded) {
    return (
      <ActionPannel>
        <div className="h-screen w-full bg-secondary">
          <div className="flex h-screen w-screen px-(--main-actual-content-padding) pb-(--main-actual-content-padding) gap-(--main-actual-content-padding)">
            <Sidebar />
            <Outlet />
          </div>
        </div>
      </ActionPannel>
    );
  }

  // Non-Tauri (browser) mode with ActionPanel
  if (!isTauriRuntime()) {
    return (
      <ActionPannel>
        <div className="h-screen w-full bg-secondary">
          <CustomTitleBar
            restrictedPaths={["/login", "/register", "/welcome"]}
          />
          <div className="flex h-(--main-window-height) w-screen gap-(--main-actual-content-padding)">
            <Sidebar />
            <Outlet />
          </div>
        </div>
      </ActionPannel>
    );
  }

  // Main window with tabs - NO ActionPanel here!
  return (
    <div className="h-screen w-full bg-secondary">
      <CustomTitleBar restrictedPaths={["/login", "/register", "/welcome"]} />
      <div className="h-(--main-window-height) w-screen">
        <WebviewTabHost />
      </div>
    </div>
  );
}
