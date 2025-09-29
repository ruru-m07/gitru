import {
	Accordion,
	AccordionContent,
	AccordionItem,
	AccordionTrigger,
} from "@noutify/ui/components/accordion";
import { Badge } from "@noutify/ui/components/badge";
import { Button, buttonVariants } from "@noutify/ui/components/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "@noutify/ui/components/dropdown-menu";
import { Input } from "@noutify/ui/components/input";
import { Label } from "@noutify/ui/components/label";
import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "@noutify/ui/components/resizable";
import { ScrollArea } from "@noutify/ui/components/scroll-area";
import { Textarea } from "@noutify/ui/components/textarea";
import { cn } from "@noutify/ui/lib/utils";
import { createFileRoute, Outlet } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import {
	AlertTriangle,
	ChevronDown,
	ChevronDownIcon,
	ChevronUp,
	CornerUpRight,
	EyeOff,
	Minus,
	Plus,
	SearchIcon,
	SquareDot,
	SquarePlus,
	SquareX,
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { useDiffViewStore } from "@/components/diff/useDiffViewStore";
import { useAppStore } from "@/store/useAppStore";
import {
	addLocalGitRepo,
	type FileStatus,
	type FileStatusKind,
	getStatus,
} from "@/tauri";

export const Route = createFileRoute("/app/git")({
	component: GitPageLayout,
});

function GitPageLayout() {
	// const items = [
	// 	{ label: "Soft", desc: "Undo commit, keep changes staged", disable: false },
	// 	{
	// 		label: "Mixed",
	// 		desc: "Undo commit, keep changes in working directory",
	// 		disable: false,
	// 	},
	// 	{
	// 		label: "Hard",
	// 		desc: "Undo commit and delete all changes",
	// 		disable: false,
	// 	},
	// 	{
	// 		label: "Revert",
	// 		desc: "Add a new commit that reverts the last one",
	// 		disable: false,
	// 	},
	// 	{
	// 		label: "Amend",
	// 		desc: "Modify the previous commit",
	// 		disable: false,
	// 	},
	// 	{
	// 		label: "Rebase",
	// 		desc: "Drop or edit any previous commit",
	// 		disable: true,
	// 	},
	// ] as const;

	const [status, setStatus] = useState<FileStatus[]>([]);

	const {
		repoSelectIsOpen,
		setRepoSelectIsOpen,
		setRepositories,
		repositories,
		setSelectedRepository,
		selectedRepository,
	} = useAppStore();

	useEffect(() => {
		(async () => {
			const data = await getStatus({
				repo_path: selectedRepository?.path || "",
			});
			console.log(data.files);
			setStatus(data.files);
		})();
	}, [selectedRepository]);

	return (
		<ResizablePanelGroup
			className={cn(
				"ml-[var(--main-actual-content-padding)] bg-accent/35 ring-1 ring-inset ring-border h-full w-full rounded-md flex overflow-hidden",
			)}
			direction="horizontal"
			autoSaveId="git-page-layout"
		>
			<ResizablePanel
				defaultSize={18}
				minSize={18}
				maxSize={44}
				className="flex flex-col h-full justify-between"
			>
				<button
					onClick={() => {
						setRepoSelectIsOpen(!repoSelectIsOpen);
					}}
					className="flex justify-between items-center border-b px-2 pt-2 pb-1 hover:bg-accent/40 cursor-pointer min-h-14 max-h-14"
					type="button"
				>
					<div className="flex-col flex items-start">
						<span className="text-xs text-muted-foreground">
							Current Repository
						</span>
						<span>{selectedRepository?.name || "No repository selected"}</span>
					</div>
					{repoSelectIsOpen ? (
						<ChevronUp size={18} />
					) : (
						<ChevronDown size={18} />
					)}
				</button>
				<div className="grid grid-cols-2 border-b w-full min-h-9 max-h-9">
					<button
						type="button"
						className={cn(
							buttonVariants({ variant: "ghost" }),
							"h-full rounded-none hover:border-border border-l border-transparent",
						)}
					>
						Changes
					</button>
					<button
						type="button"
						className={cn(
							buttonVariants({ variant: "ghost" }),
							"h-full rounded-none border-l",
						)}
					>
						History
					</button>
				</div>
				{repoSelectIsOpen ? (
					<ScrollArea type="scroll" className="max-h-full flex-1">
						<div className="w-full p-2 border-b flex justify-between items-center gap-2">
							<div className="relative">
								<Input
									className="peer ps-9 pe-9 h-8 border-border"
									placeholder="Filter..."
								/>
								<div className="text-muted-foreground/80 pointer-events-none absolute inset-y-0 start-0 flex items-center justify-center ps-3 peer-disabled:opacity-50">
									<SearchIcon size={16} />
								</div>
							</div>
							<DropdownMenu>
								<DropdownMenuTrigger asChild>
									<Button size={"sm"}>
										Add
										<ChevronDownIcon
											className="-me-1 opacity-60"
											size={16}
											aria-hidden="true"
										/>
									</Button>
								</DropdownMenuTrigger>
								<DropdownMenuContent className="min-w-(--radix-dropdown-menu-trigger-width)">
									<DropdownMenuItem
										onClick={async () => {
											console.log(repositories);
										}}
									>
										Clone repository...
									</DropdownMenuItem>
									<DropdownMenuItem
										onClick={async () => {
											const folder = await open({
												directory: true,
												multiple: false,
											});

											if (folder) {
												if (repositories.find((r) => r.path === folder)) {
													toast.error("Repository already added");
													return;
												}

												const data = await addLocalGitRepo({
													repo_path: folder,
												});

												if (data.error) {
													toast.error(data.error);
													return;
												}
												if (data.success) {
													setRepositories([...repositories, data.success]);
													setSelectedRepository(data.success);
													toast.success("Repository added successfully!");
												}
											}
										}}
									>
										Add local repository
									</DropdownMenuItem>
								</DropdownMenuContent>
							</DropdownMenu>
						</div>
						<div className="">
							{repositories.map((repo) => (
								<button
									className="py-2 px-2 flex w-full justify-between items-center hover:bg-accent/55 cursor-pointer"
									key={repo.id}
									type="button"
									onClick={() => {
										setSelectedRepository(repo);
										setRepoSelectIsOpen(false);
									}}
								>
									<span>{repo.name}</span>
									<Button
										variant={"ghost"}
										size={"icon"}
										onClick={(e) => {
											e.stopPropagation();
											setRepositories(
												repositories.filter((r) => r.id !== repo.id),
											);
											toast.success("Repository removed");

											if (selectedRepository?.id === repo.id) {
												setSelectedRepository(null);
											}
										}}
									>
										<Minus size={16} />
									</Button>
								</button>
							))}
						</div>
					</ScrollArea>
				) : (
					<>
						<div className="max-h-full overflow-auto flex-1 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
							{/* <div className="w-full p-2 border-b flex justify-between items-center">
								<Checkbox />
								<span className="text-sm">{status.length} Changed Files</span>
								<div />
							</div> */}
							{/* <div className="max-w-full">
								{status.map((file) => (
									<EachStatus key={file.path} file={file} type="Changes" />
								))}
							</div> */}
							<Accordion
								type="multiple"
								defaultValue={["Staged Changes", "Changes"]}
								className="w-full"
							>
								{(
									[
										{ name: "Staged Changes", data: [] },
										{ name: "Changes", data: status },
									] as const
								).map((cell) => {
									if (!cell.data || cell.data.length === 0) {
										return null; // Skip rendering if no data
									}
									return (
										<AccordionItem
											defaultChecked
											value={cell.name}
											key={cell.name}
											className="pt-2 pb-0"
										>
											<AccordionTrigger
												className={cn(
													"justify-start sticky top-0 rounded-none px-3 gap-2 py-0 hover:no-underline [&>svg]:-order-1",
												)}
											>
												<div className="flex items-center justify-between w-full">
													<span className="text-sm font-medium">
														{cell.name}
													</span>
													<div className="flex items-center gap-1 pointer-events-auto">
														{cell.name === "Changes" && (
															<>
																{/* <DiscardChangesDialog variant="all" /> */}

																<div
																	// onClick={(event) => {
																	// 	event.stopPropagation();
																	// 	if (selectedRepo?.path) {
																	// 		handleAddStaged(".");
																	// 	}
																	// }}
																	className={cn(
																		buttonVariants({
																			variant: "ghost",
																			className: "h-8 w-8",
																		}),
																	)}
																>
																	<Plus size={20} strokeWidth={1.25} />
																</div>
															</>
														)}
														{cell.name === "Staged Changes" && (
															<div
																// onClick={(event) => {
																// 	event.stopPropagation();
																// 	if (selectedRepo?.path) {
																// 		handleRemoveStaged(".");
																// 	}
																// }}
																className={cn(
																	buttonVariants({
																		variant: "ghost",
																		className: "h-8 w-8",
																	}),
																)}
															>
																<Minus size={20} strokeWidth={1.25} />
															</div>
														)}
														<Badge variant={"secondary"}>
															{cell.data?.length}
														</Badge>
													</div>
												</div>
											</AccordionTrigger>
											<AccordionContent className="text-muted-foreground  pb-1">
												{cell.data?.map((v) => (
													<EachStatus key={v.path} file={v} type="Changes" />
												))}
											</AccordionContent>
										</AccordionItem>
									);
								})}
							</Accordion>
						</div>
						<div className="flex flex-col gap-2 justify-between items-center border-t px-2 py-2 bg-accent/40">
							<Input
								placeholder="Summary (required)"
								className="h-8 border-border"
							/>
							<Textarea placeholder="Description" className="border-border" />
							<Button className="w-full" size={"sm"}>
								Commit to main
							</Button>
						</div>
					</>
				)}
			</ResizablePanel>
			<ResizableHandle withHandle />
			<ResizablePanel
				className={cn(
					// 'w-[calc(100vw-(var(--sidebar-width)+var(--inbox-width)+(var(--margin)))+3.5rem)]',
					"w-full relative",
					repoSelectIsOpen && "_blur-sm cursor-pointer",
				)}
				onClick={() => {
					if (repoSelectIsOpen) {
						setRepoSelectIsOpen(false);
					}
				}}
			>
				{repoSelectIsOpen && (
					<div className="absolute inset-0 bg-black/40 z-10 w-full h-full backdrop-blur-sm border border-l-0"></div>
				)}
				<Outlet />
			</ResizablePanel>
		</ResizablePanelGroup>
	);
}

interface EachStatusProps {
	file: FileStatus;
	type: "Staged Changes" | "Changes";
}

export default function EachStatus({ file, type }: EachStatusProps) {
	const { setSelectedFilePath, setSelectedFileStatus } = useDiffViewStore();

	return (
		// biome-ignore lint/a11y/noStaticElementInteractions: who cares
		// biome-ignore lint/a11y/useKeyWithClickEvents: who cares
		<div
			className={cn(
				"flex relative cursor-pointer hover:bg-muted items-center px-2 py-1",
			)}
			onClick={() => {
				setSelectedFilePath(file.path);
				setSelectedFileStatus(file.status);
			}}
		>
			<div className="flex items-center w-full min-w-0">
				<div className="flex-shrink-0">{getStatusIcon(file.status)}</div>
				<div className="flex items-center ml-2 min-w-0 flex-1">
					<Label className="flex cursor-pointer items-center min-w-0 w-full">
						{file?.path.split("/").slice(0, -1).join("/") && (
							<>
								<span className="text-muted-foreground truncate">
									{file.path.split("/").slice(0, -1).join("/")}
								</span>
								<span className="text-muted-foreground flex-shrink-0">/</span>
							</>
						)}
						<span className="flex-shrink-0 font-medium">
							{file?.path.split("/").slice(-1)[0]}
						</span>
					</Label>
				</div>
				{type === "Changes" && (
					<div className="flex ml-2 flex-shrink-0">
						<Button className="h-8 w-8" variant={"ghost"}>
							<Plus size={20} strokeWidth={1.25} />
						</Button>
					</div>
				)}
			</div>
		</div>
	);
}

export function getStatusIcon(type: FileStatusKind[]) {
	// Normalize into a Set for fast lookup
	const kinds = new Set(type);

	// PRIORITY: unreadable > deleted > renamed/typechange > modified > new
	if (kinds.has("WorktreeUnreadable")) {
		return (
			// Unreadable
			<AlertTriangle className="text-orange-500" size={20} />
		);
	}

	if (kinds.has("IndexDeleted") || kinds.has("WorktreeDeleted")) {
		// Deleted
		return <SquareX className="text-red-500" size={20} />;
	}

	if (
		kinds.has("IndexRenamed") ||
		kinds.has("WorktreeRenamed") ||
		kinds.has("IndexTypechange") ||
		kinds.has("WorktreeTypechange")
	) {
		return (
			// Renamed / Type Changed
			<CornerUpRight className="text-purple-500" size={20} />
		);
	}

	if (kinds.has("IndexModified") || kinds.has("WorktreeModified")) {
		// Modified
		return <SquareDot className="text-yellow-500" size={20} />;
	}

	if (kinds.has("IndexNew") || kinds.has("WorktreeNew")) {
		return (
			// New / Added
			<SquarePlus className="text-green-500" size={20} />
		);
	}

	// If nothing matched, show a neutral icon or nothing
	// Unchanged
	return <EyeOff className="text-gray-400" size={20} />;
}
