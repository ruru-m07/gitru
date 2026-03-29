import {
  createContext,
  type PropsWithChildren,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { repoContextRegistry } from "@/state/core/RepoContextRegistry";
import { useAppStore } from "@/store/useAppStore";

type TabContextValue = {
  scopeId: string;
  contextId: string | null;
  isInitializing: boolean;
  isContextReady: boolean;
};

const TabContext = createContext<TabContextValue>({
  scopeId: "main",
  contextId: null,
  isInitializing: false,
  isContextReady: false,
});

type TabContextProviderProps = PropsWithChildren<{
  scopeId?: string;
}>;

export function TabContextProvider({
  children,
  scopeId = "main",
}: TabContextProviderProps) {
  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const [contextId, setContextId] = useState<string | null>(
    repoContextRegistry.getActiveContextId(),
  );
  const [isInitializing, setIsInitializing] = useState(false);

  useEffect(() => {
    repoContextRegistry.setActiveScope(scopeId);

    const unsubscribe = repoContextRegistry.subscribe((nextContextId) => {
      setContextId(nextContextId);
    });

    return () => {
      unsubscribe();
      repoContextRegistry.clearActiveScope(scopeId);
    };
  }, [scopeId]);

  useEffect(() => {
    let cancelled = false;

    const syncScopeContext = async () => {
      const repoId = selectedRepository?.id;

      if (!repoId) {
        setIsInitializing(false);
        await repoContextRegistry.disposeScope(scopeId);
        return;
      }

      setIsInitializing(true);

      try {
        await repoContextRegistry.ensureScopeContext(scopeId, repoId);
      } finally {
        if (!cancelled) {
          setIsInitializing(false);
        }
      }
    };

    void syncScopeContext();

    return () => {
      cancelled = true;
    };
  }, [scopeId, selectedRepository?.id]);

  useEffect(() => {
    return () => {
      void repoContextRegistry.disposeAll();
    };
  }, []);

  const value = useMemo<TabContextValue>(
    () => ({
      scopeId,
      contextId,
      isInitializing,
      isContextReady: !!contextId && !isInitializing,
    }),
    [contextId, isInitializing, scopeId],
  );

  return <TabContext.Provider value={value}>{children}</TabContext.Provider>;
}

export function useTabContext() {
  return useContext(TabContext);
}
