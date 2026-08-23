import type {
  ActiveRepositorySelectorState,
  RepoSelectOpenSelectorState,
} from "@/types/store";
import {
  createSessionScopedRepoKey,
  getActiveRuntimeId,
  getRepositoryById,
} from "./store-helpers";

export const selectActiveRepository = (
  state: ActiveRepositorySelectorState,
) => {
  const runtimeId = state.activeSessionId ?? state.activeTabId;
  const runtimeRepositoryId = runtimeId
    ? (state.sessionsById[runtimeId]?.repositoryId ?? null)
    : null;

  return (
    getRepositoryById(state.repositories, runtimeRepositoryId) ??
    state.selectedRepository
  );
};

export const selectActiveRepositoryPath = (
  state: ActiveRepositorySelectorState,
): string | null => {
  return selectActiveRepository(state)?.path ?? null;
};

export const selectActiveSessionRepoKey = (
  state: ActiveRepositorySelectorState,
) => {
  const runtimeId = getActiveRuntimeId(state);
  const repositoryPath = selectActiveRepositoryPath(state);

  return createSessionScopedRepoKey(runtimeId, repositoryPath);
};

export const selectActiveRepoSelectIsOpen = (
  state: RepoSelectOpenSelectorState,
): boolean => {
  const runtimeId = getActiveRuntimeId(state);

  if (!runtimeId) {
    return state.repoSelectIsOpen;
  }

  return Boolean(state.repoSelectIsOpenBySession?.[runtimeId]);
};
