import {
  type StashApplyParams,
  type StashBranchParams,
  type StashEntry,
  type StashPopParams,
  type StashPushParams,
  type StashQuickStat,
  type StashShowResponse,
  stashApply,
  stashBranch,
  stashClear,
  stashDrop,
  stashList,
  stashPop,
  stashPush,
  stashQuickStat,
  stashShow,
} from "@gitru/commands";
import { QueryClient } from "@tanstack/react-query";
import { StateDomain } from "../core/StateManager";

class StashState extends StateDomain {
  private readonly baseKey: readonly string[];

  constructor(protected queryClient: QueryClient) {
    super(queryClient);
    this.baseKey = ["repository", "stash"] as const;
  }

  async list(): Promise<StashEntry[]> {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "list"],
    });

    const data = await stashList();

    this.queryClient.setQueryData([...this.baseKey, "list"], data);
    return data;
  }

  async quickStat(reference: string): Promise<StashQuickStat> {
    await this.queryClient.cancelQueries({
      queryKey: [...this.baseKey, "quickStat", reference],
    });

    const data = await stashQuickStat({ reference });

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

    const data = await stashShow({ reference });

    this.queryClient.setQueryData([...this.baseKey, "show", reference], data);
    return data;
  }

  async push(params?: Omit<StashPushParams, never>): Promise<string> {
    return await stashPush(params ?? {});
  }

  async clear(): Promise<string> {
    return await stashClear();
  }

  async pop(params?: StashPopParams): Promise<string> {
    return await stashPop(params ?? {});
  }

  async apply(params?: StashApplyParams): Promise<string> {
    return await stashApply(params ?? {});
  }

  async drop(reference: string): Promise<string> {
    return await stashDrop({ reference });
  }

  async branch(params: StashBranchParams): Promise<string> {
    return await stashBranch(params);
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
