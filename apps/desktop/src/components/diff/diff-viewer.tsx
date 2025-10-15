/** biome-ignore-all lint/correctness/noUnusedVariables: experimentel */
import { Card, CardContent } from "@gitru/ui/components/card";
import { structuredPatch } from "diff";
import { Loader2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
	type BundledLanguage,
	createHighlighter,
	type Highlighter,
} from "shiki";
import type { FileStatusKind, GetDiffResponse } from "@/tauri";
import { vesperLight } from "./custome-themes";
import { useDiffViewStore } from "./useDiffViewStore";

type DiffRowType = "added" | "removed" | "context";

interface DiffRow {
	type: DiffRowType;
	content: string;
	lineNumberOld?: number;
	lineNumberNew?: number;
	metadata?: boolean;
}

interface DiffHunk {
	id: string;
	header: string;
	lines: DiffRow[];
	additions: number;
	deletions: number;
	oldStart: number;
	newStart: number;
	oldLines: number;
	newLines: number;
}

const DEFAULT_LANGUAGE = "plaintext" as BundledLanguage;
const DEFAULT_CONTEXT_LINES = 3;

type DiffSegment =
	| { type: "hunk"; hunk: DiffHunk }
	| {
			type: "skip";
			id: string;
			oldStart: number;
			oldEnd: number;
			newStart: number;
			newEnd: number;
			lines: DiffRow[];
	  };

function HighlightedLine({
	html,
	className,
}: {
	html: string;
	className?: string;
}) {
	return (
		// biome-ignore lint/security/noDangerouslySetInnerHtml: Shiki generates sanitized HTML for syntax highlighting
		<span className={className} dangerouslySetInnerHTML={{ __html: html }} />
	);
}

const EXTENSION_LANGUAGE_MAP: Record<string, BundledLanguage | undefined> = {
	ts: "typescript",
	tsx: "tsx",
	js: "javascript",
	jsx: "jsx",
	py: "python",
	css: "css",
	html: "html",
	json: "json",
	txt: "plaintext" as BundledLanguage,
	lock: "json",
	sh: "bash",
	bash: "bash",
	rs: "rust",
	rust: "rust",
	java: "java",
	make: "makefile",
	docker: "dockerfile",
	dockerfile: "dockerfile",
	yaml: "yaml",
	yml: "yaml",
	csv: "csv",
	markdown: "markdown",
	md: "markdown",
	mdx: "mdx" as BundledLanguage,
};

const SPECIAL_FILENAMES: Record<string, BundledLanguage | undefined> = {
	makefile: "makefile",
	dockerfile: "dockerfile",
	gitignore: "bash",
	gitattributes: "bash",
	jenkinsfile: "groovy" as BundledLanguage,
	procfile: "bash",
	vagrantfile: "ruby" as BundledLanguage,
	brewfile: "ruby" as BundledLanguage,
};

const ALWAYS_AVAILABLE_LANGS = [
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
	"plaintext" as BundledLanguage,
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
	"groovy",
	"ruby",
] as BundledLanguage[];

const splitLines = (content: string): string[] => {
	if (!content) {
		return [];
	}

	const lines = content.split(/\r?\n/);
	if (lines.length > 0 && lines[lines.length - 1] === "") {
		lines.pop();
	}

	return lines;
};

const escapeHtml = (value: string): string =>
	value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

const formatRange = (start: number, count: number) => {
	const normalizedStart = Math.max(start, 0);
	if (count <= 0) {
		return `${normalizedStart},0`;
	}

	return count === 1 ? `${normalizedStart}` : `${normalizedStart},${count}`;
};

const buildRowsFromSingleContent = (
	content: string,
	variant: "added" | "removed",
): DiffRow[] => {
	const lines = splitLines(content);
	return lines.map((line, index) =>
		variant === "added"
			? {
					type: "added" as const,
					content: line,
					lineNumberNew: index + 1,
				}
			: {
					type: "removed" as const,
					content: line,
					lineNumberOld: index + 1,
				},
	);
};

