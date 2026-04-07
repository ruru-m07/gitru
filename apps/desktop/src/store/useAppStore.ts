import { type RepositoryInfo } from "@gitru/commands";
import { toast } from "sonner";
import { create } from "zustand";
import {
  createJSONStorage,
  persist,
  subscribeWithSelector,
} from "zustand/middleware";
import type {
  ActiveRepositorySelectorState,
  AppState,
  ChangesTab,
  FileSelectionIdentity,
  GitSidebarView,
  GitViewState,
  MainWindowView,
  RepoFileSelectionState,
  RepoKey,
  RepoSelectOpenSelectorState,
  SelectionSource,
  SessionLifecycle,
  StashStatusFilterMap,
  StashViewMode,
  WorkspaceSessionSnapshot,
  WorkspaceSessionState,
  WorkspaceTab,
} from "@/types/store";
import { createTauriStorage } from "./tauriStoreAdapter";

const SESSION_REPO_KEY_SEPARATOR = "::";

const isSessionScopedRepoKey = (repoKey: RepoKey) =>
  repoKey.includes(SESSION_REPO_KEY_SEPARATOR);

const createSessionScopedRepoKey = (
  sessionId: string | null | undefined,
  repositoryPath: string | null | undefined,
): RepoKey | null => {
  if (!sessionId || !repositoryPath) {
    return null;
  }

  return `${sessionId}${SESSION_REPO_KEY_SEPARATOR}${repositoryPath}`;
};

const DEFAULT_STASH_STATUS_FILTERS: StashStatusFilterMap = {
  modified: true,
  renamed: true,
  deleted: true,
  conflicted: true,
  untracked: true,
};

const createDefaultGitViewState = (): GitViewState => ({
  leftPanelView: "changes",
  changesTab: "changes",
  stashViewMode: "branch",
  selectedStashReference: null,
  selectedHistoryCommitHash: null,
  stashStatusFilters: { ...DEFAULT_STASH_STATUS_FILTERS },
});

const createDefaultRepoFileSelectionState = (): RepoFileSelectionState => ({
  worktree: null,
  stashByReference: {},
  historyByCommit: {},
});

const cloneSelectionIdentity = (
  selection: unknown,
): FileSelectionIdentity | null => {
  if (!selection || typeof selection !== "object") {
    return null;
  }

  const sourceCandidate = (selection as Partial<FileSelectionIdentity>).source;
  const source: SelectionSource =
    sourceCandidate === "stash" ||
    sourceCandidate === "history" ||
    sourceCandidate === "worktree"
      ? sourceCandidate
      : "worktree";
  const filePath = (selection as Partial<FileSelectionIdentity>).filePath;

  if (typeof filePath !== "string" || filePath.length === 0) {
    return null;
  }

  const worktreeScopeCandidate = (selection as Partial<FileSelectionIdentity>)
    .worktreeScope;
  const selectedAtCandidate = (selection as Partial<FileSelectionIdentity>)
    .selectedAt;

  return {
    filePath,
    fileNewPath:
      typeof (selection as Partial<FileSelectionIdentity>).fileNewPath ===
      "string"
        ? (selection as Partial<FileSelectionIdentity>).fileNewPath
        : undefined,
    source,
    stashReference:
      typeof (selection as Partial<FileSelectionIdentity>).stashReference ===
      "string"
        ? (selection as Partial<FileSelectionIdentity>).stashReference
        : undefined,
    historyCommitHash:
      typeof (selection as Partial<FileSelectionIdentity>).historyCommitHash ===
      "string"
        ? (selection as Partial<FileSelectionIdentity>).historyCommitHash
        : undefined,
    worktreeScope:
      worktreeScopeCandidate === "staged" ||
      worktreeScopeCandidate === "unstaged" ||
      worktreeScopeCandidate === "conflicted"
        ? worktreeScopeCandidate
        : undefined,
    selectedAt:
      typeof selectedAtCandidate === "number"
        ? selectedAtCandidate
        : Date.now(),
  };
};

const cloneSelectionMap = (
  value: unknown,
): Record<string, FileSelectionIdentity | null> => {
  if (!value || typeof value !== "object") {
    return {};
  }

  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .filter(([key]) => typeof key === "string" && key.length > 0)
      .map(([key, selection]) => [key, cloneSelectionIdentity(selection)]),
  );
};

const cloneRepoFileSelectionState = (
  value: unknown,
): RepoFileSelectionState => {
  if (!value || typeof value !== "object") {
    return createDefaultRepoFileSelectionState();
  }

  const state = value as Partial<RepoFileSelectionState>;

  return {
    worktree: cloneSelectionIdentity(state.worktree),
    stashByReference: cloneSelectionMap(state.stashByReference),
    historyByCommit: cloneSelectionMap(state.historyByCommit),
  };
};

const cloneGitViewState = (value: unknown): GitViewState => {
  const state =
    value && typeof value === "object"
      ? (value as Partial<GitViewState>)
      : null;

  const leftPanelView: GitSidebarView =
    state?.leftPanelView === "stash" ||
    state?.leftPanelView === "history" ||
    state?.leftPanelView === "changes"
      ? state.leftPanelView
      : "changes";

  const changesTab: ChangesTab =
    state?.changesTab === "history" || state?.changesTab === "changes"
      ? state.changesTab
      : "changes";

  const stashViewMode: StashViewMode =
    state?.stashViewMode === "all" || state?.stashViewMode === "branch"
      ? state.stashViewMode
      : "branch";

  const stashStatusFilters = {
    ...DEFAULT_STASH_STATUS_FILTERS,
    ...(state?.stashStatusFilters ?? {}),
  };

  return {
    leftPanelView,
    changesTab,
    stashViewMode,
    selectedStashReference:
      typeof state?.selectedStashReference === "string"
        ? state.selectedStashReference
        : null,
    selectedHistoryCommitHash:
      typeof state?.selectedHistoryCommitHash === "string"
        ? state.selectedHistoryCommitHash
        : null,
    stashStatusFilters: {
      modified: stashStatusFilters.modified,
      renamed: stashStatusFilters.renamed,
      deleted: stashStatusFilters.deleted,
      conflicted: stashStatusFilters.conflicted,
      untracked: stashStatusFilters.untracked,
    },
  };
};

const cloneSessionSnapshot = (
  snapshot: WorkspaceSessionSnapshot | null,
): WorkspaceSessionSnapshot | null => {
  if (!snapshot) {
    return null;
  }

  return {
    repositoryPath: snapshot.repositoryPath,
    mainWindowView: snapshot.mainWindowView,
    fileSelection: cloneRepoFileSelectionState(snapshot.fileSelection),
    gitViewState: cloneGitViewState(snapshot.gitViewState),
    capturedAt: snapshot.capturedAt,
  };
};

