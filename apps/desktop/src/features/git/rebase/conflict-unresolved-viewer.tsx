import type { FileStatusKind } from "@gitru/commands";
import { readWorktreeFile } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import { cn } from "@gitru/ui/lib/utils";
import {
  type FileContents,
  File as FileVanilla,
  UnresolvedFile as UnresolvedFileVanilla,
} from "@pierre/diffs";
import { getOrCreateWorkerPoolSingleton } from "@pierre/diffs/worker";
import { useQuery } from "@tanstack/react-query";
import { X } from "lucide-react";
import { useTheme } from "next-themes";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { useDiffViewerSettings } from "@/components/diff/use-diff-view-setting-store";
import { getStatusIcon } from "@/components/get-status-icon";
import LoaderIndicator from "@/components/loader-indicator";
import { SettingsPopover } from "@/features/git/components/diff-settings-popover";
import { renderPath } from "@/features/git/components/render-path";
import {
  useGetStatus,
  useGitAdd,
  useRebaseResolveConflict,
  useWriteWorktreeFile,
} from "@/hooks";
import { diffWorkerFactory } from "@/lib/diff-worker-factory";
import { useActiveRepositoryState } from "@/state/use-active-repository-state";
import { useAppStore } from "@/store/use-app-store";

const CONFLICT_MARKER = /^<<<<<<< /m;

/** Plenty of context around conflicts without MAX_SAFE_INTEGER layout blowups. */
const FULL_PATCH_CONTEXT_LINES = 10_000;

const HIGHLIGHTER_LANGS = [
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
] as const;

const DIFFS_UNSAFE_CSS = `
  @layer rendered {
    :host {
      --diffs-light-bg: color-mix(in oklab, var(--color-secondary) 20%, #ffffff);
      --diffs-dark-bg: #000000;
    }
  }
`;

const CONTENT_HEIGHT_CLASS =
  "h-full max-h-[calc(var(--layout-height)-(--spacing(23.25)))]";

type PierreInstance = { cleanUp(): void };

function getConflictWorkerPool() {
  return getOrCreateWorkerPoolSingleton({
    poolOptions: {
      workerFactory: diffWorkerFactory,
      poolSize: 2,
    },
    highlighterOptions: {
      theme: {
        dark: "github-dark",
        light: "github-light",
      },
      langs: [...HIGHLIGHTER_LANGS],
    },
  });
}

/**
 * While conflict markers remain: Pierre UnresolvedFile (Accept current / incoming / both).
 * After resolve: plain File view of the resulting contents.
 */
