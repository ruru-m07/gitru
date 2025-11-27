import { queryClient } from "./core/StateManager";
import { repositories } from "./domains/RepositoryManager";
import { RepositoryState } from "./domains/RepositoryState";

class AppState {
  readonly repositories = repositories;

  get repository(): RepositoryState | null {
    return this.repositories.current;
  }

  async invalidateAll() {
    await queryClient.invalidateQueries();
  }

  async reset() {
    await this.repositories.disposeAll();
    queryClient.clear();
  }

  get queryClient() {
    return queryClient;
  }
}

export const appState = new AppState();
