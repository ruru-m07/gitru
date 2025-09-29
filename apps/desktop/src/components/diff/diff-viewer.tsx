import { Card, CardContent } from "@noutify/ui/components/card";
import { Loader2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
	type BundledLanguage,
	createHighlighter,
	type Highlighter,
} from "shiki";
import { vesperLight } from "./custome-themes";
import { useDiffViewStore } from "./useDiffViewStore";

interface DiffLine {
	type: "added" | "removed" | "context" | "info" | "header";
	content: string;
	lineNumberOld?: number;
	lineNumberNew?: number;
}

interface DiffFile {
	oldFile: string;
	newFile: string;
	lines: DiffLine[];
	metaData: string[];
}

export function DiffViewer({
	diff,
	filePath,
}: {
	diff: string;
	filePath: string;
}) {
	const [diffText, setDiffText] = useState<string>("");
	const [parsedDiff, setParsedDiff] = useState<DiffFile[]>([]);
	const [highlighter, setHighlighter] = useState<Highlighter | null>(null);
	const [isLoading, setIsLoading] = useState(false);
	const [fileLanguage, setFileLanguage] = useState<BundledLanguage>("ts");

	const { viewMode } = useDiffViewStore();

	useEffect(() => {
		const initHighlighter = async () => {
			try {
				const shikiHighlighter = await createHighlighter({
					themes: ["vesper", vesperLight],
					langs: [
						"typescript",
						"javascript",
						"tsx",
						"jsx",
						"json",
						"css",
						"html",
						"bash",
						"python",
						"diff",
						"txt",
						"rs",
						"rust",
						"makefile",
						"make",
						"docker",
						"dockerfile",
						"java",
						"csv",
						"yaml",
						"markdown",
						"md",
						"mdx",
					],
				});
				setHighlighter(shikiHighlighter);
			} catch (error) {
				console.error("Failed to initialize Shiki highlighter:", error);
			}
		};

		initHighlighter();
	}, []);

	useEffect(() => {
		if (diff) {
			setDiffText(diff);

			const listOfSpecificFilesThatDontHaveDotExtensionLikeMakefile = [
				"makefile",
				"dockerfile",
				"gitignore",
				"gitattributes",
				"jenkinsfile",
				"procfile",
				"vagrantfile",
				"brewfile",
			];

			const fileName = filePath.split("/").pop() || "";
			// @ts-expect-error - .
			let language: BundledLanguage = "txt";

			if (fileName.includes(".")) {
				const extension = fileName.split(".").pop()?.toLowerCase() || "";
				const langMap: Record<string, BundledLanguage> = {
					ts: "typescript",
					tsx: "tsx",
					js: "javascript",
					jsx: "jsx",
					py: "python",
					css: "css",
					html: "html",
					json: "json",
					sh: "bash",
					bash: "bash",
					rs: "rust",
					java: "java",
					make: "makefile",
					docker: "dockerfile",
					dockerfile: "dockerfile",
					yaml: "yaml",
					yml: "yaml",
					csv: "csv",
					markdown: "markdown",
					md: "markdown",
					mdx: "mdx",
					lock: "json",
				};

				language = langMap[extension] || "txt";
			} else {
				const lowerFileName = fileName.toLowerCase();

				if (
					listOfSpecificFilesThatDontHaveDotExtensionLikeMakefile.includes(
						lowerFileName,
					)
				) {
					const specialFileMappings: Record<string, BundledLanguage> = {
						makefile: "makefile",
						dockerfile: "dockerfile",
						gitignore: "bash",
						gitattributes: "bash",
						jenkinsfile: "groovy",
						procfile: "bash",
						vagrantfile: "ruby",
						brewfile: "ruby",
					};

					language = specialFileMappings[lowerFileName] || "txt";
				}
			}

			setFileLanguage(language);
		}
	}, [diff, filePath]);

	useEffect(() => {
		if (diffText) {
			parseDiff(diffText);
		}
	}, [diffText]);

	const parseDiff = (text: string) => {
		setIsLoading(true);

		try {
			const lines = text.split("\n");
			const files: DiffFile[] = [];
			let currentFile: DiffFile | null = null;
			let oldLineNumber = 0;
			let newLineNumber = 0;
			let inMetaData = true;

			for (let i = 0; i < lines.length; i++) {
				const line = lines[i];

				if (line.startsWith("diff --git")) {
					if (currentFile) {
						files.push(currentFile);
					}

					const match = line.match(/diff --git a\/(.*) b\/(.*)/);
					const [, oldFile = "", newFile = ""] = match || [];

					currentFile = {
						oldFile,
						newFile,
						lines: [],
						metaData: [line],
					};

					oldLineNumber = 0;
					newLineNumber = 0;
					inMetaData = true;
				} else if (
					line.startsWith("index ") ||
					line.startsWith("---") ||
					line.startsWith("+++")
				) {
					if (currentFile) {
						currentFile.metaData.push(line);
					}
				} else if (line.startsWith("@@")) {
					if (currentFile) {
						inMetaData = false;
						const match = line.match(/@@ -(\d+),\d+ \+(\d+),\d+ @@/);
						if (match) {
							oldLineNumber = Number.parseInt(match[1], 10);
							newLineNumber = Number.parseInt(match[2], 10);

							currentFile.lines.push({
								type: "header",
								content: line,
							});
						}
					}
				} else if (line.startsWith("+")) {
					if (currentFile && !inMetaData) {
						currentFile.lines.push({
							type: "added",
							content: line.substring(1),
							lineNumberNew: newLineNumber++,
						});
					}
				} else if (line.startsWith("-")) {
					if (currentFile && !inMetaData) {
						currentFile.lines.push({
							type: "removed",
							content: line.substring(1),
							lineNumberOld: oldLineNumber++,
						});
					}
				} else {
					if (currentFile && !inMetaData) {
						currentFile.lines.push({
							type: "context",
							content: line.startsWith(" ") ? line.substring(1) : line,
							lineNumberOld: oldLineNumber++,
							lineNumberNew: newLineNumber++,
						});
					}
				}
			}

			if (currentFile) {
				files.push(currentFile);
			}

			setParsedDiff(files);
		} catch (error) {
			console.error("Error parsing diff:", error);
		} finally {
			setIsLoading(false);
		}
	};

	const highlightCode = useMemo(() => {
		return (
			code: string,
			language: BundledLanguage,
			lineType?: DiffLine["type"],
		) => {
			if (!highlighter || !code.trim()) {
				return code;
			}

			try {
				// For header lines, use diff language
				const lang = lineType === "header" ? "diff" : language;

				const highlighted = highlighter.codeToHtml(code, {
					lang,
					themes: {
						light: "vesper-light",
						"dark-classic": "vesper",
					},
					defaultColor: "light",
					cssVariablePrefix: "--shiki-",
				});

				// Extract only the inner HTML content, removing the outer pre/code wrapper
				const match = highlighted.match(/<code[^>]*>(.*?)<\/code>/s);
				return match ? match[1] : highlighted;
			} catch (error) {
				console.error("Error highlighting code:", error);
				return code;
			}
		};
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [highlighter, fileLanguage]);

	const renderUnifiedView = (file: DiffFile) => {
		if (!highlighter) return null;

		return (
			<div className="overflow-x-auto shiki bg-background">
				<table className="w-full border-collapse diff-content">
					<tbody>
						{file.lines.map((line, index) => {
							let bgClass = "";
							if (line.type === "added") bgClass = "diff-added";
							if (line.type === "removed") bgClass = "diff-removed";
							if (line.type === "header") bgClass = "diff-header";

							return (
								<tr key={index} className={`${bgClass} diff-hover`}>
									<td className="tabular-nums text-center align-middle py-1 px-2 diff-line-number-bg w-14 text-muted-foreground text-xs select-none">
										{line.lineNumberOld || ""}
									</td>
									<td className="tabular-nums text-center align-middle py-1 px-2 diff-line-number-bg w-14 text-muted-foreground text-xs select-none">
										{line.lineNumberNew || ""}
									</td>

									<td className="pl-3 pr-2 text-sm font-mono font-medium leading-6 whitespace-pre">
										<span
											dangerouslySetInnerHTML={{
												__html: highlightCode(
													line.content,
													fileLanguage,
													line.type,
												),
											}}
										/>
									</td>
								</tr>
							);
						})}
					</tbody>
				</table>
			</div>
		);
	};

	const renderSplitView = (file: DiffFile) => {
		if (!highlighter) return null;

		// Group lines for side-by-side view
		const groupedLines: Array<{
			left: DiffLine | null;
			right: DiffLine | null;
		}> = [];

		for (let i = 0; i < file.lines.length; i++) {
			const line = file.lines[i];

			if (line.type === "header") {
				groupedLines.push({
					left: line,
					right: null,
				});
				continue;
			}

			if (line.type === "context") {
				groupedLines.push({
					left: line,
					right: line,
				});
			} else if (line.type === "removed") {
				const nextLine = i + 1 < file.lines.length ? file.lines[i + 1] : null;

				if (nextLine && nextLine.type === "added") {
					groupedLines.push({
						left: line,
						right: nextLine,
					});
					i++;
				} else {
					groupedLines.push({
						left: line,
						right: null,
					});
				}
			} else if (line.type === "added") {
				groupedLines.push({
					left: null,
					right: line,
				});
			}
		}

		return (
			<div className="overflow-x-auto">
				<div className="flex">
					<table className="w-full border-collapse diff-content">
						<tbody>
							{groupedLines.map((group, index) => {
								const leftBgClass =
									group.left?.type === "removed"
										? "diff-removed"
										: group.left?.type === "header"
											? "diff-header"
											: "";

								const rightBgClass =
									group.right?.type === "added" ? "diff-added" : "";

								// For header lines, span both columns
								if (group.left?.type === "header" && !group.right) {
									return (
										<tr key={index} className={`${leftBgClass} diff-hover`}>
											<td
												colSpan={4}
												className="pl-5 pr-2 whitespace-pre text-sm font-mono font-medium leading-6"
											>
												<span
													dangerouslySetInnerHTML={{
														__html: highlightCode(
															group.left.content,
															"diff",
															"header",
														),
													}}
												/>
											</td>
										</tr>
									);
								}

								return (
									<tr key={index}>
										{/* Left side */}
										<td
											className={`tabular-nums text-center align-middle py-1 px-2 diff-line-number-bg w-14 text-muted-foreground text-xs select-none`}
										>
											{group.left?.lineNumberOld || ""}
										</td>
										<td
											className={`truncate text-sm font-mono font-medium leading-6 whitespace-pre border-r max-w-[10vw] ${leftBgClass}`}
										>
											{group.left ? (
												<span
													className="pl-3 pr-2 "
													dangerouslySetInnerHTML={{
														__html: highlightCode(
															group.left.content,
															fileLanguage,
															group.left.type,
														),
													}}
												/>
											) : (
												<span className="[--pattern-fg:var(--input)]/95 bg-gray-950 w-full h-full">
													<div className="_border-x h-6 w-full border-x-(--pattern-fg) bg-[image:repeating-linear-gradient(315deg,_var(--pattern-fg)_0,_var(--pattern-fg)_1px,_transparent_0,_transparent_50%)] bg-[size:10px_10px] bg-fixed"></div>
													<div className="_border-x border-x-(--pattern-fg) bg-[image:repeating-linear-gradient(315deg,_var(--pattern-fg)_0,_var(--pattern-fg)_1px,_transparent_0,_transparent_50%)] bg-[size:10px_10px] bg-fixed"></div>
												</span>
											)}
										</td>

										{/* Right side */}
										<td
											className={`tabular-nums text-center align-middle py-1 px-2 diff-line-number-bg w-14 text-muted-foreground text-xs select-none`}
										>
											{group.right?.lineNumberNew || ""}
										</td>
										<td
											className={`truncate text-sm font-mono font-medium leading-6 whitespace-pre max-w-[10vw] ${rightBgClass}`}
										>
											{group.right ? (
												<span
													className="pl-3  pr-2 "
													dangerouslySetInnerHTML={{
														__html: highlightCode(
															group.right.content,
															fileLanguage,
															group.right.type,
														),
													}}
												/>
											) : (
												<span className="[--pattern-fg:var(--input)]/95 bg-gray-950 w-full h-full">
													<div className="_border-x h-6 w-full border-x-(--pattern-fg) bg-[image:repeating-linear-gradient(315deg,_var(--pattern-fg)_0,_var(--pattern-fg)_1px,_transparent_0,_transparent_50%)] bg-[size:10px_10px] bg-fixed"></div>
													<div className="_border-x border-x-(--pattern-fg) bg-[image:repeating-linear-gradient(315deg,_var(--pattern-fg)_0,_var(--pattern-fg)_1px,_transparent_0,_transparent_50%)] bg-[size:10px_10px] bg-fixed"></div>
												</span>
											)}
										</td>
									</tr>
								);
							})}
						</tbody>
					</table>
				</div>
			</div>
		);
	};

	if (!highlighter) {
		return (
			<div className="flex justify-center items-center p-8">
				<Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
			</div>
		);
	}

	return (
		<div>
			{isLoading ? (
				<div className="flex justify-center items-center p-8">
					<Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
					<span className="ml-2 text-sm text-muted-foreground">
						Parsing diff...
					</span>
				</div>
			) : (
				<div className="space-y-6">
					{parsedDiff.map((file, fileIndex) => (
						<Card
							key={fileIndex}
							className="overflow-hidden rounded-none border border-border/50"
						>
							<CardContent className="p-0">
								<div className="overflow-hidden">
									{viewMode === "split"
										? renderSplitView(file)
										: renderUnifiedView(file)}
								</div>
							</CardContent>
						</Card>
					))}
				</div>
			)}
		</div>
	);
}