const normalizeRuntimeSnapshot = (params: {
  snapshot: WorkspaceSessionSnapshot | null;
  repositoryPathFallback: string | null;
}): WorkspaceSessionSnapshot | null => {
  if (!params.snapshot) {
    return null;
  }

  return {
    repositoryPath:
      typeof params.snapshot.repositoryPath === "string"
        ? params.snapshot.repositoryPath
        : (params.snapshot.repositoryPath ?? params.repositoryPathFallback),
    mainWindowView:
      params.snapshot.mainWindowView === "FileDiff" ||
      params.snapshot.mainWindowView === "HistoryGraph"
        ? params.snapshot.mainWindowView
        : null,
    fileSelection: cloneRepoFileSelectionState(params.snapshot.fileSelection),
    gitViewState: cloneGitViewState(params.snapshot.gitViewState),
    capturedAt:
      typeof params.snapshot.capturedAt === "number"
        ? params.snapshot.capturedAt
        : Date.now(),
  };
};

const isSameSessionSnapshot = (
  left: WorkspaceSessionSnapshot | null,
  right: WorkspaceSessionSnapshot | null,
) => {
  if (!left && !right) {
    return true;
  }

  if (!left || !right) {
    return false;
  }

  return (
    left.repositoryPath === right.repositoryPath &&
    left.mainWindowView === right.mainWindowView &&
    isSameRepoFileSelectionState(left.fileSelection, right.fileSelection) &&
    isSameGitViewState(left.gitViewState, right.gitViewState)
  );
};

const isSameSelectionMap = (
  left: Record<string, FileSelectionIdentity | null>,
  right: Record<string, FileSelectionIdentity | null>,
) => {
  const leftKeys = Object.keys(left);
  const rightKeys = Object.keys(right);

  if (leftKeys.length !== rightKeys.length) {
    return false;
  }

  for (const key of leftKeys) {
    if (!(key in right)) {
      return false;
    }

    if (!isSameSelectionIdentity(left[key] ?? null, right[key] ?? null)) {
      return false;
    }
  }

  return true;
};

const isSameRepoFileSelectionState = (
  left: RepoFileSelectionState,
  right: RepoFileSelectionState,
) => {
  if (!isSameSelectionIdentity(left.worktree, right.worktree)) {
    return false;
  }

  if (!isSameSelectionMap(left.stashByReference, right.stashByReference)) {
    return false;
  }

  if (!isSameSelectionMap(left.historyByCommit, right.historyByCommit)) {
    return false;
  }

  return true;
};

const isSameGitViewState = (left: GitViewState, right: GitViewState) => {
  return (
    left.leftPanelView === right.leftPanelView &&
    left.changesTab === right.changesTab &&
    left.stashViewMode === right.stashViewMode &&
    left.selectedStashReference === right.selectedStashReference &&
    left.selectedHistoryCommitHash === right.selectedHistoryCommitHash &&
    left.stashStatusFilters.modified === right.stashStatusFilters.modified &&
    left.stashStatusFilters.renamed === right.stashStatusFilters.renamed &&
    left.stashStatusFilters.deleted === right.stashStatusFilters.deleted &&
    left.stashStatusFilters.conflicted ===
      right.stashStatusFilters.conflicted &&
    left.stashStatusFilters.untracked === right.stashStatusFilters.untracked
  );
};

const getSessionRepositoryPath = (
  repositories: RepositoryInfo[],
  session: WorkspaceSessionState | null | undefined,
): string | null => {
  if (!session) {
    return null;
  }

  if (session.repositoryId) {
    const repository = repositories.find(
      (item) => item.id === session.repositoryId,
    );
    if (repository?.path) {
      return repository.path;
    }
  }

  return session.snapshot?.repositoryPath ?? null;
};

const createDefaultSessionSnapshot = (
  repositoryPath: string | null,
  capturedAt: number,
): WorkspaceSessionSnapshot => ({
  repositoryPath,
  mainWindowView: null,
  fileSelection: createDefaultRepoFileSelectionState(),
  gitViewState: createDefaultGitViewState(),
  capturedAt,
});

const createSessionSnapshot = (params: {
  session: WorkspaceSessionState;
  repositories: RepositoryInfo[];
  selectionByRepo: Record<RepoKey, RepoFileSelectionState>;
  gitViewByRepo: Record<RepoKey, GitViewState>;
  mainWindowView: MainWindowView;
  capturedAt: number;
}): WorkspaceSessionSnapshot => {
  const repositoryPath = getSessionRepositoryPath(
    params.repositories,
    params.session,
  );
  const sessionRepoKey = createSessionScopedRepoKey(
    params.session.id,
    repositoryPath,
  );

  return {
    repositoryPath,
    mainWindowView: params.mainWindowView,
    fileSelection: sessionRepoKey
      ? cloneRepoFileSelectionState(params.selectionByRepo[sessionRepoKey])
      : createDefaultRepoFileSelectionState(),
    gitViewState: sessionRepoKey
      ? cloneGitViewState(params.gitViewByRepo[sessionRepoKey])
      : createDefaultGitViewState(),
    capturedAt: params.capturedAt,
  };
};

const captureSessionSnapshot = (params: {
  sessionsById: Record<string, WorkspaceSessionState>;
  sessionId: string;
  repositories: RepositoryInfo[];
  selectionByRepo: Record<RepoKey, RepoFileSelectionState>;
  gitViewByRepo: Record<RepoKey, GitViewState>;
  mainWindowView: MainWindowView;
  at: number;
}): Record<string, WorkspaceSessionState> => {
  const session = params.sessionsById[params.sessionId];

  if (!session) {
    return params.sessionsById;
  }

  const snapshot = createSessionSnapshot({
    session,
    repositories: params.repositories,
    selectionByRepo: params.selectionByRepo,
    gitViewByRepo: params.gitViewByRepo,
    mainWindowView: params.mainWindowView,
    capturedAt: params.at,
  });

  return {
    ...params.sessionsById,
    [params.sessionId]: {
      ...session,
      snapshot,
      updatedAt: Math.max(session.updatedAt, params.at),
    },
  };
};

