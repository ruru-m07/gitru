import { cn } from "@gitru/ui/lib/utils";
import { MultiFileDiff } from "@pierre/diffs/react";
import { useTheme } from "next-themes";
import { useRef, useState } from "react";
import { ImageDiffViewer } from "@/components/diff/image/ImageDiffViewer";
import { useDiffViewerSettings } from "@/components/diff/useDiffViewSettingStore";
import LoaderIndicator from "@/components/loaderIndicator";
import { useGetDiff } from "@/hooks";
import { useSearchTextHighlight } from "@/hooks/useSearchTextHighlight";
import type { TimelineSearchHit } from "@/hooks/useTimelineSearch";
import { TimelineSearchDiffHeader } from "./TimelineSearchDiffHeader";

export function TimelineSearchDiffPreview({
  hit,
  searchQuery,
  isRegex,
}: {
  hit: TimelineSearchHit;
  searchQuery: string;
  isRegex: boolean;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const { theme } = useTheme();
  const { diffStyle, overflow } = useDiffViewerSettings();
  const [collapsed, setCollapsed] = useState(false);

  function toggleCollapsed() {
    setCollapsed((current) => !current);
  }
  const {
    data: diffData,
    isLoading,
    isFetching,
    isError,
  } = useGetDiff(
    hit.filePath,
    {
      fileNewPath: hit.fileNewPath ?? null,
      commitHash: hit.commitHash,
      parentIndex: 1,
    },
    {
      placeholderData: undefined,
      staleTime: 0,
    },
  );

  const showLoading = isLoading || isFetching;
  const assetKind = String(diffData?.asset_diff?.kind ?? "").toLowerCase();
  const isImageAssetDiff = assetKind === "image";
  const highlightReady = !showLoading && !isImageAssetDiff && !!diffData;

  useSearchTextHighlight(containerRef, searchQuery, isRegex, highlightReady);

  if (showLoading && !diffData) {
    return (
      <div className="p-4">
        <LoaderIndicator />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="p-4 text-sm text-muted-foreground">
        Unable to load patch for this result.
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className={cn(
        "timeline-search-diff-root w-full overflow-auto",
        theme?.startsWith("dark-") ? "bg-black" : "bg-secondary",
      )}
    >
      {isImageAssetDiff && diffData?.asset_diff ? (
        <ImageDiffViewer diff={diffData.asset_diff} />
      ) : (
        <MultiFileDiff
          key={`${hit.commitHash}-${hit.filePath}-${hit.fileNewPath ?? ""}`}
          className="w-full"
          oldFile={{
            contents: diffData?.oldFile?.contents ?? "",
            name: diffData?.oldFile?.name ?? hit.filePath,
          }}
          newFile={{
            contents: diffData?.newFile?.contents ?? "",
            name: diffData?.newFile?.name ?? hit.fileNewPath ?? hit.filePath,
          }}
          renderCustomHeader={(fileDiff) => (
            <TimelineSearchDiffHeader
              collapsed={collapsed}
              toggleCollapsed={toggleCollapsed}
              fileDiff={fileDiff}
              hit={hit}
            />
          )}
          options={{
            themeType: theme?.startsWith("dark-") ? "dark" : "light",
            diffStyle,
            overflow,
            collapsedContextThreshold: 0,
            lineHoverHighlight: "both",
            collapsed,
            unsafeCSS: `
              [data-background] {
                --diffs-light-bg: ${theme?.startsWith("dark-") ? "#000000" : "var(--secondary)"} !important;
                --diffs-dark-bg: ${theme?.startsWith("dark-") ? "#000000" : "var(--secondary)"} !important;
              }
            `,
          }}
        />
      )}
    </div>
  );
}
