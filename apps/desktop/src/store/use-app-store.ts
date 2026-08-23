import { create } from "zustand";
import {
  createJSONStorage,
  persist,
  subscribeWithSelector,
} from "zustand/middleware";
import type { AppState } from "@/types/store";
import { createFileSelectionSlice } from "./file-selection-store-slice";
import { createGitViewSlice } from "./git-view-store-slice";
import { createRepositorySelectorSlice } from "./repository-selector-store-slice";
import { createSessionSlice } from "./session-store-slice";
import { hydrateActiveSessionUiState } from "./store-helpers";
import { createTauriStorage } from "./tauri-store-adapter";

export {
  selectActiveRepoSelectIsOpen,
  selectActiveRepository,
  selectActiveRepositoryPath,
  selectActiveSessionRepoKey,
} from "./store-selectors";

export const useAppStore = create<AppState>()(
  subscribeWithSelector(
    persist(
      (set, get) => ({
        ...createRepositorySelectorSlice(set),
        ...createSessionSlice(set, get),
        ...createFileSelectionSlice(set, get),
        ...createGitViewSlice(set, get),
      }),
      {
        name: "app-data",
        storage: createJSONStorage(() => createTauriStorage()),
        version: 1,
        partialize: (state) => ({
          selectedRepository: state.selectedRepository,
          tabs: state.tabs,
          activeTabId: state.activeTabId,
          sessionsById: state.sessionsById,
          activeSessionId: state.activeSessionId,
          repositories: state.repositories,
          repoSelectIsOpen: state.repoSelectIsOpen,
          repoSelectIsOpenBySession: state.repoSelectIsOpenBySession,
          optimisticRepositoryCard: state.optimisticRepositoryCard,
          preferredExternalOpener: state.preferredExternalOpener,
          updateChannel: state.updateChannel,
          gitViewByRepo: state.gitViewByRepo,
        }),
        onRehydrateStorage: () => (_state, error) => {
          if (error) {
            return;
          }

          const currentState = useAppStore.getState();

          const hydrated = hydrateActiveSessionUiState(currentState);

          if (!hydrated) {
            return;
          }

          useAppStore.setState(hydrated);
        },
      },
    ),
  ),
);
