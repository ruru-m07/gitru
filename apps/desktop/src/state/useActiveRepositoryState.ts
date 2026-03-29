import { useMemo } from "react";
import { useTabContext } from "@/context/TabContextProvider";
import { useAppStore } from "@/store/useAppStore";
import { appState } from "./index";

export function useActiveRepositoryState() {
  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const { contextId } = useTabContext();

  return useMemo(() => {
    if (!selectedRepository?.path || !contextId) {
      return null;
    }

    return appState.repositories.for(selectedRepository.path, contextId);
  }, [selectedRepository?.path, contextId]);
}
