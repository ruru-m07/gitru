import type { GitViewState, MainWindowView } from "@/types/store";
import type { AppStoreGet, AppStoreSet } from "./store-helpers";
import {
  createDefaultGitViewState,
  DEFAULT_STASH_STATUS_FILTERS,
  getTargetRepoKey,
} from "./store-helpers";

export const createGitViewSlice = (set: AppStoreSet, get: AppStoreGet) => ({
  mainWindowView: null as MainWindowView,

  setMainWindowView: (view: MainWindowView) =>
    set((state) =>
      state.mainWindowView === view ? state : { mainWindowView: view },
    ),

  gitViewByRepo: {},

  setGitViewStateForRepo: (
    partial: Partial<GitViewState>,
    repoPathArg?: string,
  ) => {
    const repoKey = getTargetRepoKey(get(), repoPathArg);
    if (!repoKey) {
      return;
    }

    set((state) => {
      const current =
        state.gitViewByRepo[repoKey] ?? createDefaultGitViewState();
      const nextFilters = partial.stashStatusFilters
        ? {
            ...DEFAULT_STASH_STATUS_FILTERS,
            ...current.stashStatusFilters,
            ...partial.stashStatusFilters,
          }
        : current.stashStatusFilters;

      const nextState: GitViewState = {
        ...current,
        ...partial,
        stashStatusFilters: nextFilters,
      };

      const hasChanged =
        current.leftPanelView !== nextState.leftPanelView ||
        current.changesTab !== nextState.changesTab ||
        current.stashViewMode !== nextState.stashViewMode ||
        current.selectedStashReference !== nextState.selectedStashReference ||
        current.selectedHistoryCommitHash !==
          nextState.selectedHistoryCommitHash ||
        current.stashStatusFilters.modified !==
          nextState.stashStatusFilters.modified ||
        current.stashStatusFilters.renamed !==
          nextState.stashStatusFilters.renamed ||
        current.stashStatusFilters.deleted !==
          nextState.stashStatusFilters.deleted ||
        current.stashStatusFilters.conflicted !==
          nextState.stashStatusFilters.conflicted ||
        current.stashStatusFilters.untracked !==
          nextState.stashStatusFilters.untracked;

      if (!hasChanged) {
        return state;
      }

      return {
        gitViewByRepo: {
          ...state.gitViewByRepo,
          [repoKey]: nextState,
        },
      };
    });
  },
});
