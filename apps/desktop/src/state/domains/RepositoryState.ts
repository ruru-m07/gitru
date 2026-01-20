import {
  BranchKind,
  CreateCommitParams,
  commitById,
  createCommit,
  currentBranch,
  getPatchByFilePath,
  getStatus,
  gitAdd,
  gitDiscard,
  gitFetch,
  gitPull,
  gitPush,
  gitRemove,
  history,
  lastCommit,
  listBranches,
  repositoryOrigin,
  statusAheadBehind,
} from "@gitru/commands";
import { QueryClient } from "@tanstack/react-query";
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

    const data = await getPatchByFilePath({
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
  ) {
    super(queryClient);
    this.baseKey = ["repository", "branches"] as const;
  }

  async list(kind: BranchKind) {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "list", kind],
    });

    const data = await listBranches({
      repoPath: this.repositoryPath,
      kind,
    });

    this.queryClient.setQueryData([...this.baseKey, "list", kind], data);
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

  async statusAheadBehind() {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "statusAheadBehind"],
    });

    const data = await statusAheadBehind({
      repoPath: this.repositoryPath,
    });

    this.queryClient.setQueryData([...this.baseKey, "statusAheadBehind"], data);
    return data;
  }

  getQueryKey(key: "list" | "current" | "statusAheadBehind") {
    return [...this.baseKey, key];
  }

  async invalidate(key?: "list" | "current" | "statusAheadBehind") {
    const queryKey = key ? [...this.baseKey, key] : [...this.baseKey];
    await this.queryClient.invalidateQueries({ queryKey });
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

  async fetch() {
    const result = await gitFetch({
      repoPath: this.repositoryPath,
    });
    return result;
  }

  async push() {
    const result = await gitPush({
      repoPath: this.repositoryPath,
    });
    return result;
  }

  async pull() {
    const result = await gitPull({
      repoPath: this.repositoryPath,
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

  async createCommit(payload: CreateCommitParams["commitMeta"]) {
    const data = await createCommit({
      repoPath: this.repositoryPath,
      commitMeta: payload,
    });

    return data;
  }

  async history() {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "history"],
    });

    const data = await history({
      repoPath: this.repositoryPath,
      limit: 100,
      skip: 0,
    });

    this.queryClient.setQueryData([...this.baseKey, "history"], data);

    return data;
  }

  getQueryKey(key: "last" | "getCommitById" | "history") {
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
    this.branches = new BranchState(this.queryClient, this.path);
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
