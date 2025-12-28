import { QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  commitById,
  currentBranch,
  getDiff,
  getStatus,
  gitAdd,
  gitDiscard,
  gitRemove,
  lastCommit,
  listBranch,
  repositoryOrigin,
} from "@/tauri";
import { StateDomain } from "../core/StateManager";

class DiffState extends StateDomain {
  private readonly baseKey: readonly string[];

  constructor(
    protected queryClient: QueryClient,
    private repositoryPath: string,
  ) {
    super(queryClient);
    this.baseKey = ["repository", "diff"] as const;
  }

  async get(filePath: string) {
    const queryKey = [...this.baseKey, filePath];

    const data = await getDiff({
      repoPath: this.repositoryPath,
      filePath: filePath,
    });

    this.queryClient.setQueryData(queryKey, data);

    return data;
  }

  getQueryKey(filePath: string) {
    return [...this.baseKey, filePath];
  }

  async invalidate(filePath?: string) {
    const key = filePath ? [...this.baseKey, filePath] : [...this.baseKey];
    await this.queryClient.invalidateQueries({ queryKey: key });
  }

  async invalidateAll() {
    await this.queryClient.invalidateQueries({ queryKey: [...this.baseKey] });
  }
}

class StatusState extends StateDomain {
  private readonly baseKey: readonly string[];

  constructor(
    protected queryClient: QueryClient,
    private repositoryPath: string,
  ) {
    super(queryClient);
    this.baseKey = ["repository", "status"] as const;
  }

  async get() {
    await this.queryClient.cancelQueries({ queryKey: [...this.baseKey] });

    const data = await getStatus({
      repoPath: this.repositoryPath,
    });

    this.queryClient.setQueryData([...this.baseKey], data);

    return data;
  }

  // For React hooks
  get queryKey() {
    return [...this.baseKey];
  }

  // Get cached data without fetching
  getCached() {
    return this.queryClient.getQueryData([...this.baseKey]);
  }

  async invalidate() {
    await this.queryClient.invalidateQueries({ queryKey: [...this.baseKey] });
  }
}

class BranchState extends StateDomain {
  private readonly baseKey: readonly string[];

  constructor(
    protected queryClient: QueryClient,
    private repositoryPath: string,
    private getParent: () => RepositoryState,
  ) {
    super(queryClient);
    this.baseKey = ["repository", "branches"] as const;
  }

  async list() {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "list"],
    });

    const data = await listBranch({
      repoPath: this.repositoryPath,
    });

    this.queryClient.setQueryData([...this.baseKey, "list"], data);
    return data;
  }

  async current() {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "current"],
    });

    const data = await currentBranch({
      repoPath: this.repositoryPath,
    });

    this.queryClient.setQueryData([...this.baseKey, "current"], data);
    return data;
  }

  // For React hooks
  get listQueryKey() {
    return [...this.baseKey, "list"];
  }

  get currentQueryKey() {
    return [...this.baseKey, "current"];
  }

  async checkout(branchName: string) {
    await invoke("checkout_branch", {
      repoPath: this.repositoryPath,
      branch: branchName,
    });
    await this.invalidateAll();
    await this.getParent().status.invalidate();
  }

  async invalidateAll() {
    await this.queryClient.invalidateQueries({ queryKey: [...this.baseKey] });
  }
}

class FilesActionsState extends StateDomain {
  constructor(
    protected queryClient: QueryClient,
    private repositoryPath: string,
  ) {
    super(queryClient);
  }

  async add(filePath: string) {
    const result = await gitAdd({
      repoPath: this.repositoryPath,
      file: filePath,
    });
    return result;
  }

  async unstage(filePath: string) {
    const result = await gitRemove({
      repoPath: this.repositoryPath,
      file: filePath,
    });
    return result;
  }

  async discard(filePath: string) {
    const result = await gitDiscard({
      repoPath: this.repositoryPath,
      file: filePath,
    });
    return result;
  }

  async addAll() {
    const result = await gitAdd({
      repoPath: this.repositoryPath,
      file: ".",
    });
    return result;
  }

  async removeAll() {
    const result = await gitRemove({
      repoPath: this.repositoryPath,
      file: ".",
    });
    return result;
  }
}

class Commit extends StateDomain {
  private readonly baseKey: readonly string[];

  constructor(
    protected queryClient: QueryClient,
    private repositoryPath: string,
  ) {
    super(queryClient);
    this.baseKey = ["repository", "commit"] as const;
  }
  async last() {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "last"],
    });

    const data = await lastCommit({
      repoPath: this.repositoryPath,
    });

    this.queryClient.setQueryData([...this.baseKey, "last"], data);

    return data;
  }

  async getCommitById(hash: string) {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "getCommitById", hash],
    });

    const data = await commitById({
      repoPath: this.repositoryPath,
      hash,
    });

    this.queryClient.setQueryData(
      [...this.baseKey, "getCommitById", hash],
      data,
    );

    return data;
  }

  getQueryKey(key: "last" | "getCommitById") {
    return [...this.baseKey, key];
  }

  async invalidate() {
    await this.queryClient.invalidateQueries({ queryKey: [...this.baseKey] });
  }
}

class RepositoryState extends StateDomain {
  readonly diff: DiffState;
  readonly status: StatusState;
  readonly branches: BranchState;
  readonly file: FilesActionsState;
  readonly commit: Commit;
  private readonly baseKey: readonly string[];

  constructor(
    protected queryClient: QueryClient,
    public readonly path: string,
  ) {
    super(queryClient);

    if (!path) {
      throw new Error("[RepositoryState] Path cannot be empty!");
    }

    this.baseKey = ["repository", this.path] as const;
    this.diff = new DiffState(this.queryClient, this.path);
    this.status = new StatusState(this.queryClient, this.path);
    this.branches = new BranchState(this.queryClient, this.path, () => this);
    this.file = new FilesActionsState(this.queryClient, this.path);
    this.commit = new Commit(this.queryClient, this.path);
  }

  async getRepositoryOrigin() {
    // repositoryOrigin()
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "origin"],
    });

    const data = await repositoryOrigin({
      repoPath: this.path,
    });

    this.queryClient.setQueryData([...this.baseKey, "origin"], data);

    return data;
  }

  getQueryKey(key: "origin") {
    return [...this.baseKey, key];
  }

  async invalidateAll() {
    await this.queryClient.invalidateQueries();
  }
}

export { RepositoryState };
