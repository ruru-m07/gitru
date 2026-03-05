import type { AssetDiff } from "@gitru/commands";
import { Tabs, TabsList, TabsTab } from "@gitru/ui/components/tabs";
import { onion } from "@lucide/lab";
import {
  Blend,
  Columns2,
  Diff,
  Grip,
  Icon,
  Snail,
  SquareSplitHorizontal,
} from "lucide-react";
import { useEffect, useMemo } from "react";
import { useDiffViewerSettings } from "../useDiffViewSettingStore";
import { DifferenceView } from "./DifferenceView";
import { OnionSkinView } from "./OnionSkinView";
import { SwipeView } from "./SwipeView";
import { TwoUpView } from "./TwoUpView";
import { useImageDiffSizing } from "./useImageDiffSizing";

export const formatBytes = (bytes: number) => {
  const units = ["B", "KiB", "MiB", "GiB"];
  let value = bytes;
  let idx = 0;
  while (value >= 1024 && idx < units.length - 1) {
    value /= 1024;
    idx += 1;
  }
  return `${value.toFixed(idx === 0 ? 0 : 2)} ${units[idx]}`;
};

const getModeLabel = (mode: "twoUp" | "swipe" | "onionSkin" | "difference") => {
  if (mode === "twoUp") return "2-Up";
  if (mode === "swipe") return "Swipe";
  if (mode === "onionSkin") return "Onion Skin";
  return "Difference";
};

const getDifferenceProviderLabel = (
  provider: "worker" | "cssOnly" | "odiffNode",
) => {
  if (provider === "worker") return "Pixelmatch";
  if (provider === "odiffNode") return "ODiff";
  return "CSS blend";
};

export const ImageDiffViewer = ({ diff }: { diff: AssetDiff }) => {
  const { imageDiffMode, setImageDiffMode } = useDiffViewerSettings();
  const {
    containerRef,
    previewSize,
    beforeDimensions,
    afterDimensions,
    onBeforeImageLoad,
    onAfterImageLoad,
    resetDimensions,
  } = useImageDiffSizing();

  const differenceDiffProvider = useDiffViewerSettings(
    (state) => state.differenceDiffProvider,
  );
  const setDifferenceDiffProvider = useDiffViewerSettings(
    (state) => state.setDifferenceDiffProvider,
  );

  const beforeEntry = (diff.before ?? undefined) as
    | (typeof diff.before & { contents_base64?: string })
    | undefined;
  const afterEntry = (diff.after ?? undefined) as
    | (typeof diff.after & { contents_base64?: string })
    | undefined;

  const beforeUrl = useMemo(
    () =>
      beforeEntry?.contents_base64
        ? `data:${beforeEntry.mime};base64,${beforeEntry.contents_base64}`
        : undefined,
    [beforeEntry],
  );
  const afterUrl = useMemo(
    () =>
      afterEntry?.contents_base64
        ? `data:${afterEntry.mime};base64,${afterEntry.contents_base64}`
        : undefined,
    [afterEntry],
  );

  const hasTwoSidedDiff = Boolean(diff.before && diff.after);

  const reducedMotion =
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;

  const viewProps = {
    before: diff.before,
    after: diff.after,
    width: previewSize.width,
    height: previewSize.height,
    beforeUrl,
    afterUrl,
    beforeDimensions,
    afterDimensions,
    onBeforeImageLoad,
    onAfterImageLoad,
    reducedMotion,
  } as const;

  useEffect(() => {
    resetDimensions();
  }, [beforeUrl, afterUrl, resetDimensions]);

  return (
    <div className="p-2.5 border-b bg-background h-full flex flex-col">
      <div className="flex items-center justify-center mb-4 flex-col gap-4 select-none">
        <Tabs value={imageDiffMode} defaultValue="twoUp">
          <TabsList className={"w-200"}>
            {(
              [
                { icon: <Columns2 />, name: "twoUp" },
                { icon: <SquareSplitHorizontal />, name: "swipe" },
                { icon: <Icon iconNode={onion} />, name: "onionSkin" },
                { icon: <Diff />, name: "difference" },
              ] as const
            ).map((mode) => (
              <TabsTab
                value={mode.name}
                key={mode.name}
                onClick={() => setImageDiffMode(mode.name)}
                disabled={!hasTwoSidedDiff && mode.name !== "twoUp"}
              >
                {mode.icon}
                {getModeLabel(mode.name)}
              </TabsTab>
            ))}
          </TabsList>
        </Tabs>
        {imageDiffMode === "difference" && (
          <Tabs value={differenceDiffProvider} defaultValue="twoUp">
            <TabsList className={"w-200"}>
              {(
                [
                  { icon: <Blend />, name: "cssOnly" },
                  { icon: <Grip />, name: "worker" },
                  { icon: <Snail />, name: "odiffNode" },
                ] as const
              ).map((mode) => (
                <TabsTab
                  value={mode.name}
                  key={mode.name}
                  onClick={() => setDifferenceDiffProvider(mode.name)}
                >
                  {mode.icon}
                  {getDifferenceProviderLabel(mode.name)}
                </TabsTab>
              ))}
            </TabsList>
          </Tabs>
        )}
      </div>

      <div
        ref={containerRef}
        className="select-none w-full h-full flex items-center justify-center"
      >
        {!hasTwoSidedDiff || imageDiffMode === "twoUp" ? (
          <TwoUpView {...viewProps} />
        ) : null}
        {hasTwoSidedDiff && imageDiffMode === "swipe" ? (
          <SwipeView {...viewProps} />
        ) : null}
        {hasTwoSidedDiff && imageDiffMode === "onionSkin" ? (
          <OnionSkinView {...viewProps} />
        ) : null}
        {hasTwoSidedDiff && imageDiffMode === "difference" ? (
          <DifferenceView {...viewProps} />
        ) : null}
      </div>
    </div>
  );
};
