import { buttonVariants } from "@noutify/ui/components/button";
import { cn } from "@noutify/ui/lib/utils";
import { createFileRoute, Link } from "@tanstack/react-router";

export const Route = createFileRoute("/app/git/")({
	component: App,
});

function App() {
	return (
		<div
			className={cn(
				"ml-[var(--main-actual-content-padding)] h-full w-full px-2 rounded-md",
				"flex items-center justify-center flex-col",
			)}
		>
			<span className="text-4xl font-semibold">Hello ;D</span>
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
	);
}
