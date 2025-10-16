import { Button } from "@gitru/ui/components/button";
import {
	Popover,
	PopoverContent,
	PopoverTrigger,
} from "@gitru/ui/components/popover";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import { cn } from "@gitru/ui/lib/utils";
import { createFileRoute } from "@tanstack/react-router";
import { ChevronDown, ChevronsUp, GitBranch, Settings } from "lucide-react";
import { useEffect, useState } from "react";
import { DiffViewer } from "@/components/diff/diff-viewer";
import { useDiffViewStore } from "@/components/diff/useDiffViewStore";
import { useAppStore } from "@/store/useAppStore";
import { type GetDiffResponse, getDiff } from "@/tauri";
import { EmptyGitDiffSVG } from "./EmptyGitDiffSVG";
import { getStatusIcon } from "./route";
import { SplitSVG } from "./splitSVG";
import { UnifiedSVG } from "./unifiedSVG";
import { useGitRepository, useFileDiff } from "@/lib/git";

export const Route = createFileRoute("/app/git/")({
	component: App,
});

function App() {
	const { selectedFilePath, selectedFileStatus, setViewMode } =
		useDiffViewStore();
	const { selectedRepository } = useAppStore();
	
	// Use the OOP Git architecture to get repository instance
	const repo = useGitRepository(
		selectedRepository?.path || null,
		selectedRepository?.name
	);
	
	// Use the useFileDiff hook which automatically listens to invalidation events
	const { diff } = useFileDiff(repo, selectedFilePath || null);
	
	// Convert diff string to GetDiffResponse format for DiffViewer
	const [diffData, setDiffData] = useState<GetDiffResponse | null>(null);
	
	useEffect(() => {
		if (!selectedFilePath || !selectedRepository || !diff) {
			setDiffData(null);
			return;
		}
		
		// Fetch the full diff data structure from backend
		// (The useFileDiff gives us the content, but DiffViewer needs the full structure)
		let isCancelled = false;
		
		(async () => {
			try {
				const data = await getDiff({
					file_path: selectedFilePath,
					repo_path: selectedRepository.path,
				});
				
				if (!isCancelled) {
					setDiffData(data);
				}
			} catch (error) {
				console.error("Failed to fetch diff structure", error);
				if (!isCancelled) {
					setDiffData(null);
				}
			}
		})();
		
		return () => {
			isCancelled = true;
		};
	}, [selectedFilePath, selectedRepository, diff]); // Re-fetch when diff changes!

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
					<Popover>
						<PopoverTrigger asChild>
							<Button
								size="icon"
								variant="ghost"
								className="relative"
								aria-label="Open notifications"
							>
								<Settings size={16} aria-hidden="true" />
							</Button>
						</PopoverTrigger>
						<PopoverContent className="w-80 p-4 mr-4 mt-0.5">
							<div className="flex items-center justify-center">
								<div className="flex flex-col items-center gap-2 w-full">
									<Button
										className="rounded-none size-full h-32 shadow-none first:rounded-s-md last:rounded-e-md focus-visible:z-10"
										variant="outline"
										size="icon"
										aria-label="Flip Horizontal"
										onClick={() => {
											setViewMode("unified");
										}}
									>
										<UnifiedSVG />
									</Button>
									<span className="text-sm text-muted-foreground">Unified</span>
								</div>
								<div className="flex flex-col items-center gap-2 w-full">
									<Button
										className="rounded-none size-full h-32 shadow-none rounded-r-md border-l-0 focus-visible:z-10"
										variant="outline"
										size="icon"
										aria-label="Flip Vertical"
										onClick={() => {
											setViewMode("split");
										}}
									>
										<SplitSVG />
									</Button>
									<span className="text-sm text-muted-foreground">Split</span>
								</div>
							</div>
						</PopoverContent>
					</Popover>
				</div>
			</div>
			{selectedFilePath ? (
				<ScrollArea className={cn("h-full w-full")}>
					<DiffViewer
						diff={diffData}
						filePath={selectedFilePath}
						status={selectedFileStatus}
					/>
					<div className="h-40" />
				</ScrollArea>
			) : (
				<div className="w-full flex items-center justify-center h-full">
					<div className="w-full h-[85%] flex flex-col items-center justify-center">
						<EmptyGitDiffSVG />
						<span className="text-muted-foreground text-base">
							No changes to show
						</span>
					</div>
				</div>
			)}
		</>
	);
}
