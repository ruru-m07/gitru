"use client";

import {
  Command,
  CommandCreateHandle,
  CommandDialog,
  CommandDialogPopup,
  CommandDialogTrigger,
  CommandFooter,
  CommandInput,
} from "@gitru/ui/components/command";
import { cn } from "@gitru/ui/lib/utils";
import * as React from "react";
import {
  CommandManagerProvider,
  useCommandCurrentView,
  useCommandNavigation,
  useCommandSearch,
} from "./manager.js";
import { CommandViewRenderer } from "./resolver.js";
import type {
  CommandViewConfig,
  CommandViewContext,
  CommandViewRegistry,
} from "./types.js";

function resolveViewConfig<T extends readonly CommandViewConfig<string, any>[]>(
  registry: CommandViewRegistry<T>,
  viewId: string,
) {
  return registry.get(viewId as never) ?? registry.views[0];
}

interface CommandPanelRootProps<T extends readonly CommandViewConfig<string, any>[]> {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  views: CommandViewRegistry<T>;
  initialViewId: T[number]["id"];
  initialViewProps?: unknown;
  handle?: ReturnType<typeof CommandCreateHandle>;
  resetOnOpen?: boolean;
  className?: string;
  headerClassName?: string;
  panelClassName?: string;
  footerClassName?: string;
  inputClassName?: string;
}

function CommandPanelRoot<T extends readonly CommandViewConfig<string, any>[]>({
  open,
  onOpenChange,
  views,
  initialViewId,
  initialViewProps,
  handle,
  resetOnOpen = true,
  className,
  headerClassName,
  panelClassName,
  footerClassName,
  inputClassName,
}: CommandPanelRootProps<T>) {
  return (
    <CommandManagerProvider
      initialViewId={initialViewId}
      initialViewProps={initialViewProps}
    >
      <CommandPanelInner
        className={className}
        footerClassName={footerClassName}
        handle={handle}
        headerClassName={headerClassName}
        inputClassName={inputClassName}
        onOpenChange={onOpenChange}
        open={open}
        panelClassName={panelClassName}
        resetOnOpen={resetOnOpen}
        views={views}
      />
    </CommandManagerProvider>
  );
}

function CommandPanelInner<T extends readonly CommandViewConfig<string, any>[]>({
  open,
  onOpenChange,
  views,
  handle,
  resetOnOpen,
  className,
  headerClassName,
  panelClassName,
  footerClassName,
  inputClassName,
}: Omit<CommandPanelRootProps<T>, "initialViewId" | "initialViewProps">) {
  const { current } = useCommandCurrentView();
  const navigation = useCommandNavigation();
  const { query, setQuery } = useCommandSearch();
  const view = resolveViewConfig(views, current.id);

  const close = React.useCallback(() => onOpenChange(false), [onOpenChange]);

  // Base context without filteredCommandItems so commandConfig can be built without a cycle
  const baseContext = React.useMemo<CommandViewContext>(
    () => ({
      id: current.id,
      props: current.props,
      query,
      setQuery,
      navigate: navigation,
      close,
      isRoot: !navigation.canGoBack,
      filter: view?.command?.filter,
    }),
    [
      current.id,
      current.props,
      query,
      setQuery,
      navigation,
      close,
      view?.command?.filter,
    ],
  );

  // Build command config - allow view to provide items dynamically (use baseContext so no cycle)
  const commandConfig = React.useMemo(() => {
    const baseConfig = view?.command || {};
    if (typeof baseConfig.items === "function") {
      return {
        ...baseConfig,
        items: baseConfig.items(baseContext),
      };
    }
    return baseConfig;
  }, [view?.command, baseContext]);

  // Panel computes filtered items so the root Command and the list stay in sync (keyboard/highlight)
  const filteredCommandItems = React.useMemo(() => {
    const items = (commandConfig?.items as unknown[] | undefined) ?? [];
    const getValue =
      (commandConfig as { getItemValue?: (item: unknown) => unknown })
        ?.getItemValue ?? ((x: unknown) => x);
    const f = commandConfig?.filter;
    if (!f || !query.trim()) return items;
    return items.filter((item: unknown) => f(getValue(item), query));
  }, [commandConfig, query]);

  const context = React.useMemo<CommandViewContext>(
    () => ({
      ...baseContext,
      filteredCommandItems,
    }),
    [baseContext, filteredCommandItems],
  );

  const renderInput = React.useMemo(() => {
    if (view?.input?.render) {
      return view.input.render(context);
    }

    return (
      <CommandInput
        autoFocus={view?.input?.autoFocus ?? true}
        className={inputClassName}
        onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
          setQuery(event.target.value)
        }
        placeholder={view?.input?.placeholder}
        value={query}
      />
    );
  }, [view, context, inputClassName, setQuery, query]);

  const customHeader = view?.header?.(context);
  const actions = view?.actions?.(context);
  const footer = view?.footer?.(context);

  const handleOpenChange = React.useCallback(
    (nextOpen: boolean) => {
      onOpenChange(nextOpen);
    },
    [onOpenChange],
  );

  const prevOpenRef = React.useRef(open);
  React.useEffect(() => {
    if (resetOnOpen && open && !prevOpenRef.current) {
      navigation.reset();
      setQuery("");
    }
    prevOpenRef.current = open;
  }, [open, resetOnOpen, navigation, setQuery]);

  React.useEffect(() => {
    if (!open) return;

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (!navigation.canGoBack) return;

      event.preventDefault();
      event.stopPropagation();
      navigation.back();
    };

    document.addEventListener("keydown", handleEscape, true);
    return () => document.removeEventListener("keydown", handleEscape, true);
  }, [open, navigation]);

  // cmd + k to open
  React.useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "k" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        onOpenChange(true);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onOpenChange]);

  return (
    <CommandDialog handle={handle} onOpenChange={handleOpenChange} open={open}>
      <CommandDialogPopup className={cn(className, panelClassName)}>
        <Command
          autoHighlight={(commandConfig?.autoHighlight as any) ?? "always"}
          filter={commandConfig?.filter}
          filteredItems={(filteredCommandItems as any) ?? []}
          items={(commandConfig?.items as any) ?? []}
          keepHighlight={commandConfig?.keepHighlight ?? true}
          value={query}
          onValueChange={(value: string) => setQuery(value)}
        >
          {customHeader ? (
            <div className={cn(headerClassName)}>{customHeader}</div>
          ) : (
            <div
              className={cn(
                "relative flex items-center *:first:flex-1",
                headerClassName,
              )}
            >
              {renderInput}
              {actions ? <div className="me-2.5">{actions}</div> : null}
            </div>
          )}
          {view && <CommandViewRenderer context={context} view={view} />}
          {footer ? (
            <CommandFooter className={footerClassName}>{footer}</CommandFooter>
          ) : null}
        </Command>
      </CommandDialogPopup>
    </CommandDialog>
  );
}

export { CommandPanelRoot, CommandDialogTrigger };
