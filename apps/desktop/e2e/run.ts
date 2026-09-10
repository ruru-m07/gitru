import { spawn, spawnSync } from "node:child_process";
import {
  createWriteStream,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { isolatedGitEnvironment } from "./git-environment";

type GitOptions = {
  allowFailure?: boolean;
  cwd?: string;
};

const desktopDirectory = resolve(import.meta.dirname, "..");
const repositoryRoot = resolve(desktopDirectory, "../..");
const runId = `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`;
const artifactsDirectory = resolve(repositoryRoot, "artifacts/e2e", runId);
const fixtureRoot = mkdtempSync(join(tmpdir(), "gitru-e2e-"));
const fixtureRepository = join(fixtureRoot, "repository");
const fixtureRemote = join(fixtureRoot, "gitru-e2e-remote.git");
const fixtureGitConfig = join(fixtureRoot, "global.gitconfig");
const fixtureHooks = join(fixtureRoot, "hooks");
const runMetadata = {
  artifactsDirectory,
  fixtureRepository,
  fixtureRoot,
  platform: process.platform,
  runId,
};

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

async function run(): Promise<number> {
  createFixture();
  writeFileSync(
    join(artifactsDirectory, "run.json"),
    `${JSON.stringify(runMetadata, null, 2)}\n`,
  );

  console.log(`Gitru E2E fixture: ${fixtureRepository}`);
  console.log(`Gitru E2E artifacts: ${artifactsDirectory}`);

  const logPath = join(artifactsDirectory, "wdio.log");
  const log = createWriteStream(logPath, { flags: "a" });
  const child = spawn(
    process.execPath,
    ["x", "wdio", "run", "e2e/wdio.conf.ts"],
    {
      cwd: desktopDirectory,
      env: isolatedGitEnvironment(fixtureGitConfig, {
        GITRU_E2E_ARTIFACTS: artifactsDirectory,
        GITRU_E2E_REPO: fixtureRepository,
        GITRU_E2E_RESET: "1",
        GITRU_E2E_TEMP_ROOT: fixtureRoot,
      }),
      stdio: ["inherit", "pipe", "pipe"],
    },
  );

  child.stdout.on("data", (chunk) => {
    process.stdout.write(chunk);
    log.write(chunk);
  });
  child.stderr.on("data", (chunk) => {
    process.stderr.write(chunk);
    log.write(chunk);
  });

  const exitCode = await new Promise<number>((resolveExit, reject) => {
    child.once("error", reject);
    child.once("close", (code) => resolveExit(code ?? 1));
  });
  log.end();
  collectGitDiagnostics();
  writeFileSync(
    join(artifactsDirectory, "run.json"),
    `${JSON.stringify({ ...runMetadata, exitCode }, null, 2)}\n`,
  );
  return exitCode;
}

let exitCode = 1;
try {
  exitCode = await run();
} catch (error) {
  const message =
    error instanceof Error ? (error.stack ?? error.message) : String(error);
  console.error(message);
  writeFileSync(join(artifactsDirectory, "runner-error.txt"), `${message}\n`);
  collectGitDiagnostics();
} finally {
  if (process.env.GITRU_E2E_KEEP_TEMP === "1") {
    console.log(`Preserving E2E fixture: ${fixtureRoot}`);
  } else {
    rmSync(fixtureRoot, { force: true, recursive: true });
  }
}

if (exitCode !== 0) {
  console.error(
    `Gitru E2E failed. Inspect ${basename(artifactsDirectory)} under artifacts/e2e.`,
  );
}
process.exitCode = exitCode;