const buildHunksFromDiff = (
	before: string,
	after: string,
	options: { treatAsNewFile: boolean; treatAsDeletedFile: boolean },
): DiffHunk[] => {
	const patch = structuredPatch("head", "workdir", before, after, "", "", {
		context: DEFAULT_CONTEXT_LINES,
		// TODO: will be made configurable later
		ignoreWhitespace: true,
	});

	if (patch.hunks.length === 0) {
		if (options.treatAsNewFile) {
			const rows = buildRowsFromSingleContent(after, "added");
			return [
				{
					id: "hunk-0",
					header: `@@ -0,0 +1,${Math.max(rows.length, 1)} @@`,
					lines: rows,
					additions: rows.length,
					deletions: 0,
					oldStart: 0,
					newStart: rows.length > 0 ? 1 : 0,
					oldLines: 0,
					newLines: rows.length,
				},
			];
		}

		if (options.treatAsDeletedFile) {
			const rows = buildRowsFromSingleContent(before, "removed");
			return [
				{
					id: "hunk-0",
					header: `@@ -1,${Math.max(rows.length, 1)} +0,0 @@`,
					lines: rows,
					additions: 0,
					deletions: rows.length,
					oldStart: rows.length > 0 ? 1 : 0,
					newStart: 0,
					oldLines: rows.length,
					newLines: 0,
				},
			];
		}

		return [];
	}

	return patch.hunks.map((hunk, index) => {
		let oldLine = hunk.oldStart;
		let newLine = hunk.newStart;
		const rows: DiffRow[] = [];
		let additions = 0;
		let deletions = 0;

		for (const rawLine of hunk.lines) {
			if (!rawLine.length) continue;
			const indicator = rawLine[0];
			const content = rawLine.slice(1);

			switch (indicator) {
				case " ":
					rows.push({
						type: "context",
						content,
						lineNumberOld: oldLine++,
						lineNumberNew: newLine++,
					});
					break;
				case "-":
					rows.push({
						type: "removed",
						content,
						lineNumberOld: oldLine++,
					});
					deletions += 1;
					break;
				case "+":
					rows.push({
						type: "added",
						content,
						lineNumberNew: newLine++,
					});
					additions += 1;
					break;
				case "\\":
					rows.push({
						type: "context",
						content: rawLine,
						metadata: true,
					});
					break;
				default:
					break;
			}
		}

		const header = `@@ -${formatRange(hunk.oldStart, hunk.oldLines)} +${formatRange(hunk.newStart, hunk.newLines)} @@`;

		return {
			id: `hunk-${index}`,
			header,
			lines: rows,
			additions,
			deletions,
			oldStart: hunk.oldStart,
			newStart: hunk.newStart,
			oldLines: hunk.oldLines,
			newLines: hunk.newLines,
		};
	});
};

const buildSkipSegment = (
	oldStart: number,
	oldEnd: number,
	newStart: number,
	newEnd: number,
	index: number,
	beforeLines: string[],
	afterLines: string[],
): DiffSegment | null => {
	const hasOld = oldStart > 0 && oldEnd >= oldStart;
	const hasNew = newStart > 0 && newEnd >= newStart;

	if (!hasOld && !hasNew) {
		return null;
	}

	let currentOld = hasOld ? oldStart : 0;
	let currentNew = hasNew ? newStart : 0;
	const lines: DiffRow[] = [];

	while ((hasOld && currentOld <= oldEnd) || (hasNew && currentNew <= newEnd)) {
		const oldLineNumber =
			hasOld && currentOld <= oldEnd ? currentOld : undefined;
		const newLineNumber =
			hasNew && currentNew <= newEnd ? currentNew : undefined;
		const content =
			(typeof newLineNumber === "number"
				? afterLines[newLineNumber - 1]
				: undefined) ??
			(typeof oldLineNumber === "number"
				? beforeLines[oldLineNumber - 1]
				: "") ??
			"";

		lines.push({
			type: "context",
			content,
			lineNumberOld: oldLineNumber,
			lineNumberNew: newLineNumber,
		});

		if (typeof oldLineNumber === "number") {
			currentOld += 1;
		}
		if (typeof newLineNumber === "number") {
			currentNew += 1;
		}
	}

	if (lines.length === 0) {
		return null;
	}

	return {
		type: "skip",
		id: `skip-${index}-${oldStart}-${oldEnd}-${newStart}-${newEnd}`,
		oldStart: hasOld ? oldStart : 0,
		oldEnd: hasOld ? oldEnd : 0,
		newStart: hasNew ? newStart : 0,
		newEnd: hasNew ? newEnd : 0,
		lines,
	};
};

