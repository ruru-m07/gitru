import {
  createFileRoute,
  Outlet,
  useRouterState,
} from "@tanstack/react-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ActionPanel } from "@/components/action-panel";
import CustomTitleBar from "@/components/custom-title-bar";
import PageLayout from "@/components/page-layout";
import Sidebar from "@/components/sidebar";
import WebviewTabHost from "@/components/webview-tab-host";

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

  try {
    const label = getCurrentWebview().label;
    isEmbeddedWebviewLabel = label.startsWith("tab-webview:");
  } catch {
    isEmbeddedWebviewLabel = false;
  }

  const isEmbedded = hasEmbeddedSearchFlag || isEmbeddedWebviewLabel;

  if (isEmbedded) {
    return (
      <ActionPanel>
        <div className="h-screen w-full bg-secondary -z-10">
          <div className="flex h-screen w-screen px-(--main-actual-content-padding) pb-(--main-actual-content-padding) gap-(--main-actual-content-padding)">
            <Sidebar />
            <Outlet />
          </div>
        </div>
      </ActionPanel>
    );
  }

  return (
    <div className="max-h-screen overflow-hidden w-full bg-secondary">
      <div className="max-h-screen overflow-hidden">
        <CustomTitleBar restrictedPaths={["/login", "/register", "/welcome"]} />
        <div className="absolute top-(--main-custom-header-height) flex max-h-[calc(100vh-var(--main-custom-header-height))] h-full w-screen px-(--main-actual-content-padding) pb-(--main-actual-content-padding) gap-(--main-actual-content-padding)">
          <Sidebar />
          <PageLayout></PageLayout>
        </div>
      </div>
      <div className="h-(--main-window-height) w-screen">
        <WebviewTabHost />
      </div>
    </div>
  );
}