export function ConflictUnresolvedViewer({
  filePath,
  className,
}: {
  filePath: string;
  className?: string;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const instanceRef = useRef<PierreInstance | null>(null);
  const { theme } = useTheme();
  const { overflow } = useDiffViewerSettings();
  const repo = useActiveRepositoryState();
  const { data: status } = useGetStatus();
  const { mutateAsync: writeFile } = useWriteWorktreeFile();
  const { mutateAsync: gitAdd, isPending: staging } = useGitAdd();
  const { mutateAsync: resolve, isPending: resolving } =
    useRebaseResolveConflict();
  const clearWorktreeSelectionForRepo = useAppStore(
    (s) => s.clearWorktreeSelectionForRepo,
  );
  const setMainWindowView = useAppStore((s) => s.setMainWindowView);

  const [reloadToken, setReloadToken] = useState(0);
  const [hasMarkers, setHasMarkers] = useState(true);

  const isDark = theme?.startsWith("dark-") ?? false;
  const themeType = isDark ? ("dark" as const) : ("light" as const);

  const fileStatus = useMemo((): FileStatusKind[] => {
    const match = status?.files.find(
      (f) => f.path === filePath || f.new_path === filePath,
    );
    return match?.status ?? ["Conflicted"];
  }, [status, filePath]);

  const {
    data: fetchedContents,
    isLoading,
    error,
    refetch,
  } = useQuery({
    queryKey: ["worktree-file", repo?.contextId, filePath, reloadToken],
    queryFn: async () => {
      if (!repo) throw new Error("No repository");
      return readWorktreeFile({
        contextId: repo.contextId,
        path: filePath,
      });
    },
    enabled: !!repo && !!filePath,
  });

  useEffect(() => {
    if (fetchedContents != null) {
      setHasMarkers(CONFLICT_MARKER.test(fetchedContents));
    }
  }, [fetchedContents]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host || fetchedContents == null) return;

    host.replaceChildren();
    const wrapper = document.createElement("div");
    wrapper.className = "h-full min-h-0 w-full";
    host.appendChild(wrapper);

    const workerPool = getConflictWorkerPool();
    const markersPresent = CONFLICT_MARKER.test(fetchedContents);

    const mountFile = (file: FileContents) => {
      instanceRef.current?.cleanUp();
      wrapper.replaceChildren();
      const resolved = new FileVanilla(
        {
          themeType,
          overflow,
          disableFileHeader: true,
          unsafeCSS: DIFFS_UNSAFE_CSS,
        },
        workerPool,
      );
      resolved.render({ file, containerWrapper: wrapper });
      instanceRef.current = resolved;
    };

    if (markersPresent) {
      let file: FileContents = {
        name: filePath,
        contents: fetchedContents,
        cacheKey: `${filePath}:conflict:${reloadToken}:${fetchedContents.length}`,
      };

      const instance = new UnresolvedFileVanilla(
        {
          themeType,
          overflow,
          disableFileHeader: true,
          mergeConflictActionsType: "default",
          lineHoverHighlight: "both",
          maxContextLines: FULL_PATCH_CONTEXT_LINES,
          unsafeCSS: DIFFS_UNSAFE_CSS,
          onMergeConflictAction(payload, inst) {
            const result = inst.resolveConflict(
              payload.conflict.conflictIndex,
              payload.resolution,
            );
            if (!result) return;

            file = result.file;
            const stillConflicted = CONFLICT_MARKER.test(file.contents);
            setHasMarkers(stillConflicted);

            void writeFile({ path: filePath, contents: file.contents })
              .then(() => {
                if (!stillConflicted) {
                  toast.success(
                    "Conflicts resolved — stage the file to continue",
                  );
                }
              })
              .catch((e) => {
                toast.error(
                  e instanceof Error
                    ? e.message
                    : "Failed to write resolved file",
                );
              });

            if (stillConflicted) {
              inst.render({
                file,
                fileDiff: result.fileDiff,
                actions: result.actions,
                markerRows: result.markerRows,
                containerWrapper: wrapper,
              });
            } else {
              mountFile(file);
            }
          },
        },
        workerPool,
      );

      instance.render({
        file,
        containerWrapper: wrapper,
      });
      instanceRef.current = instance;
    } else {
      mountFile({
        name: filePath,
        contents: fetchedContents,
        cacheKey: `${filePath}:resolved:${reloadToken}:${fetchedContents.length}`,
      });
    }

    return () => {
      instanceRef.current?.cleanUp();
      instanceRef.current = null;
      host.replaceChildren();
    };
  }, [fetchedContents, filePath, overflow, reloadToken, themeType, writeFile]);

  if (isLoading && fetchedContents == null) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <LoaderIndicator />
      </div>
    );
  }

  if ((error && fetchedContents == null) || fetchedContents == null) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-muted-foreground p-4">
        {error instanceof Error ? error.message : "Failed to load file"}
      </div>
    );
  }

  return (
    <div className={cn("flex-1 min-h-0 flex flex-col", className)}>
      <div className="w-full h-9.25 border-b flex justify-between items-center shrink-0">
        <div className="items-center h-full px-2 flex gap-2 min-w-0">
          {getStatusIcon(fileStatus)}
          <span className="text-sm truncate">{renderPath(filePath)}</span>
        </div>
        <div className="flex items-center gap-1.5 pr-2 shrink-0">
          {hasMarkers ? (
            <>
              <Button
                size="sm"
                variant="ghost"
                className="h-7"
                disabled={resolving}
                onClick={async () => {
                  await resolve({ path: filePath, strategy: "ours" });
                  setReloadToken((t) => t + 1);
                  await refetch();
                }}
              >
                Ours
              </Button>
              <Button
                size="sm"
                variant="ghost"
                className="h-7"
                disabled={resolving}
                onClick={async () => {
                  await resolve({ path: filePath, strategy: "theirs" });
                  setReloadToken((t) => t + 1);
                  await refetch();
                }}
              >
                Theirs
              </Button>
            </>
          ) : (
            <Button
              size="sm"
              variant="secondary"
              className="h-7"
              disabled={staging}
              onClick={async () => {
                await gitAdd([filePath]);
                toast.success("Staged resolved file");
              }}
            >
              Stage
            </Button>
          )}
          <Button
            size="icon-xs"
            variant="outline"
            aria-label="Close file"
            onClick={() => {
              clearWorktreeSelectionForRepo();
              setMainWindowView(null);
            }}
          >
            <X />
          </Button>
          <SettingsPopover />
        </div>
      </div>

      <div
        className={cn(
          "flex-1 min-h-0 w-full relative overflow-hidden",
          isDark ? "bg-black" : "bg-secondary/20",
        )}
      >
        <div
          className={cn(
            CONTENT_HEIGHT_CLASS,
            "w-full flex overflow-auto select-auto",
          )}
        >
          <div ref={hostRef} className="h-full min-h-0 w-full overflow-auto" />
        </div>
      </div>
    </div>
  );
}
