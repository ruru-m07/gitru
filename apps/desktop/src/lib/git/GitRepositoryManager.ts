import { GitRepository } from "./GitRepository";

/**
 * Manages multiple Git repositories
 * Provides factory methods and lifecycle management
 */
export class GitRepositoryManager {
  private static instance: GitRepositoryManager;
  private repositories: Map<string, GitRepository> = new Map();

  private constructor() {}

  /**
   * Get singleton instance
   */
  static getInstance(): GitRepositoryManager {
    if (!GitRepositoryManager.instance) {
      GitRepositoryManager.instance = new GitRepositoryManager();
    }
    return GitRepositoryManager.instance;
  }

  /**
   * Get or create a repository instance
   */
  getRepository(path: string, name: string): GitRepository {
    // Normalize path
    const normalizedPath = path.replace(/\\/g, "/").replace(/\/$/, "");

    if (!this.repositories.has(normalizedPath)) {
      const repo = new GitRepository(normalizedPath, name);
      this.repositories.set(normalizedPath, repo);
    }

    return this.repositories.get(normalizedPath)!;
  }

  /**
   * Check if repository exists
   */
  hasRepository(path: string): boolean {
    const normalizedPath = path.replace(/\\/g, "/").replace(/\/$/, "");
    return this.repositories.has(normalizedPath);
  }

  /**
   * Remove a repository and clean up resources
   */
  async removeRepository(path: string): Promise<void> {
    const normalizedPath = path.replace(/\\/g, "/").replace(/\/$/, "");
    const repo = this.repositories.get(normalizedPath);

    if (repo) {
      await repo.dispose();
      this.repositories.delete(normalizedPath);
    }
  }

  /**
   * Get all repositories
   */
  getAllRepositories(): GitRepository[] {
    return Array.from(this.repositories.values());
  }

  /**
   * Clean up all repositories
   */
  async dispose(): Promise<void> {
    const promises = Array.from(this.repositories.values()).map((repo) =>
      repo.dispose(),
    );
    await Promise.all(promises);
    this.repositories.clear();
  }
}

// Export singleton instance
export const gitRepositoryManager = GitRepositoryManager.getInstance();
