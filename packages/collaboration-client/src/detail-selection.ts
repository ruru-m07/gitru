export interface DetailSelectionLease {
  /** One automatic admission per selected parent binding; explicit retries are separate. */
  hydrate(binding: string): Promise<boolean>;
  release(): void;
}

type Selection = {
  accountId: string;
  references: number;
  active: boolean;
  attemptedBinding: string | null;
  retire: ReturnType<typeof setTimeout> | null;
};

/**
 * Coalesce selected-view intent without coupling hydration to cache reads.
 * A one-turn retirement grace covers StrictMode cleanup/remount. Only active
 * selections and that bounded grace remain; no response/body history is stored.
 */
export class DetailSelectionCoordinator {
  private readonly selections = new Map<string, Selection>();

  retain(
    identity: readonly [string, string, string, string, string],
    hydrate: () => Promise<unknown>,
  ): DetailSelectionLease {
    const key = JSON.stringify(identity);
    let selection = this.selections.get(key);
    if (!selection) {
      // Extra views keep explicit Sync available rather than growing bookkeeping.
      if (this.selections.size >= 128)
        return { hydrate: async () => false, release: () => {} };
      selection = {
        accountId: identity[0],
        references: 0,
        active: true,
        attemptedBinding: null,
        retire: null,
      };
      this.selections.set(key, selection);
    }
    if (selection.retire !== null) clearTimeout(selection.retire);
    selection.retire = null;
    selection.references += 1;
    const retained = selection;
    let released = false;
    return {
      hydrate: async (binding) => {
        if (
          released ||
          !retained.active ||
          retained.attemptedBinding === binding
        )
          return false;
        // Consume before IPC, including failures; stream updates cannot retry it.
        retained.attemptedBinding = binding;
        await hydrate();
        return true;
      },
      release: () => {
        if (released) return;
        released = true;
        retained.references -= 1;
        if (!retained.active || retained.references > 0) return;
        retained.retire = setTimeout(() => {
          retained.active = false;
          if (this.selections.get(key) === retained)
            this.selections.delete(key);
        }, 0);
      },
    };
  }

  clear(accountId?: string) {
    for (const [key, selection] of this.selections) {
      if (accountId !== undefined && selection.accountId !== accountId)
        continue;
      selection.active = false;
      if (selection.retire !== null) clearTimeout(selection.retire);
      this.selections.delete(key);
    }
  }
}
