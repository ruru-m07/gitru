export class StaleAuthorizationError extends Error {
  constructor() {
    super("The account authorization changed. Read the current account again.");
    this.name = "StaleAuthorizationError";
  }
}

/** An in-memory fence supplements the native epoch check after IPC completion. */
export class AuthorizationFence {
  private generation = 0;
  private readonly accountGenerations = new Map<string, number>();

  invalidate(accountId?: string): void {
    if (accountId === undefined) {
      this.generation += 1;
      this.accountGenerations.clear();
    } else {
      this.accountGenerations.set(
        accountId,
        (this.accountGenerations.get(accountId) ?? 0) + 1,
      );
    }
  }

  async read<T>(
    accountId: string,
    load: () => Promise<T>,
    signal?: AbortSignal,
  ): Promise<T> {
    const globalGeneration = this.generation;
    const accountGeneration = this.accountGenerations.get(accountId) ?? 0;
    if (signal?.aborted) throw new StaleAuthorizationError();
    const result = await load();
    if (
      signal?.aborted ||
      globalGeneration !== this.generation ||
      accountGeneration !== (this.accountGenerations.get(accountId) ?? 0)
    ) {
      throw new StaleAuthorizationError();
    }
    return result;
  }
}