const freezeSessionWithSnapshot = (params: {
  sessionsById: Record<string, WorkspaceSessionState>;
  sessionId: string;
  repositories: RepositoryInfo[];
  selectionByRepo: Record<RepoKey, RepoFileSelectionState>;
  gitViewByRepo: Record<RepoKey, GitViewState>;
  mainWindowView: MainWindowView;
  at: number;
}): Record<string, WorkspaceSessionState> => {
  const sessionsWithSnapshot = captureSessionSnapshot(params);
  const sessionWithSnapshot = sessionsWithSnapshot[params.sessionId];

  if (!sessionWithSnapshot) {
    return params.sessionsById;
  }

  return {
    ...sessionsWithSnapshot,
    [params.sessionId]: {
      ...sessionWithSnapshot,
      lifecycle: "frozen",
      frozenAt: params.at,
      updatedAt: params.at,
    },
  };
};

const restoreSessionUiState = (params: {
  session: WorkspaceSessionState | null;
  repositories: RepositoryInfo[];
  selectionByRepo: Record<RepoKey, RepoFileSelectionState>;
  gitViewByRepo: Record<RepoKey, GitViewState>;
  mainWindowView: MainWindowView;
}) => {
  const session = params.session;

  if (!session) {
    return {
      selectionByRepo: params.selectionByRepo,
      gitViewByRepo: params.gitViewByRepo,
      mainWindowView: params.mainWindowView,
    };
  }

  const repositoryPath = getSessionRepositoryPath(params.repositories, session);
  const sessionRepoKey = createSessionScopedRepoKey(session.id, repositoryPath);
  const fallbackSnapshot = createDefaultSessionSnapshot(
    repositoryPath,
    Date.now(),
  );
  const snapshot =
    session.snapshot && session.snapshot.repositoryPath === repositoryPath
      ? {
          ...session.snapshot,
          fileSelection: cloneRepoFileSelectionState(
            session.snapshot.fileSelection,
          ),
          gitViewState: cloneGitViewState(session.snapshot.gitViewState),
        }
      : fallbackSnapshot;

  let nextSelectionByRepo = params.selectionByRepo;
  let nextGitViewByRepo = params.gitViewByRepo;

  if (sessionRepoKey) {
    const currentSelection =
      params.selectionByRepo[sessionRepoKey] ??
      createDefaultRepoFileSelectionState();

    if (
      !isSameRepoFileSelectionState(currentSelection, snapshot.fileSelection)
    ) {
      nextSelectionByRepo = {
        ...params.selectionByRepo,
        [sessionRepoKey]: cloneRepoFileSelectionState(snapshot.fileSelection),
      };
    }

    const currentGitView =
      params.gitViewByRepo[sessionRepoKey] ?? createDefaultGitViewState();

    if (!isSameGitViewState(currentGitView, snapshot.gitViewState)) {
      nextGitViewByRepo = {
        ...params.gitViewByRepo,
        [sessionRepoKey]: cloneGitViewState(snapshot.gitViewState),
      };
    }
  }

  return {
    selectionByRepo: nextSelectionByRepo,
    gitViewByRepo: nextGitViewByRepo,
    mainWindowView: snapshot.mainWindowView,
  };
};

const hydrateActiveSessionUiState = (state: {
  activeSessionId: string | null;
  activeTabId: string | null;
  sessionsById: Record<string, WorkspaceSessionState>;
  repositories: RepositoryInfo[];
  selectionByRepo: Record<RepoKey, RepoFileSelectionState>;
  gitViewByRepo: Record<RepoKey, GitViewState>;
  mainWindowView: MainWindowView;
  selectedRepository: RepositoryInfo | null;
  repoSelectIsOpen: boolean;
  repoSelectIsOpenBySession: Record<string, boolean>;
}) => {
  const activeRuntimeId = state.activeSessionId ?? state.activeTabId;

  if (!activeRuntimeId) {
    return null;
  }

  const activeSession = state.sessionsById[activeRuntimeId] ?? null;

  if (!activeSession) {
    return null;
  }

  const restoredUiState = restoreSessionUiState({
    session: activeSession,
    repositories: state.repositories,
    selectionByRepo: state.selectionByRepo,
    gitViewByRepo: state.gitViewByRepo,
    mainWindowView: state.mainWindowView,
  });

  const nextSelectedRepository = getRepositoryById(
    state.repositories,
    activeSession.repositoryId,
  );

  const hasSelectedRepositoryChange =
    (state.selectedRepository?.id ?? null) !==
    (nextSelectedRepository?.id ?? null);
  const nextRepoSelectIsOpen = Boolean(
    state.repoSelectIsOpenBySession[activeRuntimeId],
  );
  const hasUiStateChange =
    restoredUiState.selectionByRepo !== state.selectionByRepo ||
    restoredUiState.gitViewByRepo !== state.gitViewByRepo ||
    restoredUiState.mainWindowView !== state.mainWindowView;
  const hasRepoSelectChange = state.repoSelectIsOpen !== nextRepoSelectIsOpen;

  if (
    !hasSelectedRepositoryChange &&
    !hasUiStateChange &&
    !hasRepoSelectChange
  ) {
    return null;
  }

  return {
    selectionByRepo: restoredUiState.selectionByRepo,
    gitViewByRepo: restoredUiState.gitViewByRepo,
    mainWindowView: restoredUiState.mainWindowView,
    selectedRepository: nextSelectedRepository,
    repoSelectIsOpen: nextRepoSelectIsOpen,
  };
};

const normalizeSelection = (
  selection: FileSelectionIdentity,
): FileSelectionIdentity => ({
  ...selection,
  selectedAt: selection.selectedAt || Date.now(),
});

const isSameSelectionIdentity = (
  left: FileSelectionIdentity | null,
  right: FileSelectionIdentity | null,
) => {
  if (!left && !right) return true;
  if (!left || !right) return false;

  return (
    left.filePath === right.filePath &&
    left.fileNewPath === right.fileNewPath &&
    left.source === right.source &&
    left.stashReference === right.stashReference &&
    left.historyCommitHash === right.historyCommitHash &&
    left.worktreeScope === right.worktreeScope
  );
};

const getRepositoryById = (
  repositories: RepositoryInfo[],
  repositoryId: string | null,
): RepositoryInfo | null => {
  if (!repositoryId) {
    return null;
  }

  return (
    repositories.find((repository) => repository.id === repositoryId) ?? null
  );
};

const getActiveRuntimeRepositoryPath = (state: {
  selectedRepository: RepositoryInfo | null;
  activeSessionId: string | null;
  activeTabId: string | null;
  sessionsById: Record<string, WorkspaceSessionState>;
  repositories: RepositoryInfo[];
}): string | null => {
  const runtimeId = state.activeSessionId ?? state.activeTabId;
  const repositoryId = runtimeId
    ? (state.sessionsById[runtimeId]?.repositoryId ?? null)
    : null;

  const runtimeRepository = repositoryId
    ? getRepositoryById(state.repositories, repositoryId)
    : null;

  return runtimeRepository?.path ?? state.selectedRepository?.path ?? null;
};

