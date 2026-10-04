import { lstatSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { isDeepStrictEqual } from "node:util";
import { HarnessCheckpointSchema } from "@gitru/commands";
import { browser } from "@wdio/globals";
import type {
  TauriCapabilities,
  TauriServiceOptions,
} from "@wdio/tauri-service";
import { isolateCurrentProcessGitEnvironment } from "./git-environment";
import { HarnessScenarioResultSchema } from "./protocol/collaboration-harness";

function required(name: string) {
  const value = process.env[name];
  if (!value) throw new Error(`${name} must be set by the retained runner`);
  return value;
}
const root = required("GITRU_COLLABORATION_HARNESS_ROOT");
const runNonce = required("GITRU_COLLABORATION_HARNESS_RUN_NONCE");
const phase = required("GITRU_COLLABORATION_HARNESS_PHASE");
if (
  !["main", "crash-before-commit", "crash-after-commit", "restart"].includes(
    phase,
  )
)
  throw new Error("Unknown retained driver phase");
const artifacts = required("GITRU_E2E_ARTIFACTS");
const fixtureRoot = required("GITRU_E2E_TEMP_ROOT");
const gitConfig = resolve(fixtureRoot, "global.gitconfig");
isolateCurrentProcessGitEnvironment(gitConfig);
const binary = required("GITRU_E2E_BINARY");

const options: TauriServiceOptions = {
  appBinaryPath: binary,
  captureBackendLogs: true,
  captureFrontendLogs: true,
  clearMocks: false,
  driverProvider: "embedded",
  env: {
    GIT_CONFIG_GLOBAL: gitConfig,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_TERMINAL_PROMPT: "0",
    GITRU_E2E_ARTIFACTS: artifacts,
    GITRU_E2E_REPO: required("GITRU_E2E_REPO"),
    GITRU_E2E_TEMP_ROOT: fixtureRoot,
    GITRU_COLLABORATION_HARNESS_ROOT: root,
    GITRU_COLLABORATION_HARNESS_RUN_NONCE: runNonce,
    GITRU_COLLABORATION_HARNESS_PHASE: phase,
    UPDATER_BASE_URL: "http://127.0.0.1:9",
  },
  logDir: artifacts,
  resetMocks: false,
  restoreMocks: false,
  startTimeout: 120_000,
  statusPollTimeout: 10_000,
};
const capability: TauriCapabilities = {
  browserName: "tauri",
  "tauri:options": { application: binary },
};

function boundedJson(path: string, maximum: number) {
  const metadata = lstatSync(path);
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    metadata.size > maximum
  )
    throw new Error("Retained driver evidence exceeds its file bound");
  return JSON.parse(readFileSync(path, "utf8"));
}

async function acknowledgeCrash() {
  const checkpoint = HarnessCheckpointSchema.parse(
    boundedJson(resolve(root, "crash-checkpoint.json"), 4096),
  );
  const expectedKind =
    phase === "crash-before-commit" ? "before_commit" : "committed_before_hint";
  const scenario = HarnessScenarioResultSchema.parse(
    boundedJson(resolve(artifacts, `scenario-${phase}.json`), 128 * 1024),
  );
  if (
    checkpoint.run_nonce !== runNonce ||
    checkpoint.kind !== expectedKind ||
    scenario.scenario !== phase ||
    scenario.outcome !== "checkpoint" ||
    scenario.status?.core.run_nonce !== runNonce ||
    scenario.status.core.session_id !== checkpoint.session_id ||
    scenario.status.core.scenario_generation !==
      checkpoint.scenario_generation ||
    scenario.status.process_id !== checkpoint.process_id ||
    !isDeepStrictEqual(scenario.status?.checkpoint, checkpoint)
  )
    throw new Error(
      "Passed driver case did not match the native crash checkpoint",
    );
  writeFileSync(
    resolve(artifacts, "crash-driver-ack.json"),
    JSON.stringify({
      run_nonce: checkpoint.run_nonce,
      session_id: checkpoint.session_id,
      scenario_generation: checkpoint.scenario_generation,
      kind: checkpoint.kind,
      process_id: checkpoint.process_id,
    }),
    { flag: "wx", mode: 0o600 },
  );
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    try {
      const error = boundedJson(
        resolve(artifacts, "process-exit-error.json"),
        4096,
      );
      throw new Error(`Launch-owned crash failed: ${error.reason}`);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
    try {
      const proof = boundedJson(resolve(artifacts, "process-exit.json"), 4096);
      if (
        proof.run_nonce !== runNonce ||
        proof.session_id !== checkpoint.session_id ||
        proof.process_id !== checkpoint.process_id ||
        proof.phase !== phase ||
        proof.requested_signal !== "SIGKILL" ||
        proof.observed_signal !== "SIGKILL"
      )
        throw new Error(
          "Forced-exit proof belongs to a different native process",
        );
      return;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
    await new Promise((done) => setTimeout(done, 100));
  }
  throw new Error("Launch-owned native hard exit was not observed");
}

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/collaboration-harness.e2e.ts"],
  maxInstances: 1,
  capabilities: [capability],
  services: [[resolve(import.meta.dirname, "harness-service.ts"), options]],
  outputDir: artifacts,
  logLevel: "info",
  bail: 1,
  waitforTimeout: 25_000,
  connectionRetryTimeout: 10_000,
  connectionRetryCount: 0,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 190_000 },
  onPrepare: () => mkdirSync(artifacts, { recursive: true }),
  afterTest: async (test, _context, result) => {
    if (result.passed) {
      if (phase.startsWith("crash-")) await acknowledgeCrash();
      return;
    }
    const name = test.title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .slice(0, 80);
    await browser
      .saveScreenshot(resolve(artifacts, `${name}.png`))
      .catch(() => undefined);
    const source = await browser.getPageSource().catch(() => "");
    if (source) writeFileSync(resolve(artifacts, `${name}.html`), source);
  },
};
