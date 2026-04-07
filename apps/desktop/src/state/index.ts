import { useAppStore } from "@/store/useAppStore";
import { getActiveRepoContextId } from "./core/RepoContextRegistry";
import { queryClient } from "./core/StateManager";
import { repositories } from "./domains/RepositoryManager";
import { RepositoryState } from "./domains/RepositoryState";

class AppState {
  readonly repositories = repositories;

  get repository(): RepositoryState | null {
    const state = useAppStore.getState();
    const runtimeId = state.activeSessionId ?? state.activeTabId;
    const selectedRepository = runtimeId
      ? (() => {
          const repositoryId =
            state.sessionsById[runtimeId]?.repositoryId ?? null;
          if (!repositoryId) {
            return null;
          }

          return (
            state.repositories.find((repo) => repo.id === repositoryId) ?? null
          );
        })()
      : null;
    const contextId = getActiveRepoContextId();

    if (!selectedRepository?.path || !contextId) {
      return null;
    }

    return this.repositories.for(selectedRepository.path, contextId);
  }

  get queryClient() {
    return queryClient;
  }
}

export const appState = new AppState();
