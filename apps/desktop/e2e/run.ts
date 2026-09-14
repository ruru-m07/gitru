import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import {
  copyFileSync,
  createWriteStream,
  existsSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { type AddressInfo, createServer } from "node:net";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import {
  CdpClient,
  classifyGitruTargets,
  listCdpTargets,
  readCdpVersion,
  waitFor,
} from "./cdp";
import { isolatedGitEnvironment } from "./git-environment";
import { runCefSmoke } from "./specs/cef-smoke";

type GitOptions = {
  allowFailure?: boolean;
  cwd?: string;
};

const desktopDirectory = resolve(import.meta.dirname, "..");
const repositoryRoot = resolve(desktopDirectory, "../..");
const runId = `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`;
const artifactsDirectory = resolve(repositoryRoot, "artifacts/e2e", runId);
const fixtureRoot = mkdtempSync(join(tmpdir(), "gitru-cef-e2e-"));
const fixtureRepository = join(fixtureRoot, "repository");
const fixtureRemote = join(fixtureRoot, "gitru-e2e-remote.git");
const fixtureGitConfig = join(fixtureRoot, "global.gitconfig");
const fixtureHooks = join(fixtureRoot, "hooks");
const cefProfile = join(fixtureRoot, "cef-profile");

mkdirSync(artifactsDirectory, { recursive: true });

function git(args: string[], options: GitOptions = {}): string {
  const result = spawnSync("git", args, {
    cwd: options.cwd ?? fixtureRepository,
    encoding: "utf8",
    env: isolatedGitEnvironment(fixtureGitConfig),
  });

  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
  if (result.status !== 0 && !options.allowFailure) {
    throw new Error(
      `git ${args.join(" ")} failed (${result.status}):\n${output}`,
    );
  }
  return output;
}

function createFixture(): void {
  writeFileSync(fixtureGitConfig, "");
  mkdirSync(fixtureHooks);
  git(["init", "--bare", fixtureRemote], { cwd: fixtureRoot });
  git(["init", "--initial-branch=main", fixtureRepository], {
    cwd: fixtureRoot,
  });
  git(["config", "user.name", "Gitru E2E"]);
  // Keep commit avatars on their local fallback instead of requesting GitHub.
  git(["config", "user.email", ""]);
  git(["config", "commit.gpgSign", "false"]);
  git(["config", "tag.gpgSign", "false"]);
  git(["config", "core.hooksPath", fixtureHooks]);

  writeFileSync(join(fixtureRepository, "README.md"), "# Gitru E2E\n");
  writeFileSync(join(fixtureRepository, "conflict.txt"), "base\n");
  writeFileSync(join(fixtureRepository, "stash-note.txt"), "clean\n");
  git(["add", "."]);
  git(["commit", "-m", "fixture: initial state"]);
  git(["remote", "add", "origin", fixtureRemote]);
  git(["push", "-u", "origin", "main"]);

  const baseCommit = git(["rev-parse", "HEAD"]);
  git(["switch", "-c", "conflict-base"]);
  writeFileSync(
    join(fixtureRepository, "conflict.txt"),
    "conflict from base\n",
  );
  git(["add", "conflict.txt"]);
  git(["commit", "-m", "fixture: conflict base"]);

  git(["switch", "-c", "conflict-work", baseCommit]);
  writeFileSync(
    join(fixtureRepository, "conflict.txt"),
    "conflict from work\n",
  );
  git(["add", "conflict.txt"]);
  git(["commit", "-m", "fixture: conflict work"]);

  git(["switch", "main"]);
  writeFileSync(
    join(fixtureRepository, "stage-me.txt"),
    "created by the packaged E2E fixture\n",
  );
}

function collectGitDiagnostics(): void {
  if (!existsSync(fixtureRepository)) return;
  const commands: Array<[string, string[]]> = [
    ["status", ["status", "--short", "--branch"]],
    ["log", ["log", "--graph", "--decorate", "--oneline", "--all", "-20"]],
    ["branches", ["branch", "--all", "--verbose", "--verbose"]],
    ["stash", ["stash", "list"]],
    ["unstaged-diff", ["diff"]],
    ["staged-diff", ["diff", "--cached"]],
    ["unmerged", ["ls-files", "--unmerged"]],
  ];

  const report = commands
    .map(([label, args]) => {
      const output = git(args, { allowFailure: true });
      return `## ${label}\n${output || "(empty)"}`;
    })
    .join("\n\n");

  writeFileSync(join(artifactsDirectory, "git-state.txt"), `${report}\n`);
}

function resolveTargetDirectory(): string {
  const configured = process.env.CARGO_TARGET_DIR;
  if (!configured) return resolve(repositoryRoot, "target");
  if (isAbsolute(configured)) return configured;
  return resolve(desktopDirectory, configured);
}

function resolveAppBinary(): string {
  if (process.env.GITRU_E2E_BINARY) {
    return resolve(process.env.GITRU_E2E_BINARY);
  }

  const releaseDirectory = resolve(resolveTargetDirectory(), "release");
  if (process.platform === "darwin") {
    return resolve(
      releaseDirectory,
      "bundle/macos/Gitru E2E.app/Contents/MacOS/gitru",
    );
  }
  return resolve(
    releaseDirectory,
    process.platform === "win32" ? "gitru.exe" : "gitru",
  );
}

async function reservePort(): Promise<number> {
  const server = createServer();
  await new Promise<void>((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolveListen);
  });
  const address = server.address() as AddressInfo;
  await new Promise<void>((resolveClose, reject) => {
    server.close((error) => (error ? reject(error) : resolveClose()));
  });
  if (address.port < 1024) {
    throw new Error(
      `Operating system returned privileged port ${address.port}`,
    );
  }
  return address.port;
}

