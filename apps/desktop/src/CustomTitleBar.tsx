import { Button } from "@noutify/ui/components/button";
import { CaretRightIcon } from "@radix-ui/react-icons";
import { ArrowLeft, ArrowRight, GitPullRequestArrow, Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { useHistoryTracker } from "./hooks/useHistoryTracker";

type CustomTitleBarProps = {
	restrictedPaths: string[];
};

const CustomTitleBar = ({ restrictedPaths = [] }: CustomTitleBarProps) => {
	const [isMac, setIsMac] = useState(false);

	const { canGoBack, canGoForward } = useHistoryTracker();
	const navigate = useNavigate();
	const { pathname } = useLocation();

	useEffect(() => {
		setIsMac(navigator.platform.toUpperCase().indexOf("MAC") >= 0);
	}, []);

	if (restrictedPaths.includes(pathname)) {
		return null;
	}

	return (
		<div
			className="h-10 bg-background flex items-center justify-between px-4 select-none border-b"
			data-tauri-drag-region
			style={{
				// @ts-expect-error - ¯\_(ツ)_/¯
				WebkitAppRegion: "drag",
				paddingLeft: isMac ? "80px" : "16px",
			}}
		>
			{restrictedPaths.includes(pathname) ? null : (
				<>
					<div
						className="flex items-center"
						style={{
							// @ts-expect-error - ¯\_(ツ)_/¯
							WebkitAppRegion: "no-drag",
						}}
					>
						<Button
							onClick={() => navigate(-1)}
							disabled={!canGoBack}
							size={"icon"}
							className="size-7"
							variant="ghost"
						>
							<ArrowLeft size={16} aria-hidden="true" />
						</Button>
						<Button
							onClick={() => navigate(1)}
							disabled={!canGoForward}
							size={"icon"}
							className="size-7"
							variant="ghost"
						>
							<ArrowRight size={16} aria-hidden="true" />
						</Button>
					</div>
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
