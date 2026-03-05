import {
  BranchKind,
  BranchStash,
  CreateCommitParams,
  FileStatusKind,
  commitById,
  createBranch,
  createCommit,
  currentBranch,
  currentBranchStash,
  getPatchByFilePath,
  getStatus,
  gitAdd,
  gitDiscard,
  gitFetch,
  gitRemove,
  HistoryGraphParams,
  hasUncommittedChanges,
  history,
  historyGraph,
  lastCommit,
  listBranches,
  popCurrentBranchStash,
  publishBranch,
  pull,
  push,
  repositoryOrigin,
  statusAheadBehind,
  switchBranch,
  UncommittedChangesStrategy,
} from "@gitru/commands";
import { QueryClient } from "@tanstack/react-query";
import { StateDomain } from "../core/StateManager";
import { StashState } from "./StashState";

class DiffState extends StateDomain {
  private readonly baseKey: readonly string[];

  constructor(protected queryClient: QueryClient) {
    super(queryClient);
    this.baseKey = ["repository", "diff"] as const;
  }

  async get(
    filePath: string,
    options?: {
      fileNewPath?: string;
      status?: FileStatusKind[];
      stashReference?: string;
      commitHash?: string;
      parentIndex?: number;
    },
  ) {
    const sourceScope = options?.stashReference
      ? `stash:${options.stashReference}`
      : options?.commitHash
        ? `commit:${options.commitHash}:p${options.parentIndex ?? 1}`
        : "worktree";
    const queryKey = [
      ...this.baseKey,
      sourceScope,
      filePath,
      options?.fileNewPath ?? "",
      options?.status?.join(",") ?? "",
    ];

    const data = await getPatchByFilePath({
      filePath: filePath,
      fileNewPath: options?.fileNewPath,
      status: options?.status,
      stashReference: options?.stashReference,
      commitHash: options?.commitHash,
      parentIndex: options?.parentIndex,
    });

    this.queryClient.setQueryData(queryKey, data);

    return data;
  }

  getDiffQueryKey(
    filePath: string,
    options?: {
      fileNewPath?: string;
      status?: FileStatusKind[];
      stashReference?: string;
      commitHash?: string;
      parentIndex?: number;
    },
  ) {
    const sourceScope = options?.stashReference
      ? `stash:${options.stashReference}`
      : options?.commitHash
        ? `commit:${options.commitHash}:p${options.parentIndex ?? 1}`
        : "worktree";
    return [
      ...this.baseKey,
      sourceScope,
      filePath,
      options?.fileNewPath ?? "",
      options?.status?.join(",") ?? "",
    ];
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

  constructor(protected queryClient: QueryClient) {
    super(queryClient);
    this.baseKey = ["repository", "status"] as const;
  }

  async get() {
    await this.queryClient.cancelQueries({ queryKey: [...this.baseKey] });

    const data = await getStatus();

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

  constructor(protected queryClient: QueryClient) {
    super(queryClient);
    this.baseKey = ["repository", "branches"] as const;
  }

  async list(kind: BranchKind) {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "list", kind],
    });

    const data = await listBranches({
      kind,
    });

    this.queryClient.setQueryData([...this.baseKey, "list", kind], data);
    return data;
  }

  async current() {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "current"],
    });

    const data = await currentBranch();

    this.queryClient.setQueryData([...this.baseKey, "current"], data);
    return data;
  }

  async statusAheadBehind() {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "statusAheadBehind"],
    });

    const data = await statusAheadBehind();

    this.queryClient.setQueryData([...this.baseKey, "statusAheadBehind"], data);
    return data;
  }

  getQueryKey(
    key:
      | "list"
      | "current"
      | "statusAheadBehind"
      | "hasUncommittedChanges"
      | "currentBranchStash",
  ) {
    return [...this.baseKey, key];
  }

  async invalidate(
    key?: "list" | "current" | "statusAheadBehind" | "currentBranchStash",
  ) {
    const queryKey = key ? [...this.baseKey, key] : [...this.baseKey];
    await this.queryClient.invalidateQueries({ queryKey });
  }

  async invalidateAll() {
    await this.queryClient.invalidateQueries({ queryKey: [...this.baseKey] });
  }

  async hasUncommittedChanges() {
    const data = await hasUncommittedChanges();
    return data;
  }

  async currentBranchStash(): Promise<BranchStash | null> {
    const data = await currentBranchStash();
    return data;
  }

  async popCurrentBranchStash(): Promise<string> {
    return await popCurrentBranchStash();
  }

  async switchBranch(
    branchName: string,
    strategy?: UncommittedChangesStrategy,
  ) {
    const result = await switchBranch({
      branch: branchName,
      strategy,
    });
    return result;
  }

  async createBranch(
    branchName: string,
    strategy?: UncommittedChangesStrategy,
  ) {
    const result = await createBranch({
      branch: branchName,
      strategy,
    });
    return result;
  }
}

class FilesActionsState extends StateDomain {
  constructor(protected queryClient: QueryClient) {
    super(queryClient);
  }

  async add(filePath: string) {
    const result = await gitAdd({
      file: filePath,
    });
    return result;
  }

  async unstage(filePath: string) {
    const result = await gitRemove({
      file: filePath,
    });
    return result;
  }

  async discard(filePath: string) {
    const result = await gitDiscard({
      file: filePath,
    });
    return result;
  }

  async fetch() {
    const result = await gitFetch();
    return result;
  }

  async publishBranch() {
    const result = await publishBranch();
    return result;
  }

  async push() {
    const result = await push();
    return result;
  }

  async pull() {
    const result = await pull();
    return result;
  }
}

class Commit extends StateDomain {
  private readonly baseKey: readonly string[];

  constructor(protected queryClient: QueryClient) {
    super(queryClient);
    this.baseKey = ["repository", "commit"] as const;
  }
  async last() {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "last"],
    });

    const data = await lastCommit();

    this.queryClient.setQueryData([...this.baseKey, "last"], data);

    return data;
  }

  async getCommitById(hash: string) {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "getCommitById", hash],
    });

    const data = await commitById({
      hash,
    });

    this.queryClient.setQueryData(
      [...this.baseKey, "getCommitById", hash],
      data,
    );

    return data;
  }

  async createCommit(payload: CreateCommitParams) {
    const data = await createCommit({
      ...payload,
    });

    return data;
  }

  async history() {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "history"],
    });

    const data = await history({
      limit: 100,
      skip: 0,
    });

    this.queryClient.setQueryData([...this.baseKey, "history"], data);

    return data;
  }

  async historyGraph(params: HistoryGraphParams["query"]) {
    const data = await historyGraph({
      query: params,
    });

    return data;
  }

  getQueryKey(key: "last" | "getCommitById" | "history" | "historyGraph") {
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
  readonly stash: StashState;
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
    this.diff = new DiffState(this.queryClient);
    this.status = new StatusState(this.queryClient);
    this.branches = new BranchState(this.queryClient);
    this.file = new FilesActionsState(this.queryClient);
    this.commit = new Commit(this.queryClient);
    this.stash = new StashState(this.queryClient);
  }

  async getRepositoryOrigin() {
    // repositoryOrigin()
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "origin"],
    });

    const data = await repositoryOrigin();

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
