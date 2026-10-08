/** Owns one isolated saved cache across a packaged seed process and cold restart. */
import { spawn, spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  createReadStream,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { arch, cpus, platform, release, totalmem } from "node:os";
import { join, resolve } from "node:path";
import { isolatedGitEnvironment } from "./git-environment";
import { harnessEnvironment } from "./harness-environment";
import { canonicalHarnessPath } from "./harness-paths";
import { distribution } from "./performance-metrics";
import { HarnessScenarioResultSchema } from "./protocol/collaboration-harness";

const desktop = resolve(import.meta.dirname, "..");
const repository = resolve(desktop, "../..");
const id = `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`;
const retained = resolve(repository, "artifacts/collaboration-performance", id);
mkdirSync(retained, { recursive: true, mode: 0o700 });
const root = realpathSync(mkdtempSync(join(retained, ".fixture-")));
const runNonce = randomUUID();
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
const inherited = harnessEnvironment(process.env);
const storageMode = process.env.GITRU_COLLABORATION_STORAGE_MODE ?? "plaintext";
if (storageMode !== "plaintext" && storageMode !== "keyed")
  throw new Error("Storage mode must be plaintext or keyed");
writeFileSync(
  join(root, "run.json"),
  JSON.stringify({
    version: 1,
    application_id: "com.ruru.gitru.e2e.collaboration",
    run_nonce: runNonce,
    storage_mode: storageMode,
  }),
  { flag: "wx", mode: 0o600 },
);
writeFileSync(gitConfig, "", { flag: "wx", mode: 0o600 });
writeFileSync(driverEnvironment, "", { flag: "wx", mode: 0o600 });
const git = spawnSync("git", ["init", "--initial-branch=main", gitRepository], {
  env: isolatedGitEnvironment(gitConfig, {}, inherited),
  encoding: "utf8",
});
if (git.status !== 0)
  throw new Error("Could not initialize performance Git fixture");

async function binaryHash() {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(binary)) hash.update(chunk);
  return hash.digest("hex");
}

function databaseSizes() {
  const size = (name: string) => {
    try {
      const stat = statSync(join(root, name));
      return stat.isFile() ? stat.size : null;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
      throw error;
    }
  };
  return {
    database_bytes: size("collaboration.sqlite"),
    wal_bytes: size("collaboration.sqlite-wal"),
    shm_bytes: size("collaboration.sqlite-shm"),
  };
}

function storageObservation() {
  const df = spawnSync("df", ["-Pk", root], { encoding: "utf8" });
  const diskutil =
    process.platform === "darwin"
      ? spawnSync("diskutil", ["info", root], { encoding: "utf8" })
      : null;
  const detail = diskutil?.status === 0 ? diskutil.stdout : "";
  const value = (name: string) =>
    detail.match(new RegExp(`^\\s*${name}:\\s*(.+)$`, "m"))?.[1]?.trim() ??
    null;
  const solid = value("Solid State");
  const location = value("Device Location");
  return {
    fixture_path: root,
    df: df.status === 0 ? df.stdout.trim().split("\n").at(-1) : null,
    device_location: location,
    solid_state: solid,
    classification:
      location === "External" && solid === "Yes"
        ? "external_ssd"
        : location === "Internal" && solid === "Yes"
          ? "internal_ssd"
          : "unverified",
  };
}

