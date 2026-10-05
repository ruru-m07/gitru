import { describe, expect, it } from "vitest";
import { isolatedGitEnvironment } from "./git-environment";
import { harnessEnvironment } from "./harness-environment";

describe("retained runner environment", () => {
  it("never reads excluded credentials, proxies or runtime preload variables", () => {
    const inherited: NodeJS.ProcessEnv = {
      PATH: "/task/bin",
      HOME: "/unchanged-home",
    };
    for (const key of [
      "GH_TOKEN",
      "GITHUB_TOKEN",
      "GITLAB_TOKEN",
      "OPENAI_API_KEY",
      "HTTP_PROXY",
      "HTTPS_PROXY",
      "NODE_OPTIONS",
      "GIT_CONFIG_GLOBAL",
    ]) {
      Object.defineProperty(inherited, key, {
        enumerable: true,
        get: () => {
          throw new Error("Excluded environment read");
        },
      });
    }
    const safe = harnessEnvironment(inherited);
    expect(safe).toEqual({ PATH: "/task/bin", HOME: "/unchanged-home" });
    expect(
      isolatedGitEnvironment(
        "/owned/global.gitconfig",
        { GITRU_E2E_REPO: "/owned/repo" },
        safe,
      ),
    ).toEqual({
      PATH: "/task/bin",
      HOME: "/unchanged-home",
      GITRU_E2E_REPO: "/owned/repo",
      GIT_CONFIG_GLOBAL: "/owned/global.gitconfig",
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_TERMINAL_PROMPT: "0",
    });
  });
  it("keeps platform process and display paths without changing the parent", () => {
    const inherited = {
      SystemRoot: "C:\\Windows",
      APPDATA: "C:\\task\\Roaming",
      DISPLAY: ":9",
      XDG_RUNTIME_DIR: "/run/task",
      EXTRA_SECRET: "synthetic",
    };
    expect(harnessEnvironment(inherited)).toEqual({
      SystemRoot: inherited.SystemRoot,
      APPDATA: inherited.APPDATA,
      DISPLAY: ":9",
      XDG_RUNTIME_DIR: "/run/task",
    });
    expect(inherited.EXTRA_SECRET).toBe("synthetic");
  });
});
