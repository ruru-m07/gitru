import type { RepositoryInfo } from "@gitru/commands";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createStore } from "zustand/vanilla";
import { createFileSelectionSlice } from "../src/store/file-selection-store-slice";
import { createGitViewSlice } from "../src/store/git-view-store-slice";
import { createRepositorySelectorSlice } from "../src/store/repository-selector-store-slice";
import { createSessionSlice } from "../src/store/session-store-slice";
import {
  createSessionScopedRepoKey,
  DEFAULT_TAB_ID,
} from "../src/store/store-helpers";
import type { AppState, FileSelectionIdentity } from "../src/types/store";

const createRepository = (id: string, path: string): RepositoryInfo => ({
  id,
  name: id,
  path,
  current_branch: "main",
  has_uncommitted_changes: false,
  last_updated: 0,
});

const createAppStore = () =>
  createStore<AppState>()((set, get) => ({
    ...createRepositorySelectorSlice(set),
    ...createSessionSlice(set, get),
    ...createFileSelectionSlice(set, get),
    ...createGitViewSlice(set, get),
  }));

const createWorktreeSelection = (
  filePath: string,
  selectedAt: number,
): FileSelectionIdentity => ({
  filePath,
  source: "worktree",
  worktreeScope: "unstaged",
  selectedAt,
});

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2030-01-01T00:00:00.000Z"));
});

afterEach(() => {
  vi.useRealTimers();
});

describe("application store repository selection", () => {
  test("keeps the selected repository, active tab, and active session aligned", () => {
    const store = createAppStore();
    const alpha = createRepository("alpha", "/repos/alpha");
    const beta = createRepository("beta", "/repos/beta");

    store.getState().setRepositories([alpha, beta]);
    store.getState().setSelectedRepository(alpha);

    let state = store.getState();
    expect(state.selectedRepository).toBe(alpha);
    expect(
      state.tabs.find((tab) => tab.id === state.activeTabId),
    ).toMatchObject({ repositoryId: alpha.id });
    expect(state.sessionsById[state.activeSessionId ?? ""]).toMatchObject({
      repositoryId: alpha.id,
      lifecycle: "active",
    });

    const refreshedAlpha = {
      ...alpha,
      name: "alpha-renamed",
      has_uncommitted_changes: true,
      last_updated: 1,
    };
    store.getState().setRepositories([refreshedAlpha, beta]);

    state = store.getState();
    expect(state.selectedRepository).toBe(refreshedAlpha);
    expect(state.sessionsById[state.activeSessionId ?? ""]?.repositoryId).toBe(
      alpha.id,
    );

    store.getState().setRepositories([beta]);

    state = store.getState();
    expect(state.selectedRepository).toBeNull();
    expect(
      state.tabs.find((tab) => tab.id === state.activeTabId),
    ).toMatchObject({ repositoryId: alpha.id });
    expect(state.sessionsById[state.activeSessionId ?? ""]?.repositoryId).toBe(
      alpha.id,
    );

    store.getState().setSelectedRepository(beta);

    state = store.getState();
    expect(state.selectedRepository).toBe(beta);
    expect(
      state.tabs.find((tab) => tab.id === state.activeTabId),
    ).toMatchObject({ repositoryId: beta.id });
    expect(state.sessionsById[state.activeSessionId ?? ""]?.repositoryId).toBe(
      beta.id,
    );
  });
});

