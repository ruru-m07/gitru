import { createFileRoute } from "@tanstack/react-router";
import "../App.css";
import { Button } from "@noutify/ui/components/button";
import { cn } from "@noutify/ui/lib/utils";
import { getStatus } from "@/tauri";

export const Route = createFileRoute("/")({
	component: App,
});

function App() {
	return (
		<div
			className={cn(
				"ml-[var(--main-actual-content-padding)] bg-secondary h-full w-full px-2 rounded-md",
				"flex items-center justify-center flex-col",
			)}
		>
			<span className="text-4xl font-semibold">Hello ;D</span>
			<Button
				onClick={async () => {
					const data = await getStatus({
						repo_path: "/Users/ruru/sandbox/noutify",
					});
					data.files.forEach((file) => {
						console.log(file.path, file.status);
					});
				}}
			>
				Click me!
			</Button>
		</div>
	);
}
