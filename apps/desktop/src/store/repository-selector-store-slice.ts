import { type RepositoryInfo } from "@gitru/commands";
import type { ExternalOpener, UpdateChannel } from "@/types/store";
import type { AppStoreSet } from "./store-helpers";
import { getActiveRuntimeId, getRepositoryById } from "./store-helpers";

export const createRepositorySelectorSlice = (set: AppStoreSet) => ({
  selectedRepository: null as RepositoryInfo | null,

  setSelectedRepository: async (repo: RepositoryInfo | null) => {
    set((state) => {
      const nextRepositoryId = repo?.id ?? null;
      const sameSelectedRepositoryId =
        (state.selectedRepository?.id ?? null) === nextRepositoryId;

      const activeRuntimeId = state.activeSessionId ?? state.activeTabId;
      const activeTabIndex = state.activeTabId
        ? state.tabs.findIndex((tab) => tab.id === state.activeTabId)
        : -1;

      let nextTabs = state.tabs;
      let nextSessionsById = state.sessionsById;
      let didMutate = false;

      if (
        activeTabIndex !== -1 &&
        state.tabs[activeTabIndex]?.repositoryId !== nextRepositoryId
      ) {
        nextTabs = [...state.tabs];
        nextTabs[activeTabIndex] = {
          ...nextTabs[activeTabIndex],
          repositoryId: nextRepositoryId,
          updatedAt: Date.now(),
        };
        didMutate = true;
      }

      if (activeRuntimeId) {
        const activeSession = state.sessionsById[activeRuntimeId];
        if (activeSession && activeSession.repositoryId !== nextRepositoryId) {
          nextSessionsById = {
            ...nextSessionsById,
            [activeRuntimeId]: {
              ...activeSession,
              repositoryId: nextRepositoryId,
              lifecycle: "active",
              frozenAt: null,
              updatedAt: Date.now(),
            },
          };
          didMutate = true;
        }
      }

      if (sameSelectedRepositoryId && !didMutate) {
        return state;
      }

      return {
        selectedRepository: repo,
        ...(didMutate
          ? {
              tabs: nextTabs,
              sessionsById: nextSessionsById,
            }
          : {}),
      };
    });
  },

  repositories: [] as RepositoryInfo[],

  setRepositories: (repos: RepositoryInfo[]) =>
    set((state) => {
      const runtimeId = state.activeSessionId ?? state.activeTabId;
      const activeRuntime = runtimeId ? state.sessionsById[runtimeId] : null;
      const nextSelectedRepository = getRepositoryById(
        repos,
        activeRuntime?.repositoryId ?? null,
      );

      if (
        state.repositories === repos &&
        (state.selectedRepository?.id ?? null) ===
          (nextSelectedRepository?.id ?? null)
      ) {
        return state;
      }

      return {
        repositories: repos,
        selectedRepository: nextSelectedRepository,
      };
    }),

  repoSelectIsOpen: false,
  repoSelectIsOpenBySession: {} as Record<string, boolean>,

  setRepoSelectIsOpen: (isOpen: boolean, sessionId?: string) =>
    set((state) => {
      const runtimeId = sessionId ?? getActiveRuntimeId(state);

      if (!runtimeId) {
        return state.repoSelectIsOpen === isOpen
          ? state
          : { repoSelectIsOpen: isOpen };
      }

      const currentValue = Boolean(state.repoSelectIsOpenBySession[runtimeId]);

      if (currentValue === isOpen && state.repoSelectIsOpen === isOpen) {
        return state;
      }

      return {
        repoSelectIsOpen: isOpen,
        repoSelectIsOpenBySession: {
          ...state.repoSelectIsOpenBySession,
          [runtimeId]: isOpen,
        },
      };
    }),

  optimisticRepositoryCard: null as { name: string; path: string } | null,

  setOptimisticRepositoryCard: (card: { name: string; path: string } | null) =>
    set({ optimisticRepositoryCard: card }),

  preferredExternalOpener: "vscode" as ExternalOpener,

  setPreferredExternalOpener: (opener: ExternalOpener) =>
    set({ preferredExternalOpener: opener }),

  updateChannel: "stable" as UpdateChannel,

  setUpdateChannel: (channel: UpdateChannel) => set({ updateChannel: channel }),
});
