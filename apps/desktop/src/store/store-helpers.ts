import { type RepositoryInfo } from "@gitru/commands";
import type { StoreApi } from "zustand";
import type {
  AppState,
  ChangesTab,
  FileSelectionIdentity,
  GitSidebarView,
  GitViewState,
  MainWindowView,
  RepoFileSelectionState,
  RepoKey,
  SelectionSource,
  SessionLifecycle,
  StashStatusFilterMap,
  StashViewMode,
  WorkspaceSessionSnapshot,
  WorkspaceSessionState,
  WorkspaceTab,
} from "@/types/store";

export const SESSION_REPO_KEY_SEPARATOR = "::";

export const isSessionScopedRepoKey = (repoKey: RepoKey) =>
  repoKey.includes(SESSION_REPO_KEY_SEPARATOR);

export const createSessionScopedRepoKey = (
  sessionId: string | null | undefined,
  repositoryPath: string | null | undefined,
): RepoKey | null => {
  if (!sessionId || !repositoryPath) {
    return null;
  }

  return `${sessionId}${SESSION_REPO_KEY_SEPARATOR}${repositoryPath}`;
};

export const DEFAULT_STASH_STATUS_FILTERS: StashStatusFilterMap = {
  modified: true,
  renamed: true,
  deleted: true,
  conflicted: true,
  untracked: true,
};

export const createDefaultGitViewState = (): GitViewState => ({
  leftPanelView: "changes",
  changesTab: "changes",
  stashViewMode: "branch",
  selectedStashReference: null,
  selectedHistoryCommitHash: null,
  stashStatusFilters: { ...DEFAULT_STASH_STATUS_FILTERS },
});

export const createDefaultRepoFileSelectionState =
  (): RepoFileSelectionState => ({
    worktree: null,
    stashByReference: {},
    historyByCommit: {},
  });

export const cloneSelectionIdentity = (
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

export const cloneRepoFileSelectionState = (
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

export const cloneGitViewState = (value: unknown): GitViewState => {
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

export const cloneSessionSnapshot = (
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

export const normalizeRuntimeSnapshot = (params: {
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

export const isSameSelectionIdentity = (
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

export const isSameSessionSnapshot = (
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

export const getSessionRepositoryPath = (
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

export const captureSessionSnapshot = (params: {
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

export const freezeSessionWithSnapshot = (params: {
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

export const restoreSessionUiState = (params: {
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

export const hydrateActiveSessionUiState = (state: {
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

export const normalizeSelection = (
  selection: FileSelectionIdentity,
): FileSelectionIdentity => ({
  ...selection,
  selectedAt: selection.selectedAt || Date.now(),
});

export const getRepositoryById = (
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

export const getActiveRuntimeRepositoryPath = (state: {
  selectedRepository: RepositoryInfo | null;
  activeSessionId: string | null;
  activeTabId: string | null;
  sessionsById: Record<string, WorkspaceSessionState>;
  repositories: RepositoryInfo[];
}): string | null => {
  const runtimeId = getActiveRuntimeId(state);
  const repositoryId = runtimeId
    ? (state.sessionsById[runtimeId]?.repositoryId ?? null)
    : null;

  const runtimeRepository = repositoryId
    ? getRepositoryById(state.repositories, repositoryId)
    : null;

  return runtimeRepository?.path ?? state.selectedRepository?.path ?? null;
};

export const getActiveRuntimeId = (state: {
  activeSessionId: string | null;
  activeTabId: string | null;
}) => state.activeSessionId ?? state.activeTabId;

export const getTargetRepoKey = (
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

export const DEFAULT_TAB_ID = "tab-main";
export const DEFAULT_TAB_ROUTE = "/app/git";

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

export const normalizeWorkspaceRoutePath = (
  routePath: string | null | undefined,
) => {
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

export const createWorkspaceTab = (payload?: {
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

export const createDefaultTab = () =>
  createWorkspaceTab({
    id: DEFAULT_TAB_ID,
    routePath: DEFAULT_TAB_ROUTE,
    title: "Git",
    repositoryId: null,
  });

export const createSessionFromTab = (
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

export type AppStoreSet = StoreApi<AppState>["setState"];
export type AppStoreGet = StoreApi<AppState>["getState"];
