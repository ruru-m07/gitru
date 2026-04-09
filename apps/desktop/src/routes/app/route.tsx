import {
  createFileRoute,
  Outlet,
  useRouterState,
} from "@tanstack/react-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import CustomTitleBar from "@/CustomTitleBar";
import { ActionPannel } from "@/components/actionPannel";
import PageLayout from "@/components/pageLayout";
import Sidebar from "@/components/sidebar";
import WebviewTabHost from "@/components/WebviewTabHost";

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
      <ActionPannel>
        <div className="h-screen w-full bg-secondary -z-10">
          <div className="flex h-screen w-screen px-(--main-actual-content-padding) pb-(--main-actual-content-padding) gap-(--main-actual-content-padding)">
            <Sidebar />
            <Outlet />
          </div>
        </div>
      </ActionPannel>
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
