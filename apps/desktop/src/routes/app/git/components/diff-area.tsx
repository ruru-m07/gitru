import type { FileStatusKind } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import { cn } from "@gitru/ui/lib/utils";
import { parseDiffFromFile } from "@pierre/diffs";
import {
  MultiFileDiff,
  Virtualizer,
  WorkerPoolContextProvider,
} from "@pierre/diffs/react";
import { Minus, Plus, Undo } from "lucide-react";
import { useTheme } from "next-themes";
import { useEffect, useMemo, useState } from "react";
import { ImageDiffViewer } from "@/components/diff/image/ImageDiffViewer";
import { useDiffViewerSettings } from "@/components/diff/useDiffViewSettingStore";
import LoaderIndicator from "@/components/loaderIndicator";
import { useGetDiff, useGitApplyPatchBlock } from "@/hooks";
import { diffWorkerFactory } from "@/lib/diffWorkerFactory";
import type { ChunkActionMetadata } from "../lib/chunk-action-metadata";

export function DiffArea({
  filePath,
  fileNewPath,
  status,
  stashReference,
  commitHash,
  worktreeScope,
}: {
  filePath: string;
  fileNewPath: string | null;
  status: FileStatusKind[];
  stashReference: string | null;
  commitHash: string | null;
  worktreeScope?: "staged" | "unstaged" | "conflicted";
}) {
  const { diffStyle, overflow } = useDiffViewerSettings();
  const [effectiveDiffStyle, setEffectiveDiffStyle] =
    useState<typeof diffStyle>(diffStyle);
  const { theme } = useTheme();

  const derivedScope =
    worktreeScope ??
    (status?.some((s) => String(s).startsWith("Index")) &&
    !status?.some((s) => String(s).startsWith("Worktree"))
      ? "staged"
      : status?.some((s) => String(s).startsWith("Worktree"))
        ? "unstaged"
        : undefined);

  const { data: diffData, isLoading } = useGetDiff(filePath, {
    fileNewPath,
    status,
    stashReference,
    commitHash,
    parentIndex: commitHash ? 1 : undefined,
    diffScope:
      derivedScope === "staged"
        ? "Staged"
        : derivedScope === "unstaged" || derivedScope === "conflicted"
          ? "Unstaged"
          : "Worktree",
  });

  useEffect(() => {
    let mounted = true;

    const compute = () => {
      try {
        const layoutEl = document.querySelector(
          '[data-layout-id="local-git-layout"]',
        );
        if (!layoutEl) {
          return diffStyle;
        }

        const raw =
          getComputedStyle(layoutEl).getPropertyValue("--right-width") || "";
        const num = parseFloat(raw.trim().replace("px", ""));

        if (!isNaN(num) && num < 750) {
          return "unified" as typeof diffStyle;
        }
      } catch (e) {
        console.error("[DiffArea] Error reading --right-width:", e);
      }

      return diffStyle;
    };

    const recompute = () => {
      if (!mounted) return;
      setEffectiveDiffStyle(compute());
    };

    recompute();

    window.addEventListener("resize", recompute);

    const layoutEl = document.querySelector(
      '[data-layout-id="local-git-layout"]',
    );
    let mo: MutationObserver | undefined;

    if (layoutEl) {
      mo = new MutationObserver(() => {
        console.log("[DiffArea] Layout element changed, recomputing...");
        recompute();
      });
      mo.observe(layoutEl, {
        attributes: true,
        attributeFilter: ["style"],
      });
    }

    return () => {
      mounted = false;
      window.removeEventListener("resize", recompute);
      mo?.disconnect();
    };
  }, [diffStyle]);

  const { mutateAsync: applyPatchBlock } = useGitApplyPatchBlock();

  const source = stashReference ? "stash" : commitHash ? "history" : "worktree";

  const canStageOrDiscard =
    source === "worktree" && derivedScope === "unstaged";
  const canUnstage = source === "worktree" && derivedScope === "staged";
  const patchDiffScope =
    derivedScope === "staged"
      ? "Staged"
      : derivedScope === "unstaged" || derivedScope === "conflicted"
        ? "Unstaged"
        : "Worktree";

  const parsedDiff = useMemo(() => {
    if (!diffData?.oldFile || !diffData?.newFile) {
      return null;
    }

    try {
      return parseDiffFromFile(
        {
          name: diffData.oldFile.name,
          contents: diffData.oldFile.contents,
          cacheKey: `${source}:${filePath}:old`,
        },
        {
          name: diffData.newFile.name,
          contents: diffData.newFile.contents,
          cacheKey: `${source}:${filePath}:new`,
        },
      );
    } catch (error) {
      console.warn("Failed to parse diff from file contents", error);
      return null;
    }
  }, [diffData, filePath, source]);

  const blockMetadataLookup = useMemo(() => {
    if (!parsedDiff) {
      return new Map<string, ChunkActionMetadata>();
    }

    const lookup = new Map<string, ChunkActionMetadata>();

    for (let hunkIndex = 0; hunkIndex < parsedDiff.hunks.length; hunkIndex++) {
      const hunk = parsedDiff.hunks[hunkIndex];
      let additionCursor = hunk.additionStart;
      let deletionCursor = hunk.deletionStart;
      let changeIndex = 0;

      for (const content of hunk.hunkContent) {
        if (content.type === "context") {
          const contextLen = content.lines;
          additionCursor += contextLen;
          deletionCursor += contextLen;
          continue;
        }

        const additionsLen = content.additions;
        const deletionsLen = content.deletions;

        // Store metadata for the FIRST line of each block only
        // Smart pairing: if both additions and deletions exist, show only 1 annotation (on additions side)
        // If only one side exists, show annotation on that side

        if (additionsLen > 0 && deletionsLen > 0) {
          // Replacement block (paired): show one annotation on additions side
          const payload: ChunkActionMetadata = {
            source,
            filePath,
            fileNewPath,
            stashReference,
            commitHash,
            hunkIndex,
            changeIndex,
            side: "additions",
            additions: {
              start: additionCursor,
              end: additionCursor + additionsLen - 1,
              count: additionsLen,
            },
            deletions: {
              start: deletionCursor,
              end: deletionCursor + deletionsLen - 1,
              count: deletionsLen,
            },
          };
          lookup.set(`additions:${additionCursor + additionsLen - 1}`, payload);
        } else if (additionsLen > 0) {
          // Pure addition: show annotation on additions side
          const payload: ChunkActionMetadata = {
            source,
            filePath,
            fileNewPath,
            stashReference,
            commitHash,
            hunkIndex,
            changeIndex,
            side: "additions",
            additions: {
              start: additionCursor,
              end: additionCursor + additionsLen - 1,
              count: additionsLen,
            },
            deletions: {
              start: null,
              end: null,
              count: 0,
            },
          };
          lookup.set(`additions:${additionCursor + additionsLen - 1}`, payload);
        } else if (deletionsLen > 0) {
          // Pure deletion: show annotation on deletions side
          const payload: ChunkActionMetadata = {
            source,
            filePath,
            fileNewPath,
            stashReference,
            commitHash,
            hunkIndex,
            changeIndex,
            side: "deletions",
            additions: {
              start: null,
              end: null,
              count: 0,
            },
            deletions: {
              start: deletionCursor,
              end: deletionCursor + deletionsLen - 1,
              count: deletionsLen,
            },
          };
          lookup.set(`deletions:${deletionCursor + deletionsLen - 1}`, payload);
        }

        additionCursor += additionsLen;
        deletionCursor += deletionsLen;
        changeIndex += 1;
      }
    }

    return lookup;
  }, [parsedDiff, source, filePath, fileNewPath, stashReference, commitHash]);

  const blockAnnotations = useMemo(() => {
    const annotations: Array<{
      side: "additions" | "deletions";
      lineNumber: number;
      metadata: ChunkActionMetadata;
    }> = [];

    // Convert blockMetadataLookup Map entries to lineAnnotations array
    blockMetadataLookup.forEach((metadata, key) => {
      const [side, lineNumberStr] = key.split(":");
      const lineNumber = parseInt(lineNumberStr, 10);

      if (!isNaN(lineNumber)) {
        annotations.push({
          side: side as "additions" | "deletions",
          lineNumber,
          metadata,
        });
      }
    });

    return annotations;
  }, [blockMetadataLookup]);

  const assetKind = String(diffData?.asset_diff?.kind ?? "").toLowerCase();
  const isImageAssetDiff = assetKind === "image";
  const imageAssetDiff = isImageAssetDiff
    ? (diffData?.asset_diff ?? null)
    : null;

  return (
    <div
      className={cn(
        "h-full max-h-[calc(var(--layout-height)-(--spacing(23.25)))] dark:bg-black bg-secondary/20 w-full relative overflow-y-auto",
        theme?.startsWith("dark-") ? "#000000" : "var(--secondary)",
      )}
    >
      {isLoading ? (
        <div className="p-2.5">
          <LoaderIndicator />
        </div>
      ) : (
        <>
          {imageAssetDiff ? <ImageDiffViewer diff={imageAssetDiff} /> : null}
          {!isImageAssetDiff && (
            <div className="max-h-[calc(var(--layout-height)-(--spacing(23.25)))] h-full w-full flex overflow-auto select-auto">
              <WorkerPoolContextProvider
                poolOptions={{
                  workerFactory: diffWorkerFactory,
                  poolSize: 4,
                }}
                highlighterOptions={{
                  theme: {
                    dark: "github-dark",
                    light: "github-light",
                  },
                  langs: [
                    "typescript",
                    "tsx",
                    "javascript",
                    "jsx",
                    "rust",
                    "json",
                    "css",
                    "html",
                    "markdown",
                    "toml",
                    "yaml",
                  ],
                }}
              >
                <Virtualizer
                  className="max-h-[calc(var(--layout-height)-(--spacing(23.25)))] overflow-auto w-full"
                  contentClassName="space-y-4 w-full!"
                >
                  <MultiFileDiff
                    key={`${diffData?.oldFile?.name}-${diffData?.newFile?.name}-${source}-${patchDiffScope}-${diffData?.patch}`}
                    className="w-full"
                    oldFile={{
                      contents: diffData?.oldFile?.contents || "",
                      name: diffData?.oldFile?.name || "untitled.txt",
                    }}
                    newFile={{
                      contents: diffData?.newFile?.contents || "",
                      name: diffData?.newFile?.name || "untitled.txt",
                    }}
                    options={{
                      themeType: theme?.startsWith("dark-") ? "dark" : "light",
                      diffStyle: effectiveDiffStyle,
                      overflow,
                      disableFileHeader: true,
                      collapsedContextThreshold: 0,
                      lineHoverHighlight: "both",
                      unsafeCSS: `
                        @layer rendered {
                          :host {
                              --diffs-light-bg: color-mix(in oklab, var(--color-secondary) 20%, #ffffff);
                              --diffs-dark-bg: #000000;
                          }
                      }
                      `,
                    }}
                    lineAnnotations={blockAnnotations}
                    renderAnnotation={(annotation) => {
                      if (!canStageOrDiscard && !canUnstage) {
                        return null;
                      }

                      const payload = {
                        filePath: annotation.metadata.filePath,
                        fileNewPath: annotation.metadata.fileNewPath,
                        diffScope: patchDiffScope,
                        additions: {
                          start:
                            annotation.metadata.additions.start ?? undefined,
                          count: annotation.metadata.additions.count,
                        },
                        deletions: {
                          start:
                            annotation.metadata.deletions.start ?? undefined,
                          count: annotation.metadata.deletions.count,
                        },
                      } as const;

                      return (
                        <div
                          style={{
                            position: "relative",
                            zIndex: 10,
                            width: "100%",
                            overflow: "visible",
                          }}
                        >
                          <div className="absolute -top-2 right-4 flex gap-1">
                            {canStageOrDiscard && (
                              <>
                                <Button
                                  size={"icon-xs"}
                                  variant={"outline"}
                                  aria-label="Stage changes"
                                  onClick={async () => {
                                    try {
                                      await applyPatchBlock({
                                        ...payload,
                                        action: "Stage",
                                      });
                                    } catch {
                                      // handled by mutation toast
                                    }
                                  }}
                                >
                                  <Plus />
                                </Button>
                                <Button
                                  size={"icon-xs"}
                                  variant={"outline"}
                                  aria-label="Discard changes"
                                  onClick={async () => {
                                    try {
                                      await applyPatchBlock({
                                        ...payload,
                                        action: "Discard",
                                      });
                                    } catch {
                                      // handled by mutation toast
                                    }
                                  }}
                                >
                                  <Undo />
                                </Button>
                              </>
                            )}
                            {canUnstage && (
                              <Button
                                size={"icon-xs"}
                                variant={"outline"}
                                aria-label="Unstage changes"
                                onClick={async () => {
                                  try {
                                    await applyPatchBlock({
                                      ...payload,
                                      action: "Unstage",
                                    });
                                  } catch {
                                    // handled by mutation toast
                                  }
                                }}
                              >
                                <Minus />
                              </Button>
                            )}
                          </div>
                        </div>
                      );
                    }}
                  />
                </Virtualizer>
              </WorkerPoolContextProvider>
            </div>
          )}
        </>
      )}
    </div>
  );
}
