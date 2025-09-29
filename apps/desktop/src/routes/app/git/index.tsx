import { Button } from "@noutify/ui/components/button";
import { ScrollArea } from "@noutify/ui/components/scroll-area";
import { cn } from "@noutify/ui/lib/utils";
import { createFileRoute } from "@tanstack/react-router";
import {
	ChevronDown,
	ChevronsUp,
	ChevronUp,
	GitBranch,
	Settings,
} from "lucide-react";
import { useEffect, useState } from "react";
import { DiffViewer } from "@/components/diff/diff-viewer";
import { useDiffViewStore } from "@/components/diff/useDiffViewStore";
import { useAppStore } from "@/store/useAppStore";
import { type GetDiffResponse, getDiff } from "@/tauri";
import { getStatusIcon } from "./route";

export const Route = createFileRoute("/app/git/")({
	component: App,
});

function App() {
	const [diffData, setDiffData] = useState<GetDiffResponse | null>(null);
	const { setViewMode, viewMode, selectedFilePath, selectedFileStatus } =
		useDiffViewStore();
	const { selectedRepository } = useAppStore();

	useEffect(() => {
		let isCancelled = false;

		(async () => {
			if (!selectedFilePath || !selectedRepository) {
				setDiffData(null);
				return;
			}

			try {
				setDiffData(null);
				const data = await getDiff({
					file_path: selectedFilePath,
					repo_path: selectedRepository.path,
				});

				if (!isCancelled) {
					setDiffData(data);
				}
			} catch (error) {
				console.error("Failed to fetch diff", error);
				if (!isCancelled) {
					setDiffData(null);
				}
			}
		})();

		return () => {
			isCancelled = true;
		};
	}, [selectedFilePath, selectedRepository]);

	return (
		<>
			<div className="w-full min-h-14 max-h-14 h-14 border-b flex">
				<Button
					className="flex border-r justify-between items-center h-full rounded-none hover:border-t w-72"
					variant={"ghost"}
				>
					<div className="flex items-center justify-center gap-4">
						<GitBranch className="size-6" />
						<div className="flex-col flex items-start">
							<span className="text-xs text-muted-foreground font-[400]">
								Current Branch
							</span>
							<span>{"ruru/fix/whatever/sucks"}</span>
						</div>
					</div>
					<ChevronDown size={18} />
				</Button>
				<Button
					className="flex border-r justify-between items-center h-full rounded-none hover:border-t w-72"
					variant={"ghost"}
				>
					<div className="flex items-center justify-center gap-4">
						<ChevronsUp className="size-8" />
						<div className="flex-col flex items-start">
							<span className="text-xs text-muted-foreground font-[400]">
								ruru/fix/whatever/sucks
							</span>
							<span>Push 3 Commits</span>
						</div>
					</div>
					<ChevronDown size={18} />
				</Button>
			</div>
			<div className="w-full h-9 border-b flex justify-between items-center">
				<div className="items-center h-full px-2 flex gap-2">
					{selectedFileStatus && getStatusIcon(selectedFileStatus)}
					<span className="flex items-center">
						<span className="text-muted-foreground/75">
							{selectedFilePath?.slice(0, selectedFilePath.lastIndexOf("/"))}/
						</span>
						<span>{selectedFilePath?.split("/").pop()}</span>
					</span>
				</div>
				<div>
					<Button size={"icon"} variant="ghost" className="mr-1">
						<Settings />
					</Button>
				</div>
			</div>
			<ScrollArea
				className={cn(
					"h-full w-full",
					// "flex items-center justify-center flex-col",
				)}
			>
				{selectedFilePath && (
					<DiffViewer
						diff={diffData}
						filePath={selectedFilePath}
						status={selectedFileStatus}
					/>
				)}
				<Button
					onClick={() => {
						setViewMode(viewMode === "split" ? "unified" : "split");
					}}
				>
					Change View Mode
				</Button>
				<div className="h-40" />
				{/* <div className={cn("flex items-center justify-center flex-col")}>
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
			</div> */}
			</ScrollArea>
		</>
	);
}
