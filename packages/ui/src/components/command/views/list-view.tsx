"use client";

import {
  CommandCollection,
  CommandEmpty,
  CommandGroup,
  CommandGroupLabel,
  CommandItem,
  CommandList,
  CommandPanel,
  CommandSeparator,
  CommandShortcut,
} from "@gitru/ui/components/command";
import { cn } from "@gitru/ui/lib/utils";
import * as React from "react";
import { useCommandSearch } from "../manager.js";

export interface CommandListGroup<TItem> {
  id: string;
  label?: React.ReactNode;
  items: TItem[];
}

export interface CommandListViewEmptyContext {
  query: string;
}

export interface CommandListViewProps<TItem> {
  items?: TItem[];
  groups?: CommandListGroup<TItem>[];
  getItemKey?: (item: TItem) => React.Key;
  getItemValue?: (item: TItem) => unknown;
  getItemShortcut?: (item: TItem) => React.ReactNode;
  /** Filter items by query. Receives (itemValue, query) where itemValue = getItemValue(item). When not provided and query is non-empty, a default string-includes filter is used. Ignored when itemsArePreFiltered is true. */
  filter?: (itemValue: unknown, query: string) => boolean;
  /** When true, items are already filtered (e.g. from context.filteredCommandItems); skip local filtering so list and root Command stay in sync for keyboard/highlight. */
  itemsArePreFiltered?: boolean;
  onSelect?: (item: TItem) => void;
  renderItem?: (item: TItem, defaultItem: React.ReactNode) => React.ReactNode;
  renderItemContent?: (item: TItem) => React.ReactNode;
  renderGroupLabel?: (group: CommandListGroup<TItem>) => React.ReactNode;
  emptyState?: (context: CommandListViewEmptyContext) => React.ReactNode;
  showSeparators?: boolean;
  className?: string;
  listClassName?: string;
  emptyClassName?: string;
  groupClassName?: string;
  itemClassName?: string;
  shortcutClassName?: string;
}

const defaultFilter = (value: unknown, query: string): boolean =>
  String(value).toLowerCase().includes(query.toLowerCase());

function CommandListView<TItem>({
  items,
  groups,
  getItemKey,
  getItemValue,
  getItemShortcut,
  filter: filterProp,
  itemsArePreFiltered = false,
  onSelect,
  renderItem,
  renderItemContent,
  renderGroupLabel,
  emptyState,
  showSeparators = true,
  className,
  listClassName,
  emptyClassName,
  groupClassName,
  itemClassName,
  shortcutClassName,
}: CommandListViewProps<TItem>) {
  const { query } = useCommandSearch();
  const trimmedQuery = query.trim();

  const effectiveFilter = React.useMemo(() => {
    if (itemsArePreFiltered) return null;
    if (filterProp) return filterProp;
    if (trimmedQuery) return defaultFilter;
    return null;
  }, [itemsArePreFiltered, filterProp, trimmedQuery]);

  const filterItems = React.useCallback(
    (list: TItem[] | undefined): TItem[] => {
      if (!list?.length) return [];
      if (!effectiveFilter) return list;
      return list.filter((item) =>
        effectiveFilter(getItemValue?.(item) ?? item, query),
      );
    },
    [effectiveFilter, getItemValue, query],
  );

  const filteredItems = React.useMemo(
    () => filterItems(items),
    [filterItems, items],
  );
  const filteredGroups = React.useMemo(() => {
    if (!groups?.length) return undefined;
    return groups
      .map((g) => ({ ...g, items: filterItems(g.items) }))
      .filter((g) => g.items.length > 0);
  }, [groups, filterItems]);

  const hasGroups = Boolean(filteredGroups && filteredGroups.length > 0);
  const hasItems = Boolean(filteredItems && filteredItems.length > 0);
  const shouldShowEmpty = !hasGroups && !hasItems && Boolean(emptyState);

  const emptyContent = emptyState?.({ query });

  return (
    <CommandPanel className={className}>
      <CommandEmpty className={cn("not-empty:py-10", emptyClassName)}>
        {shouldShowEmpty ? emptyContent : null}
      </CommandEmpty>
      <CommandList className={listClassName}>
        {hasGroups ? (
          filteredGroups?.map((group, index) => (
            <React.Fragment key={group.id}>
              <CommandGroup className={groupClassName} items={group.items}>
                {group.label ? (
                  <CommandGroupLabel>
                    {renderGroupLabel ? renderGroupLabel(group) : group.label}
                  </CommandGroupLabel>
                ) : null}
                <CommandCollection>
                  {(item: TItem) => {
                    const key = getItemKey?.(item) ?? JSON.stringify(item);
                    const value = getItemValue?.(item) ?? item;
                    const shortcut = getItemShortcut?.(item);
                    const defaultContent = renderItemContent ? (
                      renderItemContent(item)
                    ) : (
                      <span className="flex-1">
                        {typeof item === "string" ? item : String(key)}
                      </span>
                    );

                    const defaultItem = (
                      <CommandItem
                        className={itemClassName}
                        key={key}
                        onClick={() => onSelect?.(item)}
                        value={value}
                      >
                        {defaultContent}
                        {shortcut ? (
                          <CommandShortcut className={shortcutClassName}>
                            {shortcut}
                          </CommandShortcut>
                        ) : null}
                      </CommandItem>
                    );

                    return renderItem
                      ? (renderItem(item, defaultItem) ?? defaultItem)
                      : defaultItem;
                  }}
                </CommandCollection>
              </CommandGroup>
              {showSeparators && index < (filteredGroups?.length ?? 0) - 1 ? (
                <CommandSeparator />
              ) : null}
            </React.Fragment>
          ))
        ) : filteredItems && filteredItems.length > 0 ? (
          <CommandGroup className={groupClassName} items={filteredItems}>
            <CommandCollection>
              {(item: TItem, index: number) => {
                const key = getItemKey?.(item) ?? JSON.stringify(item);
                const value = getItemValue?.(item) ?? item;
                const shortcut = getItemShortcut?.(item);
                const defaultContent = renderItemContent ? (
                  renderItemContent(item)
                ) : (
                  <span className="flex-1">
                    {typeof item === "string" ? item : String(key)}
                  </span>
                );

                const defaultItem = (
                  <CommandItem
                    className={itemClassName}
                    key={key}
                    onClick={() => onSelect?.(item)}
                    value={value}
                  >
                    {defaultContent}
                    {shortcut ? (
                      <CommandShortcut className={shortcutClassName}>
                        {shortcut}
                      </CommandShortcut>
                    ) : null}
                  </CommandItem>
                );

                return (
                  <React.Fragment key={key}>
                    {renderItem
                      ? (renderItem(item, defaultItem) ?? defaultItem)
                      : defaultItem}
                    {showSeparators && index < filteredItems.length - 1 ? (
                      <CommandSeparator />
                    ) : null}
                  </React.Fragment>
                );
              }}
            </CommandCollection>
          </CommandGroup>
        ) : null}
      </CommandList>
    </CommandPanel>
  );
}

export { CommandListView };
