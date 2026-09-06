import { TAB_GAP_PX, TAB_MAX_WIDTH_PX } from "./constants";

type ResolveTabStripLayoutOptions = {
  containerWidth: number;
  controlsWidth: number;
  tabCount: number;
};

export type TabStripLayout = {
  railWidth: number;
  tabWidth: number;
};

export const resolveTabStripLayout = ({
  containerWidth,
  controlsWidth,
  tabCount,
}: ResolveTabStripLayoutOptions): TabStripLayout => {
  if (tabCount <= 0) {
    return { railWidth: 0, tabWidth: 0 };
  }

  const availableWidth = Math.max(
    0,
    containerWidth - controlsWidth - TAB_GAP_PX,
  );
  const totalTabGaps = TAB_GAP_PX * Math.max(0, tabCount - 1);
  const tabWidth = Math.min(
    TAB_MAX_WIDTH_PX,
    Math.max(0, (availableWidth - totalTabGaps) / tabCount),
  );

  return {
    railWidth: Math.min(availableWidth, tabWidth * tabCount + totalTabGaps),
    tabWidth,
  };
};