const buildDiffSegments = (
	hunks: DiffHunk[],
	beforeLines: string[],
	afterLines: string[],
): DiffSegment[] => {
	if (hunks.length === 0) {
		return [];
	}

	const segments: DiffSegment[] = [];
	let previousOldEnd = 0;
	let previousNewEnd = 0;

	const totalOldLines = beforeLines.length;
	const totalNewLines = afterLines.length;

	hunks.forEach((hunk, index) => {
		const gapOldStart = previousOldEnd + 1;
		const gapOldEnd = hunk.oldStart > 0 ? Math.max(hunk.oldStart - 1, 0) : 0;
		const gapNewStart = previousNewEnd + 1;
		const gapNewEnd = hunk.newStart > 0 ? Math.max(hunk.newStart - 1, 0) : 0;

		const skip = buildSkipSegment(
			gapOldStart,
			gapOldEnd,
			gapNewStart,
			gapNewEnd,
			index,
			beforeLines,
			afterLines,
		);

		if (skip) {
			segments.push(skip);
		}

		segments.push({ type: "hunk", hunk });

		const hunkOldEnd =
			hunk.oldLines > 0 ? hunk.oldStart + hunk.oldLines - 1 : hunk.oldStart - 1;
		const hunkNewEnd =
			hunk.newLines > 0 ? hunk.newStart + hunk.newLines - 1 : hunk.newStart - 1;

		previousOldEnd = Math.max(previousOldEnd, hunkOldEnd);
		previousNewEnd = Math.max(previousNewEnd, hunkNewEnd);
	});

	const trailingSkip = buildSkipSegment(
		previousOldEnd + 1,
		totalOldLines,
		previousNewEnd + 1,
		totalNewLines,
		hunks.length,
		beforeLines,
		afterLines,
	);

	if (trailingSkip) {
		segments.push(trailingSkip);
	}

	return segments;
};

