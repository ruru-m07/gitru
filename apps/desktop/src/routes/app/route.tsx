import { createFileRoute, Outlet } from "@tanstack/react-router";
import CustomTitleBar from "@/CustomTitleBar";
import { ActionPannel } from "@/components/actionPannel";
import Sidebar from "@/components/sidebar";

export const Route = createFileRoute("/app")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <ActionPannel>
      <div className="h-screen w-full">
        <CustomTitleBar restrictedPaths={["/login", "/register", "/welcome"]} />
        <div className="flex h-(--main-window-height) w-screen px-(--main-actual-content-padding) pb-(--main-actual-content-padding) gap-(--main-actual-content-padding)">
          <Sidebar />
          <Outlet />
        </div>
      </div>
    </ActionPannel>
  );
}
