"use client";

import { useAutocompleteFilter } from "@gitru/ui/components/autocomplete";
import * as React from "react";

export type CommandFilterFunction<TItem = unknown> = (
  item: TItem,
  query: string,
  contains: (text: string, query: string) => boolean,
) => boolean;

export function composeFilters<TItem>(
  ...filters: CommandFilterFunction<TItem>[]
): CommandFilterFunction<TItem> {
  return (item, query, contains) =>
    filters.some((filter) => filter(item, query, contains));
}

export function createLabelFilter<TItem>(
  getLabel: (item: TItem) => string | undefined,
): CommandFilterFunction<TItem> {
  return (item, query, contains) => {
    const label = getLabel(item);
    return label ? contains(label, query) : false;
  };
}

export function createKeywordFilter<TItem>(
  getKeywords: (item: TItem) => string[] | undefined,
): CommandFilterFunction<TItem> {
  return (item, query, contains) => {
    const keywords = getKeywords(item);
    return keywords
      ? keywords.some((keyword) => contains(keyword, query))
      : false;
  };
}

export function createValueFilter<TItem>(
  getValue: (item: TItem) => string | undefined,
): CommandFilterFunction<TItem> {
  return (item, query, contains) => {
    const value = getValue(item);
    return value ? contains(value, query) : false;
  };
}

export function useCommandFilter(
  options?: Parameters<typeof useAutocompleteFilter>[0],
) {
  return useAutocompleteFilter(options);
}

export function useCommandFilterFn<TItem>(
  filter: CommandFilterFunction<TItem>,
  options?: Parameters<typeof useAutocompleteFilter>[0],
) {
  const { contains } = useAutocompleteFilter(options);

  return React.useCallback(
    (itemValue: unknown, query: string) => {
      if (!query) return true;
      return filter(itemValue as TItem, query, contains);
    },
    [contains, filter],
  );
}
