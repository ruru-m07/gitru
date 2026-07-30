import { useAppStore } from "@/store/use-app-store";
import { getActiveRepoContextId } from "./core/repo-context-registry";
import { queryClient } from "./core/state-manager";
import { repositories } from "./domains/repository-manager";
import { RepositoryState } from "./domains/repository-state";

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
