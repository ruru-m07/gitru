import { Button } from "@noutify/ui/components/button";
import { ScrollArea } from "@noutify/ui/components/scroll-area";
import { cn } from "@noutify/ui/lib/utils";
import { createFileRoute } from "@tanstack/react-router";
import { Settings } from "lucide-react";
import { useEffect, useState } from "react";
import { DiffViewer } from "@/components/diff/diff-viewer";
import { useDiffViewStore } from "@/components/diff/useDiffViewStore";
import { useAppStore } from "@/store/useAppStore";
import { getDiff } from "@/tauri";
import { getStatusIcon } from "./route";

export const Route = createFileRoute("/app/git/")({
	component: App,
});

function App() {
	const [diffText, setDiffText] = useState<string | null>(null);
	const { setViewMode, viewMode, selectedFilePath, selectedFileStatus } =
		useDiffViewStore();
	const { selectedRepository } = useAppStore();

	useEffect(() => {
		(async () => {
			if (!selectedFilePath) return;
			if (!selectedRepository) return;

			const data = await getDiff({
				file_path: selectedFilePath,
				repo_path: selectedRepository?.path,
			});
			console.log(data);
			setDiffText(data);
		})();
	}, [selectedFilePath, selectedRepository]);

	return (
		<>
			<div className="w-full min-h-14 max-h-14 border-b"></div>
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
					<Settings className="mx-2" size={18} />
				</div>
			</div>
			<ScrollArea
				className={cn(
					"h-full w-full",
					// "flex items-center justify-center flex-col",
				)}
			>
				{selectedFilePath && (
					<DiffViewer diff={diffText || ""} filePath={selectedFilePath} />
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
