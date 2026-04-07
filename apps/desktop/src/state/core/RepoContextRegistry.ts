import { createRepoContext, disposeRepoContext } from "@gitru/commands";

type ScopeId = string;

type ScopeContextEntry = {
  repoId: string;
  contextId: string;
};

class RepoContextRegistry {
  private readonly contextsByScope = new Map<ScopeId, ScopeContextEntry>();
  private readonly listeners = new Set<(contextId: string | null) => void>();
  private activeScopeId: ScopeId | null = null;

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

  async ensureScopeContext(scopeId: ScopeId, repoId: string): Promise<string> {
    const existing = this.contextsByScope.get(scopeId);

    if (existing?.repoId === repoId) {
      this.notify();
      return existing.contextId;
    }

    if (existing?.contextId) {
      await this.disposeContext(existing.contextId);
    }

    const contextId = await createRepoContext({ repoId });
    this.contextsByScope.set(scopeId, { repoId, contextId });
    this.notify();

    return contextId;
  }

  async disposeScope(scopeId: ScopeId) {
    const existing = this.contextsByScope.get(scopeId);

    this.contextsByScope.delete(scopeId);

    if (existing?.contextId) {
      await this.disposeContext(existing.contextId);
    }

    this.notify();
  }

  async disposeAll() {
    const contextIds = Array.from(this.contextsByScope.values()).map(
      (entry) => entry.contextId,
    );

    this.contextsByScope.clear();
    this.notify();

    await Promise.all(
      contextIds.map(async (contextId) => {
        await this.disposeContext(contextId);
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

  private notify() {
    const activeContextId = this.getActiveContextId();
    for (const listener of this.listeners) {
      listener(activeContextId);
    }
  }

  private async disposeContext(contextId: string) {
    try {
      await disposeRepoContext({ contextId });
    } catch {
      // Ignore cleanup errors when context is already disposed server-side.
    }
  }
}

export const repoContextRegistry = new RepoContextRegistry();

export const getActiveRepoContextId = () =>
  repoContextRegistry.getActiveContextId();
