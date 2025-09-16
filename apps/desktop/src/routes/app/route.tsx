import { createFileRoute, Outlet } from "@tanstack/react-router";
import CustomTitleBar from "@/CustomTitleBar";
import Sidebar from "@/components/sidebar";

export const Route = createFileRoute("/app")({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div className="h-screen w-full">
			<CustomTitleBar restrictedPaths={["/login", "/register", "/welcome"]} />
			<div className="flex h-[var(--main-window-height)] w-screen px-[var(--main-actual-content-padding)] pb-[var(--main-actual-content-padding)] gap-[var(--main-actual-content-padding)]">
				<Sidebar />
				<Outlet />
			</div>
		</div>
	);
}