const getActiveRuntimeId = (state: {
  activeSessionId: string | null;
  activeTabId: string | null;
}) => state.activeSessionId ?? state.activeTabId;

export const selectActiveRepository = (
  state: ActiveRepositorySelectorState,
): RepositoryInfo | null => {
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
): RepoKey | null => {
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

const getTargetRepoKey = (
  state: {
    selectedRepository: RepositoryInfo | null;
    activeSessionId: string | null;
    activeTabId: string | null;
    sessionsById: Record<string, WorkspaceSessionState>;
    repositories: RepositoryInfo[];
  },
  repoPathArg?: string,
): RepoKey | null => {
  const runtimeId = getActiveRuntimeId(state);

  if (repoPathArg) {
    if (isSessionScopedRepoKey(repoPathArg)) {
      return repoPathArg;
    }

    return createSessionScopedRepoKey(runtimeId, repoPathArg) ?? repoPathArg;
  }

  const repositoryPath = getActiveRuntimeRepositoryPath(state);

  return createSessionScopedRepoKey(runtimeId, repositoryPath);
};

const DEFAULT_TAB_ID = "tab-main";
const DEFAULT_TAB_ROUTE = "/app/git";

const getRoutePathname = (routePath: string) => {
  try {
    const origin =
      typeof window !== "undefined"
        ? window.location.origin
        : "http://localhost";
    return new URL(routePath, origin).pathname;
  } catch {
    return routePath.split("?")[0].split("#")[0];
  }
};

const normalizeWorkspaceRoutePath = (routePath: string | null | undefined) => {
  if (!routePath || typeof routePath !== "string") {
    return DEFAULT_TAB_ROUTE;
  }

  const pathname = getRoutePathname(routePath);

  if (pathname === "/app" || pathname === "/app/") {
    return DEFAULT_TAB_ROUTE;
  }

  return routePath;
};

const getTabTitleFromRoute = (routePath: string) => {
  const pathname = getRoutePathname(routePath);

  if (pathname.startsWith("/app/")) {
    const segment = pathname.slice("/app/".length).split("/")[0];

    if (segment) {
      return segment.charAt(0).toUpperCase() + segment.slice(1);
    }
  }

  return "Workspace";
};

const generateTabId = () => {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `tab-${crypto.randomUUID()}`;
  }

  return `tab-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
};

const createWorkspaceTab = (payload?: {
  id?: string;
  routePath?: string;
  repositoryId?: string | null;
  title?: string;
}): WorkspaceTab => {
  const routePath = normalizeWorkspaceRoutePath(payload?.routePath);
  const now = Date.now();

  return {
    id: payload?.id ?? generateTabId(),
    title: payload?.title?.trim() || getTabTitleFromRoute(routePath),
    routePath,
    repositoryId: payload?.repositoryId ?? null,
    createdAt: now,
    updatedAt: now,
  };
};

const createDefaultTab = () =>
  createWorkspaceTab({
    id: DEFAULT_TAB_ID,
    routePath: DEFAULT_TAB_ROUTE,
    title: "Git",
    repositoryId: null,
  });

const createSessionFromTab = (
  tab: WorkspaceTab,
  lifecycle: SessionLifecycle,
): WorkspaceSessionState => ({
  id: tab.id,
  repositoryId: tab.repositoryId,
  routePath: tab.routePath,
  title: tab.title,
  lifecycle,
  frozenAt: lifecycle === "frozen" ? Date.now() : null,
  createdAt: tab.createdAt,
  updatedAt: tab.updatedAt,
  snapshot: null,
  snapshotVersion: 1,
});

export const useAppStore = create<AppState>()(
  subscribeWithSelector(
    persist(
      (set, get) => ({
        selectedRepository: null,
        setSelectedRepository: async (repo) => {
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
              if (
                activeSession &&
                activeSession.repositoryId !== nextRepositoryId
              ) {
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

        tabs: [createDefaultTab()],
        activeTabId: DEFAULT_TAB_ID,
        sessionsById: {
          [DEFAULT_TAB_ID]: createSessionFromTab(createDefaultTab(), "active"),
        },
        activeSessionId: DEFAULT_TAB_ID,
        ensureActiveTab: (payload) => {
          set((state) => {
            const hasActiveTab =
              !!state.activeTabId &&
              state.tabs.some((tab) => tab.id === state.activeTabId);

            const ensureSessionsForTabs = (
              tabs: WorkspaceTab[],
              activeId: string | null,
            ) => {
              const now = Date.now();
              const sessionsById = { ...state.sessionsById };

              for (const tab of tabs) {
                const lifecycle: SessionLifecycle =
                  tab.id === activeId ? "active" : "frozen";
                const existing = sessionsById[tab.id];

                if (!existing) {
                  sessionsById[tab.id] = createSessionFromTab(tab, lifecycle);
                  continue;
                }

                sessionsById[tab.id] = {
                  ...existing,
                  id: tab.id,
                  repositoryId: tab.repositoryId,
                  routePath: tab.routePath,
                  title: tab.title,
                  lifecycle,
                  frozenAt:
                    lifecycle === "frozen"
                      ? existing.lifecycle === "frozen"
                        ? existing.frozenAt
                        : now
                      : null,
                  updatedAt: Math.max(tab.updatedAt, existing.updatedAt),
                };
              }

              for (const sessionId of Object.keys(sessionsById)) {
                if (!tabs.some((tab) => tab.id === sessionId)) {
                  delete sessionsById[sessionId];
                }
              }

              return sessionsById;
            };

            if (state.tabs.length > 0 && hasActiveTab) {
              const nextSessionsById = ensureSessionsForTabs(
                state.tabs,
                state.activeTabId,
              );

              const activeRepositoryId =
                state.tabs.find((tab) => tab.id === state.activeTabId)
                  ?.repositoryId ?? null;
              const nextSelectedRepository = getRepositoryById(
                state.repositories,
                activeRepositoryId,
              );

              const hasSessionShapeChange =
                Object.keys(nextSessionsById).length !==
                  Object.keys(state.sessionsById).length ||
                Object.keys(nextSessionsById).some(
                  (sessionId) =>
                    state.sessionsById[sessionId] !==
                    nextSessionsById[sessionId],
                );

              const hasSelectedRepositoryChange =
                (state.selectedRepository?.id ?? null) !==
                (nextSelectedRepository?.id ?? null);

              if (
                !hasSessionShapeChange &&
                !hasSelectedRepositoryChange &&
                (state.activeSessionId ?? null) === (state.activeTabId ?? null)
              ) {
                return state;
              }

              return {
                sessionsById: nextSessionsById,
                activeSessionId: state.activeTabId,
                selectedRepository: nextSelectedRepository,
                repoSelectIsOpen: state.activeTabId
                  ? Boolean(state.repoSelectIsOpenBySession[state.activeTabId])
                  : false,
              };
            }

            if (state.tabs.length > 0) {
              const nextActiveTabId = state.tabs[0]?.id ?? null;
              const nextSessionsById = ensureSessionsForTabs(
                state.tabs,
                nextActiveTabId,
              );
              const activeRepositoryId =
                state.tabs.find((tab) => tab.id === nextActiveTabId)
                  ?.repositoryId ?? null;
              const nextSelectedRepository = getRepositoryById(
                state.repositories,
                activeRepositoryId,
              );

              return {
                activeTabId: nextActiveTabId,
                activeSessionId: nextActiveTabId,
                sessionsById: nextSessionsById,
                selectedRepository: nextSelectedRepository,
                repoSelectIsOpen: nextActiveTabId
                  ? Boolean(state.repoSelectIsOpenBySession[nextActiveTabId])
                  : false,
              };
            }

            const fallbackTab = createWorkspaceTab({
              routePath: payload?.routePath,
              repositoryId: payload?.repositoryId ?? null,
              title: payload?.title,
            });

            const fallbackSession = createSessionFromTab(fallbackTab, "active");
            const nextSelectedRepository = getRepositoryById(
              state.repositories,
              fallbackTab.repositoryId,
            );

            return {
              tabs: [fallbackTab],
              activeTabId: fallbackTab.id,
              sessionsById: {
                [fallbackSession.id]: fallbackSession,
              },
              activeSessionId: fallbackTab.id,
              selectedRepository: nextSelectedRepository,
              repoSelectIsOpen: Boolean(
                state.repoSelectIsOpenBySession[fallbackTab.id],
              ),
            };
          });
        },
        createTab: (payload) => {
          const newTab = createWorkspaceTab({
            routePath: payload?.routePath,
            repositoryId: payload?.repositoryId ?? null,
            title: payload?.title,
          });

          set((state) => {
            const now = Date.now();
            const previousActiveSessionId =
              state.activeSessionId ?? state.activeTabId;
            let nextSessionsById = {
              ...state.sessionsById,
            };

            if (previousActiveSessionId) {
              nextSessionsById = freezeSessionWithSnapshot({
                sessionsById: nextSessionsById,
                sessionId: previousActiveSessionId,
                repositories: state.repositories,
                selectionByRepo: state.selectionByRepo,
                gitViewByRepo: state.gitViewByRepo,
                mainWindowView: state.mainWindowView,
                at: now,
              });
            }

            const inheritedSnapshot = previousActiveSessionId
              ? cloneSessionSnapshot(
                  nextSessionsById[previousActiveSessionId]?.snapshot ?? null,
                )
              : null;

            nextSessionsById[newTab.id] = {
              ...createSessionFromTab(newTab, "active"),
              snapshot: inheritedSnapshot,
            };

            const restoredUiState = restoreSessionUiState({
              session: nextSessionsById[newTab.id],
              repositories: state.repositories,
              selectionByRepo: state.selectionByRepo,
              gitViewByRepo: state.gitViewByRepo,
              mainWindowView: state.mainWindowView,
            });

            const nextSelectedRepository = getRepositoryById(
              state.repositories,
              newTab.repositoryId,
            );

            return {
              tabs: [...state.tabs, newTab],
              activeTabId: newTab.id,
              activeSessionId: newTab.id,
              sessionsById: nextSessionsById,
              repoSelectIsOpen: Boolean(
                state.repoSelectIsOpenBySession[newTab.id],
              ),
              selectionByRepo: restoredUiState.selectionByRepo,
              gitViewByRepo: restoredUiState.gitViewByRepo,
              mainWindowView: restoredUiState.mainWindowView,
              selectedRepository: nextSelectedRepository,
            };
          });

          return newTab;
        },
        activateTab: (tabId) => {
          set((state) => {
            if (state.activeTabId === tabId) {
              return state;
            }

            const exists = state.tabs.some((tab) => tab.id === tabId);

            if (!exists) {
              return state;
            }

            const targetTab =
              state.tabs.find((tab) => tab.id === tabId) ?? null;
            if (!targetTab) {
              return state;
            }

            const now = Date.now();
            const previousActiveSessionId =
              state.activeSessionId ?? state.activeTabId;
            let nextSessionsById = {
              ...state.sessionsById,
            };

            if (previousActiveSessionId && previousActiveSessionId !== tabId) {
              nextSessionsById = freezeSessionWithSnapshot({
                sessionsById: nextSessionsById,
                sessionId: previousActiveSessionId,
                repositories: state.repositories,
                selectionByRepo: state.selectionByRepo,
                gitViewByRepo: state.gitViewByRepo,
                mainWindowView: state.mainWindowView,
                at: now,
              });
            }

            const nextTargetSession = nextSessionsById[tabId]
              ? {
                  ...nextSessionsById[tabId],
                  repositoryId: targetTab.repositoryId,
                  routePath: targetTab.routePath,
                  title: targetTab.title,
                  lifecycle: "active" as const,
                  frozenAt: null,
                  updatedAt: now,
                }
              : createSessionFromTab(targetTab, "active");

            nextSessionsById[tabId] = nextTargetSession;

            const restoredUiState = restoreSessionUiState({
              session: nextTargetSession,
              repositories: state.repositories,
              selectionByRepo: state.selectionByRepo,
              gitViewByRepo: state.gitViewByRepo,
              mainWindowView: state.mainWindowView,
            });

            const nextSelectedRepository = getRepositoryById(
              state.repositories,
              targetTab.repositoryId,
            );

            return {
              activeTabId: tabId,
              activeSessionId: tabId,
              sessionsById: nextSessionsById,
              repoSelectIsOpen: Boolean(state.repoSelectIsOpenBySession[tabId]),
              selectionByRepo: restoredUiState.selectionByRepo,
              gitViewByRepo: restoredUiState.gitViewByRepo,
              mainWindowView: restoredUiState.mainWindowView,
              selectedRepository: nextSelectedRepository,
            };
          });
        },
        setEmbeddedRuntimeSession: (sessionId) => {
          set((state) => {
            if (!sessionId) {
              return state;
            }

            const targetSession = state.sessionsById[sessionId] ?? null;
            const targetTab =
              state.tabs.find((tab) => tab.id === sessionId) ?? null;

            if (!targetSession && !targetTab) {
              return state;
            }

            const now = Date.now();
            const resolvedRepositoryId =
              targetSession?.repositoryId ?? targetTab?.repositoryId ?? null;
            const resolvedRoutePath =
              targetTab?.routePath ?? targetSession?.routePath;
            const resolvedTitle = targetTab?.title ?? targetSession?.title;

            const nextSession: WorkspaceSessionState = targetSession
              ? {
                  ...targetSession,
                  repositoryId: resolvedRepositoryId,
                  routePath: resolvedRoutePath ?? targetSession.routePath,
                  title: resolvedTitle ?? targetSession.title,
                  lifecycle: "active",
                  frozenAt: null,
                  updatedAt: Math.max(targetSession.updatedAt, now),
                }
              : createSessionFromTab(
                  targetTab ??
                    createWorkspaceTab({
                      id: sessionId,
                      routePath: DEFAULT_TAB_ROUTE,
                      title: "Git",
                      repositoryId: resolvedRepositoryId,
                    }),
                  "active",
                );

            const restoredUiState = restoreSessionUiState({
              session: nextSession,
              repositories: state.repositories,
              selectionByRepo: state.selectionByRepo,
              gitViewByRepo: state.gitViewByRepo,
              mainWindowView: state.mainWindowView,
            });

            const nextSelectedRepository = getRepositoryById(
              state.repositories,
              resolvedRepositoryId,
            );
            const nextRepoSelectIsOpen = Boolean(
              state.repoSelectIsOpenBySession[sessionId],
            );
            const hasPointerChange =
              state.activeSessionId !== sessionId ||
              (targetTab ? state.activeTabId !== sessionId : false);
            const hasSelectedRepositoryChange =
              (state.selectedRepository?.id ?? null) !==
              (nextSelectedRepository?.id ?? null);
            const hasUiStateChange =
              restoredUiState.selectionByRepo !== state.selectionByRepo ||
              restoredUiState.gitViewByRepo !== state.gitViewByRepo ||
              restoredUiState.mainWindowView !== state.mainWindowView;
            const hasRepoSelectChange =
              state.repoSelectIsOpen !== nextRepoSelectIsOpen;
            const hasSessionWrite =
              !targetSession || state.sessionsById[sessionId] !== nextSession;

            if (
              !hasPointerChange &&
              !hasSelectedRepositoryChange &&
              !hasUiStateChange &&
              !hasRepoSelectChange &&
              !hasSessionWrite
            ) {
              return state;
            }

            return {
              activeSessionId: sessionId,
              activeTabId: targetTab ? sessionId : state.activeTabId,
              sessionsById: {
                ...state.sessionsById,
                [sessionId]: nextSession,
              },
              selectedRepository: nextSelectedRepository,
              repoSelectIsOpen: nextRepoSelectIsOpen,
              selectionByRepo: restoredUiState.selectionByRepo,
              gitViewByRepo: restoredUiState.gitViewByRepo,
              mainWindowView: restoredUiState.mainWindowView,
            };
          });
        },
        activateSession: (sessionId) => {
          get().activateTab(sessionId);
        },
        freezeSession: (sessionId) => {
          set((state) => {
            const targetSessionId =
              sessionId ?? state.activeSessionId ?? state.activeTabId;
            if (!targetSessionId) {
              return state;
            }

            const session = state.sessionsById[targetSessionId];
            if (!session || session.lifecycle === "frozen") {
              return state;
            }

            const nextSessionsById = freezeSessionWithSnapshot({
              sessionsById: state.sessionsById,
              sessionId: targetSessionId,
              repositories: state.repositories,
              selectionByRepo: state.selectionByRepo,
              gitViewByRepo: state.gitViewByRepo,
              mainWindowView: state.mainWindowView,
              at: Date.now(),
            });

            return {
              sessionsById: nextSessionsById,
            };
          });
        },
        disposeSession: (sessionId) => {
          set((state) => {
            if (!state.sessionsById[sessionId]) {
              return state;
            }

            const nextSessionsById = {
              ...state.sessionsById,
            };
            delete nextSessionsById[sessionId];
            const nextRepoSelectIsOpenBySession = {
              ...state.repoSelectIsOpenBySession,
            };
            delete nextRepoSelectIsOpenBySession[sessionId];

            const nextActiveSessionId =
              state.activeSessionId === sessionId
                ? state.activeTabId
                : state.activeSessionId;

            return {
              sessionsById: nextSessionsById,
              activeSessionId: nextActiveSessionId,
              repoSelectIsOpenBySession: nextRepoSelectIsOpenBySession,
              repoSelectIsOpen: nextActiveSessionId
                ? Boolean(nextRepoSelectIsOpenBySession[nextActiveSessionId])
                : false,
            };
          });
        },
        captureActiveSessionSnapshot: () => {
          set((state) => {
            const targetSessionId = state.activeSessionId ?? state.activeTabId;

            if (!targetSessionId || !state.sessionsById[targetSessionId]) {
              return state;
            }

            return {
              sessionsById: captureSessionSnapshot({
                sessionsById: state.sessionsById,
                sessionId: targetSessionId,
                repositories: state.repositories,
                selectionByRepo: state.selectionByRepo,
                gitViewByRepo: state.gitViewByRepo,
                mainWindowView: state.mainWindowView,
                at: Date.now(),
              }),
            };
          });
        },
        closeTab: (tabId) => {
          set((state) => {
            if (state.tabs.length <= 1) {
              return state;
            }

            const closingIndex = state.tabs.findIndex(
              (tab) => tab.id === tabId,
            );

            if (closingIndex === -1) {
              return state;
            }

            const nextTabs = state.tabs.filter((tab) => tab.id !== tabId);
            const isClosingActiveTab = state.activeTabId === tabId;
            let nextActiveTabId = state.activeTabId;
            let nextActiveSessionId = state.activeSessionId;

            const nextSessionsById = {
              ...state.sessionsById,
            };
            delete nextSessionsById[tabId];
            const nextRepoSelectIsOpenBySession = {
              ...state.repoSelectIsOpenBySession,
            };
            delete nextRepoSelectIsOpenBySession[tabId];

            if (isClosingActiveTab) {
              const fallbackIndex = Math.max(0, closingIndex - 1);
              nextActiveTabId =
                nextTabs[fallbackIndex]?.id ?? nextTabs[0]?.id ?? null;
              nextActiveSessionId = nextActiveTabId;
            } else if (
              nextActiveTabId &&
              !nextTabs.some((tab) => tab.id === nextActiveTabId)
            ) {
              nextActiveTabId = nextTabs[0]?.id ?? null;
              nextActiveSessionId = nextActiveTabId;
            }

            if (nextActiveSessionId && nextSessionsById[nextActiveSessionId]) {
              nextSessionsById[nextActiveSessionId] = {
                ...nextSessionsById[nextActiveSessionId],
                lifecycle: "active",
                frozenAt: null,
                updatedAt: Date.now(),
              };
            }

            const nextActiveTab =
              nextTabs.find((tab) => tab.id === nextActiveTabId) ?? null;
            const nextSelectedRepository = isClosingActiveTab
              ? getRepositoryById(
                  state.repositories,
                  nextActiveTab?.repositoryId ?? null,
                )
              : state.selectedRepository;

            const restoredUiState = restoreSessionUiState({
              session:
                isClosingActiveTab && nextActiveSessionId
                  ? (nextSessionsById[nextActiveSessionId] ?? null)
                  : null,
              repositories: state.repositories,
              selectionByRepo: state.selectionByRepo,
              gitViewByRepo: state.gitViewByRepo,
              mainWindowView: state.mainWindowView,
            });

            return {
              tabs: nextTabs,
              activeTabId: nextActiveTabId,
              activeSessionId: nextActiveSessionId,
              sessionsById: nextSessionsById,
              repoSelectIsOpenBySession: nextRepoSelectIsOpenBySession,
              repoSelectIsOpen: nextActiveSessionId
                ? Boolean(nextRepoSelectIsOpenBySession[nextActiveSessionId])
                : false,
              selectionByRepo: restoredUiState.selectionByRepo,
              gitViewByRepo: restoredUiState.gitViewByRepo,
              mainWindowView: restoredUiState.mainWindowView,
              selectedRepository: nextSelectedRepository,
            };
          });
        },
        reorderTab: (tabId, targetIndex) => {
          set((state) => {
            if (state.tabs.length <= 1) {
              return state;
            }

            const sourceIndex = state.tabs.findIndex((tab) => tab.id === tabId);

            if (sourceIndex === -1) {
              return state;
            }

            const clampedTargetIndex = Math.max(
              0,
              Math.min(targetIndex, state.tabs.length - 1),
            );

            if (clampedTargetIndex === sourceIndex) {
              return state;
            }

            const nextTabs = [...state.tabs];
            const [movedTab] = nextTabs.splice(sourceIndex, 1);

            if (!movedTab) {
              return state;
            }

            nextTabs.splice(clampedTargetIndex, 0, movedTab);

            return {
              tabs: nextTabs,
            };
          });
        },
        syncActiveTab: (payload) => {
          const activeTabId = get().activeTabId;

          if (!activeTabId) {
            return;
          }

          get().syncTabMetadata(activeTabId, payload);
        },
        syncTabMetadata: (tabId, payload) => {
          set((state) => {
            const activeIndex = state.tabs.findIndex((tab) => tab.id === tabId);

            if (activeIndex === -1) {
              return state;
            }

            const activeTab = state.tabs[activeIndex];
            const nextRoutePath = normalizeWorkspaceRoutePath(
              payload.routePath ?? activeTab.routePath,
            );
            const nextRepositoryId =
              payload.repositoryId === undefined
                ? activeTab.repositoryId
                : payload.repositoryId;
            const nextTitle = payload.title?.trim() || activeTab.title;
            const targetRepositoryPath =
              getRepositoryById(state.repositories, nextRepositoryId)?.path ??
              null;
            const hasSnapshotPayload = payload.snapshot !== undefined;
            const normalizedSnapshot = hasSnapshotPayload
              ? normalizeRuntimeSnapshot({
                  snapshot: payload.snapshot ?? null,
                  repositoryPathFallback: targetRepositoryPath,
                })
              : null;

            const didMetadataChange =
              nextRoutePath !== activeTab.routePath ||
              nextRepositoryId !== activeTab.repositoryId ||
              nextTitle !== activeTab.title;

            const nextSessionsById = {
              ...state.sessionsById,
            };
            const activeSession = nextSessionsById[tabId];
            const nextSnapshot = hasSnapshotPayload
              ? normalizedSnapshot
              : (activeSession?.snapshot ?? null);
            const didSnapshotChange = hasSnapshotPayload
              ? !isSameSessionSnapshot(
                  activeSession?.snapshot ?? null,
                  nextSnapshot,
                )
              : false;

            if (!didMetadataChange && !didSnapshotChange) {
              return state;
            }

            const now = Date.now();

            const nextTabs = [...state.tabs];
            nextTabs[activeIndex] = {
              ...activeTab,
              routePath: nextRoutePath,
              repositoryId: nextRepositoryId,
              title: nextTitle,
              updatedAt: now,
            };

            if (activeSession) {
              const activeRuntimeId =
                state.activeSessionId ?? state.activeTabId;
              const isTargetActiveSession = activeRuntimeId === tabId;

              nextSessionsById[tabId] = {
                ...activeSession,
                routePath: nextRoutePath,
                repositoryId: nextRepositoryId,
                title: nextTitle,
                lifecycle: isTargetActiveSession
                  ? "active"
                  : activeSession.lifecycle,
                frozenAt: isTargetActiveSession ? null : activeSession.frozenAt,
                snapshot: nextSnapshot,
                updatedAt: now,
              };
            } else {
              const activeRuntimeId =
                state.activeSessionId ?? state.activeTabId;
              const isTargetActiveSession = activeRuntimeId === tabId;

              nextSessionsById[tabId] = {
                ...createSessionFromTab(
                  nextTabs[activeIndex],
                  isTargetActiveSession ? "active" : "frozen",
                ),
                snapshot: nextSnapshot,
                frozenAt: isTargetActiveSession ? null : now,
                updatedAt: now,
              };
            }

            const isActiveTab = state.activeTabId === tabId;
            const nextSelectedRepository = isActiveTab
              ? getRepositoryById(state.repositories, nextRepositoryId)
              : state.selectedRepository;

            return {
              tabs: nextTabs,
              sessionsById: nextSessionsById,
              ...(isActiveTab
                ? {
                    selectedRepository: nextSelectedRepository,
                  }
                : {}),
            };
          });
        },

        repositories: [],
        setRepositories: (repos) =>
          set((state) => {
            const runtimeId = state.activeSessionId ?? state.activeTabId;
            const activeRuntime = runtimeId
              ? state.sessionsById[runtimeId]
              : null;
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
        repoSelectIsOpenBySession: {},
        setRepoSelectIsOpen: (isOpen, sessionId) =>
          set((state) => {
            const runtimeId = sessionId ?? getActiveRuntimeId(state);

            if (!runtimeId) {
              return state.repoSelectIsOpen === isOpen
                ? state
                : { repoSelectIsOpen: isOpen };
            }

            const currentValue = Boolean(
              state.repoSelectIsOpenBySession[runtimeId],
            );

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

        optimisticRepositoryCard: null,
        setOptimisticRepositoryCard: (card) =>
          set({ optimisticRepositoryCard: card }),

        selectionByRepo: {},

        setWorktreeSelectionForRepo: (selection) => {
          const repoKey = getTargetRepoKey(get());

          if (!repoKey) {
            toast.error("No repository selected");
            return;
          }

          set((state) => {
            const current =
              state.selectionByRepo[repoKey] ??
              createDefaultRepoFileSelectionState();
            const nextWorktree = selection
              ? normalizeSelection({
                  ...selection,
                  source: "worktree",
                  stashReference: undefined,
                  historyCommitHash: undefined,
                })
              : null;

            if (isSameSelectionIdentity(current.worktree, nextWorktree)) {
              return state;
            }

            return {
              selectionByRepo: {
                ...state.selectionByRepo,
                [repoKey]: {
                  ...current,
                  worktree: nextWorktree,
                },
              },
            };
          });

          if (selection) {
            get().setMainWindowView("FileDiff");
          }
        },

        setStashSelectionForRepo: (stashReference, selection) => {
          const repoKey = getTargetRepoKey(get());

          if (!repoKey) {
            toast.error("No repository selected");
            return;
          }

          if (!stashReference) {
            return;
          }

          set((state) => {
            const current =
              state.selectionByRepo[repoKey] ??
              createDefaultRepoFileSelectionState();
            const nextStashByReference = { ...current.stashByReference };
            const previousSelection =
              nextStashByReference[stashReference] ?? null;
            const nextSelection = selection
              ? normalizeSelection({
                  ...selection,
                  source: "stash",
                  stashReference,
                  historyCommitHash: undefined,
                })
              : null;

            if (isSameSelectionIdentity(previousSelection, nextSelection)) {
              return state;
            }

            nextStashByReference[stashReference] = nextSelection;

            return {
              selectionByRepo: {
                ...state.selectionByRepo,
                [repoKey]: {
                  ...current,
                  stashByReference: nextStashByReference,
                },
              },
            };
          });

          if (selection) {
            get().setMainWindowView("FileDiff");
          }
        },

        setHistorySelectionForRepo: (commitHash, selection) => {
          const repoKey = getTargetRepoKey(get());

          if (!repoKey) {
            toast.error("No repository selected");
            return;
          }

          if (!commitHash) {
            return;
          }

          set((state) => {
            const current =
              state.selectionByRepo[repoKey] ??
              createDefaultRepoFileSelectionState();
            const nextHistoryByCommit = { ...current.historyByCommit };
            const previousSelection = nextHistoryByCommit[commitHash] ?? null;
            const nextSelection = selection
              ? normalizeSelection({
                  ...selection,
                  source: "history",
                  stashReference: undefined,
                  historyCommitHash: commitHash,
                })
              : null;

            if (isSameSelectionIdentity(previousSelection, nextSelection)) {
              return state;
            }

            nextHistoryByCommit[commitHash] = nextSelection;

            return {
              selectionByRepo: {
                ...state.selectionByRepo,
                [repoKey]: {
                  ...current,
                  historyByCommit: nextHistoryByCommit,
                },
              },
            };
          });

          if (selection) {
            get().setMainWindowView("FileDiff");
          }
        },

        clearSelectionForRepo: (repoKey) =>
          set((state) => {
            const next = { ...state.selectionByRepo };
            delete next[repoKey];
            return { selectionByRepo: next };
          }),

        clearWorktreeSelectionForRepo: (repoPathArg) => {
          const repoKey = getTargetRepoKey(get(), repoPathArg);
          if (!repoKey) return;

          set((state) => {
            const current =
              state.selectionByRepo[repoKey] ??
              createDefaultRepoFileSelectionState();

            return {
              selectionByRepo: {
                ...state.selectionByRepo,
                [repoKey]: {
                  ...current,
                  worktree: null,
                },
              },
            };
          });
        },

        clearStashSelectionForRepo: (stashReference, repoPathArg) => {
          const repoKey = getTargetRepoKey(get(), repoPathArg);
          if (!repoKey || !stashReference) return;

          set((state) => {
            const current =
              state.selectionByRepo[repoKey] ??
              createDefaultRepoFileSelectionState();

            return {
              selectionByRepo: {
                ...state.selectionByRepo,
                [repoKey]: {
                  ...current,
                  stashByReference: {
                    ...current.stashByReference,
                    [stashReference]: null,
                  },
                },
              },
            };
          });
        },

        clearHistorySelectionForRepo: (commitHash, repoPathArg) => {
          const repoKey = getTargetRepoKey(get(), repoPathArg);
          if (!repoKey || !commitHash) return;

          set((state) => {
            const current =
              state.selectionByRepo[repoKey] ??
              createDefaultRepoFileSelectionState();

            return {
              selectionByRepo: {
                ...state.selectionByRepo,
                [repoKey]: {
                  ...current,
                  historyByCommit: {
                    ...current.historyByCommit,
                    [commitHash]: null,
                  },
                },
              },
            };
          });
        },

        pruneStashSelectionsForRepo: (repoKey, activeStashReferences) => {
          const targetRepoKey = getTargetRepoKey(get(), repoKey) ?? repoKey;

          set((state) => {
            const current = state.selectionByRepo[targetRepoKey];
            if (!current) return state;

            const activeSet = new Set(activeStashReferences);
            const nextStashByReference = Object.fromEntries(
              Object.entries(current.stashByReference).filter(([reference]) =>
                activeSet.has(reference),
              ),
            );

            return {
              selectionByRepo: {
                ...state.selectionByRepo,
                [targetRepoKey]: {
                  ...current,
                  stashByReference: nextStashByReference,
                },
              },
            };
          });
        },

        mainWindowView: null,
        setMainWindowView: (view) =>
          set((state) =>
            state.mainWindowView === view ? state : { mainWindowView: view },
          ),

        preferredExternalOpener: "vscode",
        setPreferredExternalOpener: (opener) =>
          set({ preferredExternalOpener: opener }),

        updateChannel: "stable",
        setUpdateChannel: (channel) => set({ updateChannel: channel }),

        gitViewByRepo: {},
        setGitViewStateForRepo: (partial, repoPathArg) => {
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
              current.selectedStashReference !==
                nextState.selectedStashReference ||
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