const formatBytes = (bytes: number) => {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

const resolveLanguage = (filePath: string): BundledLanguage => {
	const fileName = filePath.split("/").pop() || "";
	const lower = fileName.toLowerCase();

	if (lower.includes(".")) {
		const extension = lower.split(".").pop() || "";
		return EXTENSION_LANGUAGE_MAP[extension] ?? DEFAULT_LANGUAGE;
	}

	return SPECIAL_FILENAMES[lower] ?? DEFAULT_LANGUAGE;
};

export function DiffViewer({
	diff,
	filePath,
	status,
}: {
	diff: GetDiffResponse | null;
	filePath: string;
	status?: FileStatusKind[];
}) {
	const { viewMode } = useDiffViewStore();
	const [highlighter, setHighlighter] = useState<Highlighter | null>(null);
	const language = useMemo(() => resolveLanguage(filePath), [filePath]);

	useEffect(() => {
		let cancelled = false;

		(async () => {
			try {
				const hl = await createHighlighter({
					themes: ["vesper", vesperLight],
					langs: ALWAYS_AVAILABLE_LANGS,
				});
				if (!cancelled) {
					setHighlighter(hl);
				}
			} catch (error) {
				console.error("Failed to initialize Shiki highlighter:", error);
			}
		})();

		return () => {
			cancelled = true;
		};
	}, []);

	useEffect(() => {
		if (!highlighter) return;

		(async () => {
			try {
				await highlighter.loadLanguage(language);
			} catch (error) {
				console.error(`Failed to load Shiki language: ${language}`, error);
			}
		})();
	}, [highlighter, language]);

	const statusSet = useMemo(() => new Set(status ?? []), [status]);
	const treatAsNewFile =
		statusSet.has("WorktreeNew") ||
		statusSet.has("IndexNew") ||
		(!!diff?.workdir && !diff?.head);
	const treatAsDeletedFile =
		statusSet.has("WorktreeDeleted") ||
		statusSet.has("IndexDeleted") ||
		(!!diff?.head && !diff?.workdir);

	const binaryVersion = useMemo(() => {
		if (!diff) return null;
		if (diff.workdir?.is_binary) return diff.workdir;
		if (diff.head?.is_binary) return diff.head;
		return null;
	}, [diff]);

	const hunks = useMemo(() => {
		if (!diff || binaryVersion) {
			return [] as DiffHunk[];
		}

		const before = diff.head?.content ?? "";
		const after = diff.workdir?.content ?? "";

		return buildHunksFromDiff(before, after, {
			treatAsNewFile,
			treatAsDeletedFile,
		});
	}, [diff, binaryVersion, treatAsNewFile, treatAsDeletedFile]);

	const beforeLines = useMemo(
		() => splitLines(diff?.head?.content ?? ""),
		[diff?.head?.content],
	);
	const afterLines = useMemo(
		() => splitLines(diff?.workdir?.content ?? ""),
		[diff?.workdir?.content],
	);

	const segments = useMemo(() => {
		if (!diff || binaryVersion) {
			return [] as DiffSegment[];
		}

		return buildDiffSegments(hunks, beforeLines, afterLines);
	}, [diff, binaryVersion, hunks, beforeLines, afterLines]);

	const [_expandedSkips, setExpandedSkips] = useState<Set<string>>(new Set());

	useEffect(() => {
		if (!diff) {
			setExpandedSkips(new Set());
			return;
		}

		setExpandedSkips(new Set());
	}, [diff]);

	// const toggleSkip = useCallback((id: string) => {
	// 	setExpandedSkips((prev) => {
	// 		const next = new Set(prev);
	// 		if (next.has(id)) {
	// 			next.delete(id);
	// 		} else {
	// 			next.add(id);
	// 		}
	// 		return next;
	// 	});
	// }, []);

	const hasAnyRows = hunks.some((hunk) => hunk.lines.length > 0);

	const highlight = useMemo(() => {
		if (!highlighter) {
			return (code: string) => escapeHtml(code);
		}

		return (code: string) => {
			if (!code.length) {
				return "";
			}

			try {
				const html = highlighter.codeToHtml(code, {
					lang: language,
					themes: {
						light: "vesper-light",
						"dark-classic": "vesper",
					},
					defaultColor: "light",
					cssVariablePrefix: "--shiki-",
				});

				const match = html.match(/<code[^>]*>(.*?)<\/code>/s);
				return match ? match[1] : html;
			} catch (error) {
				console.error("Error highlighting code:", error);
				return escapeHtml(code);
			}
		};
	}, [highlighter, language]);

	const renderUnifiedView = (lines: DiffRow[], keyPrefix: string) => (
		<div className="overflow-x-auto shiki bg-background">
			<table className="w-full border-collapse diff-content">
				<tbody>
					{lines.map((line, index) => {
						let bgClass = "";
						if (line.type === "added") bgClass = "diff-added";
						if (line.type === "removed") bgClass = "diff-removed";

						const key = `${keyPrefix}-${line.lineNumberOld ?? "x"}-${line.lineNumberNew ?? "y"}-${index}`;
						const highlighted = line.metadata
							? escapeHtml(line.content)
							: highlight(line.content);
						const textClass = line.metadata
							? "italic text-muted-foreground"
							: undefined;

						return (
							<tr key={key} className={`${bgClass} diff-hover`}>
								<td className="tabular-nums text-center align-middle py-1 px-2 diff-line-number-bg w-14 text-muted-foreground text-xs select-none">
									{line.lineNumberOld ?? ""}
								</td>
								<td className="tabular-nums text-center align-middle py-1 px-2 diff-line-number-bg w-14 text-muted-foreground text-xs select-none">
									{line.lineNumberNew ?? ""}
								</td>
								<td className="pl-3 pr-2 text-sm font-mono font-medium leading-6 whitespace-pre">
									<HighlightedLine className={textClass} html={highlighted} />
								</td>
							</tr>
						);
					})}
				</tbody>
			</table>
		</div>
	);

	const renderSplitView = (lines: DiffRow[], keyPrefix: string) => {
		const grouped: Array<{ left: DiffRow | null; right: DiffRow | null }> = [];

		for (let i = 0; i < lines.length; i++) {
			const line = lines[i];

			if (line.type === "context") {
				grouped.push({ left: line, right: line });
				continue;
			}

			if (line.type === "removed") {
				const nextLine = lines[i + 1];
				if (nextLine && nextLine.type === "added") {
					grouped.push({ left: line, right: nextLine });
					i++;
				} else {
					grouped.push({ left: line, right: null });
				}
				continue;
			}

			if (line.type === "added") {
				grouped.push({ left: null, right: line });
			}
		}

		return (
			<div className="overflow-x-auto">
				<div className="flex">
					<table className="w-full border-collapse diff-content">
						<tbody>
							{grouped.map((pair, index) => {
								const leftBg =
									pair.left?.type === "removed" ? "diff-removed" : "";
								const rightBg =
									pair.right?.type === "added" ? "diff-added" : "";
								const key = `${keyPrefix}-${pair.left?.lineNumberOld ?? "x"}-${pair.right?.lineNumberNew ?? "y"}-${index}`;
								const leftHtml = pair.left
									? pair.left.metadata
										? escapeHtml(pair.left.content)
										: highlight(pair.left.content)
									: "";
								const rightHtml = pair.right
									? pair.right.metadata
										? escapeHtml(pair.right.content)
										: highlight(pair.right.content)
									: "";

								return (
									<tr key={key}>
										<td className="tabular-nums text-center align-middle py-1 px-2 diff-line-number-bg w-14 text-muted-foreground text-xs select-none">
											{pair.left?.lineNumberOld ?? ""}
										</td>
										<td
											className={`truncate text-sm font-mono font-medium leading-6 whitespace-pre border-r max-w-[10vw] ${leftBg}`}
										>
											{pair.left ? (
												<HighlightedLine
													className={`pl-3 pr-2 ${pair.left?.metadata ? "italic text-muted-foreground" : ""}`}
													html={leftHtml}
												/>
											) : (
												<span className="[--pattern-fg:var(--input)]/95 bg-gray-950 w-full h-full">
													<div className="_border-x h-6 w-full border-x-(--pattern-fg) bg-[image:repeating-linear-gradient(315deg,_var(--pattern-fg)_0,_var(--pattern-fg)_1px,_transparent_0,_transparent_50%)] bg-[size:10px_10px] bg-fixed"></div>
													<div className="_border-x border-x-(--pattern-fg) bg-[image:repeating-linear-gradient(315deg,_var(--pattern-fg)_0,_var(--pattern-fg)_1px,_transparent_0,_transparent_50%)] bg-[size:10px_10px] bg-fixed"></div>
												</span>
											)}
										</td>
										<td className="tabular-nums text-center align-middle py-1 px-2 diff-line-number-bg w-14 text-muted-foreground text-xs select-none">
											{pair.right?.lineNumberNew ?? ""}
										</td>
										<td
											className={`truncate text-sm font-mono font-medium leading-6 whitespace-pre max-w-[10vw] ${rightBg}`}
										>
											{pair.right ? (
												<HighlightedLine
													className={`pl-3 pr-2 ${pair.right?.metadata ? "italic text-muted-foreground" : ""}`}
													html={rightHtml}
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

	if (!diff) {
		return (
			<div className="flex justify-center items-center p-8">
				<Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
			</div>
		);
	}

	if (binaryVersion) {
		return (
			<Card className="overflow-hidden rounded-none border border-border/50">
				<CardContent className="p-6">
					<p className="text-sm text-muted-foreground">
						Binary file preview is not supported. Size:{" "}
						{formatBytes(binaryVersion.byte_length)}
					</p>
				</CardContent>
			</Card>
		);
	}

	if (!hasAnyRows) {
		return (
			<Card className="overflow-hidden rounded-none border border-border/50">
				<CardContent className="p-6">
					<p className="text-sm text-muted-foreground">
						No content to display for this file.
					</p>
				</CardContent>
			</Card>
		);
	}

	return (
		<Card className="overflow-hidden rounded-none border-0">
			{/* {treatAsNewFile || treatAsDeletedFile ? (
				<div
					className={`px-4 py-2 text-xs font-medium border-b border-border/50 ${treatAsNewFile ? "bg-emerald-500/10 text-emerald-300" : "bg-red-500/10 text-red-300"}`}
				>
					{treatAsNewFile
						? "Untracked file – displaying working tree contents"
						: "Deleted file – displaying last committed contents"}
				</div>
			) : null} */}
			<CardContent className="p-0">
				<div className="divide-y divide-border/50">
					{segments.map((segment) => {
						if (segment.type === "skip") {
							// const isExpanded = expandedSkips.has(segment.id);
							// const lineCount = segment.lines.length;
							// const oldCount =
							// 	segment.oldStart > 0 && segment.oldEnd >= segment.oldStart
							// 		? segment.oldEnd - segment.oldStart + 1
							// 		: 0;
							// const newCount =
							// 	segment.newStart > 0 && segment.newEnd >= segment.newStart
							// 		? segment.newEnd - segment.newStart + 1
							// 		: 0;
							// const header = `@@ -${formatRange(segment.oldStart, oldCount)} +${formatRange(segment.newStart, newCount)} @@`;

							return (
								<div key={segment.id} className="bg-primary/5">
									{/* <button
										type="button"
										onClick={() => toggleSkip(segment.id)}
										className="flex w-full items-center justify-between gap-4 px-4 py-2 text-left text-primary transition hover:bg-primary/10"
									>
										<span className="flex items-center gap-2">
											{isExpanded ? (
												<ChevronDown className="h-4 w-4 shrink-0" />
											) : (
												<ChevronRight className="h-4 w-4 shrink-0" />
											)}
											<span className="font-mono text-xs sm:text-sm">
												{header}
											</span>
										</span>
										<span className="text-xs font-medium text-muted-foreground">
											{isExpanded ? "Hide" : "Show"} {lineCount} unchanged{" "}
											{lineCount === 1 ? "line" : "lines"}
										</span>
									</button> */}
									{/* {isExpanded ? (
										<div className="border-t border-border/40">
											{viewMode === "split"
												? renderSplitView(segment.lines, segment.id)
												: renderUnifiedView(segment.lines, segment.id)}
										</div>
									) : null} */}
								</div>
							);
						}

						const { hunk } = segment;

						return (
							<div key={hunk.id} className="bg-background">
								<div className="flex items-center justify-between gap-4 bg-primary/10 px-4 py-1">
									<span className="font-mono text-xs sm:text-sm text-primary/80">
										{hunk.header}
									</span>
									<span className="flex items-center gap-2 text-xs font-medium">
										<span className="text-green-700 tabular-nums">
											+{hunk.additions}
										</span>
										<span className="text-red-700 tabular-nums">
											-{hunk.deletions}
										</span>
									</span>
								</div>
								<div className="border-t border-border/40">
									{viewMode === "split"
										? renderSplitView(hunk.lines, hunk.id)
										: renderUnifiedView(hunk.lines, hunk.id)}
								</div>
							</div>
						);
					})}
				</div>
			</CardContent>
		</Card>
	);
}
