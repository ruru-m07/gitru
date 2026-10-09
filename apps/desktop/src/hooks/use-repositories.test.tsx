import type { RepositoryInfo } from "@gitru/commands";
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/store/use-app-store";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../tests/mocks/tauri";
import { useRepositories } from "./use-repositories";

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: async () => ({
      get: async () => null,
      set: async () => {},
      save: async () => {},
    }),
  },
}));

const initialStore = useAppStore.getState();
afterEach(() => useAppStore.setState(initialStore, true));

function nativeRepository(origin: string | null): RepositoryInfo {
  return {
    id: "e52c9d93-0d4b-4bed-bc49-cd92973a9e60",
    name: "Local fixture",
    path: "/synthetic/local-fixture",
    origin,
    current_branch: null,
    ahead_behind: null,
    has_uncommitted_changes: true,
    last_updated: 42,
  };
}

describe("serialized native repository registration", () => {
  it.each([
    null,
    "https://example.invalid/team/project.git",
  ])("registers native metadata with origin %s and nullable branch information", async (origin) => {
    const repository = nativeRepository(origin);
    const list = mockTauriCommandResult("list_repositories", []);
    mockTauriCommandResult("add_local_git_repo", repository);
    const register = mockTauriCommand("add_repository", (payload) => {
      expect(payload).toEqual({ repo: repository });
      return repository;
    });
    const { result } = renderHook(() => useRepositories());
    await waitFor(() => expect(list).toHaveBeenCalledTimes(1));
    await act(async () => {
      expect(await result.current.addRepo(repository.path)).toEqual(repository);
    });
    expect(register).toHaveBeenCalledTimes(1);
    expect(useAppStore.getState().repositories).toEqual([repository]);
  });

  it("rejects malformed metadata before registration without changing saved repositories", async () => {
    const repository = nativeRepository(null);
    const list = mockTauriCommandResult("list_repositories", []);
    mockTauriCommandResult("add_local_git_repo", { ...repository, origin: 42 });
    const register = mockTauriCommandResult("add_repository", repository);
    const { result } = renderHook(() => useRepositories());
    await waitFor(() => expect(list).toHaveBeenCalledTimes(1));
    await act(async () => {
      await expect(result.current.addRepo(repository.path)).rejects.toThrow();
    });
    expect(register).not.toHaveBeenCalled();
    expect(useAppStore.getState().repositories).toEqual([]);
  });
});
