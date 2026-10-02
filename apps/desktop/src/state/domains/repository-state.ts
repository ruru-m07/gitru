import {
  BranchKind,
  BranchStash,
  CommitActivityParams,
  type ConflictResolveRequest,
  CreateCommitParams,
  commitActivity,
  commitById,
  createBranch,
  createCommit,
  currentBranch,
  currentBranchStash,
  deleteLocalBranch,
  deleteRemoteBranch,
  FileStatusKind,
  getPatchByFilePath,
  getRepoOperation,
  getStatus,
  gitAdd,
  gitApplyPatchBlock,
  gitDiscard,
  gitFetch,
  gitRemove,
  HistoryGraphParams,
  hasUncommittedChanges,
  history,
  historyGraph,
  lastCommit,
  listBranches,
  PatchRange,
  popCurrentBranchStash,
  publishBranch,
  pull,
  push,
  type RebasePlanEntry,
  type RebaseStartRequest,
  type RebaseUpdateTodoRequest,
  type RepoOperation,
  rebaseAbort,
  rebaseAbortPreview,
  rebaseContinue,
  rebasePlan,
  rebaseResolveConflict,
  rebaseSetCommitMessage,
  rebaseSkip,
  rebaseStart,
  rebaseUpdateTodo,
  renameBranch,
  repositoryOrigin,
  setBranchUpstream,
  statusAheadBehind,
  switchBranch,
  UncommittedChangesStrategy,
  unsetBranchUpstream,
} from "@gitru/commands";
import { QueryClient } from "@tanstack/react-query";
import { StateDomain } from "../core/state-manager";
import { StashState } from "./stash-state";

type DiffScope = "Worktree" | "Staged" | "Unstaged";
type PatchAction = "Stage" | "Unstage" | "Discard";
type CreateCommitPayload = Omit<CreateCommitParams, "contextId">;

class DiffState extends StateDomain {
  private readonly baseKey: readonly string[];
  private readonly contextId: string;

  constructor(
    protected queryClient: QueryClient,
    repositoryBaseKey: readonly string[],
    contextId: string,
  ) {
    super(queryClient);
    this.baseKey = [...repositoryBaseKey, "diff"] as const;
    this.contextId = contextId;
  }

  async get(
    filePath: string,
    options?: {
      fileNewPath?: string;
      status?: FileStatusKind[];
      stashReference?: string;
      commitHash?: string;
      parentIndex?: number;
      diffScope?: DiffScope;
    },
  ) {
    const diffScope = options?.diffScope ?? "Worktree";
    return await getPatchByFilePath({
      contextId: this.contextId,
      filePath: filePath,
      fileNewPath: options?.fileNewPath,
      status: options?.status,
      stashReference: options?.stashReference,
      commitHash: options?.commitHash,
      parentIndex: options?.parentIndex,
      diffScope,
    });
  }

