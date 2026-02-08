"use client";

import * as React from "react";

import type {
  CommandNavigation,
  CommandNavigationOptions,
  CommandViewId,
  CommandViewStackItem,
} from "./types.js";

interface CommandManagerState<TId extends CommandViewId = CommandViewId> {
  stack: CommandViewStackItem<TId, unknown>[];
  current: CommandViewStackItem<TId, unknown>;
  open: boolean;
  setOpen: React.Dispatch<React.SetStateAction<boolean>>;
  setQuery: (query: string) => void;
  push: CommandNavigation<TId>["push"];
  replace: CommandNavigation<TId>["replace"];
  back: () => void;
  reset: () => void;
  canGoBack: boolean;
}

const CommandManagerContext = React.createContext<CommandManagerState | null>(
  null,
);

function buildStackItem<TId extends CommandViewId, TProps>(
  id: TId,
  props: TProps,
  options?: CommandNavigationOptions,
  fallbackQuery = "",
): CommandViewStackItem<TId, TProps> {
  const resetQuery = options?.resetQuery ?? true;
  const query = options?.query ?? (resetQuery ? "" : fallbackQuery);

  return {
    id,
    props,
    query,
  };
}

interface CommandManagerProviderProps<TId extends CommandViewId> {
  children: React.ReactNode;
  initialViewId: TId;
  initialViewProps?: unknown;
}

function CommandManagerProvider<TId extends CommandViewId>({
  children,
  initialViewId,
  initialViewProps,
}: CommandManagerProviderProps<TId>) {
  const initialItem = React.useMemo<CommandViewStackItem<TId, unknown>>(
    () => ({
      id: initialViewId,
      props: initialViewProps ?? null,
      query: "",
    }),
    [initialViewId, initialViewProps],
  );

  const [stack, setStack] = React.useState<
    CommandViewStackItem<TId, unknown>[]
  >([initialItem]);
  const [open, setOpen] = React.useState(false);

  const current = stack[stack.length - 1] ?? initialItem;

  const setQuery = React.useCallback(
    (query: string) => {
      setStack((prev) => {
        const next = [...prev];
        const last = next[next.length - 1];
        if (!last) return prev;
        next[next.length - 1] = { ...last, query };
        return next;
      });
    },
    [setStack],
  );

  const push = React.useCallback<CommandNavigation<TId>["push"]>(
    (id: TId, props: unknown, options?: CommandNavigationOptions) => {
      setStack((prev) => {
        const currentQuery = prev[prev.length - 1]?.query ?? "";
        const item = buildStackItem(id, props ?? null, options, currentQuery);
        return [...prev, item];
      });
    },
    [],
  );

  const replace = React.useCallback<CommandNavigation<TId>["replace"]>(
    (id: TId, props: unknown, options?: CommandNavigationOptions) => {
      setStack((prev) => {
        const currentQuery = prev[prev.length - 1]?.query ?? "";
        const item = buildStackItem(id, props ?? null, options, currentQuery);
        const next = prev.slice(0, -1);
        next.push(item);
        return next.length ? next : [item];
      });
    },
    [],
  );

  const back = React.useCallback(() => {
    setStack((prev) => (prev.length > 1 ? prev.slice(0, -1) : prev));
  }, []);

  const reset = React.useCallback(() => {
    setStack([initialItem]);
  }, [initialItem]);

  const value = React.useMemo<CommandManagerState<TId>>(
    () => ({
      stack,
      current,
      open,
      setOpen,
      setQuery,
      push,
      replace,
      back,
      reset,
      canGoBack: stack.length > 1,
    }),
    [stack, current, open, setOpen, setQuery, push, replace, back, reset],
  );

  return (
    <CommandManagerContext.Provider
      value={value as unknown as CommandManagerState}
    >
      {children}
    </CommandManagerContext.Provider>
  );
}

function useCommandManager(): CommandManagerState {
  const context = React.useContext(CommandManagerContext);
  if (!context) {
    throw new Error(
      "useCommandManager must be used within CommandManagerProvider",
    );
  }
  return context;
}

function useCommandNavigation() {
  const { push, replace, back, reset, canGoBack, open, setOpen } =
    useCommandManager();

  return React.useMemo(
    () => ({
      push,
      replace,
      back,
      reset,
      canGoBack,
      open,
      setOpen,
    }),
    [push, replace, back, reset, canGoBack, open, setOpen],
  );
}

function useCommandSearch() {
  const { current, setQuery } = useCommandManager();
  return React.useMemo(
    () => ({
      query: current.query,
      setQuery,
    }),
    [current.query, setQuery],
  );
}

function useCommandCurrentView() {
  const { current, stack } = useCommandManager();
  return React.useMemo(() => ({ current, stack }), [current, stack]);
}

export {
  CommandManagerProvider,
  useCommandManager,
  useCommandNavigation,
  useCommandSearch,
  useCommandCurrentView,
};
