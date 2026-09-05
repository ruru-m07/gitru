import type { Branch, GraphRow } from "@gitru/commands";
import {
  bridgeVerticalWheel,
  createMultiColumnList,
  type MultiColumnList,
} from "@gitru/dom-virtual";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import { ROW_H } from "./geometry";
import { mountGraphInteractions } from "./interactions";
import {
  createBranchSlot,
  createCommittersSlot,
  createHashSlot,
  createStatsSlot,
  createSummarySlot,
  createTimestampSlot,
} from "./paint/cells";
import { createLanesSlot, laneContentWidth } from "./paint/lanes";
import { HistoryGraphRowStore } from "./row-store";

function maxBy<T>(items: readonly T[], score: (item: T) => number) {
  return items.reduce<T | undefined>(
    (best, item) => (!best || score(item) > score(best) ? item : best),
    undefined,
  );
}

export type HistoryGraphControllerData = {
  rows: readonly GraphRow[];
  currentBranch: Branch | null;
  pushEnabled: boolean;
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
};

type HistoryGraphControllerOptions = {
  listRoot: HTMLElement;
  initialData: HistoryGraphControllerData;
  fetchNextPage: () => void;
};

export type HistoryGraphController = {
  update: (data: HistoryGraphControllerData) => void;
  dispose: () => void;
};

export function createHistoryGraphController(
  options: HistoryGraphControllerOptions,
): HistoryGraphController {
  const store = new HistoryGraphRowStore();
  let data = options.initialData;
  let list: MultiColumnList<(typeof store.rows)[number]>;
  let fetchRequested = false;

  store.sync(data.rows);

  list = createMultiColumnList({
    root: options.listRoot,
    rowHeight: ROW_H,
    overscan: 28,
    columns: [
      {
        id: "branches",
        width: "300px",
        className: "max-w-full min-w-0 overflow-hidden text-nowrap",
        createSlot: () =>
          createBranchSlot(
            () => data.currentBranch,
            () => data.pushEnabled,
          ),
      },
      {
        id: "lanes",
        width: "300px",
        className: "min-w-0",
        overflowX: "auto",
        contentWidth: () => laneContentWidth(store.maxLane),
        createSlot: () => createLanesSlot(() => store.maxLane),
      },
      {
        id: "summary",
        width: "1fr",
        className: "min-w-0",
        overflowX: "auto",
        createSlot: createSummarySlot,
      },
      {
        id: "committers",
        width: "fit-content(100px)",
        sizingItem: (items) =>
          maxBy(items, (item) => item.row.commit.authors.co_authors.length),
        createSlot: createCommittersSlot,
      },
      {
        id: "timestamp",
        width: "fit-content(100px)",
        sizingItem: (items) =>
          maxBy(
            items,
            (item) => timeAgoFromUnixSeconds(item.row.commit.timestamp).length,
          ),
        createSlot: createTimestampSlot,
      },
      {
        id: "commit-hash",
        width: "fit-content(100px)",
        sizingItem: (items) => items[0],
        createSlot: createHashSlot,
      },
      {
        id: "stats",
        width: "fit-content(100px)",
        sizingItem: (items) =>
          maxBy(items, (item) => {
            const stats = item.row.commit.stats;
            return (
              String(stats.files_changed).length +
              String(stats.insertions).length +
              String(stats.deletions).length
            );
          }),
        createSlot: createStatsSlot,
      },
    ],
    onVisibleRangeChange(range) {
      if (
        data.hasNextPage &&
        !data.isFetchingNextPage &&
        !fetchRequested &&
        range.end >= store.rows.length - 20
      ) {
        fetchRequested = true;
        options.fetchNextPage();
      }
    },
  });

  const lanesHost = options.listRoot.querySelector<HTMLElement>(
    '[data-virtual-column="lanes"]',
  );
  const removeWheelBridge = lanesHost
    ? bridgeVerticalWheel(lanesHost, list.scrollElement)
    : () => undefined;
  const removeInteractions = mountGraphInteractions(options.listRoot);
  const loadingIndicator = document.createElement("div");
  loadingIndicator.className =
    "pointer-events-none absolute inset-x-0 bottom-0 z-30 flex h-8 items-center justify-center";
  loadingIndicator.textContent = "Loading more...";
  options.listRoot
    .querySelector<HTMLElement>("[data-dom-virtual-viewport]")
    ?.appendChild(loadingIndicator);

  function update(nextData: HistoryGraphControllerData) {
    const previousRowCount = store.rows.length;
    data = nextData;
    store.sync(nextData.rows);
    if (!nextData.isFetchingNextPage || store.rows.length > previousRowCount) {
      fetchRequested = false;
    }
    list.setItems(store.rows);
    loadingIndicator.style.display = nextData.isFetchingNextPage
      ? "flex"
      : "none";
  }

  update(data);

  return {
    update,
    dispose() {
      removeWheelBridge();
      removeInteractions();
      list.dispose();
    },
  };
}
