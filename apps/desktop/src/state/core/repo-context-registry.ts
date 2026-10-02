import { createRepoContext, disposeRepoContext } from "@gitru/commands";
import { repositories } from "../domains/repository-manager";

type ScopeId = string;

type ScopeContextEntry = {
  repoId: string;
  contextId: string;
  ownerId: string;
};

export const createRepoContextOwnerId = (
  webviewLabel: string,
  scopeId: ScopeId,
) => JSON.stringify([webviewLabel, scopeId]);

type RepoContextRegistryDependencies = {
  createContext: (input: {
    repoId: string;
    ownerId: string;
  }) => Promise<string>;
  disposeContext: (contextId: string) => Promise<void>;
  disposeRepositoryState: (contextId: string) => Promise<void>;
};

const defaultDependencies: RepoContextRegistryDependencies = {
  createContext: ({ repoId, ownerId }) =>
    createRepoContext({ repoId, ownerId }),
  disposeContext: async (contextId) => {
    await disposeRepoContext({ contextId });
  },
  disposeRepositoryState: (contextId) => repositories.disposeContext(contextId),
};

export class RepoContextRegistry {
  private readonly contextsByScope = new Map<ScopeId, ScopeContextEntry>();
  private readonly operationsByScope = new Map<ScopeId, Promise<void>>();
  private readonly listeners = new Set<(contextId: string | null) => void>();
  private activeScopeId: ScopeId | null = null;

  constructor(
    private readonly dependencies: RepoContextRegistryDependencies = defaultDependencies,
  ) {}

  getScopeContext(scopeId: ScopeId): ScopeContextEntry | null {
    const context = this.contextsByScope.get(scopeId);
    return context ? { ...context } : null;
  }

  setActiveScope(scopeId: ScopeId) {
    this.activeScopeId = scopeId;
    this.notify();
  }

  clearActiveScope(scopeId: ScopeId) {
    if (this.activeScopeId !== scopeId) return;
    this.activeScopeId = null;
    this.notify();
  }

  async ensureScopeContext(
    scopeId: ScopeId,
    repoId: string,
    ownerId: string,
  ): Promise<string> {
    return this.runSerialized(scopeId, async () => {
      return this.ensureScopeContextNow(scopeId, repoId, ownerId);
    });
  }

  async disposeScope(scopeId: ScopeId) {
    await this.runSerialized(scopeId, async () => {
      const existing = this.contextsByScope.get(scopeId);

      this.contextsByScope.delete(scopeId);
      this.notify();

      if (existing?.contextId) {
        await this.disposeContext(existing.contextId);
      }
    });
  }

  async disposeAll() {
    const scopeIds = new Set([
      ...this.contextsByScope.keys(),
      ...this.operationsByScope.keys(),
    ]);

    await Promise.all(
      Array.from(scopeIds, async (scopeId) => {
        await this.disposeScope(scopeId);
      }),
    );
  }

  getActiveContextId(): string | null {
    if (!this.activeScopeId) {
      return null;
    }

    return this.contextsByScope.get(this.activeScopeId)?.contextId ?? null;
  }

  subscribe(listener: (contextId: string | null) => void): () => void {
    this.listeners.add(listener);
    listener(this.getActiveContextId());

    return () => {
      this.listeners.delete(listener);
    };
  }

  private async ensureScopeContextNow(
    scopeId: ScopeId,
    repoId: string,
    ownerId: string,
  ): Promise<string> {
    const existing = this.contextsByScope.get(scopeId);

    if (existing?.repoId === repoId && existing.ownerId === ownerId) {
      this.notify();
      return existing.contextId;
    }

    if (existing?.contextId) {
      this.contextsByScope.delete(scopeId);
      this.notify();
      await this.disposeContext(existing.contextId);
    }

    const contextId = await this.dependencies.createContext({
      repoId,
      ownerId,
    });
    this.contextsByScope.set(scopeId, { repoId, contextId, ownerId });
    this.notify();

    return contextId;
  }

  private notify() {
    const activeContextId = this.getActiveContextId();
    for (const listener of this.listeners) {
      listener(activeContextId);
    }
  }

  private async disposeContext(contextId: string) {
    try {
      await this.dependencies.disposeContext(contextId);
    } catch {
      // Ignore cleanup errors when context is already disposed server-side.
    } finally {
      await this.dependencies.disposeRepositoryState(contextId);
    }
  }

  private runSerialized<TResult>(
    scopeId: ScopeId,
    operation: () => Promise<TResult>,
  ): Promise<TResult> {
    const previous = this.operationsByScope.get(scopeId) ?? Promise.resolve();
    const result = previous.catch(() => undefined).then(operation);
    const tail = result.then(
      () => undefined,
      () => undefined,
    );

    this.operationsByScope.set(scopeId, tail);
    void tail.then(() => {
      if (this.operationsByScope.get(scopeId) === tail) {
        this.operationsByScope.delete(scopeId);
      }
    });

    return result;
  }
}

export const repoContextRegistry = new RepoContextRegistry();

export const getActiveRepoContextId = () =>
  repoContextRegistry.getActiveContextId();
