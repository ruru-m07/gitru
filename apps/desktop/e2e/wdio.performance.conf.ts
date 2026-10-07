import { lstatSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { browser } from "@wdio/globals";
import type {
  TauriCapabilities,
  TauriServiceOptions,
} from "@wdio/tauri-service";
import { isolateCurrentProcessGitEnvironment } from "./git-environment";
import { HarnessScenarioResultSchema } from "./protocol/collaboration-harness";

function required(name: string) {
  const value = process.env[name];
  if (!value) throw new Error(`${name} must be set by the performance runner`);
  return value;
}
const root = required("GITRU_COLLABORATION_HARNESS_ROOT");
const runNonce = required("GITRU_COLLABORATION_HARNESS_RUN_NONCE");
const artifacts = required("GITRU_E2E_ARTIFACTS");
const fixtureRoot = required("GITRU_E2E_TEMP_ROOT");
const mode = required("GITRU_COLLABORATION_PERFORMANCE_MODE");
if (mode !== "seed" && mode !== "restart")
  throw new Error("Unknown performance driver mode");
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
    GITRU_COLLABORATION_HARNESS_PHASE: "main",
    GITRU_COLLABORATION_PERFORMANCE: "1",
    GITRU_COLLABORATION_PERFORMANCE_MODE: mode,
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

function assertArtifacts() {
  for (const file of ["raw-performance.json", "process-memory.json"]) {
    const path = resolve(artifacts, file);
    const stat = lstatSync(path);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 512 * 1024)
      throw new Error("Performance evidence exceeds its bound");
  }
  HarnessScenarioResultSchema.parse(
    JSON.parse(
      readFileSync(resolve(artifacts, "raw-performance.json"), "utf8"),
    ),
  );
}

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/collaboration-performance.e2e.ts"],
  maxInstances: 1,
  capabilities: [capability],
  services: [[resolve(import.meta.dirname, "harness-service.ts"), options]],
  outputDir: artifacts,
  logLevel: "info",
  bail: 1,
  waitforTimeout: 30_000,
  connectionRetryTimeout: 7 * 60_000,
  connectionRetryCount: 0,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 7 * 60_000 },
  onPrepare: () => mkdirSync(artifacts, { recursive: true }),
  onComplete: assertArtifacts,
  afterTest: async (test, _context, result) => {
    if (result.passed) return;
    const name = test.title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .slice(0, 80);
    await browser
      .saveScreenshot(resolve(artifacts, `${name}.png`))
      .catch(() => undefined);
    const source = await browser.getPageSource().catch(() => "");
    if (source)
      writeFileSync(resolve(artifacts, `${name}.html`), source, {
        encoding: "utf8",
        mode: 0o600,
      });
  },
};
