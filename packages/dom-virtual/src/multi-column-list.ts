import {
  computeScrollMetrics,
  logicalToPhysical,
  physicalToLogical,
  visibleRange,
} from "./scroll-metrics.js";
import type {
  ColumnSlot,
  MultiColumnList,
  MultiColumnListOptions,
  ScrollMetrics,
  VisibleRange,
} from "./types.js";

const DEFAULT_OVERSCAN = 24;

type MountedColumn<T> = {
  host: HTMLDivElement;
  layer: HTMLDivElement;
  slots: ColumnSlot<T>[];
  sizer?: ColumnSlot<T>;
  createSlot: MultiColumnListOptions<T>["columns"][number]["createSlot"];
  contentWidth?: MultiColumnListOptions<T>["columns"][number]["contentWidth"];
  sizingItem?: MultiColumnListOptions<T>["columns"][number]["sizingItem"];
};

export function createMultiColumnList<T>(
  options: MultiColumnListOptions<T>,
): MultiColumnList<T> {
  const { root, rowHeight, columns, maxPhysicalHeight, onVisibleRangeChange } =
    options;
  const overscan = Math.max(1, options.overscan ?? DEFAULT_OVERSCAN);
  let items: readonly T[] = [];
  let metrics: ScrollMetrics;
  let windowStart = -1;
  let poolSize = 0;
  let raf = 0;

  root.replaceChildren();
  Object.assign(root.style, {
    position: "relative",
    overflowY: "auto",
    overflowX: "hidden",
    minHeight: "0",
    contain: "layout paint size",
    overscrollBehaviorY: "contain",
    transform: "translateZ(0)",
    willChange: "transform",
  });

  const viewport = document.createElement("div");
  viewport.dataset.domVirtualViewport = "";
  viewport.setAttribute("role", "grid");
  Object.assign(viewport.style, {
    position: "sticky",
    top: "0",
    zIndex: "1",
    display: "grid",
    gridTemplateColumns: columns.map((column) => column.width).join(" "),
    width: "100%",
    overflow: "hidden",
    background: "var(--color-background)",
  });

  const spacer = document.createElement("div");
  spacer.dataset.domVirtualSpacer = "";
  spacer.setAttribute("aria-hidden", "true");
  spacer.style.pointerEvents = "none";

  const mountedColumns: MountedColumn<T>[] = columns.map((column) => {
    const host = document.createElement("div");
    host.dataset.virtualColumn = column.id;
    host.className = column.className ?? "";
    Object.assign(host.style, {
      position: "relative",
      minWidth: "0",
      height: "100%",
      overflowX: column.overflowX ?? "hidden",
      overflowY: "hidden",
    });

    const layer = document.createElement("div");
    Object.assign(layer.style, {
      position: "absolute",
      inset: "0",
      minWidth: "100%",
      height: "100%",
      willChange: "transform",
    });
    const sizer = column.sizingItem ? column.createSlot() : undefined;
    if (sizer) {
      Object.assign(sizer.element.style, {
        position: "relative",
        width: "max-content",
        height: "0",
        visibility: "hidden",
        pointerEvents: "none",
      });
      host.append(sizer.element);
    }
    host.appendChild(layer);
    viewport.appendChild(host);
    return {
      host,
      layer,
      slots: [],
      sizer,
      createSlot: column.createSlot,
      contentWidth: column.contentWidth,
      sizingItem: column.sizingItem,
    };
  });

  root.append(viewport, spacer);
  metrics = computeScrollMetrics(
    0,
    rowHeight,
    root.clientHeight,
    maxPhysicalHeight,
  );

  function disposeSlots(column: MountedColumn<T>) {
    for (const slot of column.slots) slot.dispose?.();
    column.slots = [];
    column.layer.replaceChildren();
  }

  function ensurePool() {
    const nextPoolSize =
      Math.ceil(metrics.viewportHeight / metrics.rowHeight) + overscan * 2 + 1;
    if (nextPoolSize === poolSize) return;

    poolSize = nextPoolSize;
    windowStart = -1;
    for (const column of mountedColumns) {
      disposeSlots(column);
      for (let index = 0; index < poolSize; index++) {
        const slot = column.createSlot();
        slot.element.style.position = "absolute";
        slot.element.style.insetInline = "0";
        slot.element.style.top = `${index * metrics.rowHeight}px`;
        slot.element.style.height = `${metrics.rowHeight}px`;
        slot.element.style.boxSizing = "border-box";
        column.layer.appendChild(slot.element);
        column.slots.push(slot);
      }
    }
  }

  function updateColumnWidths() {
    for (const column of mountedColumns) {
      const sizingItem = column.sizingItem?.(items);
      if (column.sizer && sizingItem !== undefined) {
        column.sizer.paint(sizingItem, -1);
      }
      const width = column.contentWidth?.(items);
      if (width !== undefined) {
        column.layer.style.width = `${Math.max(1, width)}px`;
      } else {
        column.layer.style.width = "100%";
      }
    }
  }

  function paintWindow(start: number) {
    windowStart = Math.max(
      0,
      Math.min(Math.max(0, items.length - poolSize), start),
    );

    for (const column of mountedColumns) {
      for (let slotIndex = 0; slotIndex < column.slots.length; slotIndex++) {
        const itemIndex = windowStart + slotIndex;
        const slot = column.slots[slotIndex];
        if (!slot) continue;
        if (itemIndex >= items.length) {
          slot.element.style.visibility = "hidden";
          slot.element.removeAttribute("data-index");
          continue;
        }
        const item = items[itemIndex];
        if (item === undefined) continue;
        slot.element.style.visibility = "";
        slot.element.dataset.index = String(itemIndex);
        slot.paint(item, itemIndex);
      }
    }
  }

  function emitVisibleRange(range: VisibleRange) {
    onVisibleRangeChange?.(range);
  }

  function update() {
    raf = 0;
    const logicalTop = physicalToLogical(root.scrollTop, metrics);
    const range = visibleRange(logicalTop, metrics);
    const lowerTrigger = windowStart + Math.floor(overscan / 2);
    const upperTrigger = windowStart + poolSize - Math.floor(overscan / 2);

    if (
      windowStart < 0 ||
      (windowStart > 0 && range.start < lowerTrigger) ||
      range.end > upperTrigger
    ) {
      paintWindow(Math.max(0, range.start - overscan));
    }

    const offset = logicalTop - windowStart * metrics.rowHeight;
    for (const column of mountedColumns) {
      column.layer.style.transform = `translate3d(0, ${-offset}px, 0)`;
    }
    emitVisibleRange(range);
  }

  function scheduleUpdate() {
    if (raf) return;
    raf = requestAnimationFrame(update);
  }

  function refresh(preserveLogicalTop = true) {
    const logicalTop = preserveLogicalTop
      ? physicalToLogical(root.scrollTop, metrics)
      : 0;
    metrics = computeScrollMetrics(
      items.length,
      rowHeight,
      root.clientHeight,
      maxPhysicalHeight,
    );
    viewport.style.height = `${metrics.viewportHeight}px`;
    spacer.style.height = `${metrics.physicalScrollRange}px`;
    ensurePool();
    updateColumnWidths();
    root.scrollTop = logicalToPhysical(logicalTop, metrics);
    paintWindow(
      Math.max(0, visibleRange(logicalTop, metrics).start - overscan),
    );
    update();
  }

  const resizeObserver = new ResizeObserver(() => refresh());
  resizeObserver.observe(root);
  root.addEventListener("scroll", scheduleUpdate, { passive: true });
  refresh(false);

  return {
    scrollElement: root,
    setItems(nextItems) {
      items = nextItems;
      refresh();
    },
    refresh,
    scrollToIndex(index, align = "start") {
      const clampedIndex = Math.max(0, Math.min(items.length - 1, index));
      let logicalTop = clampedIndex * metrics.rowHeight;
      if (align === "center") {
        logicalTop -= (metrics.viewportHeight - metrics.rowHeight) / 2;
      } else if (align === "end") {
        logicalTop -= metrics.viewportHeight - metrics.rowHeight;
      }
      root.scrollTop = logicalToPhysical(logicalTop, metrics);
      scheduleUpdate();
    },
    getVisibleRange() {
      return visibleRange(physicalToLogical(root.scrollTop, metrics), metrics);
    },
    getLogicalScrollTop() {
      return physicalToLogical(root.scrollTop, metrics);
    },
    dispose() {
      if (raf) cancelAnimationFrame(raf);
      resizeObserver.disconnect();
      root.removeEventListener("scroll", scheduleUpdate);
      for (const column of mountedColumns) {
        disposeSlots(column);
        column.sizer?.dispose?.();
      }
      viewport.remove();
      spacer.remove();
    },
  };
}