  getDiffQueryKey(
    filePath: string,
    options?: {
      fileNewPath?: string;
      status?: FileStatusKind[];
      stashReference?: string;
      commitHash?: string;
      parentIndex?: number;
      diffScope?: DiffScope;
    },
  ) {
    const sourceScope = options?.stashReference
      ? `stash:${options.stashReference}`
      : options?.commitHash
        ? `commit:${options.commitHash}:p${options.parentIndex ?? 1}`
        : "worktree";
    const diffScope = options?.diffScope ?? "Worktree";
    return [
      ...this.baseKey,
      sourceScope,
      diffScope,
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
  private readonly contextId: string;

  constructor(
    protected queryClient: QueryClient,
    repositoryBaseKey: readonly string[],
    contextId: string,
  ) {
    super(queryClient);
    this.baseKey = [...repositoryBaseKey, "status"] as const;
    this.contextId = contextId;
  }

  async get() {
    return await getStatus({
      contextId: this.contextId,
    });
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
  private readonly contextId: string;

  constructor(
    protected queryClient: QueryClient,
    repositoryBaseKey: readonly string[],
    contextId: string,
  ) {
    super(queryClient);
    this.baseKey = [...repositoryBaseKey, "branches"] as const;
    this.contextId = contextId;
  }

  async list(kind: BranchKind) {
    return await listBranches({
      contextId: this.contextId,
      kind,
    });
  }

  async current() {
    return await currentBranch({
      contextId: this.contextId,
    });
  }

  async statusAheadBehind() {
    return await statusAheadBehind({
      contextId: this.contextId,
    });
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
    key?:
      | "list"
      | "current"
      | "statusAheadBehind"
      | "hasUncommittedChanges"
      | "currentBranchStash",
  ) {
    const queryKey = key ? [...this.baseKey, key] : [...this.baseKey];
    await this.queryClient.invalidateQueries({ queryKey });
  }

  async invalidateAll() {
    await this.queryClient.invalidateQueries({ queryKey: [...this.baseKey] });
  }

  async hasUncommittedChanges() {
    const data = await hasUncommittedChanges({
      contextId: this.contextId,
    });
    return data;
  }

  async currentBranchStash(): Promise<BranchStash | null> {
    const data = await currentBranchStash({
      contextId: this.contextId,
    });
    return data;
  }

  async popCurrentBranchStash(): Promise<string> {
    return await popCurrentBranchStash({
      contextId: this.contextId,
    });
  }

  async switchBranch(
    branchName: string,
    strategy?: UncommittedChangesStrategy,
  ) {
    const result = await switchBranch({
      contextId: this.contextId,
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
      contextId: this.contextId,
      branch: branchName,
      strategy,
    });
    return result;
  }

  async renameBranch(branch: string, newName: string) {
    return await renameBranch({
      contextId: this.contextId,
      branch,
      newName,
    });
  }

  async deleteLocalBranch(branch: string, force: boolean) {
    return await deleteLocalBranch({
      contextId: this.contextId,
      branch,
      force,
    });
  }

  async deleteRemoteBranch(branch: string, force: boolean) {
    return await deleteRemoteBranch({
      contextId: this.contextId,
      branch,
      force,
    });
  }

  async setUpstream(branch: string, upstream: string) {
    return await setBranchUpstream({
      contextId: this.contextId,
      branch,
      upstream,
    });
  }

  async unsetUpstream(branch: string) {
    return await unsetBranchUpstream({
      contextId: this.contextId,
      branch,
    });
  }
}

class FilesActionsState extends StateDomain {
  private readonly contextId: string;

  constructor(
    protected queryClient: QueryClient,
    contextId: string,
  ) {
    super(queryClient);
    this.contextId = contextId;
  }

  async add(target: string | string[]) {
    const result = await gitAdd(
      Array.isArray(target)
        ? { contextId: this.contextId, files: target }
        : { contextId: this.contextId, file: target },
    );
    return result;
  }

  async applyPatchBlock(params: {
    filePath: string;
    fileNewPath?: string | null;
    diffScope: DiffScope;
    additions: PatchRange;
    deletions: PatchRange;
    action: PatchAction;
  }) {
    const result = await gitApplyPatchBlock({
      contextId: this.contextId,
      filePath: params.filePath,
      fileNewPath: params.fileNewPath ?? undefined,
      diffScope: params.diffScope,
      additions: params.additions,
      deletions: params.deletions,
      action: params.action,
    });
    return result;
  }

  async unstage(target: string | string[]) {
    const result = await gitRemove(
      Array.isArray(target)
        ? { contextId: this.contextId, files: target }
        : { contextId: this.contextId, file: target },
    );
    return result;
  }

  async discard(target: string | string[]) {
    const result = await gitDiscard(
      Array.isArray(target)
        ? { contextId: this.contextId, files: target }
        : { contextId: this.contextId, file: target },
    );
    return result;
  }

  async fetch() {
    const result = await gitFetch({
      contextId: this.contextId,
    });
    return result;
  }

  async publishBranch() {
    const result = await publishBranch({
      contextId: this.contextId,
    });
    return result;
  }

  async push() {
    const result = await push({
      contextId: this.contextId,
    });
    return result;
  }

  async pull() {
    const result = await pull({
      contextId: this.contextId,
    });
    return result;
  }
}

class Commit extends StateDomain {
  private readonly baseKey: readonly string[];
  private readonly contextId: string;

  constructor(
    protected queryClient: QueryClient,
    repositoryBaseKey: readonly string[],
    contextId: string,
  ) {
    super(queryClient);
    this.baseKey = [...repositoryBaseKey, "commit"] as const;
    this.contextId = contextId;
  }
  async last() {
    return await lastCommit({
      contextId: this.contextId,
    });
  }

  async getCommitById(hash: string) {
    return await commitById({
      contextId: this.contextId,
      hash,
    });
  }

  async createCommit(payload: CreateCommitPayload) {
    const data = await createCommit({
      contextId: this.contextId,
      ...payload,
    });

    return data;
  }

  async history() {
    return await history({
      contextId: this.contextId,
      limit: 100,
      skip: 0,
    });
  }

  async historyGraph(params: HistoryGraphParams["query"]) {
    const data = await historyGraph({
      contextId: this.contextId,
      query: params,
    });

    return data;
  }

  async commitActivity(query: CommitActivityParams["query"]) {
    const data = await commitActivity({
      contextId: this.contextId,
      query,
    });
    return data;
  }

  getQueryKey(
    key:
      | "last"
      | "getCommitById"
      | "history"
      | "historyGraph"
      | "commitActivity",
  ) {
    return [...this.baseKey, key];
  }

  async invalidate() {
    await this.queryClient.invalidateQueries({ queryKey: [...this.baseKey] });
  }
}

class OperationState extends StateDomain {
  private readonly baseKey: readonly string[];
  private readonly contextId: string;

  constructor(
    protected queryClient: QueryClient,
    repositoryBaseKey: readonly string[],
    contextId: string,
  ) {
    super(queryClient);
    this.baseKey = [...repositoryBaseKey, "operation"] as const;
    this.contextId = contextId;
  }

  async get(): Promise<RepoOperation> {
    return await getRepoOperation({ contextId: this.contextId });
  }

  get queryKey() {
    return [...this.baseKey];
  }

  getCached(): RepoOperation | undefined {
    return this.queryClient.getQueryData([...this.baseKey]);
  }

  async invalidate() {
    await this.queryClient.invalidateQueries({ queryKey: [...this.baseKey] });
  }

  async plan(onto: string, upstream?: string) {
    return rebasePlan({
      contextId: this.contextId,
      onto,
      upstream,
    });
  }

  async start(request: RebaseStartRequest) {
    const data = await rebaseStart({
      contextId: this.contextId,
      request,
    });
    await this.invalidate();
    return data;
  }

  async continue(message?: string) {
    const data = await rebaseContinue({
      contextId: this.contextId,
      message,
    });
    await this.invalidate();
    return data;
  }

  async skip() {
    const data = await rebaseSkip({ contextId: this.contextId });
    await this.invalidate();
    return data;
  }

  async abort() {
    const data = await rebaseAbort({ contextId: this.contextId });
    await this.invalidate();
    return data;
  }

  async abortPreview() {
    return rebaseAbortPreview({ contextId: this.contextId });
  }

  async updateTodo(entries: RebasePlanEntry[]) {
    const request: RebaseUpdateTodoRequest = { entries };
    const data = await rebaseUpdateTodo({
      contextId: this.contextId,
      request,
    });
    await this.invalidate();
    return data;
  }

  async setCommitMessage(message: string) {
    await rebaseSetCommitMessage({
      contextId: this.contextId,
      message,
    });
  }

  async resolveConflict(request: ConflictResolveRequest) {
    await rebaseResolveConflict({
      contextId: this.contextId,
      request,
    });
    await this.invalidate();
  }
}

class RepositoryState extends StateDomain {
  readonly diff: DiffState;
  readonly status: StatusState;
  readonly branches: BranchState;
  readonly file: FilesActionsState;
  readonly commit: Commit;
  readonly stash: StashState;
  readonly operation: OperationState;
  private readonly baseKey: readonly string[];
  readonly contextId: string;

  constructor(
    protected queryClient: QueryClient,
    public readonly path: string,
    contextId: string,
  ) {
    super(queryClient);

    if (!path) {
      throw new Error("[RepositoryState] Path cannot be empty!");
    }

    if (!contextId) {
      throw new Error("[RepositoryState] Context ID cannot be empty!");
    }

    this.contextId = contextId;
    this.baseKey = ["repository", this.contextId, this.path] as const;
    this.diff = new DiffState(this.queryClient, this.baseKey, this.contextId);
    this.status = new StatusState(
      this.queryClient,
      this.baseKey,
      this.contextId,
    );
    this.branches = new BranchState(
      this.queryClient,
      this.baseKey,
      this.contextId,
    );
    this.file = new FilesActionsState(this.queryClient, this.contextId);
    this.commit = new Commit(this.queryClient, this.baseKey, this.contextId);
    this.stash = new StashState(this.queryClient, this.baseKey, this.contextId);
    this.operation = new OperationState(
      this.queryClient,
      this.baseKey,
      this.contextId,
    );
  }

  async getRepositoryOrigin() {
    return await repositoryOrigin({
      contextId: this.contextId,
    });
  }

  getQueryKey(key: "origin") {
    return [...this.baseKey, key];
  }

  async invalidateAll() {
    await this.queryClient.invalidateQueries({ queryKey: [...this.baseKey] });
  }
}

export { RepositoryState };
