import { Card, CardContent } from "@gitru/ui/components/card";
import { FoldVertical } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { FileStatusKind, GetDiffResponse } from "@/tauri";
import { DiffHunk, DiffRow, DiffSegment } from "./diff-types";
import {
  buildDiffSegments,
  buildHunksFromDiff,
  computeInlineDiff,
  escapeHtml,
  formatBytes,
  formatRange,
  resolveLanguage,
  splitLines,
} from "./diff-utils";
import { HighlightedLine, highlighter } from "./highlighter";
import { useDiffViewStore } from "./useDiffViewStore";

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
  const language = useMemo(() => resolveLanguage(filePath), [filePath]);
  const [expandedSkips, setExpandedSkips] = useState<Set<string>>(new Set());

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

  const toggleSkip = useCallback((id: string) => {
    setExpandedSkips((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  // TODO(ruru-m07): will implement empty svg and something batter later
  // const hasAnyRows = hunks.some((hunk) => hunk.lines.length > 0);

  const highlight = useMemo(() => {
    if (!highlighter) {
      return (code: string) => escapeHtml(code);
    }

    return (code: string) => {
      if (!code.length) {
        return "";
      }

      try {
        const html = highlighter!.codeToHtml(code, {
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

            let contentHtml: string;

            // For changed lines, compute inline diff
            if (
              (line.type === "added" || line.type === "removed") &&
              !line.metadata
            ) {
              // Find the corresponding opposite line for inline diff
              const oppositeType = line.type === "added" ? "removed" : "added";
              const oppositeLine = lines.find(
                (l, i) => l.type === oppositeType && Math.abs(i - index) <= 1, // Look at adjacent lines
              );

              if (oppositeLine) {
                const oldText =
                  line.type === "removed" ? line.content : oppositeLine.content;
                const newText =
                  line.type === "added" ? line.content : oppositeLine.content;
                const inlineDiff = computeInlineDiff(oldText, newText);

                // Build HTML with inline highlighting
                const parts = inlineDiff
                  .filter((part) => {
                    if (line.type === "removed") return !part.added;
                    if (line.type === "added") return !part.removed;
                    return true;
                  })
                  .map((part) => {
                    const highlighted = highlight(part.value);
                    if (
                      (line.type === "removed" && part.removed) ||
                      (line.type === "added" && part.added)
                    ) {
                      // Stronger highlight for actual changes
                      return `<span class="diff-word-${line.type}">${highlighted}</span>`;
                    }
                    return highlighted;
                  })
                  .join("");

                contentHtml = parts;
              } else {
                contentHtml = highlight(line.content);
              }
            } else {
              contentHtml = line.metadata
                ? escapeHtml(line.content)
                : highlight(line.content);
            }

            const textClass = line.metadata
              ? "italic text-muted-foreground"
              : undefined;

            return (
              <tr key={key} className={`${bgClass} diff-hover`}>
                <td
                  className={`border-r py-1 w-12 text-muted-foreground text-xs select-none diff-line-number-segment-td ${line.type === "removed" && "diff-line-number-bg-removed"} ${line.type === "added" && "diff-line-number-bg-added"} diff-line-number-bg border-l-0`}
                >
                  {/* // ! we can do text-right if we want a better number alignment */}
                  <span className="font-mono px-3 block w-full text-center _text-right tabular-nums">
                    {line.lineNumberOld ?? ""}
                  </span>
                </td>
                <td
                  className={`border-r py-1 w-12 text-muted-foreground text-xs select-none diff-line-number-segment-td ${line.type === "removed" && "diff-line-number-bg-removed"} ${line.type === "added" && "diff-line-number-bg-added"} diff-line-number-bg`}
                >
                  {/* // ! we can do text-right if we want a better number alignment */}
                  <span className="font-mono px-3 block w-full text-center _text-right tabular-nums">
                    {line.lineNumberNew ?? ""}
                  </span>
                </td>
                <td className="pl-3 pr-2 text-sm font-mono leading-6 whitespace-pre bg-[var(--diff-segment-bg-unified)]">
                  <HighlightedLine className={textClass} html={contentHtml} />
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
      <div>
        <div className="flex">
          <table className="table-fixed w-full border-collapse diff-content">
            <tbody>
              {grouped.map((pair, index) => {
                const leftBg =
                  pair.left?.type === "removed" ? "diff-removed" : "";
                const rightBg =
                  pair.right?.type === "added" ? "diff-added" : "";
                const key = `${keyPrefix}-${pair.left?.lineNumberOld ?? "x"}-${pair.right?.lineNumberNew ?? "y"}-${index}`;

                let leftHtml = "";
                let rightHtml = "";

                // Compute inline diff for paired removed/added lines
                if (
                  pair.left &&
                  pair.right &&
                  pair.left.type === "removed" &&
                  pair.right.type === "added" &&
                  !pair.left.metadata &&
                  !pair.right.metadata
                ) {
                  const inlineDiff = computeInlineDiff(
                    pair.left.content,
                    pair.right.content,
                  );

                  // Build left side (removed) HTML
                  const leftParts = inlineDiff
                    .filter((part) => !part.added)
                    .map((part) => {
                      const highlighted = highlight(part.value);
                      if (part.removed) {
                        return `<span class="diff-word-removed">${highlighted}</span>`;
                      }
                      return highlighted;
                    })
                    .join("");
                  leftHtml = leftParts;

                  // Build right side (added) HTML
                  const rightParts = inlineDiff
                    .filter((part) => !part.removed)
                    .map((part) => {
                      const highlighted = highlight(part.value);
                      if (part.added) {
                        return `<span class="diff-word-added">${highlighted}</span>`;
                      }
                      return highlighted;
                    })
                    .join("");
                  rightHtml = rightParts;
                } else {
                  // Regular highlighting without inline diff
                  leftHtml = pair.left
                    ? pair.left.metadata
                      ? escapeHtml(pair.left.content)
                      : highlight(pair.left.content)
                    : "";
                  rightHtml = pair.right
                    ? pair.right.metadata
                      ? escapeHtml(pair.right.content)
                      : highlight(pair.right.content)
                    : "";
                }

                return (
                  <div key={key} className="grid grid-cols-[auto_1fr_auto_1fr]">
                    <div
                      className={`py-1 w-12 text-muted-foreground diff-line-number-segment-td text-xs select-none ${leftBg ? "diff-line-number-bg-removed" : "diff-line-number-bg border-r"} ${leftBg} border-l-0`}
                    >
                      {/* // ! we can do text-right if we want a better number alignment */}
                      <span className="px-3 block w-full text-center _text-right tabular-nums">
                        {pair.left?.lineNumberOld ?? ""}
                      </span>
                    </div>
                    <div
                      className={`truncate text-sm font-mono leading-6 whitespace-pre _max-w-[10vw] ${leftBg}`}
                    >
                      {pair.left ? (
                        <HighlightedLine
                          className={`_text-wrap _flex-wrap pl-3 pr-2 ${pair.left?.metadata ? "italic text-muted-foreground" : ""}`}
                          html={leftHtml}
                        />
                      ) : (
                        <span className="[--pattern-fg:var(--input)]/50 w-full h-full">
                          <div className="_border-x h-6 w-full border-x-(--pattern-fg) bg-[image:repeating-linear-gradient(315deg,_var(--pattern-fg)_0,_var(--pattern-fg)_1.5px,_transparent_0,_transparent_50%)] bg-[size:9px_9px] bg-fixed"></div>
                        </span>
                      )}
                    </div>
                    <div
                      className={`py-1 w-12 text-muted-foreground diff-line-number-segment-td text-xs select-none ${rightBg ? "diff-line-number-bg-added" : "diff-line-number-bg border-x"} ${rightBg}`}
                    >
                      {/* // ! we can do text-right if we want a better number alignment */}
                      <span className="px-3 block w-full text-center _text-right tabular-nums">
                        {pair.right?.lineNumberNew ?? ""}
                      </span>
                    </div>
                    <div
                      className={`truncate text-sm font-mono leading-6 whitespace-pre _max-w-[10vw] ${rightBg}`}
                    >
                      {pair.right ? (
                        <HighlightedLine
                          className={`_text-wrap _flex-wrap pl-3 pr-2 ${pair.right?.metadata ? "italic text-muted-foreground" : ""}`}
                          html={rightHtml}
                        />
                      ) : (
                        <span className="[--pattern-fg:var(--input)]/50 w-full h-full">
                          <div className="_border-x h-6 w-full border-x-(--pattern-fg) bg-[image:repeating-linear-gradient(315deg,_var(--pattern-fg)_0,_var(--pattern-fg)_1.5px,_transparent_0,_transparent_50%)] bg-[size:9px_9px] bg-fixed"></div>
                        </span>
                      )}
                    </div>
                  </div>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    );
  };

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

  // TODO(ruru-m07): will implement empty svg and something batter later
  // if (!hasAnyRows) {
  //   return (
  //     <Card className="overflow-hidden rounded-none border border-border/50">
  //       <CardContent className="p-6">
  //         <p className="text-sm text-muted-foreground">
  //           No content to display for this file.
  //         </p>
  //       </CardContent>
  //     </Card>
  //   );
  // }

  return (
    <Card className="overflow-hidden rounded-none! before:rounded-none border-0 py-0">
      {/* // TODO(ruru-m07): will do something better here */}
      {/* {treatAsNewFile || treatAsDeletedFile ? (
				<div
					className={`px-4 py-2 text-xs font-medium border-b border-border/50 ${treatAsNewFile ? "bg-emerald-500/10 text-emerald-300" : "bg-red-500/10 text-red-300"}`}
				>
					{treatAsNewFile
						? "Untracked file – displaying working tree contents"
						: "Deleted file – displaying last committed contents"}
				</div>
			) : null} */}
      <CardContent className="p-0 rounded-none!">
        <div className="divide-y divide-border/50">
          {segments.map((segment) => {
            if (segment.type === "skip") {
              const isExpanded = expandedSkips.has(segment.id);
              // const lineCount = segment.lines.length;
              const oldCount =
                segment.oldStart > 0 && segment.oldEnd >= segment.oldStart
                  ? segment.oldEnd - segment.oldStart + 1
                  : 0;
              const newCount =
                segment.newStart > 0 && segment.newEnd >= segment.newStart
                  ? segment.newEnd - segment.newStart + 1
                  : 0;
              const header = `@@ -${formatRange(segment.oldStart, oldCount)} +${formatRange(segment.newStart, newCount)} @@`;

              return (
                <div key={segment.id} className="">
                  {isExpanded ? (
                    <div className="bg-[var(--diff-segment-diff-bg)] [--diff-segment-bg-unified:var(--diff-segment-diff-bg)] diff-line-number-segment">
                      {viewMode === "split"
                        ? renderSplitView(segment.lines, segment.id)
                        : renderUnifiedView(segment.lines, segment.id)}
                    </div>
                  ) : (
                    <div className="flex items-center justify-between gap-4 bg-[var(--diff-segment-bg)] _border-y-[0.5px] _border-primary/10">
                      <div className="flex items-center">
                        <div
                          onClick={() => toggleSkip(segment.id)}
                          className="w-[calc(calc(var(--spacing)_*_12))] py-1.5 text-muted-foreground hover:text-foreground cursor-pointer hover:bg-[var(--diff-segment-expend-button-bg-hover)] bg-[var(--diff-segment-expend-button-bg)] flex items-center justify-center"
                        >
                          <FoldVertical size={16} />
                        </div>
                        <span className="font-mono text-xs sm:text-sm text-muted-foreground! pl-3 py-1 border-l tabular-nums border-[var(--diff-segment-expend-button-border)] ">
                          {header}
                        </span>
                      </div>
                      <span className="flex items-center gap-2 text-xs pr-4">
                        {/* // TODO(ruru-m07):  */}
                        {/* {lineCount} */}
                      </span>
                    </div>
                  )}
                </div>
              );
            }

            const { hunk } = segment;

            return (
              <div key={hunk.id} className="bg-background">
                {/* // TODO(ruru): find out a way to show the header only when there are changes */}
                {/* {false ? null : (
                  <div className="flex items-center justify-between gap-4 bg-primary/10 border-y border-primary/10">
                    <div className="flex items-center">
                      <div className="w-[calc(calc(var(--spacing)_*_12)+1px)] py-1.5 border-r text-muted-foreground hover:text-foreground cursor-pointer border-primary/10 hover:bg-primary/30 flex items-center justify-center">
                        <FoldVertical size={16} />
                      </div>
                      <span className="font-mono text-xs sm:text-sm text-primary/80 pl-3 py-1">
                        {hunk.header}
                      </span>
                    </div>
                    <span className="flex items-center gap-2 text-xs pr-4">
                      <span className="text-green-700 tabular-nums">
                        +{hunk.additions}
                      </span>
                      <span className="text-red-700 tabular-nums">
                        -{hunk.deletions}
                      </span>
                    </span>
                  </div>
                )} */}

                <div className="_border-t _border-border/40">
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
