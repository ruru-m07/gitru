import { Button, buttonVariants } from "@noutify/ui/components/button";
import { cn } from "@noutify/ui/lib/utils";
import { createFileRoute, Link } from "@tanstack/react-router";
import { getStatus } from "@/tauri";

export const Route = createFileRoute("/app/git/")({
	component: App,
});

function App() {
	return (
		<div
			className={cn(
				"ml-[var(--main-actual-content-padding)] bg-accent h-full w-full px-2 rounded-md",
				"flex items-center justify-center flex-col",
			)}
		>
			<span className="text-4xl font-semibold">Hello ;D</span>
			<Button
				onClick={async () => {
					const data = await getStatus({
						repo_path: "/Users/ruru/sandbox/noutify",
					})
					data.files.forEach((file) => {
						file.status
							.filter((s) => s !== "Clean")
							.forEach((status) => {
								console.log(file.path, status);
							})
					})
				}}
			>
				Click me!
			</Button>
			<div className={cn("flex items-center justify-center flex-col")}>
				<Link
					className={cn(
						buttonVariants({
							variant: "default",
						}),
					)}
					to="/app"
				>
					go to /app
				</Link>
				<Link
					className={cn(
						buttonVariants({
							variant: "default",
						}),
					)}
					to="/auth/onboarding"
				>
					go to /auth/onboarding
				</Link>
			</div>
		</div>
	)
}
