import {
  type StashApplyParams,
  type StashBranchParams,
  type StashEntry,
  type StashPopParams,
  type StashPushParams,
  type StashQuickStat,
  type StashRestoreFileParams,
  type StashShowResponse,
  stashApply,
  stashBranch,
  stashClear,
  stashDrop,
  stashList,
  stashPop,
  stashPush,
  stashQuickStat,
  stashRestoreFile,
  stashShow,
} from "@gitru/commands";
import { QueryClient } from "@tanstack/react-query";
import { StateDomain } from "../core/state-manager";

type StashPushInput = Omit<StashPushParams, "contextId">;
type StashPopInput = Omit<StashPopParams, "contextId">;
type StashApplyInput = Omit<StashApplyParams, "contextId">;
type StashBranchInput = Omit<StashBranchParams, "contextId">;
type StashRestoreFileInput = Omit<StashRestoreFileParams, "contextId">;

class StashState extends StateDomain {
  private readonly baseKey: readonly string[];
  private readonly contextId: string;

  constructor(
    protected queryClient: QueryClient,
    repositoryBaseKey: readonly string[],
    contextId: string,
  ) {
    super(queryClient);
    this.baseKey = [...repositoryBaseKey, "stash"] as const;
    this.contextId = contextId;
  }

  async list(): Promise<StashEntry[]> {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "list"],
    });

    const data = await stashList({
      contextId: this.contextId,
    });

    this.queryClient.setQueryData([...this.baseKey, "list"], data);
    return data;
  }

  async quickStat(reference: string): Promise<StashQuickStat> {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "quickStat", reference],
    });

    const data = await stashQuickStat({
      contextId: this.contextId,
      reference,
    });

    this.queryClient.setQueryData(
      [...this.baseKey, "quickStat", reference],
      data,
    );
    return data;
  }

  async show(reference: string): Promise<StashShowResponse> {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "show", reference],
    });

    const data = await stashShow({
      contextId: this.contextId,
      reference,
    });

    this.queryClient.setQueryData([...this.baseKey, "show", reference], data);
    return data;
  }

  async push(params?: StashPushInput): Promise<string> {
    return await stashPush({
      contextId: this.contextId,
      ...(params ?? {}),
    });
  }

  async clear(): Promise<string> {
    return await stashClear({
      contextId: this.contextId,
    });
  }

  async pop(params?: StashPopInput): Promise<string> {
    return await stashPop({
      contextId: this.contextId,
      ...(params ?? {}),
    });
  }

  async apply(params?: StashApplyInput): Promise<string> {
    return await stashApply({
      contextId: this.contextId,
      ...(params ?? {}),
    });
  }

  async drop(reference: string): Promise<string> {
    return await stashDrop({
      contextId: this.contextId,
      reference,
    });
  }

  async branch(params: StashBranchInput): Promise<string> {
    return await stashBranch({
      contextId: this.contextId,
      ...params,
    });
  }

  async restoreFile(params: StashRestoreFileInput): Promise<string> {
    return await stashRestoreFile({
      contextId: this.contextId,
      ...params,
    });
  }

  getQueryKey(key: "list" | "quickStat" | "show") {
    return [...this.baseKey, key];
  }

  async invalidate(key?: "list" | "quickStat" | "show") {
    const queryKey = key ? [...this.baseKey, key] : [...this.baseKey];
    await this.queryClient.invalidateQueries({ queryKey });
  }

  async invalidateAll() {
    await this.queryClient.invalidateQueries({ queryKey: [...this.baseKey] });
  }
}

export { StashState };