describe("application store sessions", () => {
  test("isolates and restores UI state for two sessions on the same repository", () => {
    const store = createAppStore();
    const repository = createRepository("alpha", "/repos/alpha");

    store.getState().setRepositories([repository]);
    store.getState().setSelectedRepository(repository);
    store
      .getState()
      .setWorktreeSelectionForRepo(createWorktreeSelection("src/alpha.ts", 1));
    store.getState().setGitViewStateForRepo({
      leftPanelView: "stash",
      stashViewMode: "all",
      selectedStashReference: "stash@{0}",
    });
    store.getState().setMainWindowView("HistoryGraph");

    vi.advanceTimersByTime(1_000);
    const secondTab = store.getState().createTab({
      repositoryId: repository.id,
      routePath: "/app/git?workspace=second",
      title: "Second",
    });

    const firstRepoKey = createSessionScopedRepoKey(
      DEFAULT_TAB_ID,
      repository.path,
    );
    const secondRepoKey = createSessionScopedRepoKey(
      secondTab.id,
      repository.path,
    );

    let state = store.getState();
    expect(state.sessionsById[DEFAULT_TAB_ID]).toMatchObject({
      lifecycle: "frozen",
      frozenAt: Date.parse("2030-01-01T00:00:01.000Z"),
      snapshot: {
        repositoryPath: repository.path,
        mainWindowView: "HistoryGraph",
        capturedAt: Date.parse("2030-01-01T00:00:01.000Z"),
      },
    });
    expect(
      state.sessionsById[DEFAULT_TAB_ID]?.snapshot?.fileSelection.worktree,
    ).toMatchObject({ filePath: "src/alpha.ts" });
    expect(state.selectionByRepo[secondRepoKey ?? ""]?.worktree).toMatchObject({
      filePath: "src/alpha.ts",
    });

    store
      .getState()
      .setWorktreeSelectionForRepo(createWorktreeSelection("src/beta.ts", 2));
    store.getState().setGitViewStateForRepo({
      leftPanelView: "history",
      selectedHistoryCommitHash: "abc123",
    });
    store.getState().setMainWindowView("FileDiff");

    vi.advanceTimersByTime(1_000);
    store.getState().activateTab(DEFAULT_TAB_ID);

    state = store.getState();
    expect(state.activeSessionId).toBe(DEFAULT_TAB_ID);
    expect(state.mainWindowView).toBe("HistoryGraph");
    expect(state.selectionByRepo[firstRepoKey ?? ""]?.worktree).toMatchObject({
      filePath: "src/alpha.ts",
    });
    expect(state.gitViewByRepo[firstRepoKey ?? ""]).toMatchObject({
      leftPanelView: "stash",
      stashViewMode: "all",
      selectedStashReference: "stash@{0}",
    });
    expect(state.sessionsById[secondTab.id]).toMatchObject({
      lifecycle: "frozen",
      frozenAt: Date.parse("2030-01-01T00:00:02.000Z"),
      snapshot: {
        repositoryPath: repository.path,
        mainWindowView: "FileDiff",
        capturedAt: Date.parse("2030-01-01T00:00:02.000Z"),
      },
    });

    vi.advanceTimersByTime(1_000);
    store.getState().activateTab(secondTab.id);

    state = store.getState();
    expect(state.activeSessionId).toBe(secondTab.id);
    expect(state.mainWindowView).toBe("FileDiff");
    expect(state.selectionByRepo[secondRepoKey ?? ""]?.worktree).toMatchObject({
      filePath: "src/beta.ts",
    });
    expect(state.gitViewByRepo[secondRepoKey ?? ""]).toMatchObject({
      leftPanelView: "history",
      selectedHistoryCommitHash: "abc123",
    });
  });

  test("closing the active tab restores the fallback session repository and picker state", () => {
    const store = createAppStore();
    const alpha = createRepository("alpha", "/repos/alpha");
    const beta = createRepository("beta", "/repos/beta");

    store.getState().setRepositories([alpha, beta]);
    store.getState().setSelectedRepository(alpha);
    store.getState().setRepoSelectIsOpen(false);

    vi.advanceTimersByTime(1_000);
    const betaTab = store.getState().createTab({
      repositoryId: beta.id,
      routePath: "/app/git?repo=beta",
      title: "Beta",
    });
    store.getState().setRepoSelectIsOpen(true);

    let state = store.getState();
    expect(state.selectedRepository).toBe(beta);
    expect(state.repoSelectIsOpen).toBe(true);
    expect(state.repoSelectIsOpenBySession[betaTab.id]).toBe(true);

    vi.advanceTimersByTime(1_000);
    store.getState().closeTab(betaTab.id);

    state = store.getState();
    expect(state.activeTabId).toBe(DEFAULT_TAB_ID);
    expect(state.activeSessionId).toBe(DEFAULT_TAB_ID);
    expect(state.selectedRepository).toBe(alpha);
    expect(state.repoSelectIsOpen).toBe(false);
    expect(state.repoSelectIsOpenBySession).not.toHaveProperty(betaTab.id);
    expect(state.sessionsById).not.toHaveProperty(betaTab.id);
    expect(state.sessionsById[DEFAULT_TAB_ID]).toMatchObject({
      repositoryId: alpha.id,
      lifecycle: "active",
      frozenAt: null,
    });
  });
});
