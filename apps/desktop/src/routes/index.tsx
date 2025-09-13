import { createFileRoute } from "@tanstack/react-router";
import "../App.css";
import { cn } from "@noutify/ui/lib/utils";

export const Route = createFileRoute("/")({
	component: App,
});

function App() {
	return (
		<div
			className={cn(
				"ml-[var(--main-actual-content-padding)] bg-secondary h-full w-full px-2 rounded-md",
				"flex items-center justify-center",
			)}
		>
			<span className="text-4xl font-semibold">Hello ;D</span>
		</div>
	);
}
