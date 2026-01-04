import { queryClient } from "./core/StateManager";
import { repositories } from "./domains/RepositoryManager";
import { RepositoryState } from "./domains/RepositoryState";

class AppState {
  readonly repositories = repositories;

  get repository(): RepositoryState | null {
    return this.repositories.current;
  }

  get queryClient() {
    return queryClient;
  }
}

export const appState = new AppState();
