import { buttonVariants } from "@noutify/ui/components/button";
import { Label } from "@noutify/ui/components/label";
import { RadioGroup, RadioGroupItem } from "@noutify/ui/components/radio-group";
import { cn } from "@noutify/ui/lib/utils";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronRightIcon } from "lucide-react";
import { useTheme } from "next-themes";
import { useEffect, useState } from "react";
import * as T from "@/components/typography";
import { Colors, type ColorsType } from "../../../lib/colors";

export const Route = createFileRoute("/auth/onboarding/")({
	component: RouteComponent,
});

function RouteComponent() {
	return <OnboardingPage />;
}

const OnboardingPage = () => {
	const { session } = {
		session: {
			user: { name: "ruru", image: "https://github.com/ruru-m07.png" },
		},
	};

	// const isOnBoardingDone = localStorage.getItem("isOnBoardingDone");
	// const navigate = useNavigate();

	// useLayoutEffect(() => {
	// 	if (!session) {
	// 		navigate("/auth/token");
	// 	}

	// 	if (isOnBoardingDone) {
	// 		navigate("/inbox");
	// 	}
	// }, [isOnBoardingDone, navigate, session]);

	return (
		<div className="flex flex-col items-center justify-center  h-screen">
			<T.H3 className="text-3xl">
				Hey{" "}
				<span className="capitalize relative">
					{" "}
					<img
						src={session?.user?.image || "/default-avatar.png"}
						alt="User Avatar"
						width={40}
						height={40}
						className="inline-block rounded-md mr-2 ml-1 border border-black/10 dark:border-white/10 -rotate-6"
					/>
					{session?.user?.name}
				</span>
				, Welcome to Onboarding!
			</T.H3>
			<T.Muted className="mt-2">
				Select your preferred theme to get started.
			</T.Muted>
			<div className="mt-7 max-w-4xl">
				<SchemaSelector />
			</div>
		</div>
	);
};

const SchemaSelector = () => {
	const { theme, setTheme } = useTheme();
	const [mounted, setMounted] = useState(false);

	// The active theme is not available on the server.
	// If you have styling that is conditionally applied based on the active-theme,
	// you have to await the mounted state before rendering the active theme.
	useEffect(() => setMounted(true), []);

	const id = "this-is-nice-UUID";

	return (
		<>
			<RadioGroup
				onValueChange={(v) => {
					setTheme(v as ColorsType);
				}}
				className="gap-10 grid grid-cols-3"
			>
				{Colors.map(({ key, name }) => (
					// <div
					// 	key={key}
					// 	className={`border-input ${key} has-data-[state=checked]:border-primary/50 relative flex w-full items-start gap-2 rounded-md border p-4 shadow-xs outline-none`}
					// 	style={{
					// 		backgroundColor: "var(--background)",
					// 		color: "var(--foreground)",
					// 		border:
					// 			mounted && theme === key
					// 				? `1px solid var(--primary)`
					// 				: `1px solid var(--border)`,
					// 	}}
					// >
					<div
						key={key}
						className={`relative flex w-full items-start ${key} gap-2 rounded-md border p-4 shadow-xs outline-none ${mounted && theme === key ? "ring-2 ring-primary" : "ring-1 ring-border"}`}
						style={{
							backgroundColor: "var(--background)",
							color: "var(--foreground)",
						}}
					>
						<div className="flex flex-col">
							<div className="flex items-center justify-between mb-4">
								<Label
									htmlFor={`${id}-${key}`}
									className="text-base"
									style={{
										color: "var(--foreground)",
									}}
								>
									{name.toUpperCase()}
								</Label>
								<RadioGroupItem
									value={key}
									id={`${id}-${key}`}
									aria-describedby={`${id}-${key}-description`}
									className="order-1 after:absolute after:inset-0"
									style={{
										backgroundColor:
											mounted && theme === key
												? "var(--primary)"
												: "transparent",
										border: `1px solid var(--border)`,
									}}
								/>
							</div>
							<div className="flex grow items-start gap-3">
								{["primary", "accent", "secondary", "border"].map(
									(colorName) => (
										<div key={colorName} className="flex items-center gap-1">
											<div
												className={`w-10 h-10 rounded-sm`}
												style={{
													backgroundColor: `var(--${colorName})`,
													border: `1px solid var(--border)`,
												}}
											></div>
										</div>
									),
								)}
							</div>
						</div>
					</div>
				))}
			</RadioGroup>
			<div className="mt-10 flex items-center justify-center">
				<Link
					className={cn(
						buttonVariants({
							variant: "default",
							className: "group",
						}),
					)}
					to="/app"
				>
					Get Started
					<ChevronRightIcon
						className="-me-1 opacity-60 transition-transform group-hover:translate-x-0.5"
						size={16}
						aria-hidden="true"
					/>
				</Link>
			</div>
		</>
	);
};
