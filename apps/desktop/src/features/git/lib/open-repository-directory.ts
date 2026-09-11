import { open } from "@tauri-apps/plugin-dialog";

export async function openRepositoryDirectory(): Promise<string | null> {
  if (import.meta.env.MODE === "e2e") {
    window.__GITRU_E2E_DIRECTORY_PICKER_CALLS__ =
      (window.__GITRU_E2E_DIRECTORY_PICKER_CALLS__ ?? 0) + 1;

    const repositoryPath = window.__GITRU_E2E_REPOSITORY_PATH__;
    if (!repositoryPath) {
      throw new Error("The E2E repository path was not configured");
    }
    return repositoryPath;
  }

  return open({
    directory: true,
    multiple: false,
  });
}
