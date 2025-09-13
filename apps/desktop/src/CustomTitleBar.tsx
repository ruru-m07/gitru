import { Button } from "@noutify/ui/components/button";
import { CaretRightIcon } from "@radix-ui/react-icons";
import { useCanGoBack, useRouterState } from "@tanstack/react-router";
import { ArrowLeft, ArrowRight, GitPullRequestArrow, Plus } from "lucide-react";

type CustomTitleBarProps = {
	restrictedPaths: string[];
};

const CustomTitleBar = ({ restrictedPaths = [] }: CustomTitleBarProps) => {
	const routerState = useRouterState();
	const pathname = routerState.location.pathname;

	const canGoBack = useCanGoBack();

	if (restrictedPaths.includes(pathname)) {
		return null;
	}

	return (
		<div
			className="h-[var(--main-custom-header-height)] bg-background flex items-center justify-between relative px-4 select-none _border-b"
			data-tauri-drag-region
			style={{
				// @ts-expect-error - ¯\_(ツ)_/¯
				WebkitAppRegion: "drag",
			}}
		>
			{restrictedPaths.includes(pathname) ? null : (
				<>
					<div
						className="flex items-center absolute"
						style={{
							// @ts-expect-error - ¯\_(ツ)_/¯
							WebkitAppRegion: "no-drag",
							paddingLeft: "66px",
						}}
					>
						<Button
							onClick={() => window.history.back()}
							disabled={!canGoBack}
							size={"icon"}
							className="size-7"
							variant="ghost"
						>
							<ArrowLeft size={16} aria-hidden="true" />
						</Button>
						<Button
							onClick={() => window.history.forward()}
							disabled={true} // TODO;
							size={"icon"}
							className="size-7"
							variant="ghost"
						>
							<ArrowRight size={16} aria-hidden="true" />
						</Button>
					</div>

					<div />

					<div
						// className="flex-1 max-w-[16rem] mx-4"
						className="flex items-center max-w-[40rem] mx-4 gap-2"
						style={{
							// @ts-expect-error - ¯\_(ツ)_/¯
							WebkitAppRegion: "no-drag",
						}}
					>
						<div className="flex items-center gap-1.5 text-sm">
							<GitPullRequestArrow
								className="text-green-600 mr-1"
								size={18}
								strokeWidth={2}
								aria-hidden="true"
							/>{" "}
							<div className="flex items-center gap-1 text-muted-foreground font-semibold">
								ruru-m07
								<CaretRightIcon />
								noutify
								<CaretRightIcon />
								pulls
								<CaretRightIcon />
								#69
							</div>
						</div>
					</div>

					<div
						className="flex items-center space-x-2"
						style={{
							// @ts-expect-error - ¯\_(ツ)_/¯
							WebkitAppRegion: "no-drag",
						}}
					>
						<Button size={"icon"} className="size-7" variant="ghost">
							<Plus size={16} aria-hidden="true" />
						</Button>
					</div>
				</>
			)}
		</div>
	);
};

export default CustomTitleBar;
