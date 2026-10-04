/** Owns fixture lifetime; each hard crash gets a fresh app/driver session. */
import { spawn, spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  cpSync,
  createReadStream,
  createWriteStream,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { isolatedGitEnvironment } from "./git-environment";
import { harnessEnvironment } from "./harness-environment";
import { canonicalHarnessPath } from "./harness-paths";
import {
  assertHarnessCrashProof,
  assertHarnessQualification,
} from "./harness-qualification";

const desktop = resolve(import.meta.dirname, "..");
const repository = resolve(desktop, "../..");
const id = `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`;
const artifacts = resolve(repository, "artifacts/e2e-harness", id);
const root = realpathSync(mkdtempSync(join(tmpdir(), "gitru-collaboration-")));
const gitConfig = join(root, "global.gitconfig");
const driverEnvironment = join(root, "driver.env");
const gitRepository = join(root, "repository");
const target = process.env.CARGO_TARGET_DIR
  ? resolve(desktop, "src-tauri", process.env.CARGO_TARGET_DIR)
  : resolve(repository, "target");
const binary = canonicalHarnessPath(
  process.env.GITRU_E2E_BINARY ??
    resolve(
      target,
      "release",
      process.platform === "win32" ? "gitru.exe" : "gitru",
    ),
);
const stages: Array<{
  phase: string;
  root: string;
  runNonce: string;
  exitCode: number;
}> = [];
const inheritedEnvironment = harnessEnvironment(process.env);
mkdirSync(artifacts, { recursive: true });
writeFileSync(gitConfig, "", { flag: "wx", mode: 0o600 });
// The installed WDIO CLI imports dotenv/config. Its default cwd .env must not
// reintroduce variables after our inherited-environment whitelist.
writeFileSync(driverEnvironment, "", { flag: "wx", mode: 0o600 });
const git = spawnSync("git", ["init", "--initial-branch=main", gitRepository], {
  env: isolatedGitEnvironment(gitConfig, {}, inheritedEnvironment),
  encoding: "utf8",
});
if (git.status !== 0)
  throw new Error("Could not initialize the isolated Git fixture");

async function binaryHash() {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(binary)) hash.update(chunk);
  return hash.digest("hex");
}
function fixture(name: string) {
  const directory = join(root, name);
  mkdirSync(directory, { mode: 0o700 });
  const runNonce = randomUUID();
  writeFileSync(
    join(directory, "run.json"),
    JSON.stringify({
      version: 1,
      application_id: "com.ruru.gitru.e2e.collaboration",
      run_nonce: runNonce,
    }),
    { flag: "wx", mode: 0o600 },
  );
  // Keep outer Git paths in their Node spelling. Only the native collaboration
  // subroot and its evidence use the exact canonical spelling Rust requires.
  return { root: canonicalHarnessPath(directory), runNonce };
}
async function runPhase(
  name: string,
  phase: "main" | "crash-before-commit" | "crash-after-commit" | "restart",
  owned: ReturnType<typeof fixture>,
) {
  const output = join(owned.root, "artifacts", name);
  const exported = join(artifacts, name);
  mkdirSync(output, { recursive: true, mode: 0o700 });
  // tauri-service owns wdio.log in this directory. Give captured outer stdout
  // a separate file so its diagnostic writer cannot truncate our stream.
  const log = createWriteStream(join(output, "runner.log"), { flags: "wx" });
  const child = spawn(
    process.execPath,
    ["x", "wdio", "run", "e2e/wdio.harness.conf.ts"],
    {
      cwd: desktop,
      env: isolatedGitEnvironment(
        gitConfig,
        {
          GITRU_E2E_BINARY: binary,
          GITRU_E2E_ARTIFACTS: output,
          GITRU_E2E_REPO: gitRepository,
          GITRU_E2E_TEMP_ROOT: root,
          GITRU_COLLABORATION_HARNESS_ROOT: owned.root,
          GITRU_COLLABORATION_HARNESS_RUN_NONCE: owned.runNonce,
          GITRU_COLLABORATION_HARNESS_PHASE: phase,
          DOTENV_CONFIG_PATH: driverEnvironment,
        },
        inheritedEnvironment,
      ),
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  child.stdout.on("data", (data) => {
    process.stdout.write(data);
    log.write(data);
  });
  child.stderr.on("data", (data) => {
    process.stderr.write(data);
    log.write(data);
  });
  let expired = false;
  const exitCode = await new Promise<number>((done, reject) => {
    const timer = setTimeout(
      () => {
        expired = true;
        // The launcher retains its actual native handles; ask it to stop those
        // before terminating our own driver handle. No descendant PID lookup.
        void (async () => {
          let stopError: unknown;
          try {
            writeFileSync(
              join(output, "stop-driver.json"),
              JSON.stringify({ run_nonce: owned.runNonce }),
              { flag: "wx", mode: 0o600 },
            );
            const until = Date.now() + 15_000;
            let stopped = false;
            while (Date.now() < until) {
              try {
                const receipt = JSON.parse(
                  readFileSync(join(output, "driver-stopped.json"), "utf8"),
                );
                if (receipt.run_nonce !== owned.runNonce)
                  throw new Error(
                    "Driver stop proof belongs to a different run",
                  );
                stopped = true;
                break;
              } catch (error) {
                if ((error as NodeJS.ErrnoException).code !== "ENOENT")
                  throw error;
              }
              await new Promise((settle) => setTimeout(settle, 100));
            }
            if (!stopped)
              throw new Error("Native owned-driver stop was not observed");
          } catch (error) {
            stopError = error;
          } finally {
            // Even invalid stop evidence cannot orphan the exact WDIO child
            // we created. This never substitutes for native crash proof.
            if (child.exitCode === null && child.signalCode === null) {
              await new Promise<void>((closed, failed) => {
                const forced = setTimeout(() => child.kill("SIGKILL"), 5_000);
                const limit = setTimeout(() => {
                  clearTimeout(forced);
                  failed(new Error("Owned WDIO driver did not exit"));
                }, 10_000);
                child.once("close", () => {
                  clearTimeout(forced);
                  clearTimeout(limit);
                  closed();
                });
                child.kill("SIGTERM");
              });
            }
          }
          throw new Error(
            `Retained driver phase ${name} exceeded its deadline`,
            {
              cause: stopError,
            },
          );
        })().catch(reject);
      },
      phase === "main" ? 20 * 60_000 : 5 * 60_000,
    );
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("close", (code) => {
      clearTimeout(timer);
      if (!expired) done(code ?? 1);
    });
  }).finally(async () => {
    await new Promise<void>((done) => log.end(done));
    cpSync(output, exported, {
      recursive: true,
      errorOnExist: true,
      force: false,
    });
  });
  stages.push({ phase: name, ...owned, exitCode });
  if (expired)
    throw new Error(`Retained driver phase ${name} exceeded its deadline`);
  if (exitCode !== 0)
    throw new Error(`Retained driver phase ${name} failed (${exitCode})`);
  assertHarnessQualification(output, owned.runNonce, phase);
  if (phase.startsWith("crash-")) {
    assertHarnessCrashProof(output, owned.runNonce, phase, binary);
  }
}

let passed = false;
try {
  writeFileSync(
    join(artifacts, "run.json"),
    JSON.stringify(
      {
        id,
        platform: process.platform,
        root,
        binary,
        binary_sha256: await binaryHash(),
        application_id: "com.ruru.gitru.e2e.collaboration",
        normal_preferences:
          "fixed harness-ID OS namespace; never reset by runner",
      },
      null,
      2,
    ),
  );
  await runPhase("main", "main", fixture("main"));
  const before = fixture("before");
  await runPhase("before", "crash-before-commit", before);
  await runPhase("restart-before", "restart", before);
  const after = fixture("after");
  await runPhase("after", "crash-after-commit", after);
  await runPhase("restart-after", "restart", after);
  passed = true;
} catch (error) {
  const message =
    error instanceof Error ? (error.stack ?? error.message) : String(error);
  writeFileSync(join(artifacts, "runner-error.txt"), message);
  console.error(message);
  process.exitCode = 1;
} finally {
  writeFileSync(
    join(artifacts, "stages.json"),
    JSON.stringify({ passed, stages }, null, 2),
  );
  if (passed && process.env.GITRU_E2E_KEEP_TEMP !== "1") {
    rmSync(root, { recursive: true });
  } else {
    console.log(`Retained task-owned fixture: ${root}`);
  }
  console.log(`Retained qualification artifacts: ${artifacts}`);
}
