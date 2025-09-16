import { createFileRoute, Outlet } from "@tanstack/react-router";

export const Route = createFileRoute("/auth")({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div className="h-screen w-full relative">
			<DragAble />
			<Outlet />
		</div>
	);
}

const DragAble = () => {
	return (
		<div
			className="h-[var(--main-custom-header-height)] bg-background absolute flex items-center justify-between w-full px-4 select-none border-b"
			data-tauri-drag-region
			style={{
				// @ts-expect-error - ¯\_(ツ)_/¯
				WebkitAppRegion: "drag",
			}}
		></div>
	);
};