async function stopApp(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return;

  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], {
      stdio: "ignore",
    });
    return;
  }

  child.kill("SIGTERM");
  await Promise.race([
    new Promise<void>((resolveExit) => child.once("exit", () => resolveExit())),
    new Promise<void>((resolveDelay) => setTimeout(resolveDelay, 3_000)),
  ]);
  if (child.exitCode === null && child.signalCode === null) {
    child.kill("SIGKILL");
  }
}

function collectCefDiagnostics(): void {
  const cefLog = resolve(cefProfile, "cef.log");
  if (existsSync(cefLog)) {
    copyFileSync(cefLog, resolve(artifactsDirectory, "cef.log"));
  }
}

async function run(): Promise<number> {
  createFixture();
  const cdpPort = await reservePort();
  const appBinaryPath = resolveAppBinary();
  if (!existsSync(appBinaryPath)) {
    throw new Error(
      `CEF E2E binary not found at ${appBinaryPath}. Run bun run e2e:build first or set GITRU_E2E_BINARY.`,
    );
  }

  const metadata = {
    appBinaryPath,
    artifactsDirectory,
    cdpPort,
    fixtureRepository,
    fixtureRoot,
    platform: process.platform,
    runId,
    runtime: "cef",
  };
  writeFileSync(
    join(artifactsDirectory, "run.json"),
    `${JSON.stringify(metadata, null, 2)}\n`,
  );

  console.log(`Gitru CEF E2E fixture: ${fixtureRepository}`);
  console.log(`Gitru CEF E2E artifacts: ${artifactsDirectory}`);
  console.log(`Gitru CEF CDP endpoint: http://127.0.0.1:${cdpPort}`);

  const logPath = join(artifactsDirectory, "app.log");
  const log = createWriteStream(logPath, { flags: "a" });
  const child = spawn(appBinaryPath, [], {
    cwd: dirname(appBinaryPath),
    env: isolatedGitEnvironment(fixtureGitConfig, {
      GITRU_E2E_ARTIFACTS: artifactsDirectory,
      GITRU_E2E_CDP_PORT: String(cdpPort),
      GITRU_E2E_REPO: fixtureRepository,
      GITRU_E2E_RESET: "1",
      GITRU_E2E_TEMP_ROOT: fixtureRoot,
      UPDATER_BASE_URL: "http://127.0.0.1:9",
    }),
    stdio: ["ignore", "pipe", "pipe"],
  });
  let spawnError: Error | undefined;
  child.once("error", (error) => {
    spawnError = error;
  });
  child.stdout?.on("data", (chunk) => {
    log.write(chunk);
  });
  child.stderr?.on("data", (chunk) => {
    log.write(chunk);
  });

  try {
    const initialTargets = await waitFor(
      async () => {
        if (spawnError) throw spawnError;
        if (child.exitCode !== null || child.signalCode !== null) {
          throw new Error(
            `Gitru exited before CDP was ready (code=${child.exitCode}, signal=${child.signalCode})`,
          );
        }
        return listCdpTargets(cdpPort);
      },
      (targets) => {
        const classified = classifyGitruTargets(targets);
        return Boolean(classified.host);
      },
      "the Gitru host target",
      { intervalMs: 200, timeoutMs: 120_000 },
    );
    const version = await readCdpVersion(cdpPort);
    if (!/(?:Chrome|Chromium)\//.test(version.Browser ?? "")) {
      throw new Error(
        `Remote endpoint did not identify a Chromium runtime: ${JSON.stringify(version)}`,
      );
    }
    writeFileSync(
      join(artifactsDirectory, "cdp-version.json"),
      `${JSON.stringify(version, null, 2)}\n`,
    );
    writeFileSync(
      join(artifactsDirectory, "targets-initial.json"),
      `${JSON.stringify(initialTargets, null, 2)}\n`,
    );

    const classified = classifyGitruTargets(initialTargets);
    if (!classified.host) {
      throw new Error("CEF target topology changed after discovery");
    }

    const host = await CdpClient.connect(classified.host);
    const result = await runCefSmoke({
      artifactsDirectory,
      cdpPort,
      fixtureRemote,
      fixtureRepository,
      fixtureRoot,
      host,
    });
    writeFileSync(
      join(artifactsDirectory, "frontend-diagnostics.json"),
      `${JSON.stringify(result.diagnostics, null, 2)}\n`,
    );
    writeFileSync(
      join(artifactsDirectory, "run.json"),
      `${JSON.stringify({ ...metadata, browser: version.Browser, exitCode: 0 }, null, 2)}\n`,
    );
    return 0;
  } finally {
    await stopApp(child);
    log.end();
  }
}

let exitCode = 1;
try {
  exitCode = await run();
} catch (error) {
  const message =
    error instanceof Error ? (error.stack ?? error.message) : String(error);
  console.error(message);
  writeFileSync(join(artifactsDirectory, "runner-error.txt"), `${message}\n`);
} finally {
  collectGitDiagnostics();
  collectCefDiagnostics();
  if (process.env.GITRU_E2E_KEEP_TEMP === "1") {
    console.log(`Preserving CEF E2E fixture: ${fixtureRoot}`);
  } else {
    rmSync(fixtureRoot, { force: true, recursive: true });
  }
}

if (exitCode !== 0) {
  console.error(
    `Gitru CEF E2E failed. Inspect ${basename(artifactsDirectory)} under artifacts/e2e.`,
  );
}
process.exitCode = exitCode;
