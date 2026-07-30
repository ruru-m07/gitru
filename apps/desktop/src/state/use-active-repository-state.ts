import { useMemo } from "react";
import { useTabContext } from "@/context/tab-context-provider";
import { useAppStore } from "@/store/use-app-store";
import { appState } from "./index";

export function useActiveRepositoryState() {
  const activeRepository = useAppStore((state) => {
    const runtimeId = state.activeSessionId ?? state.activeTabId;
    if (!runtimeId) {
      return null;
    }

    const repositoryId = state.sessionsById[runtimeId]?.repositoryId ?? null;
    if (!repositoryId) {
      return null;
    }

    return state.repositories.find((repo) => repo.id === repositoryId) ?? null;
  });
  const { contextId } = useTabContext();

  return useMemo(() => {
    if (!activeRepository?.path || !contextId) {
      return null;
    }

    return appState.repositories.for(activeRepository.path, contextId);
  }, [activeRepository?.path, contextId]);
}
