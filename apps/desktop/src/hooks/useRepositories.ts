import {
  addLocalGitRepo,
  addRepository,
  cancelCloneRepository,
  cloneRepository,
  initRepository,
  listRepositories,
  type RepositoryInfo,
  refreshRepositoryInfo,
  removeRepository,
} from "@gitru/commands";
import { useEffect } from "react";
import { toast } from "sonner";
import { useAppStore } from "@/store/useAppStore";

export function useRepositories() {
  const repositories = useAppStore((state) => state.repositories);
  const setRepositories = useAppStore((state) => state.setRepositories);

  useEffect(() => {
    loadRepositories();
  }, []);

  async function loadRepositories() {
    try {
      const repos = await listRepositories({
        refreshStale: true,
      });

      setRepositories(repos);
    } catch (error) {
      console.error("Failed to load repositories:", error);
      toast.error("Failed to load repositories");
    }
  }

  async function addRepo(repoPath: string): Promise<RepositoryInfo | null> {
    try {
      const basicInfo = await addLocalGitRepo({ repoPath });

      if (!basicInfo) {
        return null;
      }

      const repo = await addRepository({ repo: basicInfo });
      setRepositories([...repositories, repo]);

      return repo;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(message);
      throw error;
    }
  }

  async function cloneRepo(params: {
    url: string;
    destinationPath: string;
    operationId: string;
  }): Promise<RepositoryInfo> {
    try {
      const repo = await cloneRepository(params);
      setRepositories([...repositories, repo]);
      return repo;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(message);
      throw error;
    }
  }

  async function cancelClone(operationId: string): Promise<boolean> {
    try {
      return await cancelCloneRepository({ operationId });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(message);
      throw error;
    }
  }

  async function initRepo(repoPath: string): Promise<RepositoryInfo> {
    try {
      const repo = await initRepository({ repoPath });
      setRepositories([...repositories, repo]);
      return repo;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(message);
      throw error;
    }
  }

  async function removeRepo(repoId: string) {
    try {
      await removeRepository({ repoId });
      setRepositories(repositories.filter((r) => r.id !== repoId));
      toast.success("Repository removed");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(message);
      throw error;
    }
  }

  async function refreshRepo(repoId: string): Promise<RepositoryInfo | null> {
    try {
      const updatedRepo = await refreshRepositoryInfo({ repoId });

      setRepositories(
        repositories.map((r) => (r.id === repoId ? updatedRepo : r)),
      );

      return updatedRepo;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`Failed to refresh repository: ${message}`);
      return null;
    }
  }

  return {
    repositories,
    loadRepositories,
    addRepo,
    cloneRepo,
    cancelClone,
    initRepo,
    removeRepo,
    refreshRepo,
  };
}
