import { type RepositoryInfo } from "@gitru/commands";

export type SelectionSource = "worktree" | "stash" | "history";

export type FileSelectionIdentity = {
  filePath: string;
  fileNewPath?: string;
  source: SelectionSource;
  stashReference?: string;
  historyCommitHash?: string;
  worktreeScope?: "staged" | "unstaged" | "conflicted";
  selectedAt: number;
};

export type RepoFileSelectionState = {
  worktree: FileSelectionIdentity | null;
  stashByReference: Record<string, FileSelectionIdentity | null>;
  historyByCommit: Record<string, FileSelectionIdentity | null>;
};

export type ExternalOpener =
  | "vscode"
  | "cursor"
  | "finder"
  | "terminal"
  | "ghostty";

export type WorkspaceTab = {
  id: string;
  title: string;
  routePath: string;
  repositoryId: string | null;
  createdAt: number;
  updatedAt: number;
};

export type SessionLifecycle = "active" | "frozen";

export type WorkspaceSessionSnapshot = {
  repositoryPath: string | null;
  mainWindowView: MainWindowView;
  fileSelection: RepoFileSelectionState;
  gitViewState: GitViewState;
  capturedAt: number;
};

export type WorkspaceSessionState = {
  id: string;
  repositoryId: string | null;
  routePath: string;
  title: string;
  lifecycle: SessionLifecycle;
  frozenAt: number | null;
  createdAt: number;
  updatedAt: number;
  snapshot: WorkspaceSessionSnapshot | null;
  snapshotVersion: number;
};

export type UpdateChannel = "stable" | "beta";
export type GitSidebarView = "changes" | "stash" | "history";
export type StashViewMode = "branch" | "all";
export type ChangesTab = "changes" | "history";

export type StashStatusFilterMap = Record<
  "modified" | "renamed" | "deleted" | "conflicted" | "untracked",
  boolean
>;

export type MainWindowView = "FileDiff" | "HistoryGraph" | null;

export type GitViewState = {
  leftPanelView: GitSidebarView;
  changesTab: ChangesTab;
  stashViewMode: StashViewMode;
  selectedStashReference: string | null;
  selectedHistoryCommitHash: string | null;
  stashStatusFilters: StashStatusFilterMap;
};

export type RepoKey = string;

// ! Main AppStore structure
export type AppState = {
  selectedRepository: RepositoryInfo | null;
  setSelectedRepository: (repo: RepositoryInfo | null) => void;

  tabs: WorkspaceTab[];
  activeTabId: string | null;
  sessionsById: Record<string, WorkspaceSessionState>;
  activeSessionId: string | null;
  ensureActiveTab: (payload?: {
    routePath?: string;
    repositoryId?: string | null;
    title?: string;
  }) => void;
  createTab: (payload?: {
    routePath?: string;
    repositoryId?: string | null;
    title?: string;
  }) => WorkspaceTab;
  activateTab: (tabId: string) => void;
  setEmbeddedRuntimeSession: (sessionId: string) => void;
  activateSession: (sessionId: string) => void;
  freezeSession: (sessionId?: string) => void;
  disposeSession: (sessionId: string) => void;
  captureActiveSessionSnapshot: () => void;
  closeTab: (tabId: string) => void;
  reorderTab: (tabId: string, targetIndex: number) => void;
  syncActiveTab: (payload: {
    routePath?: string;
    repositoryId?: string | null;
    title?: string;
  }) => void;
  syncTabMetadata: (
    tabId: string,
    payload: {
      routePath?: string;
      repositoryId?: string | null;
      title?: string;
      snapshot?: WorkspaceSessionSnapshot | null;
    },
  ) => void;

  repositories: RepositoryInfo[];
  setRepositories: (repos: RepositoryInfo[]) => void;

  repoSelectIsOpen: boolean;
  repoSelectIsOpenBySession: Record<string, boolean>;
  setRepoSelectIsOpen: (isOpen: boolean, sessionId?: string) => void;

  optimisticRepositoryCard: { name: string; path: string } | null;
  setOptimisticRepositoryCard: (
    card: { name: string; path: string } | null,
  ) => void;

  selectionByRepo: Record<RepoKey, RepoFileSelectionState>;

  setWorktreeSelectionForRepo: (
    selection: FileSelectionIdentity | null,
  ) => void;
  setStashSelectionForRepo: (
    stashReference: string,
    selection: FileSelectionIdentity | null,
  ) => void;
  setHistorySelectionForRepo: (
    commitHash: string,
    selection: FileSelectionIdentity | null,
  ) => void;
  clearSelectionForRepo: (repoKey: RepoKey) => void;
  clearWorktreeSelectionForRepo: (repoKey?: RepoKey) => void;
  clearStashSelectionForRepo: (
    stashReference: string,
    repoKey?: RepoKey,
  ) => void;
  clearHistorySelectionForRepo: (commitHash: string, repoKey?: RepoKey) => void;
  pruneStashSelectionsForRepo: (
    repoKey: RepoKey,
    activeStashReferences: string[],
  ) => void;

  mainWindowView: MainWindowView;
  setMainWindowView: (view: MainWindowView) => void;

  preferredExternalOpener: ExternalOpener;
  setPreferredExternalOpener: (opener: ExternalOpener) => void;

  updateChannel: UpdateChannel;
  setUpdateChannel: (channel: UpdateChannel) => void;

  gitViewByRepo: Record<RepoKey, GitViewState>;
  setGitViewStateForRepo: (
    partial: Partial<GitViewState>,
    repoPath?: string,
  ) => void;
};

export type ActiveRepositorySelectorState = {
  selectedRepository: RepositoryInfo | null;
  activeSessionId: string | null;
  activeTabId: string | null;
  sessionsById: Record<string, WorkspaceSessionState>;
  repositories: RepositoryInfo[];
};

export type RepoSelectOpenSelectorState = {
  repoSelectIsOpen: boolean;
  repoSelectIsOpenBySession?: Record<string, boolean>;
  activeSessionId: string | null;
  activeTabId: string | null;
};
