import type * as React from "react";

export type CommandViewId = string;

export interface CommandViewStackItem<
  TId extends CommandViewId = CommandViewId,
  TProps = unknown,
> {
  id: TId;
  props: TProps;
  query: string;
}

export interface CommandNavigationOptions {
  query?: string;
  resetQuery?: boolean;
}

export interface CommandNavigation<TId extends CommandViewId = CommandViewId> {
  push: <TProps = unknown>(
    id: TId,
    props?: TProps,
    options?: CommandNavigationOptions,
  ) => void;
  replace: <TProps = unknown>(
    id: TId,
    props?: TProps,
    options?: CommandNavigationOptions,
  ) => void;
  back: () => void;
  reset: () => void;
  canGoBack: boolean;
}

export interface CommandViewContext<TProps = unknown> {
  id: CommandViewId;
  props: TProps;
  query: string;
  setQuery: (query: string) => void;
  navigate: CommandNavigation;
  close: () => void;
  isRoot: boolean;
  filter?: (itemValue: unknown, query: string) => boolean;
  /** When the panel computes filtered items (filter + getItemValue on view config), this is set so the view can pass the same list to CommandListView for keyboard/highlight sync. */
  filteredCommandItems?: unknown[];
}

export interface CommandInputConfig<TProps = unknown> {
  placeholder?: string;
  autoFocus?: boolean;
  render?: (context: CommandViewContext<TProps>) => React.ReactNode;
}

export interface CommandViewCommandConfig<TProps = unknown> {
  // items?: TProps;
  items?: (context: CommandViewContext<TProps>) => TProps[];
  /** Used by the panel to compute filtered items for keyboard nav. When provided, filteredCommandItems will be in context. */
  getItemValue?: (item: TProps) => string;
  filter?: (itemValue: TProps, query: string) => boolean;
  keepHighlight?: boolean;
  autoHighlight?: "always" | "never";
}

export interface CommandViewConfig<
  TId extends CommandViewId = CommandViewId,
  TProps = unknown,
> {
  id: TId;
  input?: CommandInputConfig<TProps>;
  header?: (context: CommandViewContext<TProps>) => React.ReactNode;
  command?: CommandViewCommandConfig<TProps>;
  render: (context: CommandViewContext<TProps>) => React.ReactNode;
  footer?: (context: CommandViewContext<TProps>) => React.ReactNode;
  suspenseFallback?: React.ReactNode;
  errorFallback?: (error: Error) => React.ReactNode;
  actions?: (context: CommandViewContext<TProps>) => React.ReactNode;
}

/** Relaxed constraint so arrays of configs with different TProps are accepted (variance). */
type CommandViewConfigArray = readonly CommandViewConfig<string, any>[];

export type CommandViewList<T extends CommandViewConfigArray> = T;

export type CommandViewIdFromList<T extends CommandViewConfigArray> =
  T[number]["id"];

export type CommandViewById<
  T extends CommandViewConfigArray,
  TId extends CommandViewIdFromList<T>,
> = Extract<T[number], { id: TId }>;

export interface CommandViewRegistry<T extends CommandViewConfigArray> {
  views: T;
  ids: CommandViewIdFromList<T>[];
  get: <TId extends CommandViewIdFromList<T>>(
    id: TId,
  ) => CommandViewById<T, TId> | undefined;
  has: (id: CommandViewIdFromList<T>) => boolean;
}

export function createCommandViewRegistry<
  const T extends CommandViewConfigArray,
>(views: T): CommandViewRegistry<T> {
  const entries = views.map((view) => [view.id, view] as const);
  const map = new Map<CommandViewId, CommandViewConfig>(entries);
  const ids = views.map((view) => view.id) as CommandViewIdFromList<T>[];

  return {
    views,
    ids,
    get: (id) => map.get(id) as CommandViewById<T, typeof id> | undefined,
    has: (id) => map.has(id),
  } as const;
}
