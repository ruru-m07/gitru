export type VisibleRange = {
  start: number;
  end: number;
};

export type ScrollMetrics = {
  count: number;
  rowHeight: number;
  viewportHeight: number;
  logicalHeight: number;
  logicalScrollRange: number;
  physicalScrollRange: number;
  scale: number;
};

export type ColumnSlot<T> = {
  element: HTMLElement;
  paint: (item: T, index: number) => void;
  dispose?: () => void;
};

export type VirtualColumn<T> = {
  id: string;
  width: string;
  className?: string;
  overflowX?: "auto" | "hidden";
  contentWidth?: (items: readonly T[]) => number | undefined;
  sizingItem?: (items: readonly T[]) => T | undefined;
  createSlot: () => ColumnSlot<T>;
};

export type MultiColumnListOptions<T> = {
  root: HTMLElement;
  rowHeight: number;
  columns: readonly VirtualColumn<T>[];
  overscan?: number;
  maxPhysicalHeight?: number;
  onVisibleRangeChange?: (range: VisibleRange) => void;
};

export type MultiColumnList<T> = {
  readonly scrollElement: HTMLElement;
  setItems: (items: readonly T[]) => void;
  refresh: () => void;
  scrollToIndex: (index: number, align?: "start" | "center" | "end") => void;
  getVisibleRange: () => VisibleRange;
  getLogicalScrollTop: () => number;
  dispose: () => void;
};
