export type PickaxeSearchOptions = {
  isRegex: boolean;
  matchCase: boolean;
  matchWholeWord: boolean;
};

export const DEFAULT_PICKAXE_SEARCH_OPTIONS: PickaxeSearchOptions = {
  isRegex: false,
  matchCase: false,
  matchWholeWord: false,
};