async function runPhase(mode: "seed" | "restart") {
  const output = join(root, "artifacts", mode);
  mkdirSync(output, { recursive: true, mode: 0o700 });
  const before = databaseSizes();
  const child = spawn(
    process.execPath,
    ["x", "wdio", "run", "e2e/wdio.performance.conf.ts"],
    {
      cwd: desktop,
      env: isolatedGitEnvironment(
        gitConfig,
        {
          GITRU_E2E_BINARY: binary,
          GITRU_E2E_ARTIFACTS: output,
          GITRU_E2E_REPO: gitRepository,
          GITRU_E2E_TEMP_ROOT: root,
          GITRU_COLLABORATION_HARNESS_ROOT: root,
          GITRU_COLLABORATION_HARNESS_RUN_NONCE: runNonce,
          GITRU_COLLABORATION_HARNESS_PHASE: "main",
          GITRU_COLLABORATION_PERFORMANCE: "1",
          GITRU_COLLABORATION_PERFORMANCE_MODE: mode,
          DOTENV_CONFIG_PATH: driverEnvironment,
        },
        inherited,
      ),
      stdio: "inherit",
    },
  );
  let expired = false;
  const exitCode = await new Promise<number>((done, reject) => {
    const timeout = setTimeout(() => {
      expired = true;
      child.kill("SIGTERM");
      setTimeout(() => {
        if (child.exitCode === null && child.signalCode === null)
          child.kill("SIGKILL");
      }, 5_000);
    }, 10 * 60_000);
    child.once("error", (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    child.once("close", (code) => {
      clearTimeout(timeout);
      done(code ?? 1);
    });
  });
  if (exitCode !== 0) {
    for (const entry of readdirSync(output, { withFileTypes: true })) {
      if (
        !entry.isFile() ||
        !/^wdio(?:-[A-Za-z0-9_.-]+)?\.log$/.test(entry.name)
      )
        continue;
      const source = join(output, entry.name);
      const stat = lstatSync(source);
      if (stat.isSymbolicLink() || stat.size > 1024 * 1024) continue;
      writeFileSync(
        join(retained, `${mode}-${entry.name}`),
        readFileSync(source),
        {
          flag: "wx",
          mode: 0o600,
        },
      );
    }
  }
  if (expired)
    throw new Error(`Performance ${mode} phase exceeded ten minutes`);
  if (exitCode !== 0)
    throw new Error(`Performance ${mode} phase failed (${exitCode})`);
  const raw = HarnessScenarioResultSchema.parse(
    JSON.parse(readFileSync(join(output, "raw-performance.json"), "utf8")),
  );
  if (!raw.performance || !raw.status)
    throw new Error("Performance phase omitted bounded evidence");
  const memory = JSON.parse(
    readFileSync(join(output, "process-memory.json"), "utf8"),
  );
  return { mode, before, after: databaseSizes(), raw, memory };
}

function summarize(phases: Awaited<ReturnType<typeof runPhase>>[]) {
  return phases.flatMap(
    (phase) =>
      phase.raw.performance?.views.flatMap((view) =>
        view.cases.map((entry) => ({
          phase: phase.mode,
          webview_label: view.webview_label,
          role: view.role,
          name: entry.name,
          boundary: entry.boundary,
          duration_ms: distribution(
            entry.samples.map((sample) => sample.duration_ms),
          ),
          payload_bytes:
            entry.samples[0]?.payload_bytes === null
              ? null
              : distribution(
                  entry.samples.map((sample) => sample.payload_bytes ?? 0),
                ),
        })),
      ) ?? [],
  );
}

function nativeSummaries(phases: Awaited<ReturnType<typeof runPhase>>[]) {
  return phases.flatMap((phase) => {
    const grouped = new Map<string, number[]>();
    for (const sample of phase.raw.status?.performance_queries ?? []) {
      const key = `${sample.webview_label}\0${sample.kind}`;
      const list = grouped.get(key) ?? [];
      list.push(Number(sample.elapsed_micros) / 1000);
      grouped.set(key, list);
    }
    return [...grouped].map(([key, samples]) => {
      const [webview_label, kind] = key.split("\0", 2);
      return {
        phase: phase.mode,
        webview_label,
        kind,
        duration_ms: distribution(samples),
      };
    });
  });
}

let passed = false;
try {
  const seed = await runPhase("seed");
  const restart = await runPhase("restart");
  const phases = [seed, restart];
  const report = {
    version: 1,
    id,
    source_sha: spawnSync("git", ["rev-parse", "HEAD"], {
      cwd: repository,
      encoding: "utf8",
    }).stdout.trim(),
    binary_sha256: await binaryHash(),
    build_profile: "release/no-bundle/collaboration-harness",
    storage_mode: storageMode,
    dataset: {
      version: 1,
      item_count: 10_000,
      account_count: 2,
      repositories_per_account: 5,
      items_per_account: 5_000,
      seed_page_size: 100,
      measurement_query_limit: 50,
    },
    hardware: {
      platform: platform(),
      release: release(),
      architecture: arch(),
      cpu_model: cpus()[0]?.model ?? null,
      logical_cpu_count: cpus().length,
      total_memory_bytes: totalmem(),
    },
    storage: storageObservation(),
    database: phases.map(({ mode, before, after }) => ({
      mode,
      before,
      after,
    })),
    startup: phases.flatMap((phase) =>
      (phase.raw.performance?.views ?? []).map((view) => ({
        phase: phase.mode,
        role: view.role,
        webview_label: view.webview_label,
        native_runtime_open_ms:
          Number(phase.raw.status?.runtime_open_micros ?? "0") / 1000,
        landing_mode: view.landing_mode,
        runtime_ready_to_useful_ms:
          view.first_useful_epoch_ms -
          Number(phase.raw.status?.runtime_ready_epoch_ms ?? "0"),
        document_navigation_to_useful_ms: view.navigation_to_first_useful_ms,
        document_navigation_to_workspace_mount_ms:
          view.workspace_mount_epoch_ms -
          (view.first_useful_epoch_ms - view.navigation_to_first_useful_ms),
        workspace_mount_to_useful_ms:
          view.first_useful_epoch_ms - view.workspace_mount_epoch_ms,
        benchmark_request_offset_from_useful_ms:
          view.benchmark_request_epoch_ms - view.first_useful_epoch_ms,
      })),
    ),
    distributions: summarize(phases),
    native_sqlite_projections: nativeSummaries(phases),
    process_memory: phases.map(({ mode, memory }) => ({ mode, ...memory })),
    correctness: {
      provider_calls_unchanged: phases.every(
        (phase) =>
          phase.raw.performance?.provider_call_count_before ===
          phase.raw.performance?.provider_call_count_after,
      ),
      vault_loads_unchanged: phases.every(
        (phase) =>
          phase.raw.performance?.vault_load_count_before ===
          phase.raw.performance?.vault_load_count_after,
      ),
      generated_ipc_validation: true,
      real_react_workspace: true,
      retained_native_view_count: 2,
    },
    limits: [
      storageMode === "keyed"
        ? "The database key is synthetic and confined to this task-owned fixture; this does not qualify a personal platform vault."
        : "This is the unchanged plaintext control for comparison with an explicitly keyed run.",
      "This 10,000-item macOS run does not qualify the 100,000-summary or 500,000-child-record memory target.",
      "WebKit RSS is reported only for proven native-process descendants; per-view RSS is unavailable for a shared runtime.",
      "Process-launch latency is not claimed; native setup/runtime readiness and document useful-content clocks are recorded separately.",
      "Timing thresholds remain provisional workstation observations, not universal CI gates.",
    ],
  };
  writeFileSync(
    join(retained, "performance-report.json"),
    `${JSON.stringify(report, null, 2)}\n`,
    { flag: "wx", mode: 0o600 },
  );
  for (const phase of phases) {
    writeFileSync(
      join(retained, `raw-${phase.mode}.json`),
      `${JSON.stringify(phase.raw, null, 2)}\n`,
      { flag: "wx", mode: 0o600 },
    );
    writeFileSync(
      join(retained, `memory-${phase.mode}.json`),
      `${JSON.stringify(phase.memory, null, 2)}\n`,
      { flag: "wx", mode: 0o600 },
    );
  }
  passed = true;
  console.log(
    `Retained performance report: ${join(retained, "performance-report.json")}`,
  );
} catch (error) {
  writeFileSync(
    join(retained, "performance-error.txt"),
    error instanceof Error ? (error.stack ?? error.message) : String(error),
    { mode: 0o600 },
  );
  console.error(error);
  process.exitCode = 1;
} finally {
  if (passed && process.env.GITRU_E2E_KEEP_TEMP !== "1")
    rmSync(root, { recursive: true });
  else console.log(`Retained task-owned fixture: ${root}`);
}
